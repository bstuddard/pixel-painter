# Pixel Painter crate

This package builds the reusable `painter` library and the `painter` CLI.
See the [project README](https://github.com/bstuddard/pixel-painter),
[CLI/format guide](https://github.com/bstuddard/pixel-painter/blob/main/docs/cli-and-formats.md),
and [review gallery](examples/gallery/README.md).
The [Village Corner example](examples/village/README.md) exercises coherent asset
families, parameterized geometry, placement, and offline scene composition.
The [v0.4 workshop](examples/workshop/README.md) demonstrates material ramps,
masked brushes, isolated layers, and built-in lighting/diagnostic reviews.

```sh
cargo run --locked -- --help
cargo run --locked -- --file examples/soccer-ball.json view
cargo test --locked
cargo test --no-default-features --locked
cargo test --release --locked
cargo clippy --all-targets --locked -- -D warnings
cargo doc --no-deps --locked
```

All library edits are in memory; saving/exporting is explicit. A failed batch
preserves its input. Canvas and document construction/deserialization enforce
valid dimensions and storage. See the crate documentation for a library example.

The default `cli` feature includes the binary and Clap. Library dependencies can
set `default-features = false`; all reusable editing/export APIs remain available.
`Error::code` is a typed `ErrorCode`; `as_str()` exposes its stable JSON spelling.
Native IO, JSON, and PNG causes are available through `std::error::Error::source()`.

Licensed under [GPL-3.0-only](https://github.com/bstuddard/pixel-painter/blob/main/LICENSE).
The packaged crate includes the license text. Registry publication is disabled because
the crates.io name `painter` belongs to another project. This package is currently
distributed through its Git repository and GitHub release archives; the executable
and library retain their existing `painter` names.
