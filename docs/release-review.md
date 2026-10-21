# Release preparation

Pixel Painter v0.4.0 is a stable release candidate. The [verification record](verification.md)
describes completed Linux checks; hosted Windows/macOS checks and a complete
four-platform trial download set must pass before publication. The optional
Macroquad consumer retains the [documented advisories](security-review.md).

## Candidate and shared gates

Include all implementation, tests, examples, documentation, and workflows in the
candidate commit. Release preparation rejects modified or untracked files and
requires any supplied version tag to point at that exact commit. Packaging uses
`git archive HEAD`, preventing ignored local experiments and build output from
entering the source download. The version is read from `rust/Cargo.toml`.

The [shared check workflow](../.github/workflows/checks.yml) runs formatting,
Clippy, debug/release tests, library tests without the default CLI feature,
documentation, minimum Rust 1.88 tests, independent
consumer checks, advisory audits, gallery reproduction, release-tool regressions,
and standalone Cargo package verification. The release workflow calls these
checks with the candidate's commit ID before building downloads. The former
separate CI workflow has been consolidated; any repository branch-protection
requirements should reference the current check jobs.

The core audit denies vulnerabilities and informational warnings. The independent
experimental consumer reports its known informational advisories without
suppressions and fails vulnerabilities. Its successful rendering tests do not
establish memory soundness or make its audit clean.

## Local packaging

The release helper requires Python 3.12+, Git, and Cargo, and does not publish,
push, or create tags. Run from a clean committed checkout. Choose new output and
notice directories; existing archives and notice directories are not overwritten.
For example, using a fresh directory outside the checkout:

```sh
python3 -m unittest discover -s tools -v
python3 tools/release.py prepare
python3 tools/release.py source --output /tmp/painter-candidate/dist --notices /tmp/painter-candidate/notices
python3 tools/release.py verify /tmp/painter-candidate/dist/painter-0.4.0-source.tar.gz --offline-build
cargo build --release --locked --manifest-path rust/Cargo.toml
python3 tools/release.py binary --binary rust/target/release/painter --platform linux-x86_64 --notices /tmp/painter-candidate/notices --output /tmp/painter-candidate/dist
python3 tools/release.py verify /tmp/painter-candidate/dist/painter-0.4.0-linux-x86_64.tar.gz --smoke
```

Use the platform corresponding to the actual compiler target. The native workflow
builds/tests on Ubuntu 22.04, Windows Server 2022, macOS 15 Apple Silicon, and
macOS 15 Intel. Mac builds use deployment target 13.0; execution on earlier systems
than the CI baseline remains unverified. Windows links its C runtime statically;
Linux requires GNU libc. Extracted executable tests clear `PATH` and verify version
reporting and an exact RGBA PNG round trip.

Source packaging vendors both lockfiles and installs Cargo configuration at the
archive root so both packages can rebuild offline. Source verification uses an
empty Cargo cache and new target directory, testing the core with and without its
default CLI feature and the independent consumer. The graphical consumer still needs
the host's display/OpenGL libraries at runtime. Standalone Cargo packages also
include the GPL text; a regression prevents drift from the repository license.
The `painter` package name is already owned by an unrelated registry project, so
`publish = false` prevents accidental crates.io publication. Choose an owned
registry name and review downstream imports before changing that policy.

Every native download includes its own README, gallery, CLI/format and security
guides, changelog, GPL text, dependency inventory, original core dependency license
and attribution texts, and `BUILD-INFO.json`. The latter records the source commit,
compiler, target, and binary hash. These records do not constitute a signed
attestation. Original license files also remain in the vendored source.

`checksums --directory DIR --version 0.4.0` requires exactly one source archive
and all four native archives. It validates their layouts, notice file hashes,
local Markdown links, and shared version/commit/core lock identity before writing
`SHA256SUMS`. Filenames must match their platform contents; notice records must
represent every locked registry package exactly once, including its checksum and
dependency edges. Do not mix artifacts from separate workflow runs.

## Hosted trial and publication

Run Actions → **Build releases** with an empty tag and `publish` left false for a
trial. A matching existing version tag may also be supplied with publication
disabled. Tag pushes build artifacts without creating a public release. Review
the resulting `release-downloads` artifact, including extracted documentation,
examples, source rebuild, and each native executable. The changelog must describe
the final candidate; keep known limitations visible.

Only a manual dispatch with `publish` explicitly true and a matching existing
tag can enter the write-enabled publication job after every required job passes.
It creates or updates a draft release and uploads the validated five archives and
checksums before publishing it as a stable release and marking it latest. Existing public releases are not
overwritten. Binaries are unsigned and not notarized. Configure signing, macOS
notarization, or stronger provenance separately if the intended distribution
requires them.

The preparation and verification work does not push changes, create a project
commit/tag, or publish anything. Edits remain sequential: atomic replacement does
not prevent lost updates from concurrent writers. Export bundles use a manifest
completion marker; tests do not establish power-loss durability.
