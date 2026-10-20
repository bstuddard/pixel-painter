# Verification

Local verification targets Pixel Painter v0.2.0 on Linux, using Rust 1.99.0 and
the declared minimum Rust 1.88.0. Hosted Windows/macOS checks and a complete
four-platform download trial remain required before publication.

## Core checks

Run from the repository root:

```sh
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
cargo clippy --manifest-path rust/Cargo.toml --all-targets --locked -- -D warnings
cargo clippy --manifest-path rust/Cargo.toml --all-targets --no-default-features --locked -- -D warnings
cargo test --manifest-path rust/Cargo.toml --locked
cargo test --manifest-path rust/Cargo.toml --no-default-features --locked
cargo test --manifest-path rust/Cargo.toml --release --locked
cargo test --manifest-path rust/Cargo.toml --release --no-default-features --locked
RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path rust/Cargo.toml --no-deps --locked
```

The default configuration passes 178 tests, including the doctest, in debug and
release. Disabling the default `cli` feature passes 135 tests, including the
doctest, in both profiles; its dependency graph omits
Clap. Both feature configurations also pass on Rust 1.88.0. CI repeats the core
checks on each candidate, including both feature configurations of the packaged crate.

Tests exercise these contracts:

- [Three-map contracts](../rust/tests/maps.rs) and [CLI workflows](../rust/tests/maps_cli.rs): channel isolation, coverage, vectors, generated geometry, shared layout, combined limits, atomic edits, legacy exports, and tampered companion detection.
- [Coherent authoring](../rust/tests/authoring.rs): typed recipes, transformed
  components, shadows, exact scene pixels, preview limits, stable animation-sheet
  ordering, placement/split exports, pack integrity, and transactional failures.
- [Core operations](../rust/tests/core.rs): exact pixels, alpha rounding, clipping,
  transforms, palettes, aggregate work limits, transactional rollback, projects,
  layers/frames/tags, PNG formats, atlas/APNG timing, seams, and deterministic seeds.
- [Error API](../rust/tests/errors.rs): typed categories preserve every JSON code,
  native I/O/JSON/PNG causes, cloning, and `Send + Sync`.
- [Review regressions](../rust/tests/review_regressions.rs): large-file round trips,
  bounded reads/writes, PNG trailing validation, native filenames, coordinate limits,
  palette work, and seam budgets.
- [Security](../rust/tests/security.rs): terminal controls, argument diagnostics,
  symlink collisions, output permissions, and traversal-shaped display names.
- [CLI workflows](../rust/tests/workflows.rs) and [original CLI cases](../rust/tests/cli.rs):
  real subprocesses, structured errors, dry-run, persistence, and export failures.
- [Gallery replay](../rust/tests/gallery.rs): checked-in operations and metadata
  reproduce their documented outputs.

## Examples and independent consumer

Regenerate into a new directory; the generator refuses existing destinations:

```sh
cargo build --manifest-path rust/Cargo.toml --release --locked
python3 rust/examples/generate_gallery.py --painter rust/target/release/painter --output /tmp/painter-gallery
diff rust/examples/gallery/SHA256SUMS /tmp/painter-gallery/SHA256SUMS
```

All 120 generated artifact hashes match the checked-in gallery. The independent
[Macroquad consumer](../integrations/macroquad/README.md) passes formatting,
Clippy, and its manifest/timing and map-contract tests on stable Rust and Rust 1.88.0.
Its graphical smoke test rendered three frames under a temporary Xvfb display
with software OpenGL and exited successfully. It does not link the painter library.

The Village Corner generator also reproduces all 148 artifact hashes. Together,
the three generators verify 336 artifacts, including editable inputs and encoded
exports. The three-map implementation preserves the existing 268 hashes; the new lighting generator reproduces 68 additional artifacts. Both consumer modes pass graphical smoke tests, including 70 GPU/reference samples across the sign and cottage, flips, partial alpha, and fallbacks.

## Preview performance

A Linux release-build comparison used three runs of each case, reporting medians.
A contact sheet containing one opaque 4096×4096 image, one column, zero padding,
and scale 1 was rendered five times. Removing the redundant output clone reduced
peak process RSS from 198,716 KiB to 133,236 KiB (about 64 MiB) and elapsed time
from 0.597 s to 0.510 s. This measures input, output, and process overhead together.

A separate case rendered 200 animation sheets from 256 one-pixel frames and 256
tags, each referencing every frame. Tracking visited frames directly reduced the
median from 0.348 s to 0.0115 s. Document construction was outside the timer.
These cases measure the changed paths, not representative end-to-end workloads
or a performance guarantee. Pixel and frame-order regressions run in the suite.

## Packaging and security

The [release preparation guide](release-review.md) gives the full packaging
commands and publication gates. Local checks cover:

- All 20 release-tool tests, Ruff formatting/linting, workflow validation with
  actionlint, local Markdown links, and `git diff --check`.
- Standalone `cargo package` verification and tests of the extracted crate.
- An isolated clean source snapshot with both dependency graphs vendored. Its
  extracted archive passes release tests with a new empty Cargo cache, including
  both core feature configurations and the independent consumer.
- An extracted Linux download, executed with an empty `PATH`, passes version
  reporting, the bundled CLI guide commands, and an exact RGBA PNG round trip.
- License/attribution inventory integrity, archive paths, matching version/commit
  identities, platform filenames, and complete download sets. Fixture checks of
  Windows/macOS layouts do not execute native binaries.

The strict core dependency audit reports no vulnerabilities or informational
warnings. The experimental consumer retains two unsuppressed informational
advisories; successful rendering does not establish its memory soundness.
See the [security review](security-review.md) for the threat model and details.

## Limits of this evidence

These results describe local Linux execution, not a completed hosted release
trial. Native Windows/macOS execution and the release Linux baseline must pass in
CI. Binaries are unsigned and not notarized. No release has been published.

Tests cover ordinary write/publication failures and export cleanup. They do not
establish power-loss durability. Export bundles use a manifest-last completion
protocol; concurrent writers remain unsupported, so edits must run sequentially.
