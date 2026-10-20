# Pixel Painter review gallery

All pixels were drawn by `painter`; no image-generation service was used.

![Gallery overview](overview.png)

| Folder | Review output | What it exercises |
|---|---|---|
| [Crystal keeper](01-crystal-keeper/) | [Eight-frame preview](01-crystal-keeper/export/preview.png), [animated PNG](01-crystal-keeper/export/animation.png) | Four layers, four directions, two walk poses, pivots, tags, checkpoints |
| [Moss cottage](02-moss-cottage/) | [Enlarged preview](02-moss-cottage/export/preview.png) | Layered architecture, polygons, ellipse details, partial alpha |
| [Forest tiles](03-forest-tiles/) | [Tile atlas](03-forest-tiles/export/preview.png), [seam report](03-forest-tiles/seams.json) | Named palette, seeded texture, eight tiles, adjacency rules, padding |
| [Seeded slimes](04-seeded-slimes/) | [Six-variant preview](04-seeded-slimes/export/preview.png) | One reusable template, six seeds, components, atlas metadata |
| [Primitives](05-primitives-and-components/) | [Preview](05-primitives-and-components/preview.png), [transformed preview](05-primitives-and-components/transformed-preview.png) | Shapes, clipping, copy/paste, erase/fill, alpha, PNG import, rotate/flip |

Each asset folder contains editable JSON, operation/template inputs, a native PNG,
and an inspection report. Export bundles contain `sheet.png`, `preview.png`,
`frames/`, and `metadata.json`; temporal projects also contain `animation.png`.
The manifest is the completion marker. Coordinates and sizes are native pixels.

Regenerate into a new folder from the repository root:

```sh
cargo build --release --locked --manifest-path rust/Cargo.toml
python3 rust/examples/generate_gallery.py --painter rust/target/release/painter --output /tmp/painter-gallery
```

`SHA256SUMS` covers every gallery file except itself. Repeated runs with the same
binary produce identical bytes; inspect enlarged PNGs and APNGs in an image viewer.
