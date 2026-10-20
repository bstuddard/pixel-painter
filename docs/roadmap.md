# Pixel Painter capability roadmap

Repository: https://github.com/bstuddard/pixel-painter

**Status:** All six implementation phases are implemented locally in v0.2.0.
Linux verification passes; native Windows/macOS checks and a graphical consumer
smoke test are configured in CI. Full undo/redo and automatic map synthesis remain
outside this roadmap, as specified below.

This is the implementation contract for the roadmap reviewed on 2026-10-19.
The original baseline was v0.1.1 at commit `4e97077`. Implementation stays in
one Rust package, with a reusable library and a supported `painter` CLI.

The pre-1.0 policy now allows API and file-format breaks between minor releases;
the changelog records them. The coherent-authoring update consolidates template
formats into one recipe schema rather than retaining parallel readers.

## Principles and public contracts

- Keep flat RGBA canvases, drawing commands, native PNG pixels, and refusal to
  overwrite newly created artwork or exports.
- Every public canvas construction/deserialization path validates dimensions
  and pixel count. Invalid storage data cannot become a usable `Canvas`.
- Version batches, palettes, recipes, projects, tile rules, inspection reports,
  errors, and atlas metadata. Reject unsupported schema versions, unknown fields,
  and unknown operations in input documents.
- Batch edits compute in memory and commit once. Failures leave the original
  canvas and saved file unchanged. Dry-run computes the same result without saving.
- JSON command errors appear on stderr as `{version, ok, error}`; errors carry
  a stable code, message, and zero-based operation index where applicable.
- Use integer rasterization, ordered operations, sorted palette names, and the
  specified SplitMix64 stream. Reproducibility covers pixels and metadata.
- Drawing replaces RGBA by default. `over: true` explicitly requests source-over.
  Legacy `set-pixel` always replaces RGBA. Layer compositing uses straight alpha,
  byte sRGB arithmetic, and integer rounding. No filtering or color correction.
- Shapes use signed coordinates and clip to the canvas. Rectangles are half-open;
  lines include both endpoints and canonicalize endpoint order. Ellipses test pixel
  centers; outlines use the inside pixels adjacent to an outside 4-neighbor.
  Polygons use even-odd pixel-center fill plus Bresenham boundaries, blending once.
- Flood fill uses exact RGBA equality and four-connected neighbors. Region copies
  are snapshots, including hidden RGB; off-canvas source samples are transparent.
- All work is local. The core has no game engine, GUI, cloud, or image-generation
  dependency. Components and animation directions remain generic user data.

## Implementation sequence and acceptance

### 0. Validated reusable core

Expose `Canvas`, `Pixel`, errors, persistence, and drawing through `lib.rs`.
Keep CLI dispatch separate from editing. Preserve legacy CLI integration tests.
Add library-only creation/export and malformed-deserialization regressions.
Keep `main.rs` as the process entry point, `cli.rs` for arguments, and `app.rs`
for dispatch/presentation; reusable behavior belongs in the library modules.

Acceptance: legacy workflows pass unchanged; invalid JSON never creates an
invalid public canvas; library callers can create and export without a process.

### 1. Minimum complete agent workflow

Implement lines, rectangles, versioned ordered batches, indexed errors, dry-run,
inspection, and nearest-neighbor previews together. Establish the schema before
expanding the operation set. Check in a small hand-specified golden fixture.

Acceptance: one batch creates the fixture; dry-run matches the committed summary;
decoded pixels match the fixture; a failing final operation changes no saved bytes.

### 2. Complete drawing and image operations

Add ellipse/polygon outlines and fills, flood fill, erase/clear, named copy/paste,
reusable canvas stamps, extract, flips, clockwise quarter-turns, translate, crop,
and nearest-neighbor resize. Add named palettes with strict and nearest policies.
Nearest mapping uses squared RGBA distance and sorted-name tie breaking; transparent
pixels retain hidden RGB. Strict checking validates the final visible pixels.

PNG import expands gray/indexed input to RGBA8, preserves alpha, and never silently
resizes or maps colors. Sixteen-bit input requires explicit high-byte reduction.
APNG import is rejected; animation is edited as project frames.

Acceptance: exact-pixel shape/transform tests, alpha round trips, palette rejection
rollback, PNG color-type tests, and generate → render → inspect → revise → export.

### 3. Layered project format and checkpoints

Version-1 projects have shared dimensions/layer definitions and one cel per layer
per frame. Layers are ordered bottom-to-top with names, visibility, and opacity.
Layer selection, creation, duplication, reordering, merging, and deletion are
supported. Legacy migration writes a new project; it never rewrites its source.
Snapshots save independent checkpoints; full undo/redo remains a future feature.

Acceptance: layered projects round-trip without loss, opacity/visibility composite
correctly, invalid cel geometry is rejected, and failed edits preserve the project.

### 4. Animation and safe asset bundles

Support blank/duplicate frames, selection, ordering, deletion, 1..=60,000 ms frame
durations, optional pixel pivots, and tags containing ordered indices plus an
optional arbitrary direction string. Reorder/delete remaps tags by frame identity.
Export individual PNG frames, a deterministic grid atlas, a scaled preview, APNG,
and versioned metadata with rectangles, durations, pivots, tags, and file names.
Keep fixed cells and transparent padding; no automatic trimming or rotation.

Export to a **new directory**, rather than unrelated sheet/metadata destinations.
Stage and sync output files first, exclusively reserve the destination, publish
images, then publish `metadata.json` last as the completion marker. Ordinary
publication failures clean up the reserved directory. A process/power failure can
leave an incomplete directory: consumers must require the manifest. This is not
a promise of power-loss durability or one atomic filesystem operation across files.
Existing directories are never replaced. Preview replacement is explicit and
cannot target the source artwork. Continue running edits sequentially.

Acceptance: atlas rectangles decode to the expected composites, APNG pixels and
timing match frames, existing destinations survive unchanged, and late publication
failure cannot leave a completed manifest pointing at missing images. Include a
small engine consumer example without adding an engine dependency to the library.

### 5. Tiles and reusable procedural templates

Slice complete fixed-size source grids using explicit input margins/spacing.
Assign stable row-major tile IDs, export configurable atlas padding/margins, and
carry optional directed right/down adjacency rules in metadata. Validate exact
opposing-edge RGBA equality for requested connections. Rules describe compatible
neighbors; automatic terrain synthesis and a map editor are outside this increment.

Recipes specify dimensions, optional seed/palette/policy, and operations. Named
palette colors, reusable stamped canvas components, and seeded noise enable data-driven variations. Seeds are u64;
noise visits visible pixels row-major with the documented SplitMix64 stream.

Acceptance: checked-in tile seams pass; atlas metadata contains stable IDs/rules;
the same recipe and seed reproduce bytes; different seeds produce useful variants.

## Resource and compatibility boundaries

- A canvas or complete project's cels: at most 16,777,216 pixels.
- Projects: at most 128 layers, 256 frames, 256 tags, and 256 indices per tag.
- Batches: at most 10,000 operations and 67,108,864 estimated work units shared
  by raster edits and final palette enforcement. Palette scans charge one unit
  per pixel and one per candidate on a cache miss; cache entries are capped at 65,536.
- Polygons: 3..=256 points. Named palettes/noise color lists: 1..=256 colors.
- Shape coordinates: +/-1,000,000; shape extents: at most 2,000,000.
  Legacy pixel replacement still accepts every valid canvas coordinate.
- Named clipboard buffers together: at most 16,777,216 pixels.
- Tile seams: at most 16,384 rules and 67,108,864 total edge-pixel comparisons.
  Right edges charge tile height; down edges charge tile width. Validate all
  references and the comparison budget before scanning edges or emitting a report.
- Batch/palette/metadata JSON and encoded PNG input: at most 64 MiB.
  Recipes and other coherent-authoring configurations: at most 4 MiB.
  Artwork JSON: at most 2 GiB, streamed through a metadata-only preflight that
  validates actual pixel counts and aggregate cels before allocating rasters.
  Artwork readers cap encoded string literals at 4096 bytes and nesting at 128.
  This supports the permitted raster sizes in legacy pretty JSON without buffering
  the whole file. Bounded sequence decoders avoid trusting allocation hints.
  Project constructors check capacity before adding cels. Direct serde callers
  should also bound their input reader for an aggregate memory limit.
- Atlases: 1..=4096 equal-sized frames; at most 16,777,216 source/sheet pixels.
  Source margin/spacing and output margin/padding: at most 1024 pixels.
- CLI previews: integer scales 1..=64, still subject to the canvas pixel limit.
- Palette mapping is explicit. Transparent hidden RGB is preserved by flat
  storage/transforms; compositing need not preserve invisible source RGB.
- All timestamps, random filesystem names, and absolute paths stay out of asset
  metadata. Format changes need updated documentation, fixtures, and behavior tests.

## Verification and examples

Run formatting, Clippy with warnings denied, debug/release tests, documentation
checks, and a reproducible gallery generator. CI runs checks on pull requests and
pushes on Linux, Windows, and macOS; release packaging remains cross-platform.
Tests cover golden pixels, all line octants, clipping, degenerate dimensions,
alpha, round trips, malformed inputs, resource limits, no-clobber writes, rollback,
dry-run, selection, tags, atlas/APNG decoding, seams, and deterministic seeds.

Gallery subfolders contain inputs, editable projects/canvases, inspections, native
PNGs, enlarged previews, frame exports, and manifests. Generate artwork through
the CLI so the examples also exercise the real agent workflow. Review the images
visually; automated metrics do not establish artistic quality.

GPL-3.0-only remains the repository license. This change does not introduce a
licensing change or an automatic GPL requirement for independently created art.
