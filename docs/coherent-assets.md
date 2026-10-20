# Coherent asset families

Pixel Painter creates ordinary 2D pixel art with depth cues, shared palettes,
reproducible variants, and optional placement metadata. These are offline authoring
tools. Exported PNGs and JSON need no painter dependency at runtime.

The [Village Corner example](../rust/examples/village/README.md) contains working
inputs and actual CLI outputs for all features below. Build the CLI once:

```sh
cargo build --release --locked --manifest-path rust/Cargo.toml
```

Commands below run from the repository root. Output destinations must not exist;
choose new filenames/directories when trying them again. JSON paths referenced
inside scene and pack configs are relative to that config's directory. Exported
part paths are relative to their placement sidecar. References cannot be absolute,
contain `.`/`..`, backslashes or colons, or follow symlinks outside that directory.
Top-level CLI input/output arguments remain explicit caller-selected paths.

## Shared style

A version 1 profile contains a named palette and six required roles: `outline`,
`top`, `front`, `side`, `highlight`, and `shadow`. Each role names a palette entry.
See [profile.json](../rust/examples/village/profile.json) for a complete example.

Optional checks are `width`/`height` ranges (`min`, `max`), dimension multiples
(`grid`), `max_colors`, `alpha` (`any`, `binary`, `opaque`), and
`outline_thickness` (0 disables; 1–4 checks opaque boundary rings). Invisible RGB
does not count as visible color or an off-palette pixel. Alpha is part of each
palette color. Compositing translucent layers can create new colors; include
those colors or adjust the source before applying a strict profile.

```sh
./rust/target/release/painter style reference --profile rust/examples/village/profile.json --output /tmp/style-reference.png
./rust/target/release/painter style check --profile rust/examples/village/profile.json --input rust/examples/village/character/project.painter.json
```

`perspective` (`three_quarter`, `top_down`) and `light` (`northwest`, `northeast`,
`southwest`, `southeast`) describe artistic conventions. The reference shows role
swatches and a light direction marker. The validator checks measurable constraints;
it cannot establish perspective, lighting consistency, anatomy, or artistic quality.
Review the scene and contact sheets for those properties.

Checks return a versioned JSON report on stdout in both human and `--json` mode:
`valid`, `checked`, `issues`, and `omitted` (details beyond the 4,096-issue cap).
Each issue has a stable code, severity, asset, and explanation. Errors set exit 1;
warnings preserve exit 0. Invalid configuration/I/O uses the established CLI error
on stderr instead. Authoring commands support one success JSON object with `--json`; preview helpers
emit that object in both modes.

## Recipes

One version-2 recipe format supports seeded drawings, typed scalar parameters,
variants, embedded canvas components, and geometry helpers:

```json
{
  "version": 2,
  "width": 32,
  "height": 32,
  "parameters": {
    "size": {"type": "integer", "default": 18, "min": 8, "max": 24},
    "wall": {"type": "color", "default": "front"}
  },
  "variants": {"small": {"size": 12}},
  "operations": [{
    "shape": "prism", "x": 3, "y": 10,
    "width": {"param": "size"}, "height": 16, "depth": 4,
    "faces": {"top": "top", "front": {"param": "wall"}, "side": "side", "outline": "outline"}
  }]
}
```

A reference must be exactly `{"param":"name"}`. There are no expressions,
external includes, script execution, or implicit arithmetic. Defaults are overlaid
by the selected variant, then `--parameters` (a JSON name/value object); `--seed`
overrides the recipe seed. Unknown names, invalid defaults, and invalid variants
(including unselected variants) fail. Integer parameters must be within their
declared inclusive range. Color parameters accept the existing literal/named
color syntax. A recipe's `palette` takes precedence over an optional profile palette.
Optional `policy` applies the batch palette rules after rendering: `unrestricted`
(default), `strict`, or `nearest`. The latter two require an effective palette.
`generate --profile` also validates the rendered result before saving it.

```sh
./rust/target/release/painter --file /tmp/workshop.json generate --input rust/examples/village/recipes/cottage.json --variant workshop --profile rust/examples/village/profile.json
./rust/target/release/painter --file /tmp/workshop.json render --output /tmp/workshop.png --scale 4
```

`parts` maps names to ordinary canvas JSON. An operation
`{"op":"component","name":"window","x":10,"y":12,"flip_x":true,"over":true}`
copies a part. `flip_y` is also supported. `substitutions` contains `from` RGBA
pixels and `to` colors; mappings apply simultaneously, without cascading. Duplicate
source colors fail. Transparency is pasted exactly unless `over` is true.

## Depth geometry and shadows

Each shape expands into existing transactional polygon operations. All coordinates
are integers, with positive depth extending up/right. `prism` uses `(x,y)` as the
front face's top-left; `width` and `height` span that face and `depth` controls
its top/right sides. `roof` uses `(x,y)` at the left eave and `height` as rise.
`cottage` uses `(x,y)` at the front facade's bottom-left, plus `roof_height`.
`steps` adds `count` raised prisms: each rises by `height` and advances by `depth`.
The `faces` object supplies `top`, `front`, `side`, and optional `outline` colors;
`cottage` has an additional `roof` faces object. Surfaces draw back to front.

Dimensions are 2–4,096, depth 1–1,024, and steps 1–32. Resulting points must stay
within the existing ±1,000,000 drawing-coordinate bounds. Shapes can be included
in recipes or applied to the selected cel with `geometry --input SHAPE.json`
and optional `--palette PALETTE.json`. Recessed doors, windows, slats, and other
details remain reusable parts or ordinary drawing operations.

`shadow --input ARTWORK --options OPTIONS.json --output SHADOW.png` projects the
source silhouette into a separate PNG. Options are `width`, `height`, `origin`
and `step` points, `steps` (0–64), `squash_y` (1–64), and RGBA `color`.
Projection moves down/right for positive step components. Coverage preserves source
alpha multiplied by shadow alpha; overlapping samples take maximum coverage,
avoiding repeated darkening. Hidden RGB contributes nothing. Place the resulting
PNG beneath the asset explicitly. Direction and material ramps remain artist choices.
See [the example options](../rust/examples/village/shadow.options.json).

## Placement and split layers

Placement is an optional authoring sidecar supplied to `project export PATH --placement PLACEMENT.json`:

```json
{
  "version": 1,
  "anchors": {
    "ground": {"x": 24, "y": 58},
    "attachments": {"branch": {"x": 29, "y": 30}},
    "footprint": [{"x":18,"y":54},{"x":31,"y":54},{"x":31,"y":59},{"x":18,"y":59}]
  },
  "parts": [
    {"id":"trunk","layers":["base","trunk"],"order":0},
    {"id":"canopy","layers":["canopy"],"order":2}
  ]
}
```

An optional `frames` map overrides anchors by frame name. Ground falls back to
the frame pivot; attachments merge by name; a nonempty frame footprint replaces
the shared footprint. Resolved frames allow at most 32 attachment names, including
inherited names. Anchors need not lie inside visible sprite bounds. Footprints
are visual placement metadata, not collision detection. Parts require unique IDs
and disjoint nonempty selections of existing layers; visibility and opacity apply.

The export adds `placement.json` (dimensions, resolved per-frame anchors, parts)
and `parts/NNNN/NNNN.png`. Part images use the full native canvas dimensions so
all anchors retain the same coordinates. The flattened frames remain available.
Use metadata paths, rather than deriving filenames from names. The
`metadata.json` completion marker is published after every extra file succeeds.

## Scenes and contact sheets

A scene version 1 declares `width`, `height`, optional `background` RGBA or
`tile` input, optional `y_sort`, and 1–256 `assets`. Each entry has a unique `id`,
`input`, `position` point, and optional `scale` (1–64), `order`, `frame`, `anchor`,
`placement` path, and `part` ID. Input can be PNG, canvas, or project JSON.
The chosen anchor lands at `position`. An explicit anchor wins over the placement
ground; otherwise bottom center is used. PNG inputs use frame 0; select the relevant
PNG path or a named exported part for other frames. Projects use the requested frame.

Draw order sorts by entry `order` plus part `order`, then ground position Y when
`y_sort` is true, then original list position. Larger order/Y draws later. Background
tiles repeat from `(0,0)`. Assets may clip at scene edges.

```sh
./rust/target/release/painter scene render --input rust/examples/village/scene.json --output /tmp/village.png --scale 4
./rust/target/release/painter scene contact-sheet --input rust/examples/village/scene.json --output /tmp/scene-assets.png --columns 8 --padding 2 --scale 2
```

Contact sheets use uniform cells sized to the largest image, retaining transparent
margins and input order. Animation contact sheets order tagged frames first,
deduplicate shared frame indices, then append untagged frames. Nearest-neighbor
scaling preserves native pixels. This is a review compositor, with no runtime
camera, world simulation, physics, or dynamic lighting.

## Animation QA

Profile `animation` rules support `required_tags`, optional ground `baseline`,
`margin`, `max_size_delta`, and named `movement_exceptions`/`clipping_exceptions`.
An inferred baseline comes from the first anchored frame outside motion exceptions.
If any baseline exists, every nonexempt frame requires an anchor. Baseline drift and missing required tags are errors;
clipping margins and occupied-size drift are warnings. Size comparisons use the first
nonempty frame, so directional silhouette changes may require human review.
Exceptions must name existing frames; exempted motion still receives style checks.

```sh
./rust/target/release/painter animation check --input rust/examples/village/character/project.painter.json --profile rust/examples/village/profile.json
./rust/target/release/painter animation contact-sheet --input rust/examples/village/character/project.painter.json --output /tmp/animation.png --scale 6
```

## Packs and integrity checks

See [pack.recipe.json](../rust/examples/village/pack.recipe.json). A pack recipe
version 1 has `seed`, optional `profile` path, and 1–64 uniquely named assets.
Sources are `{"kind":"artwork","input":"PATH"}` or
`{"kind":"recipe","input":"PATH","variant":"NAME","parameters":{},"seed":47}`.
Each asset optionally supplies a placement source path. Without a source seed,
recipe seed is pack seed plus asset list index, wrapping as u64. Reordering assets
therefore changes their default seeds; use explicit source seeds when needed.

```sh
./rust/target/release/painter pack generate --input rust/examples/village/pack.recipe.json --output /tmp/village-pack
./rust/target/release/painter pack check --input /tmp/village-pack
```

Packs include editable source projects, placement sources, a shared profile,
per-asset complete exports, a contact sheet, a generation report, and `pack.json`.
Publication reserves a new directory and writes `pack.json` last. Invalid profiles
or failed assets leave no successful output bundle. `pack check` rebuilds each
export from its stored source and compares generated files byte-for-byte, including
frame timing, tags, APNGs, previews, and split layers. It also verifies the contact
sheet and generation report. Extra unrelated files are ignored. Recompressed or
hand-edited generated exports intentionally fail; regenerate after changing sources.
An explicit `--profile` overrides the stored profile for additional validation.
Checks establish internal consistency, not authenticity or cryptographic trust.

## Limits and pre-1.0 changes

Authoring configs have a 4 MiB byte cap, strict unknown-field rejection, and
bounded lists/maps. Recipe limits are 64 parameters, 32 variants, 64 parts,
10,000 expanded operations, 100,000 expansion nodes, and 32 nested levels. Retained
and expanded component pixels are each capped at 16,777,216. Scene and pack source artwork
share a 16,777,216-pixel aggregate cap, counting every frame/layer cel before
flattening; scene backgrounds count too. Scaled scene assets have a separate
16,777,216-pixel aggregate cap. Referenced source/config file lengths share a
2 GiB allowance, charged even when decoding fails; per-file caps still apply.
Existing canvas/output pixel limits still apply. A pack shares one 67,108,864-work
raster allowance across all recipes, including component preparation.
Style/animation scan budgets include outline neighbors; large outline checks may
fail before scanning. Shadows have a separate projection work preflight. These are
bounded offline operations, not a sandbox for hostile concurrent filesystem writers.

Before 1.0, APIs and formats may change between minor releases; the changelog
records breaking changes. The former nested `batch` templates are replaced by
the version-2 recipe schema. All checked-in recipe inputs use that schema. Generated pixels remain regression-tested. Authoring modules are
available without Clap when the default `cli` feature is disabled.
