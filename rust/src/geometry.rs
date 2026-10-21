// SPDX-License-Identifier: GPL-3.0-only
//! Pixel-aligned face assemblies compiled to ordinary transactional operations.

use crate::{
    Error, Pixel, Result,
    drawing::Point,
    operations::Operation,
    palette::{Color, Palette},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Faces {
    pub top: Color,
    pub front: Color,
    pub side: Color,
    #[serde(default)]
    pub outline: Option<Color>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "snake_case", deny_unknown_fields)]
pub enum Shape {
    Cottage {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: u32,
        roof_height: u32,
        faces: Faces,
        roof: Faces,
    },
    Prism {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: u32,
        faces: Faces,
    },
    Roof {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: u32,
        faces: Faces,
    },
    Steps {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        depth: u32,
        count: u32,
        faces: Faces,
    },
}
fn point(x: i64, y: i64) -> Result<Point> {
    let point = Point {
        x: i32::try_from(x).map_err(|_| Error::limit("geometry x overflow"))?,
        y: i32::try_from(y).map_err(|_| Error::limit("geometry y overflow"))?,
    };
    point.validate()?;
    Ok(point)
}
struct Face {
    points: Vec<Point>,
    color: Color,
    outline: Option<Color>,
    normal: Pixel,
}
impl Face {
    fn append(
        &self,
        output: &mut Vec<Operation>,
        normal: bool,
        palette: Option<&Palette>,
    ) -> Result<()> {
        let color = |value: &Color| -> Result<Color> {
            if normal {
                Ok(Color::Rgba(Pixel {
                    a: value.resolve(palette)?.a,
                    ..self.normal
                }))
            } else {
                Ok(value.clone())
            }
        };
        output.push(Operation::Polygon {
            points: self.points.clone(),
            color: color(&self.color)?,
            fill: true,
            over: false,
        });
        if let Some(outline) = &self.outline {
            output.push(Operation::Polygon {
                points: self.points.clone(),
                color: color(outline)?,
                fill: false,
                over: false,
            });
        }
        Ok(())
    }
}
fn face(
    points: Vec<Point>,
    color: Color,
    outline: &Option<Color>,
    normal: Pixel,
    faces: &mut Vec<Face>,
) {
    faces.push(Face {
        points,
        color,
        outline: outline.clone(),
        normal,
    });
}
impl Shape {
    pub fn operations(&self) -> Result<Vec<Operation>> {
        let mut operations = Vec::new();
        for face in self.faces()? {
            face.append(&mut operations, false, None)?;
        }
        Ok(operations)
    }

    /// Stylized face directions, rasterized with the exact color masks and order.
    pub fn normal_operations(&self, palette: Option<&Palette>) -> Result<Vec<Operation>> {
        let mut operations = Vec::new();
        for face in self.faces()? {
            face.append(&mut operations, true, palette)?;
        }
        Ok(operations)
    }

    pub(crate) fn paired_operations(
        &self,
        palette: Option<&Palette>,
    ) -> Result<(Vec<Operation>, Vec<Operation>)> {
        let (mut color, mut normal) = (Vec::new(), Vec::new());
        for face in self.faces()? {
            face.append(&mut color, false, None)?;
            face.append(&mut normal, true, palette)?;
        }
        Ok((color, normal))
    }

    fn faces(&self) -> Result<Vec<Face>> {
        let mut operations = Vec::new();
        match self {
            Self::Cottage {
                x,
                y,
                width,
                height,
                depth,
                roof_height,
                faces,
                roof,
            } => {
                let top = point(i64::from(*x), i64::from(*y) - i64::from(*height) + 1)?;
                operations.extend(
                    Self::Prism {
                        x: top.x,
                        y: top.y,
                        width: *width,
                        height: *height,
                        depth: *depth,
                        faces: faces.clone(),
                    }
                    .faces()?,
                );
                let eave = point(i64::from(top.x) - 2, i64::from(top.y))?;
                operations.extend(
                    Self::Roof {
                        x: eave.x,
                        y: eave.y,
                        width: width
                            .checked_add(4)
                            .ok_or_else(|| Error::limit("roof width overflow"))?,
                        height: *roof_height,
                        depth: *depth,
                        faces: roof.clone(),
                    }
                    .faces()?,
                );
            }
            Self::Prism {
                x,
                y,
                width,
                height,
                depth,
                faces,
            }
            | Self::Roof {
                x,
                y,
                width,
                height,
                depth,
                faces,
            } => {
                if *width < 2
                    || *height < 2
                    || *depth == 0
                    || *width > 4096
                    || *height > 4096
                    || *depth > 1024
                {
                    return Err(Error::invalid(
                        "faces require width/height 2..=4096 and depth 1..=1024",
                    ));
                }
                let (x, y, w, h, d) = (
                    i64::from(*x),
                    i64::from(*y),
                    i64::from(*width) - 1,
                    i64::from(*height) - 1,
                    i64::from(*depth),
                );
                if matches!(self, Self::Roof { .. }) {
                    let ridge = point(x + w / 2, y - h)?;
                    let left = point(x, y)?;
                    let right = point(x + w, y)?;
                    // The far roof plane covers the wall's exposed top behind the gable.
                    face(
                        vec![
                            left,
                            point(x + d, y - d)?,
                            point(x + w / 2 + d, y - h - d)?,
                            ridge,
                        ],
                        faces.top.clone(),
                        &faces.outline,
                        Pixel::rgba(64, 218, 191, 255),
                        &mut operations,
                    );
                    face(
                        vec![left, ridge, right],
                        faces.front.clone(),
                        &faces.outline,
                        crate::maps::FLAT_NORMAL,
                        &mut operations,
                    );
                    face(
                        vec![
                            ridge,
                            point(x + w / 2 + d, y - h - d)?,
                            point(x + w + d, y - d)?,
                            right,
                        ],
                        faces.top.clone(),
                        &faces.outline,
                        Pixel::rgba(191, 218, 191, 255),
                        &mut operations,
                    );
                    face(
                        vec![
                            right,
                            point(x + w + d, y - d)?,
                            point(x + w + d, y + 2 - d)?,
                            point(x + w, y + 2)?,
                        ],
                        faces.side.clone(),
                        &faces.outline,
                        Pixel::rgba(218, 128, 218, 255),
                        &mut operations,
                    );
                } else {
                    let (a, b, c, e) = (
                        point(x, y)?,
                        point(x + w, y)?,
                        point(x + w, y + h)?,
                        point(x, y + h)?,
                    );
                    face(
                        vec![a, point(x + d, y - d)?, point(x + w + d, y - d)?, b],
                        faces.top.clone(),
                        &faces.outline,
                        Pixel::rgba(128, 218, 218, 255),
                        &mut operations,
                    );
                    face(
                        vec![b, point(x + w + d, y - d)?, point(x + w + d, y + h - d)?, c],
                        faces.side.clone(),
                        &faces.outline,
                        Pixel::rgba(218, 128, 218, 255),
                        &mut operations,
                    );
                    face(
                        vec![a, b, c, e],
                        faces.front.clone(),
                        &faces.outline,
                        crate::maps::FLAT_NORMAL,
                        &mut operations,
                    );
                }
            }
            Self::Steps {
                x,
                y,
                width,
                height,
                depth,
                count,
                faces,
            } => {
                if *count == 0 || *count > 32 {
                    return Err(Error::invalid("steps require 1..=32 levels"));
                }
                for level in 0..*count {
                    let p = point(
                        i64::from(*x) + i64::from(level) * i64::from(*depth),
                        i64::from(*y) - i64::from(level) * i64::from(*height),
                    )?;
                    operations.extend(
                        Self::Prism {
                            x: p.x,
                            y: p.y,
                            width: *width,
                            height: *height,
                            depth: *depth,
                            faces: faces.clone(),
                        }
                        .faces()?,
                    );
                }
            }
        }
        Ok(operations)
    }
}
