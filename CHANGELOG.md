# Changelog

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
Public Rust APIs may evolve before 1.0; versioned file compatibility changes require
a documented migration. Reproducibility tests cover exact pixels and repeated
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
