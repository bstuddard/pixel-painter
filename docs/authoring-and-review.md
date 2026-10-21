# Material shading, masks, and review bundles

Available in v0.4.0. Start with the [editable workshop](../rust/examples/workshop/README.md).
All features work offline in the CLI and the Rust library, with no new runtime dependencies.

## Material palette ramps

`shade` moves each visible pixel along an explicitly named, dark-to-light ramp.
Materials are identified by exact RGB membership, rather than inferred from an
image. Each RGB must occur once across all ramps in that operation, including
within a ramp. Unmatched colors and transparent pixels stay unchanged. Positive
steps lighten; negative steps darken; the endpoints clamp. Alpha is preserved.

```json
{
  "version": 1,
  "operations": [{
    "op": "shade",
    "steps": 1,
    "ramps": [
      {"name": "wood", "colors": ["#623C40", "#A85F46", "#D99460"]},
      {"name": "steel", "colors": ["#3E5169", "#728DA0", "#B6CFCD"]}
    ]
  }]
}
```

Save as `shade.ops.json`, then run:

```sh
painter --file chest.json --layer base batch --input shade.ops.json --dry-run
painter --file chest.json --layer base batch --input shade.ops.json
```

Ramp entries can be named colors in the batch palette. Ramps require 2..=256
opaque colors and unique material names; one operation accepts 1..=64 ramps.
Shading is restricted to color layers. It never shades encoded normal vectors or
emission. The batch's final palette policy still applies: use unrestricted or
a matching strict palette to retain exact ramp colors. Global nearest mapping
can remap the result to any palette entry. A mask can limit shading to one region.
Recipes also accept the `shade` operation.

This adds explicit material-specific shading to v0.3's global nearest-palette
mapping and component recoloring. It does not infer materials or simulate PBR.

## Mask-constrained drawing and brushes

Existing lines, polygons, rectangles, ellipses, fills, noise, and component stamps
can use a selection. A mask must match the target cel's dimensions. Any nonzero
mask alpha selects a pixel; mask RGB and partial alpha do not blend paint.

```sh
painter --file sentinel.json draw --mask cloak-mask.png --alpha-lock line --x1 4 --y1 38 --x2 44 --y2 47 --color '#B4D77C'
painter --file sentinel.json stamp --input moss-brush.json --x 18 --y 34 --over --mask cloak-mask.json --alpha-lock
```

CLI masks accept PNG or flat canvas JSON. `--alpha-lock` additionally selects
only pixels that were covered before the command and preserves their alpha.
Transparent replacement brush pixels and erase operations become no-ops while
alpha is locked. Use `--over` for source-over RGB blending of partially opaque
brushes; normal layers continue to reject paint blending.

Batch JSON supports a reusable selection for every operation:

```json
{
  "version": 1,
  "mask": {"source": "alpha"},
  "alpha_lock": true,
  "seed": 42,
  "operations": [{
    "op": "noise", "x": 0, "y": 0, "width": 48, "height": 64,
    "colors": ["#82B66C", "#518369"], "density": 40
  }]
}
```

Alternatively use `{"source":"canvas","canvas":{...}}` with an embedded canvas.
CLI `batch --mask PATH` overrides that source and `--alpha-lock` enables locking.
The selection is captured once before the batch. Every unselected RGBA byte is
preserved, including hidden RGB. Flood fills cannot traverse unselected barriers.
Noise uses the existing seed stream, consuming samples at the original rectangle
positions even outside the selection. Masking therefore does not reposition its texture.

Coordinate/dimension transforms are rejected in masked or alpha-locked batches;
apply them separately before capturing the selection. Copy remains a read-only
snapshot of its full rectangle. Palette validation retains the existing whole-canvas
contract, while nearest mapping cannot change unselected pixels. Any late failure
rolls back the entire batch. Masks, embedded brushes, clipboard storage, and
raster work remain bounded.

## Explicit layer inspection

The existing global `--layer` selects an edit target. Use `--solo-layer` to inspect
one cel without changing visibility or the saved active layer:

```sh
painter --file chest.json render --solo-layer base --output solo.png --scale 4
painter --file chest.json render --solo-layer normals --raw --output raw-normals.png --scale 4
painter --file chest.json render --channel normal --output effective-normals.png --scale 4
```

- **Solo:** includes the named layer even when hidden and applies its opacity.
  Other layers, coverage clipping, and normal fallback are absent.
- **Raw:** exact cel RGBA, ignoring layer opacity and visibility. Requires solo.
- **Channel:** composites visible layers using the export contract, including
  color coverage, emission clipping, and flat normals for missing directions.

`--channel` and `--solo-layer` conflict. Global `--frame` selects the inspected
frame. These commands never save the project. Existing output files are refused
unless `render --replace` is explicitly supplied; input artwork remains protected.

## Built-in lighting review

```sh
painter --file chest.json preview --output chest-review --all-frames --scale 4
```

Open `chest-review/contact-sheet.png`. Each frame occupies one row; columns are
**color**, **effective emission**, **effective normals**, **unlit + emission**,
**left light**, **right light**, and **top light**. Frame order is project order;
without `--all-frames`, only the selected frame is included. Flat canvases are
also accepted, with empty emission and fallback normals.

`frames/0000/*.png` retains native resolution. Only the contact sheet is enlarged.
`review.json` records column names, light parameters, original frame indices,
durations, and these pixel counts before export coverage is applied:

| Field | Meaning |
|---|---|
| `color_pixels` | Nonzero visible color coverage |
| `authored_normal_pixels` | Covered color pixels with a visible authored normal |
| `fallback_normal_pixels` | Covered pixels requiring a flat normal |
| `clipped_emission_pixels` | Visible emission pixels completely outside color coverage |
| `reduced_emission_pixels` | Visible emission pixels under partial color coverage |

Lighting decodes color and emission from sRGB, normalizes linear UNORM8 normals,
and computes directional Lambert diffuse plus ambient and emission. Output is
straight-alpha sRGB. Emission coverage is applied once. There is no bloom,
specular material model, cast shadow, or interactive renderer. Light parameters
are recorded for reproducibility; custom lights are available through the library.

Review bundles refuse existing destinations. All dimensions, combined view
pixels, scale (1..=64), and work are checked before output. The shared limits are
16,777,216 pixels per image and across native views, with 67,108,864 raster work
units. Large animations can be reviewed one frame at a time or at a smaller scale.
Files are staged next to the destination and `review.json` is published last as
the completion marker. No source path, local absolute filename, or environment
value is written into the bundle.
