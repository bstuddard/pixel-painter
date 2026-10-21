// SPDX-License-Identifier: GPL-3.0-only
use painter::{
    Canvas, Document, ErrorCode, Pixel,
    maps::{Channel, FLAT_NORMAL},
    operations::Batch,
    preview::{self, Light},
    storage,
};
use serde_json::{Value, json};

fn batch(value: Value) -> Batch {
    serde_json::from_value(value).unwrap()
}
fn image(pixels: &[&str]) -> Canvas {
    Canvas::from_pixels(
        pixels.len() as u32,
        1,
        pixels.iter().map(|p| p.parse().unwrap()).collect(),
    )
    .unwrap()
}
fn ramp() -> Value {
    json!({"name":"wood","colors":["#382438", "#805048", "#D99858"]})
}

#[test]
fn material_shading_stays_in_each_ramp_and_preserves_alpha_and_unmatched_pixels() {
    let mut canvas = image(&["#80504880", "#667788", "#010203", "#80504800"]);
    let original = canvas.clone();
    batch(
        json!({"version":1,"operations":[{"op":"shade","steps":1,"ramps":[ramp(),
        {"name":"stone","colors":["#223344", "#667788", "#AABBCC"]}]}]}),
    )
    .apply(&mut canvas)
    .unwrap();
    assert_eq!(
        canvas,
        image(&["#D9985880", "#AABBCC", "#010203", "#80504800"])
    );
    for steps in [i32::MIN, i32::MAX] {
        let mut canvas = original.clone();
        batch(json!({"version":1,"operations":[{"op":"shade","steps":steps,"ramps":[ramp()]}]}))
            .apply(&mut canvas)
            .unwrap();
        assert_eq!(canvas.pixel(0, 0).unwrap().a, 128);
        assert_eq!(
            canvas.pixel(0, 0).unwrap().r,
            if steps < 0 { 56 } else { 217 }
        );
    }
}

#[test]
fn ambiguous_or_invalid_materials_roll_back_with_operation_index() {
    for ramps in [
        json!([]),
        json!([ramp(), ramp()]),
        json!([{"name":"wood","colors":["#805048", "#805048"]}]),
        json!([{"name":"wood","colors":["#80504880", "#FFFFFF"]}]),
        json!([ramp(), {"name":"stone","colors":["#000000", "#805048"]}]),
        json!([{"name":"bad\nname","colors":["#000000", "#FFFFFF"]}]),
    ] {
        let mut canvas = image(&["#805048"]);
        let original = canvas.clone();
        let error = batch(
            json!({"version":1,"operations":[{"op":"clear","color":"#FFFFFF"},
            {"op":"shade","steps":1,"ramps":ramps}]}),
        )
        .apply(&mut canvas)
        .unwrap_err();
        assert_eq!(canvas, original);
        assert_eq!(error.operation, Some(1));
    }
}

#[test]
fn named_material_colors_resolve_from_batch_palette_and_strict_policy() {
    let mut canvas = image(&["#805048"]);
    batch(json!({"version":1,"palette":{"version":1,"colors":{"dark":"#805048","light":"#D99858"}},
        "policy":"strict","operations":[{"op":"shade","steps":1,"ramps":[{"name":"wood","colors":["dark","light"]}]}]})).apply(&mut canvas).unwrap();
    assert_eq!(canvas, image(&["#D99858"]));
}

#[test]
fn masks_snapshot_alpha_and_treat_partial_alpha_as_binary_selection() {
    let mut canvas = image(&["#11223380", "#33445500", "#77889901"]);
    batch(
        json!({"version":1,"mask":{"source":"alpha"},"operations":[{"op":"clear","color":"#FFFFFF"},
        {"op":"line","x1":0,"y1":0,"x2":2,"y2":0,"color":"#D99858"}]}),
    )
    .apply(&mut canvas)
    .unwrap();
    assert_eq!(canvas, image(&["#D99858", "#33445500", "#D99858"]));
    let mut canvas = image(&["#11223380", "#33445500", "#77889901"]);
    batch(json!({"version":1,"alpha_lock":true,"operations":[{"op":"clear","color":"#FFFFFF"}]}))
        .apply(&mut canvas)
        .unwrap();
    assert_eq!(canvas, image(&["#FFFFFF80", "#33445500", "#FFFFFF01"]));
}

#[test]
fn alpha_lock_ignores_transparent_brush_texels_and_erasing_without_losing_prior_edits() {
    let mut canvas = image(&["#11223380", "#445566"]);
    batch(json!({"version":1,"alpha_lock":true,"operations":[
        {"op":"clear","color":"#AABBCC"},
        {"op":"stamp","canvas":image(&["#00000000","#FFFFFF"]),"x":0,"y":0},
        {"op":"erase","x":0,"y":0,"width":2,"height":1}]}))
    .apply(&mut canvas)
    .unwrap();
    assert_eq!(canvas, image(&["#AABBCC80", "#FFFFFF"]));
}

#[test]
fn external_mask_preserves_every_unselected_byte_and_clips_stamp_noise_and_shading() {
    let original = image(&["#80504880", "#11223300", "#805048FF", "#010203"]);
    let mask = image(&["#00000001", "#FFFFFF00", "#000000FF", "#FFFFFF00"]);
    for operation in [
        json!({"op":"clear","color":"#FFFFFF"}),
        json!({"op":"stamp","canvas":image(&["#AABBCC";4]),"x":0,"y":0}),
        json!({"op":"noise","x":0,"y":0,"width":4,"height":1,"colors":["#AABBCC", "#DDEEFF"]}),
        json!({"op":"shade","steps":1,"ramps":[ramp()]}),
    ] {
        let edit = batch(
            json!({"version":1,"seed":42,"mask":{"source":"canvas","canvas":mask},"alpha_lock":true,"operations":[operation]}),
        );
        let mut a = original.clone();
        let mut b = original.clone();
        edit.apply(&mut a).unwrap();
        edit.apply(&mut b).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, original);
        assert_eq!(a.pixels()[1], original.pixels()[1]);
        assert_eq!(a.pixels()[3], original.pixels()[3]);
        assert_eq!(a.pixels()[0].a, 128);
        assert_eq!(a.pixels()[2].a, 255);
    }
}

#[test]
fn flood_fill_cannot_cross_a_mask_barrier_or_jump_to_an_island() {
    let mut canvas = image(&["#FFFFFF"; 5]);
    batch(json!({"version":1,"mask":{"source":"canvas","canvas":image(&["#000000","#000000","#00000000","#000000","#000000"])},
        "operations":[{"op":"fill","x":0,"y":0,"color":"#FF0000"}]})).apply(&mut canvas).unwrap();
    assert_eq!(
        canvas,
        image(&["#FF0000", "#FF0000", "#FFFFFF", "#FFFFFF", "#FFFFFF"])
    );
}

#[test]
fn masked_errors_and_coordinate_transforms_are_transactional() {
    for operation in [
        json!({"op":"resize","width":2,"height":2}),
        json!({"op":"flip","axis":"horizontal"}),
        json!({"op":"rotate","turns":1}),
        json!({"op":"move","dx":1,"dy":0}),
        json!({"op":"crop","x":0,"y":0,"width":1,"height":1}),
        json!({"op":"set_pixel","x":100,"y":0,"color":"#FFFFFF"}),
    ] {
        let mut canvas = image(&["#112233", "#33445500"]);
        let before = canvas.clone();
        let error = batch(json!({"version":1,"mask":{"source":"alpha"},"operations":[{"op":"clear","color":"#FFFFFF"},operation]})).apply(&mut canvas).unwrap_err();
        assert_eq!(canvas, before);
        assert_eq!(error.operation, Some(1));
    }
    let mut canvas = image(&["#112233"]);
    let before = canvas.clone();
    assert!(batch(json!({"version":1,"mask":{"source":"canvas","canvas":image(&["#FFFFFF";2])},"operations":[]})).apply(&mut canvas).is_err());
    assert_eq!(canvas, before);
}

#[test]
fn masked_nearest_mapping_cannot_modify_pixels_outside_selection() {
    let mut canvas = image(&["#010203", "#AABBCC"]);
    batch(
        json!({"version":1,"mask":{"source":"canvas","canvas":image(&["#FFFFFF00", "#FFFFFF"])},
        "palette":{"version":1,"colors":{"white":"#FFFFFF"}},"policy":"nearest","operations":[]}),
    )
    .apply(&mut canvas)
    .unwrap();
    assert_eq!(canvas, image(&["#010203", "#FFFFFF"]));
}

#[test]
fn companion_layers_reject_material_shading_and_keep_normal_vector_rules() {
    for channel in [Channel::Normal, Channel::Emissive] {
        let mut document = Document::new(1, 1).unwrap();
        document
            .add_layer_with_channel("map".into(), channel)
            .unwrap();
        let before = document.clone();
        assert!(
            document
                .edit(&batch(
                    json!({"version":1,"operations":[{"op":"shade","steps":1,"ramps":[ramp()]}]})
                ))
                .is_err()
        );
        assert_eq!(document, before);
    }
    let mut document = Document::new(1, 1).unwrap();
    document
        .add_layer_with_channel("normal".into(), Channel::Normal)
        .unwrap();
    document
        .edit(&batch(
            json!({"version":1,"operations":[{"op":"clear","color":"#8080FF"}]}),
        ))
        .unwrap();
    let before = document.clone();
    assert!(document.edit(&batch(json!({"version":1,"mask":{"source":"alpha"},"operations":[{"op":"clear","color":"#808080"}]}))).is_err());
    assert_eq!(document, before);
}

fn layered() -> Document {
    let mut document = Document::from_canvas(image(&["#C8643280", "#FFFFFF00"])).unwrap();
    document.add_layer("hidden".into()).unwrap();
    document
        .edit(&batch(
            json!({"version":1,"operations":[{"op":"clear","color":"#204080"}]}),
        ))
        .unwrap();
    document
        .configure_layer("hidden", Some(false), Some(128))
        .unwrap();
    document
        .add_layer_with_channel("glow".into(), Channel::Emissive)
        .unwrap();
    document
        .edit(&batch(
            json!({"version":1,"operations":[{"op":"clear","color":"#402010"}]}),
        ))
        .unwrap();
    document
        .add_layer_with_channel("normals".into(), Channel::Normal)
        .unwrap();
    document
        .edit(&batch(
            json!({"version":1,"operations":[{"op":"set_pixel","x":1,"y":0,"color":"#FF8080"}]}),
        ))
        .unwrap();
    document
}

#[test]
fn isolated_layer_views_distinguish_raw_opacity_and_effective_maps_without_mutation() {
    let document = layered();
    let before = document.clone();
    assert_eq!(
        document.render_layer(0, "hidden", true).unwrap(),
        image(&["#204080"; 2])
    );
    assert_eq!(
        document.render_layer(0, "hidden", false).unwrap(),
        image(&["#20408080"; 2])
    );
    assert_eq!(
        document.render_layer(0, "normals", true).unwrap(),
        image(&["#00000000", "#FF8080"])
    );
    assert_eq!(
        document.render_channel(0, Channel::Normal).unwrap(),
        image(&["#8080FF80", "#00000000"])
    );
    assert!(document.render_layer(1, "hidden", false).is_err());
    assert!(document.render_layer(0, "missing", false).is_err());
    assert_eq!(document, before);
}

fn light(direction: [f64; 3]) -> Light {
    Light {
        direction,
        ambient: 0.0,
        diffuse: 1.0,
        emission: 1.0,
    }
}

#[test]
fn linear_lighting_has_known_pixels_and_preserves_straight_alpha_once() {
    let color = image(&["#80402080"]);
    let normal = image(&["#8080FF80"]);
    let emission = image(&["#00000000"]);
    let full = preview::lighting(&color, &emission, &normal, light([0.0, 0.0, 1.0])).unwrap();
    assert_eq!(full, color);
    let dark = preview::lighting(&color, &emission, &normal, light([0.0, 0.0, -1.0])).unwrap();
    assert_eq!(dark, image(&["#00000080"]));
    let output = preview::lighting(
        &image(&["#00000080"]),
        &image(&["#80402040"]),
        &normal,
        Light {
            diffuse: 0.0,
            ..light([0.0, 0.0, 1.0])
        },
    )
    .unwrap();
    // Independent sRGB half-intensity reference: 128->92, 64->44, 32->20.
    assert_eq!(output, image(&["#5C2C1480"]));
    let transparent = preview::lighting(
        &image(&["#ABCDEF00"]),
        &image(&["#FFFFFF"]),
        &image(&["#00000000"]),
        light([0.0, 0.0, 1.0]),
    )
    .unwrap();
    assert_eq!(transparent, image(&["#00000000"]));
}

#[test]
fn lighting_direction_and_invalid_numeric_inputs_are_checked() {
    let color = image(&["#FFFFFF"]);
    let empty = image(&["#00000000"]);
    let normal = image(&["#FF8080"]);
    assert_eq!(
        preview::lighting(&color, &empty, &normal, light([1.0, 0.0, 0.0])).unwrap(),
        color
    );
    assert_eq!(
        preview::lighting(&color, &empty, &normal, light([-1.0, 0.0, 0.0])).unwrap(),
        image(&["#000000"])
    );
    for direction in [[0.0; 3], [f64::NAN, 0.0, 1.0], [f64::INFINITY, 1.0, 1.0]] {
        assert!(preview::lighting(&color, &empty, &normal, light(direction)).is_err());
    }
    assert!(
        preview::lighting(
            &color,
            &empty,
            &normal,
            Light {
                emission: f64::NAN,
                ..light([1.0, 0.0, 0.0])
            }
        )
        .is_err()
    );
    assert!(
        preview::lighting(&color, &empty, &image(&["#808080"]), light([1.0, 0.0, 0.0])).is_err()
    );
    assert!(
        preview::lighting(
            &color,
            &image(&["#00000000"; 2]),
            &normal,
            light([1.0, 0.0, 0.0])
        )
        .is_err()
    );
    assert!(preview::lighting(&color, &empty, &normal, light([f64::MAX; 3])).is_ok());
}

#[test]
fn review_bundle_reports_fallback_clipping_and_frames_exactly_and_refuses_overwrite() {
    let mut document = layered();
    document.add_frame("second".into(), Some(0), 100).unwrap();
    let before = document.clone();
    let root = tempfile::tempdir().unwrap();
    let output = root.path().join("review");
    let review = preview::write(&document, &output, 2, true).unwrap();
    assert_eq!(review.frames.len(), 2);
    for frame in &review.frames {
        assert_eq!(frame.color_pixels, 1);
        assert_eq!(frame.authored_normal_pixels, 0);
        assert_eq!(frame.fallback_normal_pixels, 1);
        assert_eq!(frame.clipped_emission_pixels, 1);
        assert_eq!(frame.reduced_emission_pixels, 1);
    }
    let sheet = storage::import_png(&output.join("contact-sheet.png"), false).unwrap();
    assert_eq!((sheet.width(), sheet.height()), (28, 4));
    let raw = storage::import_png(&output.join("frames/0000/normal.png"), false).unwrap();
    assert_eq!(
        raw.pixels()[0],
        Pixel {
            a: 128,
            ..FLAT_NORMAL
        }
    );
    assert_eq!(raw.pixels()[1], Pixel::TRANSPARENT);
    let manifest = std::fs::read(output.join("review.json")).unwrap();
    assert_eq!(
        preview::write(&document, &output, 2, true)
            .unwrap_err()
            .code,
        ErrorCode::AlreadyExists
    );
    assert_eq!(std::fs::read(output.join("review.json")).unwrap(), manifest);
    assert_eq!(document, before);
}

#[test]
fn review_limits_fail_before_creating_output() {
    let document = Document::new(1024, 1024).unwrap();
    let root = tempfile::tempdir().unwrap();
    for scale in [0, 64, u32::MAX] {
        let output = root.path().join("absent");
        assert!(preview::write(&document, &output, scale, true).is_err());
        assert!(!output.exists());
    }
}

#[test]
fn every_small_mask_matches_an_independent_flood_connectivity_reference() {
    // Exhaust all 3x3 binary masks, including disconnected islands and corners.
    for bits in 0u16..512 {
        let mut canvas =
            Canvas::from_pixels(3, 3, vec![Pixel::rgba(255, 255, 255, 255); 9]).unwrap();
        let mask = Canvas::from_pixels(
            3,
            3,
            (0..9)
                .map(|i| Pixel::rgba(0, 0, 0, if bits & (1 << i) != 0 { 255 } else { 0 }))
                .collect(),
        )
        .unwrap();
        batch(json!({"version":1,"mask":{"source":"canvas","canvas":mask},"operations":[{"op":"fill","x":1,"y":1,"color":"#FF0000"}]})).apply(&mut canvas).unwrap();
        let mut reached = [false; 9];
        if bits & (1 << 4) != 0 {
            reached[4] = true;
        }
        loop {
            let before = reached;
            for y in 0..3usize {
                for x in 0..3usize {
                    let index = y * 3 + x;
                    if bits & (1 << index) != 0
                        && (0..9).any(|other| {
                            before[other] && x.abs_diff(other % 3) + y.abs_diff(other / 3) == 1
                        })
                    {
                        reached[index] = true;
                    }
                }
            }
            if reached == before {
                break;
            }
        }
        for (index, pixel) in canvas.pixels().iter().enumerate() {
            assert_eq!(pixel.g == 0, reached[index], "mask {bits}, pixel {index}");
        }
    }
}

#[test]
fn masked_fill_resolution_and_bounds_errors_keep_the_operation_index() {
    for (x, color, code) in [
        (0, "missing", ErrorCode::InvalidInput),
        (-1, "#FFFFFF", ErrorCode::OutOfBounds),
    ] {
        let mut canvas = image(&["#112233"]);
        let before = canvas.clone();
        let error = batch(json!({"version":1,"mask":{"source":"alpha"},"operations":[
            {"op":"clear","color":"#000000"},{"op":"fill","x":x,"y":0,"color":color}]}))
        .apply(&mut canvas)
        .unwrap_err();
        assert_eq!(error.operation, Some(1));
        assert_eq!(error.code, code);
        assert_eq!(canvas, before);
    }
}

#[test]
fn untrusted_material_and_mask_schemas_reject_oversized_arrays_and_unknown_fields() {
    for operation in [
        json!({"op":"shade","steps":1,"ramps":vec![ramp();65]}),
        json!({"op":"shade","steps":1,"ramps":[{"name":"wood","colors":vec!["#FFFFFF";257]}]}),
    ] {
        assert!(
            serde_json::from_value::<Batch>(json!({"version":1,"operations":[operation]})).is_err()
        );
    }
    assert!(
        serde_json::from_value::<Batch>(
            json!({"version":1,"mask":{"source":"alpha","path":"../secret"},"operations":[]})
        )
        .is_err()
    );
}
