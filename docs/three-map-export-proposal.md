> Original proposal, retained for scope review. Phases 1–3 are implemented;
> [the final contract](three-map-export.md) records the concrete API and clarified
> semantics. Optional per-part maps, curved surfaces, extra material channels,
> bloom, and cast-shadow data remain deferred. The implementation keeps a single
> additive schema reader rather than a migration/compatibility framework.

# Pixel Painter: Three-Map Export Proposal

**Status:** Original implementation proposal; see final contract above
**Prepared:** October 20, 2026
**Repository:** [bstuddard/pixel-painter](https://github.com/bstuddard/pixel-painter)

## Executive recommendation

Extend Pixel Painter's existing **project export** so a single project can optionally emit three **pixel-aligned texture channels**:

1. **Color (albedo)** — the ordinary, visible RGBA sprite or sprite sheet; required.
2. **Emissive** — colored light-emission information, such as a neon tube or glowing eyes; optional.
3. **Normal** — a per-pixel surface-facing direction for dynamic lighting; optional.

**Implement emissive first, normal maps second, and automatic normal generation from geometry third.** Keep ordinary one-PNG exports and existing projects working as they do today. Pixel Painter should create the data; a game engine such as Macroquad should perform real-time lighting, bloom, and optional shadows.

This proposal targets `painter project export` first because it already exports frames, an atlas, and metadata. A simple legacy `painter export --output image.png` should continue producing a single PNG; it does not need a breaking change.

## 1. Existing foundation and gaps

Based on the repository's current public documentation:

- Versioned projects already store **ordered layers, per-frame cels, visibility, opacity, animation durations, pivots, and tags**.
- `painter project export` already creates `frames/0000.png`, `sheet.png`, `preview.png`, animation output where applicable, and `metadata.json`.
- The atlas already uses stable, untrimmed frame rectangles, with configurable columns/padding. This makes map alignment straightforward.
- Optional placement data already supports **split-layer exports**, but those splits serve world overlap and layering; they are not emissive/normal render channels.
- Geometry recipes already distinguish **top/front/side faces**, and the shadow helper already projects a **baked silhouette shadow**. These can inform future normal-map generation, but a baked shadow is not a normal map.
- The standalone Macroquad example consumes exported sprite metadata, making it a natural place for a lighting demonstration.

**Gap:** The documented exporter does not yet define semantic render channels or a companion-map contract. This is an **export and authoring extension**, not a reason to replace the canvas or animation model.

Repository references: [CLI and formats](https://github.com/bstuddard/pixel-painter/blob/main/docs/cli-and-formats.md), [coherent assets / geometry / placement](https://github.com/bstuddard/pixel-painter/blob/main/docs/coherent-assets.md), [README](https://github.com/bstuddard/pixel-painter).

## 2. Define the texture contract first

| Channel | Suggested filename | Per-pixel format | Engine interpretation | Default |
|---|---|---|---|---|
| Color | `sheet.png` | RGBA8 | Ordinary visible artwork | Required; preserve existing output |
| Emissive | `emissive/sheet.png` | RGBA8 | RGB = emission color, A = emission strength/coverage | Transparent black `(0,0,0,0)` |
| Normal | `normal/sheet.png` | RGBA8 | RGB = encoded XYZ unit normal, A = coverage | Flat normal over color coverage |

All maps must have the **same width, height, frame order, atlas rectangles, pivots, and native-pixel alignment**. There is one shared packing layout, not three independently packed atlases. Sprite-sheet and per-frame PNGs are both useful: an animated export can contain *three sheet PNGs plus three PNGs per frame*, along with existing previews/metadata.

### Color

Preserve the current color compositing and `sheet.png` bytes for existing assets whenever possible. Exporting emissive or normal must **not** bake their colors into the albedo image.

### Emissive

Recommended first-pass semantics:

- RGB is colored radiance (for example, red neon), and alpha controls its strength/coverage.
- In the shader, a simple first pass is `final_rgb += emissive.rgb * emissive.a * emission_strength` (appropriately handling the texture/color-space convention).
- A black or transparent emissive pixel emits nothing. Pixel Painter should not invent emission unless an author supplies it.
- For ordinary sprites, prevent unintended glowing pixels outside the rendered color silhouette. An explicit opt-in could later allow stand-alone light particles.
- **Emission is not bloom.** Bloom/glow halos around adjacent pixels are a separate engine post-process or effect.

Store emissive data in a separate authoring channel; painting bright yellow into a normal color layer does **not** automatically mean it glows.

### Normal

A normal map stores a 3D *direction*, not a shadow intensity. For a normalized vector `n = (nx, ny, nz)` with components from `-1` to `+1`, encode:

```text
R = round((nx + 1) * 127.5)
G = round((ny + 1) * 127.5)
B = round((nz + 1) * 127.5)
```

A flat surface facing the viewer is approximately **`(128,128,255)`**. On import, the shader decodes `normalize(2 * rgb - 1)` and uses its dot product with the light direction (plus a chosen ambient term).

**Specify the convention:** use screen-space normals for the initial 2D integration: +X right, +Y up, +Z toward the viewer. Image coordinates increase downward, so shaders or asset generation must handle that sign deliberately. Mark the convention in metadata. Treat normal data as **linear data, not color to gamma-correct**, and use nearest-neighbor texture sampling.

For a color pixel with no authored normal, supply the neutral normal. Outside the visible sprite, use transparent pixels. A partially transparent color pixel should have consistent corresponding normal coverage. A normal map changes **how the surface receives light**; it does **not** create cast shadows or know which objects block a light.

## 3. Minimal authoring-model extension

Prefer **semantic layer channels** over a new project format or three unrelated source projects. A proposed *new, optional* field on each existing layer:

```json
{
  "name": "glowing-eyes",
  "visible": true,
  "opacity": 255,
  "channel": "emissive"
}
```

Allowed values initially: `color` (default), `emissive`, and `normal`. Existing projects with no `channel` field deserialize as `color` layers. Their frames still hold matching ordered cels. An author can draw to a map cel using the same layer-selection mechanism already used for ordinary art.

A typical project might have layers named `body` (color), `jacket` (color), `eyes-glow` (emissive), and `surface-normals` (normal). The glow artist paints only the emitting pixels; the normal artist paints encoded directions, not visible colors.

**Important implementation distinction:**

- **Color** cels use current RGBA compositing and palette behavior.
- **Emissive** cels use documented emission/alpha composition; avoid letting the ordinary strict color palette silently quantize the map.
- **Normal** cels represent vectors. Do **not** blindly alpha-blend RGB normal values as if they were paint: either use topmost nontransparent normal, or interpolate *decoded vectors* and re-normalize. Start with topmost-wins for simplicity.

Authoring commands shown here are **proposed additions, not current CLI syntax**:

```bash
painter project layer hero.painter.json add eyes-glow --channel emissive
painter project layer hero.painter.json add normals --channel normal
```

If the project's serialized schema is strictly versioned, increment its version and supply an explicit migration. Preserve the existing no-field `color` default and existing CLI behavior.

## 4. Extend the exporter, not the independent packers

Proposed opt-in syntax:

```bash
# Current behavior remains the default.
painter project export hero.painter.json --output hero-export

# Proposed map-enabled exports.
painter project export hero.painter.json --output hero-export-lit --maps emissive
painter project export hero.painter.json --output hero-export-all --maps emissive,normal
```

For an explicitly requested but unpainted channel, produce a deterministic fallback: transparent emissive or flat normals derived from color coverage. For an unrequested channel, write no companion PNG. This keeps disk use and consumer complexity low.

### Suggested bundle layout

```text
hero-export-all/
├── frames/
│   ├── 0000.png                 # existing color frame
│   └── 0001.png
├── sheet.png                    # existing color atlas
├── emissive/
│   ├── frames/
│   │   ├── 0000.png
│   │   └── 0001.png
│   └── sheet.png
├── normal/
│   ├── frames/
│   │   ├── 0000.png
│   │   └── 0001.png
│   └── sheet.png
├── preview.png                  # existing preview behavior
├── animation.png                # if already emitted for this project
└── metadata.json               # publish last, as today
```

**Do not use preview scaling for map textures.** Emit companion files at the same native resolution as the existing frames and sheet. Keep the existing placement/parts mechanism separate; if map support is later added to individual parts, each part gets aligned companions with the existing full-size placement canvas.

### Shared layout, three raster channels

A suggested conceptual Rust type (illustrative, not a verified repository symbol):

```rust
struct RenderedFrameChannels {
    color: Canvas,
    emissive: Option<Canvas>,
    normal: Option<Canvas>,
}
```

Suggested exporter flow:

1. Render all `color` layers per frame using the existing code.
2. For requested channels, render the corresponding `emissive`/`normal` cels, apply fallbacks, and validate dimensions.
3. Build the **atlas layout once** from existing frame width/height, ordering, columns, and padding.
4. Apply the *same* frame rectangles to every channel. Preserve transparent padding.
5. Save all requested per-frame PNGs and atlases through the current PNG-writing machinery.
6. Write the extended `metadata.json` **only after all files succeed**, preserving current no-overwrite/file-safety behavior.

Avoid independently trimming, rotating, or packing the different channels. Do not infer a normal map from the finished color PNG; colored highlights and shadows are not reliable surface geometry.

## 5. Metadata: additive and backwards-compatible

Keep every current metadata field, including the existing color sheet path, frame rectangles, durations, pivots, and tags. Add an **optional** `maps` field. For example, the following is a **proposed JSON fragment**, not a complete reproduction of today's metadata structure:

```json
{
  "maps": {
    "emissive": {
      "sheet": "emissive/sheet.png",
      "frames": [
        "emissive/frames/0000.png",
        "emissive/frames/0001.png"
      ],
      "encoding": "rgba8_emission"
    },
    "normal": {
      "sheet": "normal/sheet.png",
      "frames": [
        "normal/frames/0000.png",
        "normal/frames/0001.png"
      ],
      "encoding": "xyz_unorm8",
      "space": "screen",
      "positive_y": "up",
      "default": "flat"
    }
  }
}
```

The frame paths above are indexed in the **same order** as the existing metadata frame list; all channels reuse the **existing rects**, not their own rect arrays. Paths should be relative to the bundle, validated using the repository's current path-safety rules. If the metadata contract requires a version bump, publish the corresponding schema and update consumers; older consumers should still be able to ignore the optional `maps` object where their JSON handling permits it.

## 6. Automatic normals from procedural geometry — phase 3

Pixel Painter's recipe geometry already distinguishes `top`, `front`, and `side` faces. The most useful long-term feature would generate an aligned normal raster while those faces are drawn:

- Assign each face a **unit normal vector** consistent with the chosen screen-space convention and three-quarter projection.
- Rasterize each face into the normal channel using **the identical masks, draw order, and pixel boundaries** used for the visible face.
- For drawn outlines and manually decorated areas, use either the owning surface's normal or a neutral default; document the choice.
- Later, optionally add curved-surface generators (ellipses/spheres) and semantic materials.

This must be done **during shape generation**, while the tool still knows the face identity. Trying to reverse-engineer normals from a shaded final PNG would be fragile. Treat the initial face vectors as a stylized lighting approximation, not physically accurate 3D reconstruction.

Also handle spatial transforms deliberately: horizontally flipping a normal map must both reposition pixels **and invert its X component**; vertical flips invert Y; quarter-turn rotations rotate X/Y components. A simple PNG flip without rotating its encoded vectors will produce wrong lighting.

## 7. Macroquad consumer demonstration

Provide one small demo loading `sheet.png`, optional `emissive/sheet.png`, optional `normal/sheet.png`, and the current animation rects:

- Use nearest-neighbor filtering for each texture.
- Select **one shared frame rectangle** to sample all textures.
- Sample/decode the normal map, compute diffuse illumination from a moving light plus ambient light, and add emissive contribution.
- Show toggleable color-only, color+emissive, and color+emissive+normal modes.
- Handle the normal texture as linear data; establish explicit shader conventions for screen-space Y and any sprite flipping.

Pixel Painter itself should remain an **offline authoring/export** tool. Dynamic lighting, bloom, and real-time cast shadows belong in the runtime/engine.

## 8. Implementation phases and acceptance criteria

### Phase 1 — Export plumbing and schema

- [ ] Add an optional `--maps` request to `project export`; unchanged defaults.
- [ ] Introduce a channel-aware frame renderer and one atlas layout reused by all channels.
- [ ] Emit deterministic fallback companion images at correct dimensions when requested.
- [ ] Extend `metadata.json` additively; keep the legacy color paths stable.
- [ ] Preserve existing file-exists refusals and completion-marker-last behavior.
- [ ] Test no-map export against pre-existing fixtures for compatibility.

### Phase 2 — Authorable emissive channel (**first visible win**)

- [ ] Add optional `channel: emissive` on project layers with legacy `color` default.
- [ ] Allow normal batch/draw tools to target an emissive cel without contaminating color.
- [ ] Define alpha, strength, occlusion/coverage, and composition semantics.
- [ ] Export per-frame emissive files and atlas; write metadata paths.
- [ ] Add a two-frame glowing-sign fixture and a minimal Macroquad emission shader.

### Phase 3 — Manual normals and geometry-aware defaults

- [ ] Add `channel: normal` plus neutral fallback on nontransparent color pixels.
- [ ] Document XYZ encode/decode and screen-up convention.
- [ ] Add vector-aware flip/rotation handling and validation (unit length/tolerance).
- [ ] Extend prism/roof/facade generation to optionally populate the normal channel while drawing faces.
- [ ] Add a moving-light demo and compare static versus dynamically lit buildings.

### Phase 4 — Polish, only if useful

- [ ] Companion maps for placement `parts`, using identical full-canvas coordinates.
- [ ] Debug previews (emissive-only, color+emission, raw normals, moving-light test).
- [ ] Optional stronger normal authoring, material presets, and curved surfaces.
- [ ] Optional **separate** occluder/height data for true cast shadows (not part of this three-map MVP).

### Required regression tests

- **Dimensions/alignment:** all sheets and frames have matching dimensions, frame order, and rectangles for 1-frame and animated exports.
- **Alpha and transparency:** blank emission is black/transparent; flat normals exist only over expected visible coverage.
- **Determinism:** repeated exports from the same source are byte-for-byte stable.
- **Compatibility:** legacy project JSON, existing `painter export`, project export without `--maps`, old color-only Macroquad consumer.
- **Validation:** reject unknown map names, invalid layer channel types, mismatched dimensions, and invalid normal encodings where applicable.
- **Safety:** existing-file refusal, no partial successful metadata on a failed export, safe relative paths.
- **Geometry:** identical face masks in color/normal output; flips/rotations adjust encoded vectors.

## 9. Deliberately out of scope

These are often conflated with the three PNGs and should not become blockers:

- **Bloom/halo:** post-processing, not contained in the emissive PNG by itself.
- **Cast shadows and occlusion:** require light/geometry or separate occluder/depth data; normals alone cannot do it.
- **Height, roughness, metallic, ambient-occlusion maps:** possible later, but not justified for the first Pixel Painter lighting pipeline.
- **Photorealistic physically based shading:** not the goal for readable, stylized pixel art.

## Decision

**Ship color + emissive first** through a nonbreaking `project export --maps emissive` extension. Keep the format general enough to accept a **normal** channel next, then leverage the existing geometry helpers to emit normals automatically. The single most important architecture rule is: **all channels are different data over the same pixels and atlas layout**.
