# v0.4 authoring workshop

![Material variants, masked brushes, and built-in lighting](hero.png)

Every raster edit and preview was produced by the actual `painter` executable.
The cottage reuses the editable v0.3 lighting example. No image service is needed.

| Example | Start here | What to inspect |
|---|---|---|
| [Material chest](01-material-chest/) | [Three variants](01-material-chest/export/preview.png) | Baseline, shadow, and highlight preserve wood/steel ownership and the cyan rune. Compare [ramps](01-material-chest/materials.json), [shading batch](01-material-chest/highlight.ops.json), and [lighting sheet](01-material-chest/review/contact-sheet.png). |
| [Masked sentinel](02-masked-sentinel/) | [Before / after](02-masked-sentinel/export/preview.png) | Moss stamps and a long line stay inside the [cloak mask](02-masked-sentinel/mask-preview.png), preserving outline, face, and silhouette. [Brush source](02-masked-sentinel/moss-brush.json), [editable project](02-masked-sentinel/project.painter.json). |
| [Cottage review](03-cottage-review/) | [Seven review views](03-cottage-review/review/contact-sheet.png) | Lighting responds to generated roof/wall normals. Compare [raw normal cel](03-cottage-review/raw-normals.png), [solo base layer](03-cottage-review/color-solo.png), and [diagnostics](03-cottage-review/review/review.json). |

The hero's top row shows the three chest variants and textured sentinel; the
bottom row shows three light directions and the unpainted sentinel.

From the repository root, build once and review an example without editing it:

```sh
cargo build --release --locked --manifest-path rust/Cargo.toml
rust/target/release/painter --file rust/examples/workshop/01-material-chest/project.painter.json preview --output /tmp/chest-review --all-frames --scale 4
```

To experiment with shading, copy the project to a working file first:

```sh
cp rust/examples/workshop/01-material-chest/project.painter.json chest.json
rust/target/release/painter --file chest.json --frame 0 --layer base batch --input rust/examples/workshop/01-material-chest/highlight.ops.json
rust/target/release/painter --file chest.json render --output chest.png --scale 4
```

Reproduce all generated assets in a fresh directory:

```sh
python3 rust/examples/generate_workshop.py --painter rust/target/release/painter --output /tmp/painter-workshop
diff rust/examples/workshop/SHA256SUMS /tmp/painter-workshop/SHA256SUMS
```

`SHA256SUMS` covers 97 generated artifacts. This README is maintained separately.
The [authoring and review guide](../../../docs/authoring-and-review.md) specifies
mask snapshots, alpha locking, material ownership, raw/solo/channel semantics,
lighting math, diagnostics, and limits.
