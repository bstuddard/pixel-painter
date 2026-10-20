# Pixel Painter ${version} preview — ${platform}

Extract the whole archive into a writable folder. Rust is not required to run the
included executable. These commands create new files and refuse overwrites.

Linux/macOS:

```sh
./painter --help
./painter --file examples/soccer-ball.json view
./painter --file examples/soccer-ball.json render --output preview.png --scale 8
```

Windows PowerShell:

```powershell
.\painter.exe --help
.\painter.exe --file examples/soccer-ball.json view
.\painter.exe --file examples/soccer-ball.json render --output preview.png --scale 8
```

Open the [review gallery](examples/gallery/README.md) or the
[CLI and format guide](docs/cli-and-formats.md). The [changelog](CHANGELOG.md)
describes compatibility and limitations. The [security review](docs/security-review.md)
explains input protections and the experimental consumer's known advisories.

The [Village Corner example](examples/village/README.md) includes coherent recipes,
split placement exports, and scene previews. See [coherent asset families](docs/coherent-assets.md)
for profiles, parameterized recipes, animation checks, and pack generation.

The [lighting examples](examples/lighting/README.md) include editable emissive and
normal maps. The [three-map guide](docs/three-map-export.md) defines their shared
layout, coverage, vector convention, and geometry authoring commands.

Release CI tests this target on **${baseline}**. Local builds may require newer
system libraries; `BUILD-INFO.json` records the actual build host. The macOS
deployment target describes how release binaries are built; versions earlier than
the CI baseline are unverified. Linux downloads require GNU libc. Other systems can build
the matching source. Binaries are unsigned and not notarized; this preview is
intended for evaluation. Keep editable artwork backups and run edits sequentially.

`BUILD-INFO.json` identifies the source commit, compiler, target, and binary hash.
Verify the archive against `SHA256SUMS` from the same download set. Checksums detect
mismatches; they do not authenticate the publisher.

Licensed [GPL-3.0-only](LICENSE). [Dependency notices](THIRD_PARTY_NOTICES.md) and
the original upstream license texts accompany the executable. Your independently
created artwork does not automatically inherit the software's license.

The matching **${source_archive}** is distributed alongside this download and
includes application source, build scripts, and vendored dependencies. Extract it
and run from its root:

```sh
cargo build --release --locked --offline --manifest-path rust/Cargo.toml
```

The optional consumer also has vendored dependencies, but its graphical runtime
still requires the host's display/OpenGL libraries. It remains an experimental
example with the advisories described in the security review.
