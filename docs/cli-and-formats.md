# CLI and version-1 formats

Run `painter --help` or any subcommand's `--help` for arguments. Existing `new`,
`set-pixel`, `print`, `view`, and `export` commands remain supported.
Global flags can occur before or after subcommands:

- `--file PATH`: working artwork, default `canvas.json`.
- `--json`: structured results on stdout and errors on stderr. `inspect` and
  `project show` already emit JSON. `print --json` emits canvas data; `view --json`
  emits inspection data rather than ANSI escapes. Help/version remain plain text.
- `--frame INDEX`, `--layer NAME`: override project selection for commands that
  read the working `--file`. Successful edits save those selections; dry-run
  never saves them. The `project` subcommands use their explicit path and action.

Paths in JSON responses remain strings when they contain valid UTF-8. Otherwise
they are objects with a readable `display` plus a lossless platform representation:
`{encoding:"unix_bytes",bytes:[...]}` on Unix, or
`{encoding:"windows_wide",units:[...]}` on Windows. Native filenames are accepted
in human and JSON modes; response formatting never converts a successful save
into a panic because of its filename.
Human status and error messages escape control characters so filenames cannot
inject terminal commands. Ordinary Unicode and path separators remain readable;
JSON path/error values retain the original data through normal JSON serialization.

## Generate, render, inspect, revise, export

Use `painter` below as shorthand for your executable. In a source checkout, build
from `rust/` with `cargo build --release --locked` and substitute
`target/release/painter`. In a binary download, run from the extracted root and
substitute `./painter` (`.\painter.exe` in Windows PowerShell).

```sh
painter --file gem.json new --width 8 --height 8
painter --file gem.json batch --input tests/fixtures/gem.ops.json --dry-run --json
painter --file gem.json batch --input tests/fixtures/gem.ops.json --json
painter --file gem.json render --output gem-preview.png --scale 16
painter --file gem.json inspect --json
painter --file gem.json draw rect --x 0 --y 0 --width 2 --height 2 --fill --color '#7299BB'
painter --file gem.json render --output gem-preview.png --scale 16 --replace
painter --file gem.json export --output gem.png
```

All creation/import/generation/extraction/snapshot/export commands refuse to
overwrite existing destinations. `render --replace` permits atomic preview
replacement and also creates a missing preview, while protecting the source.
Edit commands replace their working file only after success. Run edits sequentially;
there is no locking or conflict resolution between concurrent processes.

## Batches

```json
{
  "version": 1,
  "seed": 42,
  "palette": {"version": 1, "colors": {"ink": "#18243C", "cloth": "#5864A0"}},
  "policy": "strict",
  "operations": [
    {"op":"fill_rect","x":2,"y":2,"width":12,"height":12,"color":"cloth"},
    {"op":"line","x1":2,"y1":2,"x2":13,"y2":2,"color":"ink"}
  ]
}
```

`version` and `operations` are required. `seed` defaults to zero, `palette` to null,
and `policy` to `unrestricted`. Unknown fields/operations are errors. Colors accept
`#RRGGBB`, `#RRGGBBAA`, `{r,g,b,a}` byte objects (`a` defaults to 255), or a palette
name. Output always serializes pixels as RGBA objects, preserving legacy JSON.

Every operation is ordered. A later failure or palette violation rolls back the
whole batch. The CLI saves once. Dry-run executes the same algorithm on temporary
data. `changed_pixels` counts final RGBA differences, not pixel writes; if dimensions
change it reports the final pixel count. Palette enforcement checks final pixels.

| `op` | Required fields | Optional fields |
|---|---|---|
| `set_pixel` | `x,y,color` | `over` |
| `line` | `x1,y1,x2,y2,color` | `over` |
| `rect`, `ellipse` | `x,y,width,height,color` | `fill,over` |
| `fill_rect` | `x,y,width,height,color` | `over` |
| `polygon` | `points:[{x,y},...],color` | `fill,over` |
| `erase` | `x,y,width,height` | — |
| `clear` | — | `color` (default transparent black) |
| `fill` | `x,y,color` | — |
| `copy` | `name,x,y,width,height` | — |
| `paste` | `name,x,y` | `over` |
| `stamp` | `canvas,x,y` | `over` |
| `flip` | `axis` (`horizontal` or `vertical`) | — |
| `rotate` | `turns` (clockwise quarter turns) | — |
| `move` | `dx,dy` | — |
| `crop` | `x,y,width,height` | — |
| `resize` | `width,height` | — |
| `noise` | `x,y,width,height,colors` | `density` (default 255) |

`fill` is flood fill. All booleans default to false. `set_pixel` and flood-fill
origins must be in bounds; shapes and pastes clip. Copy/crop return transparent
pixels outside the source. Clipboard names are local to a batch and copying an
existing name replaces its snapshot. `stamp.canvas` is a complete validated legacy
canvas object, so components remain ordinary reusable files. The CLI `extract`
and `stamp --input` avoid manually embedding canvas JSON.

For precise rasterization, alpha, and limits see [the roadmap](roadmap.md).
Palette `strict` rejects visible off-palette pixels; `nearest` maps visible pixels
by squared RGBA distance with sorted-name tie breaking. Zero-alpha pixels retain
hidden RGB. Alpha is not silently preserved during nearest mapping: the selected
palette entry supplies all four channels. `--palette`/`--policy` override a batch's
embedded settings. A constrained policy requires a palette.

Palette enforcement shares the batch's work budget. It charges a scan unit for
each pixel and a candidate unit for each palette entry on a cache miss. A bounded
65,536-entry cache accelerates repeated colors; overflow recomputes misses within
the same budget. Mapping budget failures roll back the batch. The standalone
`Palette::constrain` API also preserves its input on a mapping failure.

## Templates and deterministic noise

A template is `{version:1,width,height,batch:<version-1 batch>}`. Run:

```sh
painter --file slime.json generate --input examples/gallery/04-seeded-slimes/slime.template.json --seed 42
```

Noise uses SplitMix64 with u64 wrapping arithmetic. For each **visible** pixel in
row-major order, consume `s = next()`. Paint if `s % 255 < density` (0 paints none;
255 paints all), then consume another sample and choose `colors[next() % len]`.
Operations share one stream per batch. Clipping, density, and operation order can
change subsequent stream consumption; seed alone is not the complete input.

SplitMix64 increments state by `0x9E3779B97F4A7C15`, then applies the xor/shift
and multiply steps `(z ^ (z >> 30)) * 0xBF58476D1CE4E5B9`,
`(z ^ (z >> 27)) * 0x94D049BB133111EB`, and `z ^ (z >> 31)`.

## Layered projects and animation

```sh
painter project new hero.painter.json --width 32 --height 32
painter project layer hero.painter.json add clothes
painter --file hero.painter.json batch --input clothes.ops.json
painter project frame hero.painter.json add walk-1 --duplicate 0 --duration-ms 120
painter project frame hero.painter.json set 1 --pivot-x 16 --pivot-y 31
painter project tag hero.painter.json set walk-south --frames 0,1 --direction south
painter project export hero.painter.json --output hero-export --columns 2 --padding 2 --scale 8
```

`project migrate legacy.json hero.painter.json` explicitly creates a new project.
`snapshot --output checkpoint.json` saves a separate checkpoint. Copy a checkpoint
to a new working path to resume it; snapshots are not full undo/redo history.

Projects serialize `{version,width,height,layers,frames,tags,active_layer,active_frame}`.
Layers have `{name,visible,opacity}` and are ordered bottom-to-top. Frames have
`{name,duration_ms,pivot,cels}`; each cel is a legacy canvas matching project size,
and cel order matches layer order. Names are unique within layers and within frames.
Tags map names to `{frames:[indices...],direction:null|string}`. A pivot is null or
`{x,y}` in native pixels and may lie outside the frame. Reordering or deleting
frames remaps tags by frame identity and removes tags that become empty.

Layer `merge` bakes the chosen layer into the one beneath it across all frames.
If integer rounding would change the visible composite over still-lower layers,
the merge fails with `merge_changes_composite` and leaves the project untouched.
Use visibility/opacity via `project layer PATH set NAME --visible false --opacity 128`.

Batch edits target the selected cel; geometry must remain the project's size.
`project resize PATH --width W --height H` resizes all cels together. Pivots remain
explicit native-pixel values; adjust them with `frame set` after a resize if needed.

Exported bundles contain native `frames/0000.png` files, `sheet.png`, enlarged
`preview.png`, APNG `animation.png` for temporal projects with multiple frames,
and a `metadata.json` completion marker. Metadata has version, image/preview names,
sheet dimensions, frames with file/rect/duration/pivot, and animation tags. Grid
packing never trims transparent frame boundaries or rotates cells. Transparent
padding is a spacing option; texture samplers should still use nearest filtering.
See [Macroquad integration](../integrations/macroquad/README.md) for a real consumer.

Require `metadata.json` before reading an export. Publication failures clean up
ordinary partial bundles; process/power failure can leave a directory without a
manifest. Inspect and remove that incomplete directory before retrying. Existing
directories are never replaced, including empty ones.

## Tiles

```sh
painter --file terrain.json tiles --tile-width 16 --tile-height 16 seams --rules connections.json
painter --file terrain.json tiles --tile-width 16 --tile-height 16 export --output tiles-export --columns 4 --padding 2 --rules connections.json
```

Source margin/spacing are `--source-margin`/`--source-spacing`, before the tile
subcommand. The remaining source grid must contain only complete tiles. IDs are
zero-based row-major indices. A rules file is
`{version:1,connections:[{from:0,to:1,edge:"right"}]}`; edges are `right` and `down`.
Seams compare exact RGBA across opposing borders, including hidden RGB. A failed
check emits its report to stdout and `seam_mismatch` to stderr with exit code 1.
Checks allow at most 16,384 rules and 67,108,864 total pixel comparisons, charging
tile height for each `right` rule and tile width for each `down` rule. An excessive
request fails with `resource_limit` before comparing edges or emitting a report.
Export can carry validly structured rules even when seams mismatch; check first
when seamlessness is required. Tile collections do not produce animation files.

## Errors and resource limits

With `--json`, failures emit one stderr object:

```json
{"version":1,"ok":false,"error":{"code":"out_of_bounds","message":"pixel is outside canvas","operation":2}}
```

`operation` is present for execution/work-budget failures within a decoded batch;
it is absent for JSON decoding or final palette validation. Codes include
`invalid_arguments`, `invalid_json`, `invalid_input`, `unsupported_version`,
`out_of_bounds`, `resource_limit`, `palette_violation`, `not_found`, `already_exists`,
`permission_denied`, `io_error`, `invalid_png`, `png_error`, `seam_mismatch`,
`merge_changes_composite`, and `incomplete_export`. Command errors exit 1; argument
parsing errors exit 2. Successful results contain a format version; legacy canvas
data itself remains intentionally unversioned for compatibility.

See the [resource boundary table](roadmap.md#resource-and-compatibility-boundaries).
Configuration JSON (batches, templates, palettes, metadata) is capped at 64 MiB.
Artwork JSON is capped at 2 GiB and streamed: a metadata-only pass validates actual
pixel counts and aggregate cel storage before the typed raster allocation pass.
Both passes read the same open file, limit nesting to 128, and reject encoded
string literals above 4096 bytes. Artwork saves retain legacy pretty JSON and can
represent the full advertised pixel budget. Use `storage::load`/`save`,
`load_artwork`/`save_artwork`, or `load_project`/`save_project` for artwork;
generic `read_json`/`write_json` retain the smaller configuration limit.
Direct serde callers should bound their reader too; individual bounded sequences
do not provide an aggregate parser memory cap.
