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

The default configuration passes 116 unit/integration tests and one doctest in
debug and release. Disabling the default `cli` feature passes 82 library and
integration tests plus one doctest in both profiles; its dependency graph omits
Clap. Both feature configurations also pass on Rust 1.88.0. CI repeats the core
checks on each candidate, including both feature configurations of the packaged crate.

Tests exercise these contracts:

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
Clippy, and its exported-manifest/timing test on stable Rust and Rust 1.88.0.
Its graphical smoke test rendered three frames under a temporary Xvfb display
with software OpenGL and exited successfully. It does not link the painter library.

## Packaging and security

The [release preparation guide](release-review.md) gives the full packaging
commands and publication gates. Local checks cover:

- All 18 release-tool tests, Ruff formatting/linting, workflow validation with
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
