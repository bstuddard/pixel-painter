// SPDX-License-Identifier: GPL-3.0-only
//! Bounded scalar parameter substitution and named parts, without an expression language.

use crate::{
    Canvas, Error, ErrorCode, Pixel, Result, drawing,
    geometry::Shape,
    operations::{Batch, Operation},
    palette::{Color, Palette, Policy},
    style::Profile,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub type Parameters = BTreeMap<String, Value>;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Parameter {
    Integer {
        default: i32,
        min: i32,
        max: i32,
    },
    Color {
        default: Color,
    },
}
/// One authoring format for seeded drawings, parameterized components, and geometry.
/// Operations expand into the transactional batch renderer under one work budget.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub palette: Option<Palette>,
    #[serde(default)]
    pub policy: Policy,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 64>")]
    pub parameters: BTreeMap<String, Parameter>,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 32>")]
    pub variants: BTreeMap<String, Parameters>,
    #[serde(default, deserialize_with = "crate::limits::map::<'de, _, _, 64>")]
    pub parts: BTreeMap<String, Canvas>,
    #[serde(deserialize_with = "crate::limits::sequence::<'de, _, _, 10_000>")]
    pub operations: Vec<Value>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum ComponentOp {
    Component,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PartUse {
    #[serde(rename = "op")]
    _op: ComponentOp,
    name: String,
    x: i32,
    y: i32,
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
    #[serde(
        default,
        deserialize_with = "crate::limits::sequence::<'de, _, _, 256>"
    )]
    substitutions: Vec<Substitution>,
    #[serde(default)]
    over: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Substitution {
    from: Pixel,
    to: Color,
}
impl Recipe {
    /// Render defaults, then variant values, then explicit parameter overrides.
    /// An explicit seed overrides the recipe seed; the recipe palette overrides the profile.
    pub fn render(
        &self,
        overrides: &Parameters,
        variant: Option<&str>,
        seed: Option<u64>,
        style: Option<&Profile>,
    ) -> Result<Canvas> {
        let mut remaining = drawing::MAX_WORK;
        self.render_budgeted(overrides, variant, seed, style, &mut remaining)
    }
    pub(crate) fn render_budgeted(
        &self,
        overrides: &Parameters,
        variant: Option<&str>,
        seed: Option<u64>,
        style: Option<&Profile>,
        remaining: &mut u64,
    ) -> Result<Canvas> {
        if self.version != 2 {
            return Err(Error::new(
                ErrorCode::UnsupportedVersion,
                "recipe version must be 2",
            ));
        }
        if self.parameters.len() > 64
            || self.variants.len() > 32
            || self.parts.len() > 64
            || self.operations.len() > 10_000
            || overrides.len() > 64
        {
            return Err(Error::limit(
                "recipe parameter, variant, part, or operation limit exceeded",
            ));
        }
        let mut canvas = Canvas::new(self.width, self.height)?;
        let palette = self.palette.as_ref().or(style.map(|s| &s.palette));
        if let Some(style) = style {
            style.validate()?;
        }
        if let Some(palette) = palette {
            palette.validate()?;
        }
        let retained: u64 = self.parts.values().map(|c| c.pixels().len() as u64).sum();
        if retained > crate::canvas::MAX_PIXELS {
            return Err(Error::limit("recipe parts exceed aggregate pixel limit"));
        }
        for name in self
            .parts
            .keys()
            .chain(self.parameters.keys())
            .chain(self.variants.keys())
        {
            crate::document::name_valid(name)?;
        }
        let mut values = Parameters::new();
        for (name, parameter) in &self.parameters {
            values.insert(
                name.clone(),
                match parameter {
                    Parameter::Integer {
                        default,
                        min,
                        max,
                    } => {
                        if min > max || default < min || default > max {
                            return Err(Error::invalid("invalid parameter default or range"));
                        }
                        Value::from(*default)
                    }
                    Parameter::Color {
                        default,
                    } => {
                        default.resolve(palette)?;
                        serde_json::to_value(default)?
                    }
                },
            );
        }
        if let Some(name) = variant {
            let variant = self
                .variants
                .get(name)
                .ok_or_else(|| Error::invalid(format!("unknown variant {name:?}")))?;
            values.extend(variant.clone());
        }
        values.extend(overrides.clone());
        for (name, value) in self
            .variants
            .values()
            .flat_map(|variant| variant.iter())
            .chain(values.iter())
        {
            match self
                .parameters
                .get(name)
                .ok_or_else(|| Error::invalid(format!("unknown parameter {name:?}")))?
            {
                Parameter::Integer {
                    min,
                    max,
                    ..
                } => {
                    let value = value
                        .as_i64()
                        .ok_or_else(|| Error::invalid("integer parameter requires an integer"))?;
                    if value < i64::from(*min) || value > i64::from(*max) {
                        return Err(Error::invalid(format!(
                            "parameter {name:?} is outside its range"
                        )));
                    }
                }
                Parameter::Color {
                    ..
                } => {
                    serde_json::from_value::<Color>(value.clone())?.resolve(palette)?;
                }
            }
        }
        let mut operations = Vec::new();
        let mut embedded = 0u64;
        let mut nodes = 100_000usize;
        for operation in &self.operations {
            let value = substitute(operation, &values, &mut nodes, 0)?;
            let expanded = if value.get("op").and_then(Value::as_str) == Some("component") {
                let part: PartUse = serde_json::from_value(value)?;
                let source = self
                    .parts
                    .get(&part.name)
                    .ok_or_else(|| Error::invalid("unknown component"))?;
                embedded = embedded
                    .checked_add(source.pixels().len() as u64)
                    .ok_or_else(|| Error::limit("component pixel overflow"))?;
                if embedded > crate::canvas::MAX_PIXELS {
                    return Err(Error::limit(
                        "expanded components exceed aggregate pixel limit",
                    ));
                }
                let preparation = source.pixels().len() as u64
                    * (2 + u64::from(part.flip_x) + u64::from(part.flip_y));
                *remaining = remaining
                    .checked_sub(preparation)
                    .ok_or_else(|| Error::limit("component preparation exceeds work budget"))?;
                let mut source = source.clone();
                if part.flip_x {
                    source = drawing::flip(&source, drawing::Axis::Horizontal)?;
                }
                if part.flip_y {
                    source = drawing::flip(&source, drawing::Axis::Vertical)?;
                }
                let mut substitutions = BTreeMap::new();
                for swap in part.substitutions {
                    if substitutions
                        .insert(swap.from, swap.to.resolve(palette)?)
                        .is_some()
                    {
                        return Err(Error::invalid("duplicate component color substitution"));
                    }
                }
                for pixel in source.pixels_mut() {
                    if let Some(color) = substitutions.get(pixel) {
                        *pixel = *color;
                    }
                }
                vec![Operation::Stamp {
                    canvas: source,
                    x: part.x,
                    y: part.y,
                    over: part.over,
                }]
            } else if value.get("shape").is_some() {
                serde_json::from_value::<Shape>(value)?.operations()?
            } else {
                vec![serde_json::from_value::<Operation>(value)?]
            };
            operations.extend(expanded);
            if operations.len() > 10_000 {
                return Err(Error::limit("expanded recipe exceeds 10,000 operations"));
            }
        }
        let mut batch = Batch::new(operations);
        batch.palette = palette.cloned();
        batch.policy = self.policy;
        batch.apply_budgeted(&mut canvas, seed.unwrap_or(self.seed), remaining)?;
        Ok(canvas)
    }
}
fn substitute(
    value: &Value,
    values: &Parameters,
    remaining: &mut usize,
    depth: usize,
) -> Result<Value> {
    if depth > 32 {
        return Err(Error::limit("recipe expansion exceeds nesting limit"));
    }
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| Error::limit("recipe expansion exceeds node budget"))?;
    Ok(match value {
        Value::Object(map) if map.contains_key("param") => {
            if map.len() != 1 {
                return Err(Error::invalid("parameter references contain only param"));
            }
            let name = map["param"]
                .as_str()
                .ok_or_else(|| Error::invalid("parameter reference requires a name"))?;
            values
                .get(name)
                .cloned()
                .ok_or_else(|| Error::invalid(format!("unknown parameter {name:?}")))?
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(key, value)| {
                    Ok((
                        key.clone(),
                        substitute(value, values, remaining, depth + 1)?,
                    ))
                })
                .collect::<Result<_>>()?,
        ),
        Value::Array(array) => Value::Array(
            array
                .iter()
                .map(|value| substitute(value, values, remaining, depth + 1))
                .collect::<Result<_>>()?,
        ),
        value => value.clone(),
    })
}
#[cfg(test)]
mod tests {
    use super::{Parameters, Recipe};
    use crate::{Canvas, ErrorCode, Pixel};
    use serde_json::json;

    #[test]
    fn component_preparation_and_rasterization_share_the_same_budget() {
        let source = Canvas::from_pixels(1, 1, vec![Pixel::rgba(255, 0, 0, 255)]).unwrap();
        let value = json!({"version":2,"width":1,"height":1,"parts":{"part":source},"operations":[{"op":"component","name":"part","x":0,"y":0,"flip_x":true}]});
        let recipe: Recipe = serde_json::from_value(value).unwrap();
        let mut remaining = 3;
        let error = recipe
            .render_budgeted(&Parameters::new(), None, None, None, &mut remaining)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert_eq!(remaining, 0);
        let mut remaining = 4;
        assert_eq!(
            recipe
                .render_budgeted(&Parameters::new(), None, None, None, &mut remaining)
                .unwrap(),
            source
        );
        assert_eq!(remaining, 0);
    }

    #[test]
    fn component_transforms_cannot_run_before_their_preflight() {
        let source = Canvas::from_pixels(1, 1, vec![Pixel::rgba(255, 0, 0, 255)]).unwrap();
        let value = json!({"version":2,"width":1,"height":1,"parts":{"part":source},"operations":[{"op":"component","name":"part","x":0,"y":0,"flip_x":true,"flip_y":true}]});
        let recipe: Recipe = serde_json::from_value(value).unwrap();
        let mut remaining = 3;
        let error = recipe
            .render_budgeted(&Parameters::new(), None, None, None, &mut remaining)
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::ResourceLimit);
        assert!(error.message.contains("component preparation"));
        assert_eq!(remaining, 3);
    }
}
