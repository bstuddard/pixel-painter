# Coherent asset implementation review

Reviewed locally on Linux, 2026-10-19. Scope comes from the
[public 2.5D roadmap](coherent-assets-roadmap.md); working schemas and commands are
in the [authoring guide](coherent-assets.md).

## Implemented scope

| Capability | Implementation and evidence |
|---|---|
| Introduction | Root README with actual village output, quick start, and advanced guide |
| Shared style | Bounded profiles, role references, dimension/grid/palette/alpha/outline checks |
| Parameterized authoring | One typed recipe format, integer/color parameters, variants, transformed/recolored parts |
| Depth and baked lighting | Prism, roof, cottage, and stair assemblies; palette roles and silhouette shadows |
| Placement and composition | Ground/attachments/footprints, frame overrides, split-layer exports, scenes/contact sheets |
| Animation QA | Directional tags, baselines, clipping/proportion warnings, explicit frame exceptions |
| Cohesive families and elevation | Transactional packs, editable sources, complete export checks, curb/stair recipes |
| Public acceptance demo | Village Corner: 12 character frames, cottage/workshop, tree parts, props, terrain, stairs, and scene |

## Engineering decisions

- Templates and parameterized authoring use `recipe::Recipe`, version 2, and the
  4 MiB authoring reader. The duplicate `Template` module, fallback parsing, and
  untyped version-dispatch helpers are removed. Before 1.0, breaking API/format
  changes are documented without retaining old readers.
- The CLI and packs share the typed renderer and transactional batch operations.
  Seed, palette, and strict/nearest palette policy are explicit recipe fields.
  One pack shares its raster/preparation work allowance across every recipe.
- Flat canvases, layered projects, and optional placement sidecars serve distinct
  workflows. Resolved attachment maps validate after inheritance, before export.
  Animation motion exceptions cannot become proportion references.
- Shared asset loading charges referenced bytes, every source cel, and scaled
  images under explicit budgets. Configuration references stay inside a canonical
  root. Validation details and expansion work are bounded.
- Packs publish their completion marker last and check exports by rebuilding
  through the renderer. Release downloads read examples/docs from the selected
  Git commit, excluding ignored local files and caches. Source and binary
  packaging share the same validated Git-tree extraction.

## Verification

| Check | Local result |
|---|---|
| Stable Rust, debug and release | 158 default-feature and 118 library-only tests per configuration |
| Minimum Rust 1.88 | 158 default-feature, 118 library-only, and one standalone consumer test |
| Clippy, warnings denied | Linux, Windows x86_64, macOS ARM64/x86_64; both core feature configurations; standalone consumer |
| Rustdoc and formatting | Passed |
| Release-tool regressions | All 20 tests passed |
| Python and workflow checks | Lint, format, compilation, and actionlint passed |
| Example artifacts | All 268 checksum entries verify; both generators reproduce their manifests |
| Links and whitespace | Passed |
| Core dependency audit | No reported vulnerabilities or informational advisories |
| Secrets | No findings in the candidate files or reachable Git history |

Regression coverage includes exact pixels/alpha, seeds, variants, resource budgets,
scoped symlinks, traversal/platform prefixes, no-clobber outputs, rollback, staging
cleanup, stale placement, corrupt/missing exports, merged attachment limits,
animation exceptions, structured CLI failures, terminal controls, and UTF-8 write
boundaries. Release tests cover all four binary archive layouts, ignored local
files, immutable source selection, and rejection of committed symlinks.

The two older gallery recipe inputs and their checksum manifest were regenerated.
Every image in both collections remains byte-for-byte unchanged; all 148 village
artifact hashes remain unchanged. No runtime engine, GUI, script evaluator,
external asset service, or core dependency was added.

## Remaining boundaries

Native Windows/macOS execution and the graphical consumer smoke test require CI
on the committed PR candidate. Cross-target Clippy does not establish runtime
behavior. Release publication retains its separate source/binary archive gates.

Perspective, light direction, proportions, and artistic coherence require visual
review; validators check measurable properties. Footprints describe placement,
not collision geometry. Tile adjacency uses explicit tileset rules. Packs verify
internal consistency, not authenticity. Caller-selected directories must not have
hostile concurrent writers. The optional Macroquad consumer retains two unresolved
upstream advisories; see the [security review](security-review.md).
