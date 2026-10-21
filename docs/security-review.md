# Security review

Reviewed on Linux on 2026-10-20. The intended use is a local CLI/library running
as an ordinary user in a directory they control. Artwork contents may be untrusted;
command-line paths and the surrounding directory are selected by the caller.
This source review, regression testing, and advisory scan are not a penetration
test or proof of the absence of vulnerabilities.

## Findings and changes

- **Fixed: terminal escape injection through filenames.** Human status messages
  printed paths verbatim, allowing an artwork filename containing an OSC sequence
  to reach the terminal. Status and error formatting now escapes control characters
  while preserving ordinary Unicode, quotes, and path separators. JSON retains
  the original data, with its normal JSON escaping. The intentional pixel preview
  still emits ANSI colors built from numeric RGBA values.
- **Fixed: controls in argument-parser diagnostics.** Invalid argument values
  could retain carriage returns and C1 terminal controls in Clap's human errors.
  Argument contexts are now escaped before Clap adds its own layout/styling;
  JSON error strings retain their data. Usage/help use the fixed `painter` name,
  preventing an executable's supplied process name from injecting controls.
- **Fixed: mutable CI action references.** Checkout, artifact upload, and artifact
  download references in every workflow now use full commit SHAs verified against
  the official repositories. Read-only job permissions and disabled checkout
  credential persistence remain in place; only release publication has write access.
- **Added: dependency auditing.** CI audits both lockfiles using cargo-audit 0.22.2.
  Core vulnerabilities and informational warnings fail the core audit. The consumer
  audit reports its known informational notices without suppressions and fails
  vulnerabilities; those notices remain unresolved and are not a clean audit.
- **Added: crate-level unsafe-code prohibition.** The library and CLI forbid
  unsafe code in their own compilation units. This does not prohibit or establish
  the soundness of dependency implementations.
- **Added: release gates and archive verification.** Release jobs reuse the audit,
  minimum-Rust, formatting, test, documentation, consumer, and packaging checks
  against one source commit. Packaging rejects dirty candidates and mismatched
  tags, validates archive paths and local documentation links, preserves upstream
  license texts, and tests extracted binaries and offline source. Tag pushes only
  build artifacts; public releases require an explicit manual publication request.
  Verification also requires archive filenames to match their platform/layout and
  recomputes the complete dependency lock identity from notice records, rejecting
  missing, duplicate, or altered package entries before checksums are produced.

Regression tests cover terminal controls in native filenames, unchanged JSON path
and error data, live/dangling symlink no-clobber behavior, private new files and
permission-preserving edits on Unix, and frame names containing traversal syntax.
Additional checks cover parser values, parser JSON, and a hostile Unix `argv[0]`.
Export frame paths remain generated numeric names, independent of display names.

## Coherent authoring review

The new recipe/scene/pack configurations have a separate 4 MiB cap, bounded
collections, strict fields, and explicit expansion/depth/work limits. Scalar
parameters cannot execute code. Scene/pack references are confined to a canonical
configuration root and reject traversal, absolute/platform-prefixed paths, and
symlink escapes. Export paths remain generated numeric names. These protections
assume caller-controlled directories and no hostile concurrent writers. Generated
pack references use forward slashes on every OS, including recursive integrity
checks; resolving them retains the same backslash/traversal/root-confinement checks.

Placement metadata validates indices, anchors, names, frame counts, and part sizes;
export rejects stale source documents. Style scans retain at most 257 colors and
preflight outline work. Animation and pack scans share aggregate source budgets.
Split outputs and packs use the existing transactional manifest-last publication.
Pack checks rebuild deterministic exports and compare all generated files, rather
than implementing a second renderer. This checks consistency, not authenticity.

Regression tests cover scoped symlinks, traversal/platform prefixes, invalid/deep
parameters, stale placement, part ordering, source frame selection, corrupt/missing
export files, structured CLI failures, no-clobber outputs, and large-outline budgets.
The examples include only independently authored generic assets and relative paths.

## Additional security pass

The follow-up review reproduced and fixed three weaknesses in the unreleased
coherent-authoring implementation:

- **Scene resource accounting:** only displayed flattened pixels counted toward
  the scene source budget. Other frames/layers could multiply parsing and raster
  work across references. A shared input budget now charges every project cel
  before flattening, and includes tiled backgrounds. Scenes retain a separate
  aggregate cap for scaled asset images. Pack source loading uses the same budget.
- **Cumulative pack work:** every recipe previously started a new raster budget.
  One pack now shares the 67,108,864 raster-work allowance across all
  recipes. Component cloning, flips, and color replacement also consume
  that allowance before their pixel work starts. Failed batches preserve their
  input canvas, and failed packs clean up staging without publishing a marker.
- **Terminal controls in validation JSON:** a crafted pack directory containing a
  C1 control could appear literally in a default validation report. CLI JSON now
  escapes C1 and DEL in both output streams, including reports printed without
  `--json`. Decoded strings and ordinary Unicode remain unchanged. A streaming
  encoder avoids buffering large reports and handles split UTF-8 writes.

A shared 2 GiB referenced-source/config byte allowance also charges file lengths
before decoding, including malformed inputs. This prevents continued pack checks
from avoiding the aggregate allowance through parse failures. Existing per-file
limits still apply; artwork preflight may read a file twice. Raster accounting
charges a decoded source before flattening; the final source that exceeds the
remaining budget can be decoded under its existing per-file cap before rejection.
These are bounded local operations, not hard process RAM/CPU quotas.

Regression tests exercise hidden frames/layers, malformed input accounting,
special-file reference rejection, combined recipe exhaustion, component
preflight, transaction rollback, staging cleanup, crafted report paths, every
UTF-8 write boundary, and exact JSON string round trips. All example checksum
manifests verify after regeneration. The refreshed advisory database reports a clean
core audit and the same two unresolved consumer notices listed below.

Final Rust verification passed 178 CLI/default-feature tests and 135 library-only
tests in debug/release and on Rust 1.88. Clippy passed with warnings denied locally
and for Windows x86_64 and both Mac architectures, with/without the CLI. Rustdoc,
formatting, and local links passed. All three example generators reproduce their 336
artifact checksum entries. These checks are evidence for this source snapshot,
not proof that every possible secret or defect has been detected.

Two final correctness fixes validate resolved attachment maps after inheritance
and exclude motion-exempt poses from animation proportion references. Regressions
cover failure before publication, the valid 32-name boundary, and genuine drift
following an exempt first pose.

Preview requests now reject invalid scales before source loading and reject
oversized scaled scene/contact-sheet dimensions before raster allocation or
composition. Unscaled previews transfer their canvas instead of cloning it.
Regression tests cover zero/oversized scales, dimension overflow, pixel limits,
native/scaled pixel identity, and animation ordering with overlapping tags.

The final packaging review also reproduced ignored local `.env` files and caches
entering binary downloads through a working-directory copy. Release packaging now
reads examples, fixtures, and documentation from the selected immutable Git tree.
Shared extraction validates paths and rejects symlinks/special files before writing
any tree contents. Tests exercise ignored files in all four binary layouts,
immutable commit selection, and a Git symlink fixture without OS symlink privileges.
No actual credential was found; the reproduction used synthetic local-only data.

## Secret scanning

A separate scan used the official, SHA-256-verified
[Gitleaks 8.30.1 release](https://github.com/gitleaks/gitleaks/releases/tag/v8.30.1).
Its embedded default rules found no secrets in all 424 tracked and nonignored
untracked candidate files or in reachable Git history, including deleted
content. The scan used full redaction, no project ignore list or baseline, and
ignored inline suppression comments. A synthetic token verified detection.

Supplemental checks found no credential/private-key filenames in the candidate or
reachable history, no credential literals, credential-bearing URLs, private-key
headers, or personal home paths in candidate contents, and no PNG text/EXIF metadata.
The candidate scan was repeated after the pre-1.0 recipe simplification.
Ignored local files and build output were outside the candidate scan; tracked files
were included regardless of ignore rules. These checks cannot prove the absence of
every possible secret.

## Dependency results

The local scan used the official cargo-audit 0.22.2 Linux release, verified against
its GitHub release asset SHA256, and a freshly fetched RustSec database at commit
`e6f94f3760d5e6c54c1b05f204002263bc025524`. It checked both complete lockfiles,
without CPU/OS advisory filters or advisory ignore lists.

| Package scope | Result |
|---|---|
| Core library and CLI | No reported vulnerabilities or informational advisories |
| Independent Macroquad example | No vulnerability-class advisories; two unresolved informational advisories |

The consumer findings are:

- [RUSTSEC-2025-0035](https://rustsec.org/advisories/RUSTSEC-2025-0035.html):
  Macroquad 0.4.16 is affected by memory-soundness issues, including possible
  use-after-free through safe APIs. RustSec lists no patched release. Its INFO
  classification does not mean memory corruption is harmless.
- [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html):
  the consumer's `ttf-parser` 0.25.1 dependency is unmaintained. This is a
  maintenance notice, not evidence that this example has an exploitable font bug.

The consumer is an independent Cargo package and does not enter the core dependency
graph. It embeds the generated atlas and uses a built-in font. Avoid extending it
to accept untrusted assets or using it in production before replacing or repairing
the affected dependencies. Ordinary rendering tests do not establish memory soundness.

Recheck against current advisories with:

```sh
cargo install cargo-audit --version 0.22.2 --locked
cargo audit --file rust/Cargo.lock --deny warnings
cargo audit --file integrations/macroquad/Cargo.lock --deny warnings
```

The strict consumer command is expected to fail on the two documented notices.
These are database-driven results, not a source audit of every dependency.

## Existing protections and remaining boundaries

The core does not execute shell commands, evaluate template code, load plugins,
or make network requests at runtime. Recipes contain typed drawing operations and
embedded pixel data. Input schemas reject unknown fields. Artwork preflight checks
actual pixel counts and aggregate cels before raster allocation; readers bound
bytes, nesting, and string sizes. Configuration input, PNG decoding, raster work,
palette mapping, clipboard storage, and seam comparisons have explicit limits.

New outputs use no-clobber persistence. Edits use sibling temporary files and atomic
replacement; exports publish their manifest last. These mechanisms prevent ordinary
partial writes and accidental replacement, and are tested against symlink collisions.

They do not provide a filesystem sandbox or protection against hostile concurrent
writers. In particular, the artwork reader reuses one file handle across its two
passes, which prevents a path replacement from swapping the input, but does not
prevent another process from rewriting that open file in place. Parent directories
must also remain under the caller's control during output publication.

The advertised limits allow large files and images; they are not a hard process
memory/time quota. Inspection of high-color images, multiple image buffers, and
large output can consume substantial memory and disk. Hosting this as an upload
service would require private immutable input copies, isolated output directories,
and externally enforced memory, CPU, disk, and execution-time limits. That deployment
model has not been implemented or tested here.

A final local check rebuilt the latest source and Linux binary archives after the
security fixes. Extracted Cargo packages, binary smoke tests, and vendored source
release tests with an empty Cargo cache all passed. Native Windows/macOS execution
is checked in CI on the committed candidate.

Release archives have checksums but are not publisher-signed or notarized. Checksums
detect mismatches; authenticating the publisher requires a separate signing or
attestation process. Each new release requires hosted native checks and a full
packaging trial. `BUILD-INFO.json` records source and compiler
information; it is descriptive provenance, not a cryptographic attestation. The
crate manifest disables registry publication until an owned package name is chosen.


## Three-map review (v0.3.0)

Semantic channels use one document schema and compositor. Ordinary flattening
selects color layers; split parts reject map-layer selections. Normal vectors
are validated on read and after transactional edits, including hidden cels.
Map edits reject palette quantization, and normal edits reject RGB paint blending.
Vector flips/quarter turns preserve screen-space directions. Geometry color and
normal cels share one work budget and commit together; a regression exhausts that
combined budget after color alone succeeds and verifies both original cels remain.

One atlas layout and a combined channel/frame pixel cap precede raster allocation.
Companion paths use fixed channel names and numeric indices. All files stage before
completion-marker publication, and placement checks compare companion pixels too.
Asset packs retain their map requests and regenerate/compare every companion.
The usual controlled-directory and concurrent-writer limitations still apply.

The separate consumer uses only embedded fixtures and fixed shader sources. Lit
assets retain every GPU texture handle for the material's full lifetime; shader
samplers alone retain raw IDs. Readback tests compare 70 samples against the CPU
reference across both assets, all modes, flips, fallback samplers, and partial
coverage. This does not repair or change the upstream Macroquad advisories above.
No dependency was added to either package.

## Authoring and review (v0.4.0)

- No new dependencies, network access, external processes, or unsafe Rust were
  introduced in the core. Strict core advisory audit remains clean. The separate
  Macroquad consumer retains the two unsuppressed upstream advisories documented
  above; a stable core release does not change that demo's experimental status.
- Material ramp arrays, colors, and names are bounded. Ambiguous RGB ownership,
  invalid alpha, and map-layer shading fail transactionally. Extreme signed step
  counts use widened arithmetic before clamping.
- Mask and brush storage share the existing aggregate pixel bound. Selections and
  alpha snapshots are charged against the batch work limit. Mask dimensions and
  incompatible transforms are rejected. Excluded RGBA bytes, normal validation,
  palette checks, indexed diagnostics, and whole-batch rollback are preserved.
- Review bundles preflight dimensions, combined native views, scaled sheets, and
  work before creating output. They use fixed indexed filenames and the shared
  staged-directory publisher, refuse overwrites, and publish their completion
  marker last. Layer/material/frame names never become output paths.
- Solo/raw/channel and bundle previews leave the source project unchanged.
  Diagnostics contain frame names and counts, without source paths, credentials,
  environment values, or arbitrary PNG metadata. Generated PNGs contain no text
  or EXIF chunks. Source and release downloads are scanned before publication.
