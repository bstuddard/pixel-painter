# Color, emissive, and normal exports

Unreleased after v0.2.0; use a build containing the three-map changes.

Projects can author three semantic layer channels. Color remains the visible
artwork; emissive and normal layers never enter ordinary flattening, previews,
style checks, split color parts, or legacy single-PNG exports.

```sh
painter project layer hero.painter.json add glow --channel emissive
painter project layer hero.painter.json add normals --channel normal
painter project export hero.painter.json --output hero-lit --maps emissive,normal
```

Select these layers with the existing layer command, then use normal draw/batch
tools on their active cels. `--maps emissive` and `--maps normal` are also valid.
Unrequested maps produce no files. Unknown names, repeated names, and `color` as
a companion request fail. Layer duplication preserves its channel; merging
different channels fails. Same-channel merges preserve their composite exactly.

## Texture and coverage contract

All textures are native RGBA8. They share one packing layout: dimensions, frame
order, rectangles, padding, durations, pivots, and tags. There is no independent
trimming, rotation, or packing. `--scale` affects only the existing color preview.

| Channel | Sheet and frame paths | RGB | Alpha |
| --- | --- | --- | --- |
| Color | `sheet.png`, `frames/0000.png` | Existing artwork | Existing coverage |
| Emissive | `emissive/sheet.png`, `emissive/frames/0000.png` | sRGB emission color | Strength multiplied by color coverage |
| Normal | `normal/sheet.png`, `normal/frames/0000.png` | Linear XYZ UNORM8 | Exactly the color coverage |

Color and emissive layers use the existing integer source-over compositor within
their own channels, including visibility and opacity. Emission is then clipped
to the final color silhouette: `round(emission_alpha * color_alpha / 255)`.
Zero resulting alpha canonicalizes to transparent black. Black RGB emits no light.
Layer ordering across channels does not assign materials to individual color
layers or infer occlusion. Paint the emission mask explicitly.

Normals use **topmost visible vector wins**. A cel's alpha times layer opacity
determines whether it contributes; nonzero coverage selects its direction without
mixing RGB. Export alpha is the final color alpha. Missing normals become
`(128,128,255)` over visible color. All maps and padding are transparent black
outside that silhouette. Hidden normal cels are validated too.

Normal axes are +X right, +Y up, +Z toward the viewer. Encode a unit direction as
`round((component + 1) * 127.5)`; decode with `normalize(2 * RGB / 255 - 1)`.
Validation permits 3% error in squared length for byte quantization. Zero-length,
nonfinite, and ordinary nonunit painted normals fail. Transparent RGB is ignored.

Normal cel flips both move pixels and invert the relevant vector component.
Positive quarter turns rotate clockwise in image coordinates; their screen-space
XY vectors rotate accordingly. Copy, move, crop, and nearest-neighbor resize retain
directions. Normal drawing replaces vector data: `over: true` is rejected instead
of blending encoded RGB. Both map channels reject strict/nearest palette policies;
named palette colors can still be resolved with unrestricted policy.

## Geometry-generated normals

Geometry uses one face decomposition for color and normals, including polygon
masks, draw order, transparency, and outlines. There is no normal inference from
a finished color image. Existing recipe geometry uses the same decomposition.

```sh
painter project layer house.painter.json add surfaces --channel normal
painter project layer house.painter.json select base
painter project geometry house.painter.json --input geometry.json --normal-layer surfaces
painter project export house.painter.json --output house-lit --maps normal
```

`geometry.json` is one existing `prism`, `roof`, `cottage`, or `steps` shape object,
as used inside recipes. `--palette` resolves named face colors. Omitting
`--normal-layer` draws color alone. The active layer must be color and the target
must be normal. Both cels commit together under one raster-work budget.

Stylized face directions are front `(0,0,1)`, top `(0,1,1)`, side `(1,0,1)`, and
roof planes `(-0.5,sqrt(0.5),0.5)` / `(0.5,sqrt(0.5),0.5)`, normalized before
byte quantization. Outlines inherit their owning face direction. Later color
decoration retains existing surface normals; visible pixels without surface
information use the flat fallback. Manual normal edits can override any face.
These are lighting approximations, not reconstructed 3D geometry.

## Metadata, packs, and safety

`metadata.json` adds an optional `maps` object. Each requested entry contains
`sheet`, `frames`, and `encoding`. Emissive uses `rgba8_emission`; normal uses
`xyz_unorm8` plus `space: screen`, `positive_y: up`, and `default: flat`.
The frame paths follow the existing metadata frame order and reuse its rectangles.

This is an additive pre-1.0 version-1 schema extension. Layer `channel` defaults to
`color` and is omitted for color layers; `maps` is omitted when empty. Existing
color-only sources and exports retain their bytes. Older strict readers can reject
new map-enabled sources; there is no duplicate format reader or compatibility
renderer. Use the new binary when authoring maps.

Pack assets may request `"maps": ["emissive", "normal"]`. The manifest retains
that request so pack checks rebuild and compare every companion PNG and metadata
file. Placement parts remain color selections; selecting a map layer as a part
fails. Per-part companions are deferred.

Native frame pixels summed across color and requested maps, and atlas pixels
summed across those channels, must each fit the existing 16,777,216-pixel cap.
Layout and these limits are checked before rendering. Source cels still share
the existing project/input budgets. Generated paths contain fixed channel names
and numeric frame indices; artwork names never become output paths.

Exports stage all files and publish `metadata.json` last. Existing output
directories are refused, failures leave no successful completion marker, and
placement export checks stale companion data as well as stale color data.
The existing local-filesystem/concurrent-writer limitations still apply.

## Review and runtime lighting

See [editable examples](../rust/examples/lighting/README.md), especially
`overview.png`, the three `review.png` files, and exported normal/emissive sheets.
The deterministic Python generator uses painter for authoring and export, with a
small separate CPU lighting reference for static review images.

The independent [Macroquad demo](../integrations/macroquad/README.md) samples all
three textures through one frame rectangle, uses nearest sampling, converts
color/emission RGB through sRGB transfer functions, and leaves normal RGB linear.
Its shader corrects normal directions when sprites flip. GPU readback tests compare
the shader against known CPU values for modes, flips, fallbacks, and partial alpha.

For engines using straight-alpha blending, remember that emission alpha already
contains color coverage. Divide it by nonzero color alpha to recover straight
emission strength before the final color-alpha blend. Otherwise coverage would be
applied twice. A linear premultiplied formulation is
`lit_color * color_alpha + emission_color * emission_alpha`.

Emission does not produce bloom, halos, cast shadows, or light occlusion. Those
remain runtime effects. Additional material maps, curved-surface generators, and
per-part companions are outside this implementation. The demo remains experimental
with its previously documented upstream advisories.
