// SPDX-License-Identifier: GPL-3.0-only
//! Experimental runtime lighting, kept outside the offline painter dependency graph.

use super::{Manifest, frame_at, parse_manifest};
use macroquad::{material::Material, miniquad::*, prelude::*};

const COLOR: &[u8] =
    include_bytes!("../../../rust/examples/lighting/01-neon-sign/export/sheet.png");
const EMISSION: &[u8] =
    include_bytes!("../../../rust/examples/lighting/01-neon-sign/export/emissive/sheet.png");
const NORMAL: &[u8] =
    include_bytes!("../../../rust/examples/lighting/01-neon-sign/export/normal/sheet.png");
const VERTEX: &str = "#version 100\nattribute vec3 position;\nattribute vec2 texcoord;\nuniform mat4 Model;\nuniform mat4 Projection;\nvarying highp vec2 uv;\nvoid main(){uv=texcoord;gl_Position=Projection*Model*vec4(position,1.0);}";
const FRAGMENT: &str = include_str!("lighting.glsl");

fn manifest(text: &str) -> Manifest {
    let manifest = parse_manifest(text);
    for (name, map) in &manifest.maps {
        assert!(matches!(name.as_str(), "emissive" | "normal"));
        assert_eq!(map.sheet, format!("{name}/sheet.png"));
        assert_eq!(map.frames.len(), manifest.frames.len());
        for (index, path) in map.frames.iter().enumerate() {
            assert_eq!(path, &format!("{name}/frames/{index:04}.png"));
        }
        if name == "normal" {
            assert_eq!(map.encoding, "xyz_unorm8");
            assert_eq!(map.space.as_deref(), Some("screen"));
            assert_eq!(map.positive_y.as_deref(), Some("up"));
        } else {
            assert_eq!(map.encoding, "rgba8_emission");
        }
    }
    manifest
}
fn texture(bytes: Option<&[u8]>, fallback: [u8; 4]) -> Texture2D {
    let texture = bytes.map_or_else(
        || Texture2D::from_rgba8(1, 1, &fallback),
        |bytes| Texture2D::from_file_with_format(bytes, Some(ImageFormat::Png)),
    );
    texture.set_filter(FilterMode::Nearest);
    texture
}
fn lighting_material(emission: &Texture2D, normal: &Texture2D) -> Material {
    let material = load_material(
        ShaderSource::Glsl {
            vertex: VERTEX,
            fragment: FRAGMENT,
        },
        MaterialParams {
            uniforms: vec![
                UniformDesc::new("Mode", UniformType::Float1),
                UniformDesc::new("Light", UniformType::Float3),
                UniformDesc::new("Flip", UniformType::Float2),
            ],
            textures: vec!["EmissionTexture".into(), "NormalTexture".into()],
            pipeline_params: PipelineParams {
                color_blend: Some(BlendState::new(
                    Equation::Add,
                    BlendFactor::Value(BlendValue::SourceAlpha),
                    BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
                )),
                ..Default::default()
            },
        },
    )
    .expect("lighting shader compiles");
    material.set_texture("EmissionTexture", emission.clone());
    material.set_texture("NormalTexture", normal.clone());
    material
}
#[derive(Clone, Copy)]
struct Lighting {
    mode: f32,
    light: Vec3,
    flips: (bool, bool),
}
fn draw(
    material: &Material,
    color: &Texture2D,
    rect: Rect,
    position: Vec2,
    scale: f32,
    lighting: Lighting,
) {
    let Lighting { mode, light, flips } = lighting;
    material.set_uniform("Mode", mode);
    material.set_uniform("Light", light);
    material.set_uniform(
        "Flip",
        vec2(
            if flips.0 { -1.0 } else { 1.0 },
            if flips.1 { -1.0 } else { 1.0 },
        ),
    );
    gl_use_material(material);
    draw_texture_ex(
        color,
        position.x,
        position.y,
        WHITE,
        DrawTextureParams {
            source: Some(rect),
            dest_size: Some(vec2(rect.w * scale, rect.h * scale)),
            flip_x: flips.0,
            flip_y: flips.1,
            ..Default::default()
        },
    );
    gl_use_default_material();
}

// Material samplers retain GPU IDs, not texture ownership. Keep all three
// handles alive until after the material is dropped.
struct Textures {
    color: Texture2D,
    emissive: Texture2D,
    normal: Texture2D,
}
struct LitAsset {
    material: Material,
    textures: Textures,
    manifest: Manifest,
    bytes: [&'static [u8]; 3],
    probes: &'static [(u32, u32)],
}
impl LitAsset {
    fn new(text: &str, bytes: [&'static [u8]; 3], probes: &'static [(u32, u32)]) -> Self {
        let manifest = manifest(text);
        let color = texture(Some(bytes[0]), [0; 4]);
        let emission = texture(
            manifest.maps.contains_key("emissive").then_some(bytes[1]),
            [0; 4],
        );
        let normal = texture(
            manifest.maps.contains_key("normal").then_some(bytes[2]),
            [128, 128, 255, 255],
        );
        for (name, image) in [
            ("color", &color),
            ("emissive", &emission),
            ("normal", &normal),
        ] {
            assert_eq!(
                (image.width() as u32, image.height() as u32),
                if name == "color" || manifest.maps.contains_key(name) {
                    (manifest.width, manifest.height)
                } else {
                    (1, 1)
                }
            );
        }
        let textures = Textures {
            color,
            emissive: emission,
            normal,
        };
        Self {
            material: lighting_material(&textures.emissive, &textures.normal),
            textures,
            manifest,
            bytes,
            probes,
        }
    }
}
const SIGN_MANIFEST: &str =
    include_str!("../../../rust/examples/lighting/01-neon-sign/export/metadata.json");
const COTTAGE_MANIFEST: &str =
    include_str!("../../../rust/examples/lighting/02-night-cottage/export/metadata.json");
const COTTAGE: [&[u8]; 3] = [
    include_bytes!("../../../rust/examples/lighting/02-night-cottage/export/sheet.png"),
    include_bytes!("../../../rust/examples/lighting/02-night-cottage/export/emissive/sheet.png"),
    include_bytes!("../../../rust/examples/lighting/02-night-cottage/export/normal/sheet.png"),
];

pub(super) async fn run(smoke: bool) {
    let assets = [
        LitAsset::new(
            SIGN_MANIFEST,
            [COLOR, EMISSION, NORMAL],
            &[(6, 12), (8, 5), (28, 10)],
        ),
        LitAsset::new(
            COTTAGE_MANIFEST,
            COTTAGE,
            &[(15, 20), (40, 15), (30, 31), (52, 33), (15, 34)],
        ),
    ];
    if smoke {
        for asset in &assets {
            smoke_test(asset).await;
        }
        return;
    }
    let (mut mode, mut selected) = (3.0, 0);
    let mut flips = (false, false);
    loop {
        clear_background(Color::from_rgba(24, 37, 53, 255));
        if is_key_pressed(KeyCode::Key1) {
            mode = 1.0;
        }
        if is_key_pressed(KeyCode::Key2) {
            mode = 2.0;
        }
        if is_key_pressed(KeyCode::Key3) {
            mode = 3.0;
        }
        if is_key_pressed(KeyCode::Tab) {
            selected = (selected + 1) % assets.len();
        }
        if is_key_pressed(KeyCode::F) {
            flips.0 = !flips.0;
        }
        if is_key_pressed(KeyCode::V) {
            flips.1 = !flips.1;
        }
        let asset = &assets[selected];
        let indices = asset
            .manifest
            .tags
            .get("pulse")
            .map_or(&[0][..], |tag| tag.frames.as_slice());
        let frame = &asset.manifest.frames
            [frame_at(&asset.manifest, indices, (get_time() * 1000.0) as u64)];
        let r = &frame.rect;
        let light = vec3((get_time() as f32).cos(), (get_time() as f32).sin(), 0.7).normalize();
        draw(
            &asset.material,
            &asset.textures.color,
            Rect::new(r.x as f32, r.y as f32, r.width as f32, r.height as f32),
            vec2(300.0, 50.0),
            256.0 / r.height as f32,
            Lighting { mode, light, flips },
        );
        draw_text(
            "1 Color  2 Emission  3 Light  Tab Sign/House  F/V Flip  Esc Exit",
            20.0,
            365.0,
            22.0,
            LIGHTGRAY,
        );
        draw_text(format!("Mode {}", mode as u32), 20.0, 30.0, 24.0, LIGHTGRAY);
        next_frame().await;
        if is_key_pressed(KeyCode::Escape) {
            break;
        }
    }
}

fn linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
fn srgb(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}
fn reference(
    color: Color,
    emission: Color,
    normal: Color,
    mode: f32,
    light: Vec3,
    flips: (bool, bool),
) -> [f32; 3] {
    let direction = vec3(
        (2.0 * normal.r - 1.0) * if flips.0 { -1.0 } else { 1.0 },
        (2.0 * normal.g - 1.0) * if flips.1 { -1.0 } else { 1.0 },
        2.0 * normal.b - 1.0,
    )
    .normalize();
    let diffuse = if mode < 2.5 {
        1.0
    } else {
        0.18 + 0.82 * direction.dot(light).max(0.0)
    };
    let emission_strength = if mode < 1.5 || color.a == 0.0 {
        0.0
    } else {
        emission.a / color.a
    };
    let color = [color.r, color.g, color.b];
    let emission = [emission.r, emission.g, emission.b];
    std::array::from_fn(|index| {
        srgb(linear(color[index]) * diffuse + linear(emission[index]) * emission_strength)
    })
}
async fn smoke_test(asset: &LitAsset) {
    let LitAsset {
        manifest,
        material,
        textures,
        ..
    } = asset;
    let color = &textures.color;
    let frame = &manifest.frames[0].rect;
    let target = render_target(frame.width, frame.height);
    let mut camera =
        Camera2D::from_display_rect(Rect::new(0.0, 0.0, frame.width as f32, frame.height as f32));
    camera.render_target = Some(target.clone());
    let images = asset
        .bytes
        .map(|bytes| Image::from_file_with_format(bytes, Some(ImageFormat::Png)).unwrap());
    let light = vec3(0.0, 1.0, 1.0).normalize();
    let mut checked = 0;
    for rendered_frame in &manifest.frames {
        let frame = &rendered_frame.rect;
        for (mode, flips) in [
            (1.0, (false, false)),
            (2.0, (false, false)),
            (3.0, (false, false)),
            (3.0, (true, false)),
            (3.0, (false, true)),
            (3.0, (true, true)),
        ] {
            set_camera(&camera);
            clear_background(BLACK);
            draw(
                material,
                color,
                Rect::new(
                    frame.x as f32,
                    frame.y as f32,
                    frame.width as f32,
                    frame.height as f32,
                ),
                Vec2::ZERO,
                1.0,
                Lighting { mode, light, flips },
            );
            set_default_camera();
            next_frame().await;
            let rendered = target.texture.get_texture_data();
            for &(x, y) in asset.probes {
                let pixels = images
                    .each_ref()
                    .map(|image| image.get_pixel(frame.x as u32 + x, frame.y as u32 + y));
                assert_eq!(
                    pixels[0].a, 1.0,
                    "reference probes must cover actual opaque surfaces"
                );
                let expected = reference(pixels[0], pixels[1], pixels[2], mode, light, flips);
                let actual = rendered.get_pixel(
                    if flips.0 { frame.width - 1 - x } else { x },
                    // OpenGL readback starts at the bottom; artwork rows start at the top.
                    frame.height - 1 - if flips.1 { frame.height - 1 - y } else { y },
                );
                for (actual, expected) in [actual.r, actual.g, actual.b].into_iter().zip(expected) {
                    assert!(
                        (actual - expected).abs() < 4.0 / 255.0,
                        "GPU/reference mismatch at ({x},{y}), mode {mode}, flip {flips:?}: {actual} != {expected}"
                    );
                }
                checked += 1;
            }
        }
    }
    // Fallback samplers and partially covered emission use the same shader path.
    // The final alpha blend must apply albedo coverage once, not twice.
    for (albedo, emission) in [
        ([80, 100, 160, 128], Some([100, 160, 200, 64])),
        ([80, 100, 160, 255], None),
    ] {
        let color = Texture2D::from_rgba8(1, 1, &albedo);
        let emitted = emission.map(|value| Texture2D::from_rgba8(1, 1, &value));
        let emitted = emitted.unwrap_or_else(|| texture(None, [0; 4]));
        let normal = texture(None, [128, 128, 255, 255]);
        let material = lighting_material(&emitted, &normal);
        set_camera(&camera);
        clear_background(BLACK);
        draw(
            &material,
            &color,
            Rect::new(0.0, 0.0, 1.0, 1.0),
            Vec2::ZERO,
            1.0,
            Lighting {
                mode: 3.0,
                light: Vec3::Z,
                flips: (false, false),
            },
        );
        set_default_camera();
        next_frame().await;
        let actual = target
            .texture
            .get_texture_data()
            .get_pixel(0, frame.height - 1);
        let albedo = Color::from_rgba(albedo[0], albedo[1], albedo[2], albedo[3]);
        let emission = emission.unwrap_or([0; 4]);
        let expected = reference(
            albedo,
            Color::from_rgba(emission[0], emission[1], emission[2], emission[3]),
            Color::from_rgba(128, 128, 255, 255),
            3.0,
            Vec3::Z,
            (false, false),
        );
        for (actual, expected) in [actual.r, actual.g, actual.b].into_iter().zip(expected) {
            assert!(
                (actual - expected * albedo.a).abs() < 4.0 / 255.0,
                "partial-alpha/fallback GPU mismatch"
            );
        }
        checked += 1;
    }
    println!(
        "Lighting smoke passed: {checked} GPU/reference samples for modes, flips, alpha, and fallbacks"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn map_manifest_alignment_and_transfer_functions_are_valid() {
        for (text, bytes) in [
            (SIGN_MANIFEST, [COLOR, EMISSION, NORMAL]),
            (COTTAGE_MANIFEST, COTTAGE),
        ] {
            let manifest = manifest(text);
            assert_eq!(manifest.maps.len(), 2);
            for bytes in bytes {
                let image = Image::from_file_with_format(bytes, Some(ImageFormat::Png)).unwrap();
                assert_eq!(
                    (u32::from(image.width), u32::from(image.height)),
                    (manifest.width, manifest.height)
                );
            }
        }
        for value in 0..=255 {
            let value = value as f32 / 255.0;
            assert!((srgb(linear(value)) - value).abs() < 0.00001);
        }
    }
}
