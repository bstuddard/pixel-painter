// SPDX-License-Identifier: GPL-3.0-only
//! Read-only review bundles and a small linear-light reference renderer.
//! These previews have directional diffuse light and emission, without cast shadows.

use crate::{
    Canvas, Document, Error, Pixel, Result,
    canvas::MAX_PIXELS,
    drawing,
    maps::{self, Channel},
    storage,
};
use serde::Serialize;
use std::{fs, path::Path};

const VIEWS: [&str; 7] = [
    "color",
    "emissive",
    "normal",
    "unlit-emission",
    "light-left",
    "light-right",
    "light-top",
];

#[derive(Clone, Copy, Debug, Serialize)]
/// Screen-space +X right, +Y up, +Z viewer; strengths must be finite in 0..=8.
pub struct Light {
    pub direction: [f64; 3],
    pub ambient: f64,
    pub diffuse: f64,
    pub emission: f64,
}
impl Light {
    fn normalized(self) -> Result<[f64; 3]> {
        if !self.direction.into_iter().all(f64::is_finite)
            || ![self.ambient, self.diffuse, self.emission]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=8.0).contains(&v))
        {
            return Err(Error::invalid(
                "light requires a finite nonzero direction and strengths in 0..=8",
            ));
        }
        let maximum = self.direction.into_iter().map(f64::abs).fold(0.0, f64::max);
        if maximum == 0.0 {
            return Err(Error::invalid("light direction must be nonzero"));
        }
        let scaled = self.direction.map(|v| v / maximum);
        let length = scaled[0].hypot(scaled[1]).hypot(scaled[2]);
        Ok(scaled.map(|v| v / length))
    }
}

fn linear(value: u8) -> f64 {
    let value = f64::from(value) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let value = if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (value * 255.0).round() as u8
}

/// Render aligned effective maps. Map alpha already includes color coverage, so
/// divide it out before adding emission, preserving straight output alpha once.
pub fn lighting(
    color: &Canvas,
    emissive: &Canvas,
    normal: &Canvas,
    light: Light,
) -> Result<Canvas> {
    let direction = light.normalized()?;
    if [emissive, normal]
        .into_iter()
        .any(|map| (map.width(), map.height()) != (color.width(), color.height()))
    {
        return Err(Error::invalid(
            "lighting maps must have identical dimensions",
        ));
    }
    maps::validate_normals(normal)?;
    let mut output = color.clone();
    for (((out, color), emission), normal) in output
        .pixels_mut()
        .iter_mut()
        .zip(color.pixels())
        .zip(emissive.pixels())
        .zip(normal.pixels())
    {
        if color.a == 0 {
            *out = Pixel::TRANSPARENT;
            continue;
        }
        let normal = if normal.a == 0 {
            maps::FLAT_NORMAL
        } else {
            *normal
        };
        let vector = [normal.r, normal.g, normal.b].map(|v| f64::from(v) / 127.5 - 1.0);
        let length = vector[0].hypot(vector[1]).hypot(vector[2]);
        let diffuse = vector
            .into_iter()
            .zip(direction)
            .map(|(n, l)| n / length * l)
            .sum::<f64>()
            .max(0.0);
        let intensity = light.ambient + light.diffuse * diffuse;
        let strength = (f64::from(emission.a) / f64::from(color.a)).min(1.0) * light.emission;
        let channel =
            |color, emission| srgb(linear(color) * intensity + linear(emission) * strength);
        *out = Pixel::rgba(
            channel(color.r, emission.r),
            channel(color.g, emission.g),
            channel(color.b, emission.b),
            color.a,
        );
    }
    Ok(output)
}

#[derive(Debug, Serialize)]
pub struct FrameDiagnostics {
    pub index: usize,
    pub name: String,
    pub duration_ms: u32,
    pub color_pixels: usize,
    pub authored_normal_pixels: usize,
    pub fallback_normal_pixels: usize,
    pub clipped_emission_pixels: usize,
    pub reduced_emission_pixels: usize,
}
#[derive(Debug, Serialize)]
pub struct Review {
    pub version: u32,
    pub width: u32,
    pub height: u32,
    pub scale: u32,
    pub views: Vec<&'static str>,
    pub lights: Vec<Light>,
    pub frames: Vec<FrameDiagnostics>,
}

/// Native-size per-frame views and a scaled sheet: one row per frame, columns in
/// `views` order. No input state is changed. The destination must not exist.
pub fn write(
    document: &Document,
    destination: &Path,
    scale: u32,
    all_frames: bool,
) -> Result<Review> {
    crate::scene::validate_preview_scale(scale)?;
    let indices: Vec<_> = if all_frames {
        (0..document.frames().len()).collect()
    } else {
        vec![document.active_frame()]
    };
    let width = document.width();
    let height = document.height();
    // Check every output and aggregate raster work before allocating or creating files.
    let sheet_width = width
        .checked_mul(VIEWS.len() as u32)
        .ok_or_else(|| Error::limit("review width overflow"))?;
    let sheet_height = height
        .checked_mul(indices.len() as u32)
        .ok_or_else(|| Error::limit("review height overflow"))?;
    let scaled_width = sheet_width
        .checked_mul(scale)
        .ok_or_else(|| Error::limit("review width overflow"))?;
    let scaled_height = sheet_height
        .checked_mul(scale)
        .ok_or_else(|| Error::limit("review height overflow"))?;
    Canvas::pixel_count(scaled_width, scaled_height)?;
    let pixels = u64::from(width) * u64::from(height) * indices.len() as u64;
    if pixels * VIEWS.len() as u64 > MAX_PIXELS
        || pixels * (document.layers().len() as u64 + 32)
            + u64::from(scaled_width) * u64::from(scaled_height)
            > drawing::MAX_WORK
    {
        return Err(Error::limit(
            "review exceeds aggregate pixel or rasterization budget",
        ));
    }
    if destination.try_exists()? {
        return Err(Error::new(
            crate::ErrorCode::AlreadyExists,
            "review destination already exists",
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = tempfile::tempdir_in(parent)?;
    fs::create_dir(stage.path().join("frames"))?;
    let lights = [[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [0.0, 1.0, 0.25]].map(|direction| Light {
        direction,
        ambient: 0.2,
        diffuse: 0.8,
        emission: 1.0,
    });
    let mut review = Review {
        version: 1,
        width,
        height,
        scale,
        views: VIEWS.to_vec(),
        lights: lights.to_vec(),
        frames: Vec::new(),
    };
    let mut sheet = Canvas::new(sheet_width, sheet_height)?;
    for (row, index) in indices.into_iter().enumerate() {
        let frame = &document.frames()[index];
        let color = document.flatten(index)?;
        let mut emissive = document.composite_layers(index, Channel::Emissive, None)?;
        let mut normal = document.composite_layers(index, Channel::Normal, None)?;
        let mut diagnostics = FrameDiagnostics {
            index,
            name: frame.name.clone(),
            duration_ms: frame.duration_ms,
            color_pixels: 0,
            authored_normal_pixels: 0,
            fallback_normal_pixels: 0,
            clipped_emission_pixels: 0,
            reduced_emission_pixels: 0,
        };
        for ((color, emission), normal) in color
            .pixels()
            .iter()
            .zip(emissive.pixels())
            .zip(normal.pixels())
        {
            if color.a != 0 {
                diagnostics.color_pixels += 1;
                if normal.a == 0 {
                    diagnostics.fallback_normal_pixels += 1;
                } else {
                    diagnostics.authored_normal_pixels += 1;
                }
            }
            if emission.a != 0 {
                if color.a == 0 {
                    diagnostics.clipped_emission_pixels += 1;
                } else if color.a != 255 {
                    diagnostics.reduced_emission_pixels += 1;
                }
            }
        }
        maps::apply_coverage(&mut emissive, &color, Channel::Emissive);
        maps::apply_coverage(&mut normal, &color, Channel::Normal);
        let unlit = lighting(
            &color,
            &emissive,
            &normal,
            Light {
                ambient: 1.0,
                diffuse: 0.0,
                ..lights[0]
            },
        )?;
        let lit = lights.map(|light| lighting(&color, &emissive, &normal, light));
        let [left, right, top] = lit;
        let views = [color, emissive, normal, unlit, left?, right?, top?];
        let directory = stage.path().join("frames").join(format!("{index:04}"));
        fs::create_dir(&directory)?;
        for (column, (name, image)) in VIEWS.iter().zip(&views).enumerate() {
            storage::export_png(&directory.join(format!("{name}.png")), image)?;
            sheet.copy_from(image, column as u32 * width, row as u32 * height)?;
        }
        review.frames.push(diagnostics);
    }
    if scale != 1 {
        sheet = drawing::resize(&sheet, scaled_width, scaled_height)?;
    }
    storage::export_png(&stage.path().join("contact-sheet.png"), &sheet)?;
    storage::write_json(&stage.path().join("review.json"), &review, true)?;
    storage::publish_bundle(stage.path(), destination, "review.json")?;
    Ok(review)
}
