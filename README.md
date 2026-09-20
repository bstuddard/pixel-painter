# Pixel Painter

A Rust CLI for people and agents to draw pixel art, preview it in the terminal,
and export PNGs. Built as a Rust learning project.

![Soccer ball painted through the CLI](rust/examples/soccer-ball-preview.png)

## Quick start

With stable Rust installed, run from the repository root:

```sh
cd rust
cargo run --locked -- new --width 32 --height 32
cargo run --locked -- set-pixel --x 4 --y 2 --r 255 --g 0 --b 0
cargo run --locked -- view
cargo run --locked -- export --output artwork.png
```

Commands share `canvas.json`. Use `--file PATH` for another canvas and `--help`
for options. `print` shows numbered rows of RGBA values; `view` needs a terminal
with 24-bit color. Coordinates start at `(0, 0)` in the top-left corner.

Try the included soccer ball:

```sh
cargo run --locked -- --file examples/soccer-ball.json view
```

Keep the JSON for editing. PNG import and undo aren't implemented yet.
New canvases and PNG exports refuse to overwrite files. Run edits sequentially.

## Downloads and releases

Once published, GitHub Releases will offer Linux, Windows, and Mac executables
(no Rust required). Extract your download and run `./painter --help`, or
`.\painter.exe --help` in Windows PowerShell.

The [release workflow](.github/workflows/release.yml) tests and builds Linux x86_64,
Windows x86_64, and both Mac architectures. Push a tag matching the committed
Cargo version to create a draft release, then review and publish it on GitHub:

```sh
git tag v0.1.0
git push origin v0.1.0
```

Downloads include examples, source with dependencies, and checksums. Binaries are
unsigned. Use **Actions → Build releases → Run workflow** for a trial build.
The first hosted run is still pending.

**Testing:** The Linux release build passes all nine tests and a soccer-ball
preview/export check. Earlier local Windows and Mac builds were cross-compiled
on Linux and still need testing on those devices. GitHub Actions is configured
to build and test each platform on its own OS.

## Development

From `rust/`:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

## License

[GNU GPLv3 only](LICENSE). Includes sample artwork; your own artwork does not
automatically inherit this license.
