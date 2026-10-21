# Macroquad consumer

This standalone example embeds the checked-in character atlas and reads frame
rectangles, timing, and direction tags from its metadata. No coordinates are
entered manually. It depends on Macroquad and serde, not the painter library.
The core package retains its existing dependencies.

Macroquad has an unresolved [RustSec soundness advisory](https://rustsec.org/advisories/RUSTSEC-2025-0035.html),
and its `ttf-parser` dependency is [unmaintained](https://rustsec.org/advisories/RUSTSEC-2026-0192.html).
Both notices apply to this separate demonstration, not the painter library or CLI.
The example uses embedded, checked-in artwork and the engine's built-in font; it
does not load user-supplied fonts or images. This reduces input exposure but does
not repair Macroquad's soundness issues. Replace the affected engine/dependencies
before using this consumer as production application code. See the
[security review](../../docs/security-review.md) for the audit results and scope.

From the repository root:

```sh
cargo run --locked --manifest-path integrations/macroquad/Cargo.toml
cargo test --locked --manifest-path integrations/macroquad/Cargo.toml
```

Four directions animate side by side with nearest-neighbor sampling. Escape exits.
`--smoke-test` draws three frames and exits; use a virtual X server in headless CI.
Tests validate the real exported manifest and timing boundaries without a display.

The three-map mode embeds the generated neon-sign export:

```sh
cargo run --locked --manifest-path integrations/macroquad/Cargo.toml -- --lighting
```

Keys 1/2/3 select color, color+emission, and moving-light shading. Tab switches
between the animated sign and cottage. F/V flip the
sprite and its normal directions; Escape exits. Color/emission use explicit sRGB
transfer functions; normals remain linear. All samplers share one metadata frame
rectangle. Missing companions have transparent-emission / flat-normal fallbacks.
`--smoke-test --lighting` compares 70 GPU samples with a CPU reference for the three
modes, both flip axes, partial alpha, and missing-map fallbacks. CI runs it under
software OpenGL in Xvfb. The shader uses the OpenGL backend.

The offline [review examples](../../rust/examples/lighting/README.md) and
[texture contract](../../docs/three-map-export.md) explain the authoring data.
Lighting does not provide bloom, cast shadows, or an occlusion model. The existing
upstream advisories above still apply to this experimental demo.

Drawing uses Macroquad's [draw_texture_ex](https://docs.rs/macroquad/0.4.16/macroquad/texture/fn.draw_texture_ex.html)
and [Texture2D](https://docs.rs/macroquad/0.4.16/macroquad/texture/struct.Texture2D.html).
Shader integration follows Macroquad's [material API](https://docs.rs/macroquad/0.4.16/macroquad/material/fn.load_material.html).
