# Three-map review examples

Start with [overview.png](overview.png). Rows are the neon sign, night cottage,
and manual-normal tile. Columns are **color**, **color + emission**, then lighting
from **left**, **right**, and **above**. These are static CPU reference images;
there is no bloom or cast-shadow effect.

- [Neon sign](01-neon-sign/review.png): two frames share the same color art while
  emission strength changes. Review its [normal sheet](01-neon-sign/export/normal/sheet.png),
  [emission sheet](01-neon-sign/export/emissive/sheet.png), and [metadata](01-neon-sign/export/metadata.json).
- [Night cottage](02-night-cottage/review.png): generated face normals change the
  roof/wall lighting while window emission remains visible. Inspect the
  [normals](02-night-cottage/export/normal/sheet.png) and [editable project](02-night-cottage/project.painter.json).
- [Vector transforms](03-vector-transforms/review.png): authored asymmetric
  normals have original, horizontal-flip, and clockwise-turn frames. Review the
  [normal sheet](03-vector-transforms/export/normal/sheet.png) and corresponding
  [color sheet](03-vector-transforms/export/sheet.png).

Every folder includes editable projects, authoring inputs, a color-only export,
and a map-enabled export. Companion files stay native size while `preview.png`
is enlarged. Ordinary color outputs match between the two export modes.

Reproduce into a new directory:

```sh
python3 rust/examples/generate_lighting.py --painter rust/target/release/painter --output /tmp/painter-lighting
diff rust/examples/lighting/SHA256SUMS /tmp/painter-lighting/SHA256SUMS
```

The manifest covers 68 generated artifacts. This README is maintained separately.
The [format guide](../../../docs/three-map-export.md) defines coverage, vector axes,
palette policy, limits, and how to use the experimental moving-light demo.
