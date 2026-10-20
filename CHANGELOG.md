# Changelog

## Unreleased

- Added semantic color/emissive/normal layers and opt-in three-map exports with
  shared atlas rectangles, coverage rules, vector validation/transforms, and
  combined raster limits. Existing color-only outputs remain unchanged.
- Added transactional geometry color/normal generation, map-aware asset-pack
  checks, reproducible lighting examples, and an experimental moving-light demo
  with GPU/reference comparisons, including flip and partial-alpha coverage.
- Prevented cross-channel merges and map layers in color placement parts; stale
  companion data is checked before placement export. Map palette quantization and
  RGB paint blending of normals are rejected explicitly.

- **Breaking:** unified templates and parameterized recipes under
  `recipe::Recipe` and one version-2 JSON schema. `seed`, `palette`, `policy`, and
  `operations` now live at the top level; the nested `batch` template schema is
  removed. Recipes use the 4 MiB authoring configuration limit. The `template` Rust
  module and untyped recipe rendering helpers are removed; deserialize `Recipe` and call its `render` method.

- Hardened scene/pack source accounting and shared raster/preparation budgets;
  escaped terminal controls in CLI JSON while preserving decoded strings.
- Excluded ignored local files and caches from binary downloads by reading release
  examples/docs from the committed Git tree, shared with source packaging.
- Kept generated pack references in portable forward-slash form on Windows,
  including placement paths and recursive export validation.
- Eliminated full-canvas copies for unscaled previews and repeated frame-list
  scans in animation contact sheets; preserved pixel output and frame ordering.
  Preview scale/dimensions are checked before rendering. The `scene::scaled`
  helper is now internal; use `drawing::resize` for standalone resizing.
- Enforced attachment limits after frame overrides merge, before exporting files;
  excluded motion-exempt poses from animation proportion references.

- Added shared style profiles, role references, and measurable style/animation reports.
- Added bounded parameterized recipes, variants, transformed components,
  pixel-aligned face/roof/stair assemblies, and baked silhouette shadows.
- Added optional placement/attachment/footprint sidecars and split-layer exports,
  with flattened atlas images available alongside the parts.
- Added offline scenes/contact sheets, transactional pack generation, and complete
  export integrity checks from stored editable sources.
- Added an independently authored, reproducible Village Corner review example.

## 0.2.0 — preview

- Added a reusable Rust library, transactional operation batches, dry-run, typed
  JSON results/errors, named palettes, inspection, and deterministic templates.
- Added integer drawing primitives, components, transforms, PNG import, and
  nearest-neighbor previews with explicit overwrite protection.
- Added version-1 layered projects, frames, durations, pivots, directional tags,
  atlas/APNG export bundles, tile grids, adjacency rules, and seam validation.
- Added an editable review gallery, an isolated experimental Macroquad consumer,
  bounded parsing/work, atomic persistence, and security regression coverage.
- Hardened release packaging with license texts, dependency notices, build
  provenance, archive smoke tests, offline source tests, and explicit publication.
- Made the CLI parser optional through the default `cli` feature. Library users
  can disable default features without losing editing/export functionality.
- Added typed `ErrorCode` categories and preserved native IO/JSON/PNG error causes.
  `Error::new` and `Error::code` use the enum; JSON code strings are unchanged.

Existing v0.1.1 canvas JSON and the original CLI commands remain supported. Native
project, batch, template, and export formats use version 1. Project migration is
explicit and creates a new file; keep the original artwork as a backup. Frame
durations are milliseconds, indices start at zero, and alpha uses straight RGBA.
Before 1.0, Rust APIs and file formats may change between minor releases; breaking
changes are documented here without a requirement to retain old readers. Reproducibility tests cover exact pixels and repeated
encoded output with the tested version/dependencies; cross-platform byte-identical
compressed files are not promised.

This is a local tool, not a sandboxed upload service. Edits are sequential, with no
concurrent-writer conflict resolution or full undo history. Binaries are unsigned.
The Macroquad consumer has unresolved soundness/maintenance advisories and is not
production application code. See [security details](docs/security-review.md).

The CLI/library retain the `painter` name. Registry publication is disabled because
that crates.io name belongs to another project; distributions use GitHub archives.

## 0.1.1

Legacy flat RGBA canvases, pixel replacement, terminal previews, and PNG export.
