# Security review

Reviewed on Linux on 2026-10-19. The intended use is a local CLI/library running
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
  build artifacts; public previews require an explicit manual publication request.
  Verification also requires archive filenames to match their platform/layout and
  recomputes the complete dependency lock identity from notice records, rejecting
  missing, duplicate, or altered package entries before checksums are produced.

Regression tests cover terminal controls in native filenames, unchanged JSON path
and error data, live/dangling symlink no-clobber behavior, private new files and
permission-preserving edits on Unix, and frame names containing traversal syntax.
Additional checks cover parser values, parser JSON, and a hostile Unix `argv[0]`.
Export frame paths remain generated numeric names, independent of display names.

## Dependency results

The local scan used the official cargo-audit 0.22.2 Linux release, verified against
its GitHub release asset SHA256, and a freshly fetched RustSec database at commit
`999ec89d5d3f10f20be14750ec8c2d8b44e0ac9b`. It checked both complete lockfiles,
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

Release archives have checksums but are not publisher-signed or notarized. Checksums
detect mismatches; authenticating the publisher requires a separate signing or
attestation process. Hosted Windows/macOS checks and release packaging remain
pending until their workflows run. `BUILD-INFO.json` records source and compiler
information; it is descriptive provenance, not a cryptographic attestation. The
crate manifest disables registry publication until an owned package name is chosen.
