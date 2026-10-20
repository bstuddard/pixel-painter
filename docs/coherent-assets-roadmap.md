# Pixel Painter — Generic 2.5D Pixel-Art Roadmap

- **Status:** Implemented locally; retained as the design record
- **Scope:** Public Pixel Painter repository
- **Repository:** https://github.com/bstuddard/pixel-painter
- **Primary interface:** Rust `painter` CLI backed by a reusable Rust library
- **Purpose:** Improve the creation of coherent, slightly dimensional **2D pixel art**, while keeping Pixel Painter a general-purpose tool.

> Pre-1.0 policy: API and format breaks are allowed when they simplify the implementation; update documentation, examples, and tests together. Earlier compatibility requirements below are superseded by this policy.

> This roadmap supplied the implementation scope. Working schemas and commands are documented in [coherent asset families](coherent-assets.md); verification and deliberate limits are recorded in the [implementation review](coherent-assets-review.md). All examples use independently authored generic artwork suitable for this public repository.

## 1. Product Vision

Pixel Painter is an offline, scriptable pixel-art creation toolkit for humans, Rust developers, and coding agents. It should help a creator make, inspect, revise, animate, and export a **consistent collection** of sprites, tiles, objects, and backgrounds through reproducible CLI commands.

The next improvement is to make ordinary 2D artwork **feel dimensional** through carefully controlled visual conventions:

- Fixed top-down / three-quarter perspectives.
- Visible top, front, and narrow side surfaces for objects and buildings.
- Consistent scale, silhouettes, outlines, and palette relationships.
- Ground-contact points, baked shadows, and intentional visual overlap.
- Clear directional animation and coherent tile transitions.

This is an **asset-generation** project, not a 3D engine, game renderer, or level editor. It should produce standard PNG files and optional JSON metadata that any downstream engine can load. No downstream game or application should need to link to Pixel Painter at runtime.

## 2. Current Foundation — Extend, Do Not Rebuild

The public v0.2.0 baseline described in the repository already provides:

- Pixel canvases, drawing primitives, import/export, transformations, and batch operations.
- Transactional editing and deterministic workflows.
- Palettes, editable layers, reusable stamps/templates, and seeded generation.
- Animation frames and tags, pivots, previews, sprite sheets, and metadata.
- Tilesets, adjacency/seam checks, plus the Rust CLI and library.

Confirm the actual APIs and commands against the current source before implementation. Do **not** mark a feature as missing or add a replacement until existing support has been examined.

The remaining challenge is making independently created assets look like they belong to **the same visual world**, especially when created by an automated agent.

## 3. Public-Repository Content Boundaries

Apply these rules to **every committed file**, not just the root README:

1. **Keep the project generic.** Do not refer to any private or unreleased downstream game, internal design document, proprietary world, plot, character, venue, or product plan.
2. **Use independent examples.** Demonstrations should use neutral subjects such as a generic adventurer, cottage, workshop, garden, crate, tree, bench, cobblestone path, and terrain tiles.
3. **Do not copy private artwork or recipes.** Public fixtures, source projects, palettes, scripts, tests, and screenshots must be authored specifically for this open-source toolkit or otherwise have documented redistribution rights.
4. **Keep private production assets elsewhere.** A consumer of the CLI can maintain its own asset sources, generation scripts, style profiles, and exports in a separate repository. No downstream project's files are required to build or test Pixel Painter.
5. **Audit all surfaces.** Check `README.md`, `docs/`, `examples/`, `tests/`, fixtures, gallery images, generated metadata, commit messages, and PR descriptions before merging.
6. **Respect existing licenses.** Keep the repository's license and third-party attribution accurate. Document sample-art provenance when needed. Using a GPL-licensed tool to generate art does not, by itself, require independently authored output to use the GPL; embedded copyrighted sample material must be assessed separately.

These requirements should be treated as normal open-source project hygiene, not as a reason to weaken the generic tooling.

## 4. Capability Priorities

| Priority | Capability | Outcome |
|---|---|---|
| **P0** | Human-readable root README | New visitors understand the product and can make a PNG quickly. |
| **P0** | Shared visual style profiles | Assets follow compatible scale, palettes, perspective, and shading rules. |
| **P0** | Parameterized templates and components | One recipe creates multiple distinct, coherent assets. |
| **P0** | Three-quarter geometry helpers | 2D roofs, walls, side faces, steps, and raised props look dimensional. |
| **P0** | Offline multi-asset scene preview | Creators can judge perspective and proportions across an entire asset set. |
| **P1** | Ground anchors and split-layer exports | Sprites can be placed and overlapped reliably by a game engine. |
| **P1** | Baked shading and shadow tools | Objects feel grounded under a consistent light direction. |
| **P1** | Directional animation validation | Frames preserve proportions, baselines, outlines, and pivots. |
| **P2** | Elevation and tile-transition recipes | Curbs, stairs, retaining walls, and raised terrain share consistent depth cues. |
| **P2** | Cohesive variant and pack tooling | Creators produce and check complete asset families deterministically. |

## 5. P0 Implementation Details

### 5.1 Rewrite the Root README for Humans

**Deliverable:** A welcoming, accurate `README.md` in the root of the **public** repository.

Organize it around what a newcomer wants to know, not around internal Rust modules.

Suggested outline:

1. **What is Pixel Painter?** One short paragraph explaining the CLI-first pixel-art toolkit in plain English.
2. **See what it makes.** Lead with a small gallery of verified, generic examples (e.g., character sprite, cottage, tree, tile strip, animation contact sheet).
3. **What can I create?** Explain sprites, animation, tilesets, reusable recipes, palettes, layers, and PNG/JSON export without lengthy jargon.
4. **Quick start.** Include one **tested, copy-and-paste-able** sequence for installing or invoking `painter`, creating art, previewing it, and exporting a PNG. Clarify whether commands require `cargo run` or a built executable.
5. **How it works.** Summarize `recipe or CLI commands → editable source → preview → PNG/JSON`.
6. **More examples.** Link to working generic examples and gallery images already included in the repository.
7. **Automation and AI agents.** Briefly explain deterministic batch operations, inspection, and structured output.
8. **For Rust developers.** Explain the library/CLI split and link to API documentation.
9. **Detailed documentation.** Link to separate pages for CLI commands, schemas, formats, and advanced recipes.
10. **Development, limitations, and license.** Keep the statements accurate, concise, and current.

**README quality requirements:**

- Use ordinary language and short examples; avoid making the README read like a design specification.
- Show genuine outputs produced by the tool, not a mockup falsely labeled as generated output.
- Verify every example command against the actual CLI; do not publish proposed commands as working features.
- Keep advanced schemas and long command references in `docs/`, not in the root README.
- Do not feature proprietary downstream projects or their art.
- Confirm images load correctly when viewed on GitHub.

**Acceptance:** Someone unfamiliar with Rust graphics can understand the project and generate their first PNG without browsing source code.

### 5.2 Shared Style Profiles

Introduce an opt-in, versioned JSON profile that describes measurable visual constraints shared across many assets.

Proposed rules:

- Canvas and tile-size conventions, plus preferred sprite/object size ranges.
- Perspective and visible-face conventions.
- Palette roles such as outline, top, front, side, highlight, and shadow.
- Outline thickness, permitted palette colors, maximum visible color count, and alpha policy.
- Fixed light direction and preferred ground-contact position.
- Optional character proportion guidance and animation baseline rules.

Provide **automated checks only where measurable**; for subjective rules, generate a visual reference sheet and actionable warnings. Do not imply that a validator can judge artistic beauty.

**Acceptance:** Several separately generated assets can be checked against one shared profile, with predictable machine-readable violations.

### 5.3 Parameterized Templates and Reusable Parts

Consolidate template functionality into one current recipe format:

- Named parameters (`wall_color`, `roof_color`, `height`, `side_depth`, `seed`, etc.).
- Reusable components with offsets, flips, and palette substitutions.
- Dimension constraints and small, explicit variants.
- A versioned schema and stable default values.
- Seeded, deterministic generation of multiple related assets.

Avoid embedding an unrestricted scripting language; reuse existing batch/drawing operations where feasible.

**Acceptance:** A single cottage recipe can create several sizes and color schemes without hand-editing hundreds of drawing steps.

### 5.4 Pixel-Perfect 2D Depth and Perspective Helpers

Add generic 2D shape/assembly operations for:

- Top/front/side polygon faces of box-like props.
- Roof planes, eaves, side walls, and facade offsets.
- Door recesses, windows, awnings, platforms, stairs, and short walls.
- Configurable apparent height and side-face depth.
- Consistent shading between visible faces using the style-profile palette.

Outputs must remain ordinary, pixel-aligned raster artwork. No voxels, meshes, perspective cameras, or 3D scene graph are required.

**Acceptance:** An example cottage and a crate convincingly show top/front/side surfaces while maintaining crisp pixels and consistent perspective.

### 5.5 Offline Scene Preview and Contact Sheets

Add an **offline preview** for displaying multiple generated assets together:

- Accept a small, versioned JSON list of asset paths, positions, scale, and optional draw order.
- Respect supplied ground anchors and render-order layers.
- Optionally sort by ground-contact Y to approximate sprite overlap.
- Export enlarged nearest-neighbor PNGs and organized contact sheets.
- Provide a simple solid or tiled background for judging outlines and transparency.

**Keep this lightweight.** It is a quality-assurance preview, not a runtime renderer, editable map designer, or replacement for a game's scene system.

**Acceptance:** A scene containing a character, two buildings, a tree, a bench, and terrain can be rendered as one PNG without external image-editing software.

## 6. P1 Implementation Details

### 6.1 Ground Anchors, Attachment Points, and Split Layers

Build on existing pivots and animation metadata:

- Ground-contact anchor (typically at a character's feet or an object's base).
- Optional hand/item/door/attachment anchors, independent of sprite bounds.
- Optional footprint/placement rectangle or polygon.
- Separate exportable visual parts for objects requiring overlap, such as a tree trunk versus canopy or a doorway behind versus in front of a character.
- Retain convenient flattened PNG output for users who do not need split exports.

Use an engine-neutral, versioned metadata format. Update bundled consumers and tests together when the format changes.

**Acceptance:** A downstream renderer can position a character consistently and draw parts of a tall object in front of or behind it, using exported files and documented metadata alone.

### 6.2 Baked Lighting and Shadow Helpers

Provide offline pixel-art helpers for:

- Role-based light/shadow palette ramps.
- Consistent top/front/side face shading.
- Contact shadows beneath objects and characters.
- Pixel-snapped cast-shadow silhouettes with transparency.
- Optional alternate palette variants, such as daytime and evening.

This does **not** include runtime shaders, dynamic point lights, or a lighting engine.

**Acceptance:** Generic props, buildings, and a character share a coherent shadow direction and can be exported without blur or fractional-pixel artifacts.

### 6.3 Animation Alignment and Consistency Checks

Use existing frames, tags, and pivots to detect common authoring mistakes:

- Unintended changes to ground-contact position.
- Unexpected clipping or changing transparent margins.
- Inconsistent outline thickness or palette usage.
- Major directional proportion drift.
- Missing frames/tags and invalid pivot metadata.

Support explicit exceptions for intentional movement such as jumps or tall poses. Export a JSON validation report and a directional-animation contact sheet.

**Acceptance:** A four-direction walking-character example passes validation, while intentionally damaged frames produce understandable warnings.

## 7. P2 Implementation Details

### 7.1 Elevation and Terrain Transitions

Add optional templates or recipes for steps, raised pavement, curbs, low retaining walls, ledges, and simple bridges. Produce visually consistent transitions using existing tileset APIs. Preview neighboring combinations to detect obvious seams.

Do not add terrain physics, pathfinding, world generation, or collision simulation to the asset tool.

### 7.2 Cohesive Families and Pack Validation

Allow a single data file and seed to produce related neutral assets—e.g., cottages with different roofs, several trees, crates, outdoor furniture, or clothing variations.

Add pack-level checks for:

- Missing exports or manifests.
- Dimensions and palette discrepancies.
- Missing animation directions and bad anchors.
- Incompatible naming or duplicate IDs.
- Unexpected alpha, tile-boundary, or metadata errors.

**Acceptance:** Re-running a pack recipe generates identical outputs and a machine-readable validation report.

## 8. CLI-First Design and Example Future Commands

**These command names are proposals, not claims about the current CLI.** Final syntax must follow existing conventions and be documented only after implementation and tests.

```bash
# Validate an asset against a generic style profile.
painter style check --profile styles/village.json --input assets/character.painter.json --json

# Produce an asset from a parameterized template.
painter generate --input recipes/cottage.template.json --parameters variants/red-roof.json

# Preview exported assets as an offline composition.
painter scene render --input previews/village-corner.json --output preview.png --scale 4

# Validate directional animation consistency.
painter animation check --input assets/character.painter.json --profile styles/village.json --json

# Check all exports in a public example pack.
painter pack check --input exports/village/ --profile styles/village.json --json
```

General CLI requirements:

- Preserve existing commands and flags.
- Keep library behavior and CLI commands consistent.
- Support scriptable paths, deterministic output, useful exit codes, and JSON diagnostics.
- Prefer composable commands over introducing a second application or GUI.
- Avoid introducing a required dependency on any game engine.

## 9. Public, Generic Acceptance Demonstration

### Sample: Village Corner Asset Pack

Build a small, cohesive set of **independently authored public demonstration assets**:

- One neutral character with four-direction idle/walk frames.
- A cottage and a workshop with visible roof/front/side depth.
- Grass, path, pavement, and simple curb tiles.
- A tree with separately exportable trunk/canopy layers.
- A crate, bench, and streetlamp.
- Ground shadows, consistent palette, and a shared style profile.
- A contact sheet and one assembled scene preview.
- PNG exports, optional animation/tile JSON metadata, and reproducible source recipes.

All source art, names, recipe files, metadata, and documentation must be generic and cleared for inclusion in the open-source repository. The example pack is a **tooling demonstration**, not an asset pack for any private production project.

**Pass criteria:**

1. Assets appear to share a scale, camera convention, palette, and light direction.
2. Buildings and props exhibit a modest but convincing 2.5D illusion using only 2D artwork.
3. Sprite baselines and overlap behavior can be inspected in an offline scene preview.
4. Repeated generation with identical inputs creates identical pixels and metadata.
5. An automated agent can generate, inspect, revise, and export without a GUI.
6. Existing v0.2.0 canvases, commands, exports, and relevant integrations continue to work.
7. The public repository contains **no content borrowed from an unrelated private project**.

## 10. Recommended Pull Request Sequence

1. **PR 1 — Human-first README.** Restructure `README.md`, verify a quick start, surface existing generic gallery assets, and link advanced references.
2. **PR 2 — Style profiles.** Implement versioned profile schema, validation, and a generic reference sheet.
3. **PR 3 — Parameterized templates and depth helpers.** Create reusable geometry recipes and one neutral example building.
4. **PR 4 — Offline scene preview.** Add image composition and contact-sheet generation.
5. **PR 5 — Anchors and split-layer exports.** Extend metadata and add example overlap output.
6. **PR 6 — Baked shading and animation QA.** Add reusable shadow helpers, validation, and visual tests.
7. **PR 7+ — Elevation and pack automation.** Add only after the public sample reveals a clear need.

Keep pull requests reasonably small and independently testable. A README update should not require unrelated architecture changes.

## 11. Testing and Definition of Done

For each relevant feature:

- Unit and integration tests cover normal cases, malformed inputs, bounds, alpha, and metadata consistency.
- Golden pixel fixtures make raster behavior reproducible.
- CLI tests cover failures, `--json`, and deterministic output.
- File writes preserve existing atomicity and overwrite safety.
- Documentation distinguishes implemented commands from future proposals.
- New examples are generated by reproducible commands rather than manually edited binaries alone.
- Source files, screenshots, test data, templates, and docs receive a **public-content and licensing review**.
- README screenshots and links are tested or manually verified before release.

### Final Responsibility Boundary

**Pixel Painter owns:** Authoring primitives, recipes, palettes, style checking, offline previews, animation/atlas exports, and optional placement metadata.

**The downstream application owns:** Runtime cameras, world rendering, real-time Y sorting, collision, movement, dynamic lighting, interaction, and gameplay.

**Guiding principle:** Improve the ability to create beautiful, consistent, reusable 2D artwork with convincing visual depth. Keep the public toolkit generic, accessible to humans, and easy for coding agents to automate.
