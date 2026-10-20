// SPDX-License-Identifier: GPL-3.0-only
//! Measurable frame alignment checks with explicit named motion exceptions.

use crate::{
    Canvas, Document, Error, Result, inspect, placement::Placement, style::Profile,
    validation::Report,
};

pub fn check(
    document: &Document,
    profile: &Profile,
    placement: Option<&Placement>,
) -> Result<Report> {
    profile.validate()?;
    if let Some(placement) = placement {
        placement.validate(document)?;
    }
    profile.check_work(
        u64::from(document.width()) * u64::from(document.height()) * document.frames().len() as u64,
    )?;
    let rules = &profile.animation;
    for name in rules
        .movement_exceptions
        .iter()
        .chain(&rules.clipping_exceptions)
    {
        if !document.frames().iter().any(|frame| &frame.name == name) {
            return Err(Error::invalid(format!(
                "unknown animation exception {name:?}"
            )));
        }
    }
    let mut report = Report::default();
    for name in &rules.required_tags {
        if !document.tags().contains_key(name) {
            report.error(
                "missing_tag",
                &profile.name,
                format!("required tag {name:?} is missing"),
            );
        }
    }
    let anchors = document
        .frames()
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            Ok(placement
                .map(|p| p.frame_anchors(document, index))
                .transpose()?
                .and_then(|a| a.ground)
                .or(frame.pivot))
        })
        .collect::<Result<Vec<_>>>()?;
    let baseline = rules.baseline.or_else(|| {
        document
            .frames()
            .iter()
            .zip(&anchors)
            .filter(|(frame, _)| !rules.movement_exceptions.contains(&frame.name))
            .find_map(|(_, anchor)| anchor.map(|p| p.y))
    });

    let mut first_size = None;
    for (index, frame) in document.frames().iter().enumerate() {
        let image = document.flatten(index)?;
        report.extend(profile.check(&image, &frame.name)?);
        let bounds = inspect::occupied_bounds(&image);
        let anchor = anchors[index];
        if !rules.movement_exceptions.contains(&frame.name) {
            if let Some(anchor) = anchor {
                let expected = baseline.unwrap_or(anchor.y);
                if anchor.y != expected {
                    report.error(
                        "ground_drift",
                        &frame.name,
                        format!("ground Y {} differs from baseline {expected}", anchor.y),
                    );
                }
            } else if baseline.is_some() {
                report.error(
                    "missing_ground",
                    &frame.name,
                    "frame requires a pivot or ground anchor",
                );
            }
        }
        if let Some(bounds) = bounds {
            if !rules.clipping_exceptions.contains(&frame.name) {
                let right = i64::from(bounds.x) + i64::from(bounds.width);
                let bottom = i64::from(bounds.y) + i64::from(bounds.height);
                if i64::from(bounds.x) < i64::from(rules.margin)
                    || i64::from(bounds.y) < i64::from(rules.margin)
                    || right > i64::from(document.width()) - i64::from(rules.margin)
                    || bottom > i64::from(document.height()) - i64::from(rules.margin)
                {
                    report.warning(
                        "clipping_margin",
                        &frame.name,
                        "visible pixels touch the configured margin",
                    );
                }
            }
            if let Some(delta) = rules.max_size_delta
                && !rules.movement_exceptions.contains(&frame.name)
            {
                let size = (bounds.width, bounds.height);
                let expected = *first_size.get_or_insert(size);
                if size.0.abs_diff(expected.0) > delta || size.1.abs_diff(expected.1) > delta {
                    report.warning(
                        "proportion_drift",
                        &frame.name,
                        "occupied width/height differs from the reference frame",
                    );
                }
            }
        }
    }
    Ok(report)
}
pub fn contact_sheet(document: &Document, scale: u32) -> Result<Canvas> {
    crate::scene::validate_preview_scale(scale)?;
    let mut seen = vec![false; document.frames().len()];
    let mut indices = Vec::with_capacity(seen.len());
    // Tags may repeat or share frames. Keep first appearances, then untagged frames.
    for tag in document.tags().values() {
        for index in &tag.frames {
            if !seen[*index] {
                seen[*index] = true;
                indices.push(*index);
            }
        }
    }
    for (index, seen) in seen.into_iter().enumerate() {
        if !seen {
            indices.push(index);
        }
    }
    let frames = indices
        .into_iter()
        .map(|i| document.flatten(i))
        .collect::<Result<Vec<_>>>()?;
    crate::scene::contact_sheet(&frames, None, 2, crate::Pixel::TRANSPARENT, scale)
}
