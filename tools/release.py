#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
"""Build and verify release archives locally. Requires Python 3.12+ and Cargo.

This tool never pushes, creates tags, or publishes. Source archives require a
clean committed checkout; binary archives reuse its validated dependency notices.
"""

import argparse
import hashlib
import json
import os
import platform as host_platform
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile
from pathlib import Path, PurePosixPath
from string import Template
from urllib.parse import unquote, urlsplit

PLATFORMS = {
    "linux-x86_64": ("x86_64-unknown-linux-gnu", "Ubuntu 22.04, GNU libc"),
    "windows-x86_64": ("x86_64-pc-windows-msvc", "Windows Server 2022"),
    "macos-aarch64": ("aarch64-apple-darwin", "macOS 15, deployment target 13.0"),
    "macos-x86_64": ("x86_64-apple-darwin", "macOS 15, deployment target 13.0"),
}
ROOT = Path(__file__).resolve().parents[1]


class ReleaseError(Exception):
    """An invalid candidate or incomplete artifact; no publication is attempted."""


def run(*args, cwd, env=None):
    result = subprocess.run(args, cwd=cwd, env=env, capture_output=True, text=True)
    if result.returncode:
        raise ReleaseError(
            f"{args[0]} failed ({result.returncode}): {result.stderr.strip()}"
        )
    return result.stdout


def digest(path):
    with path.open("rb") as contents:
        return hashlib.file_digest(contents, "sha256").hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(
        json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def project(root):
    return tomllib.loads((root / "rust/Cargo.toml").read_text(encoding="utf-8"))[
        "package"
    ]


def validate_tag(version, tag, publish):
    if not re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:[-+][0-9A-Za-z.+-]+)?", version):
        raise ReleaseError("invalid package version for a release archive")
    if tag and tag != f"v{version}":
        raise ReleaseError(f"tag must be v{version}")
    if publish and not tag:
        raise ReleaseError("publication requires an existing version tag")


def prepare(root, tag="", publish=False):
    metadata = project(root)
    validate_tag(metadata["version"], tag, publish)
    if run(
        "git", "status", "--porcelain", "--untracked-files=normal", cwd=root
    ).strip():
        raise ReleaseError(
            "commit the complete candidate before packaging; checkout is dirty"
        )
    commit = run("git", "rev-parse", "HEAD", cwd=root).strip()
    if tag:
        tagged = run(
            "git", "rev-parse", "--verify", f"refs/tags/{tag}^{{commit}}", cwd=root
        ).strip()
        if tagged != commit:
            raise ReleaseError("tag does not point at the checked-out candidate")
    return {"version": metadata["version"], "tag": tag, "commit": commit}


def registry_packages(lockfile):
    packages = tomllib.loads(lockfile.read_text(encoding="utf-8"))["package"]
    return sorted(
        (p for p in packages if "source" in p),
        key=lambda p: (p["name"], p["version"], p["source"]),
    )


def lock_digest(lockfile):
    # Parsed content is stable across Windows CRLF and Unix LF checkouts.
    return package_digest(registry_packages(lockfile))


def package_digest(packages):
    """Identify complete lock records, independent of inventory/check-out order."""
    data = json.dumps(
        sorted(
            packages,
            key=lambda package: (
                package["name"],
                package["version"],
                package["source"],
            ),
        ),
        sort_keys=True,
        separators=(",", ":"),
    )
    return hashlib.sha256(data.encode()).hexdigest()


def within(root, relative):
    path = PurePosixPath(relative)
    if path.is_absolute() or ".." in path.parts or "\\" in relative or ":" in relative:
        raise ReleaseError(f"unsafe artifact path: {relative!r}")
    candidate = (root / path).resolve()
    if not candidate.is_relative_to(root.resolve()):
        raise ReleaseError(f"artifact path leaves its directory: {relative!r}")
    return candidate


def make_notices(vendor, lockfile, destination):
    """Preserve upstream licenses/attributions, including LICENSES/ and AUTHORS."""
    destination.mkdir(parents=True, exist_ok=False)
    available = {}
    for manifest in vendor.glob("*/Cargo.toml"):
        package = tomllib.loads(manifest.read_text(encoding="utf-8"))["package"]
        available[(package["name"], package["version"])] = (manifest.parent, package)
    records = []
    lines = [
        "# Third-party notices",
        "",
        "Core dependency license expressions and original notice texts.",
        "Original upstream license and attribution files accompany each entry; this inventory does not replace their terms.",
        "",
        "| Package | Version | Declared license | Texts |",
        "|---|---|---|---|",
    ]
    for dependency in registry_packages(lockfile):
        key = (dependency["name"], dependency["version"])
        if not dependency["source"].startswith("registry+") or key not in available:
            raise ReleaseError(f"missing vendored registry dependency: {key}")
        directory, package = available[key]
        texts = []
        for path in sorted(directory.rglob("*")):
            parts = path.relative_to(directory).parts
            if path.is_file() and any(
                any(
                    word in part.upper()
                    for word in ("LICENSE", "COPYING", "COPYRIGHT", "NOTICE", "AUTHORS")
                )
                for part in parts
            ):
                texts.append(path)
        if package.get("license-file"):
            explicit = within(directory, package["license-file"])
            if explicit not in texts:
                texts.append(explicit)
        if not texts:
            raise ReleaseError(f"review missing license texts for {key}")
        files = {}
        for text in texts:
            relative = (
                Path("third-party-licenses")
                / f"{key[0]}-{key[1]}"
                / text.relative_to(directory)
            )
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(text, target)
            files[relative.as_posix()] = digest(target)
        record = {
            **dependency,
            "license": package.get("license", "see license text"),
            "files": files,
        }
        records.append(record)
        links = ", ".join(f"[{Path(name).name}]({name})" for name in files)
        lines.append(f"| {key[0]} | {key[1]} | {record['license']} | {links} |")
    (destination / "THIRD_PARTY_NOTICES.md").write_text(
        "\n".join(lines) + "\n", encoding="utf-8"
    )
    write_json(
        destination / "dependency-notices.json",
        {"version": 1, "lock_digest": lock_digest(lockfile), "packages": records},
    )
    verify_notices(destination, lock_digest(lockfile))


def verify_notices(root, expected):
    inventory = read_json(root / "dependency-notices.json")
    # Recompute from the actual records: a copied digest alone cannot establish
    # completeness, uniqueness, or agreement with dependency versions/checksums.
    dependencies = [
        {
            key: value
            for key, value in package.items()
            if key not in {"license", "files"}
        }
        for package in inventory["packages"]
    ]
    if (
        inventory["version"] != 1
        or inventory["lock_digest"] != expected
        or not inventory["packages"]
        or package_digest(dependencies) != expected
    ):
        raise ReleaseError("dependency notices do not match the candidate lockfile")
    for package in inventory["packages"]:
        if not package["files"]:
            raise ReleaseError("dependency has no license texts")
        for name, checksum in package["files"].items():
            if digest(within(root, name)) != checksum:
                raise ReleaseError(f"dependency notice changed: {name}")


def copy_notices(source, destination):
    for name in ["THIRD_PARTY_NOTICES.md", "dependency-notices.json"]:
        shutil.copy2(source / name, destination / name)
    shutil.copytree(
        source / "third-party-licenses", destination / "third-party-licenses"
    )


def check_links(root):
    """Check project Markdown links; upstream vendored license prose is untouched."""
    excluded = {
        "vendor",
        "third-party-licenses",
        "target",
        ".git",
        "release",
        "local-assets",
        ".agents",
        ".codex",
    }
    for directory, subdirectories, filenames in os.walk(root):
        subdirectories[:] = [name for name in subdirectories if name not in excluded]
        for name in filenames:
            if not name.endswith(".md"):
                continue
            markdown = Path(directory) / name
            for target in re.findall(
                r"!?\[[^\]]*\]\(([^\s)]+)\)", markdown.read_text(encoding="utf-8")
            ):
                parsed = urlsplit(target)
                if parsed.scheme or parsed.netloc or not parsed.path:
                    continue
                path = (markdown.parent / unquote(parsed.path)).resolve()
                if not path.is_relative_to(root.resolve()) or not path.exists():
                    raise ReleaseError(
                        f"broken packaged link in {markdown.relative_to(root)}: {target}"
                    )


def unpack(archive, destination):
    destination.mkdir(parents=True, exist_ok=False)
    if archive.name.endswith(".zip"):
        with zipfile.ZipFile(archive) as packed:
            for member in packed.infolist():
                within(destination, member.filename)
                if stat.S_ISLNK(member.external_attr >> 16):
                    raise ReleaseError("release archives must not contain symlinks")
            packed.extractall(destination)
    else:
        with tarfile.open(archive) as packed:
            for member in packed.getmembers():
                within(destination, member.name)
                if not member.isfile() and not member.isdir():
                    raise ReleaseError(
                        "release archives must contain only files and directories"
                    )
            packed.extractall(destination, filter="data")
    entries = list(destination.iterdir())
    if len(entries) != 1 or not entries[0].is_dir():
        raise ReleaseError(
            "release archive must contain exactly one top-level directory"
        )
    return entries[0]


def pack(directory, output, windows=False):
    archive = output / (directory.name + (".zip" if windows else ".tar.gz"))
    if archive.exists():
        raise ReleaseError(f"refusing to overwrite {archive}")
    if windows:
        with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as packed:
            for path in sorted(directory.rglob("*")):
                if path.is_file():
                    packed.write(path, path.relative_to(directory.parent).as_posix())
    else:
        with tarfile.open(archive, "x:gz") as packed:
            packed.add(directory, arcname=directory.name)
    return archive


def build_info(root, candidate, kind, **fields):
    return {
        "format_version": 1,
        "package": project(root)["name"],
        **candidate,
        "kind": kind,
        "rustc": run("rustc", "-Vv", cwd=root).strip(),
        "build_host": host_platform.platform(),
        "rustflags": os.environ.get("RUSTFLAGS", ""),
        "macos_deployment_target": os.environ.get("MACOSX_DEPLOYMENT_TARGET", ""),
        "lock_digest": lock_digest(root / "rust/Cargo.lock"),
        **fields,
    }


def export_tree(root, commit, destination, paths=()):
    """Read committed files only; ignored local files never enter a download."""
    destination.mkdir()
    with tempfile.TemporaryFile(dir=destination.parent) as archive:
        subprocess.run(
            ["git", "archive", "--format=tar", commit, *paths],
            cwd=root,
            stdout=archive,
            check=True,
        )
        archive.seek(0)
        with tarfile.open(fileobj=archive) as packed:
            for member in packed.getmembers():
                within(destination, member.name)
                if not member.isfile() and not member.isdir():
                    raise ReleaseError(
                        "source checkout contains a symlink or special file"
                    )
            packed.extractall(destination, filter="data")


def source_archive(root, output, notices, offline):
    candidate = prepare(root)
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="painter-source-", dir=output) as temporary:
        stage = Path(temporary)
        source = stage / f"painter-{candidate['version']}-source"
        export_tree(root, candidate["commit"], source)
        command = [
            "cargo",
            "vendor",
            "--locked",
            "--manifest-path",
            "rust/Cargo.toml",
            "--sync",
            "integrations/macroquad/Cargo.toml",
        ]
        if offline:
            command.append("--offline")
        configuration = run(*command, "vendor", cwd=source)
        (source / ".cargo").mkdir(exist_ok=True)
        (source / ".cargo/config.toml").write_text(configuration, encoding="utf-8")
        make_notices(source / "vendor", source / "rust/Cargo.lock", notices)
        copy_notices(notices, source)
        write_json(source / "BUILD-INFO.json", build_info(root, candidate, "source"))
        check_links(source)
        return pack(source, output)


def binary_archive(root, output, notices, binary, platform):
    candidate = prepare(root)
    verify_notices(notices, lock_digest(root / "rust/Cargo.lock"))
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="painter-binary-", dir=output) as temporary:
        stage = Path(temporary)
        checkout = stage / "checkout"
        export_tree(
            root,
            candidate["commit"],
            checkout,
            (
                "LICENSE",
                "CHANGELOG.md",
                "release/README.md",
                "docs",
                "rust/examples",
                "rust/tests/fixtures",
            ),
        )
        kit = stage / f"painter-{candidate['version']}-{platform}"
        kit.mkdir()
        executable = "painter.exe" if platform.startswith("windows-") else "painter"
        shutil.copy2(binary, kit / executable)
        for name in ["LICENSE", "CHANGELOG.md"]:
            shutil.copy2(checkout / name, kit / name)
        shutil.copytree(checkout / "rust/examples", kit / "examples")
        shutil.copytree(checkout / "rust/tests/fixtures", kit / "examples/recipes")
        copy_notices(notices, kit)
        (kit / "docs").mkdir()
        repository = project(root)["repository"].removesuffix(".git")
        for name in [
            "cli-and-formats.md",
            "roadmap.md",
            "security-review.md",
            "coherent-assets.md",
        ]:
            text = (checkout / "docs" / name).read_text(encoding="utf-8")
            text = text.replace("tests/fixtures/", "examples/recipes/")
            text = text.replace("rust/examples/", "examples/")
            text = text.replace("./rust/target/release/painter", "./painter")
            if name == "coherent-assets.md":
                text = text.replace(
                    "Build the CLI once:\n\n```sh\ncargo build --release --locked --manifest-path rust/Cargo.toml\n```",
                    "Use the included executable; no Rust installation is required.",
                )
            text = text.replace(
                "../integrations/macroquad/README.md",
                f"{repository}/blob/{candidate['commit']}/integrations/macroquad/README.md",
            )
            (kit / "docs" / name).write_text(text, encoding="utf-8")
        template = Template(
            (checkout / "release/README.md").read_text(encoding="utf-8")
        )
        (kit / "README.md").write_text(
            template.substitute(
                version=candidate["version"],
                platform=platform,
                baseline=PLATFORMS[platform][1],
                source_archive=f"painter-{candidate['version']}-source.tar.gz",
            ),
            encoding="utf-8",
        )
        write_json(
            kit / "BUILD-INFO.json",
            build_info(
                root,
                candidate,
                "binary",
                platform=platform,
                target=PLATFORMS[platform][0],
                ci_baseline=PLATFORMS[platform][1],
                binary_sha256=digest(kit / executable),
            ),
        )
        check_links(kit)
        return pack(kit, output, windows=platform.startswith("windows-"))


def verify_archive(archive, smoke=False, offline_build=False):
    with tempfile.TemporaryDirectory(prefix="painter-verify-") as temporary:
        base = Path(temporary)
        root = unpack(archive, base / "unpacked")
        info = read_json(root / "BUILD-INFO.json")
        if info["format_version"] != 1 or not re.fullmatch(
            r"[0-9a-f]{40}", info["commit"]
        ):
            raise ReleaseError("invalid build provenance")
        extension = (
            ".zip" if info.get("platform", "").startswith("windows-") else ".tar.gz"
        )
        if archive.name != root.name + extension:
            raise ReleaseError("archive filename disagrees with its contents")
        if not (root / "LICENSE").is_file():
            raise ReleaseError("release archive is missing the project license")
        verify_notices(root, info["lock_digest"])
        check_links(root)
        if info["kind"] == "binary":
            if (
                root.name != f"painter-{info['version']}-{info['platform']}"
                or info["target"] != PLATFORMS[info["platform"]][0]
            ):
                raise ReleaseError("archive layout and build target disagree")
            executable = root / (
                "painter.exe" if info["platform"].startswith("windows-") else "painter"
            )
            if digest(executable) != info["binary_sha256"]:
                raise ReleaseError(
                    "packaged executable checksum differs from build provenance"
                )
            if smoke:
                environment = os.environ.copy()
                environment["PATH"] = ""

                def painter(*args):
                    return run(str(executable), *args, cwd=root, env=environment)

                if painter("--version").strip() != f"painter {info['version']}":
                    raise ReleaseError(
                        "packaged executable version differs from archive version"
                    )
                # Exercise the guide with its bundled recipe, including dry-run.
                painter("--file", "gem.json", "new", "--width", "8", "--height", "8")
                for arguments in [("--dry-run",), ()]:
                    painter(
                        "--file",
                        "gem.json",
                        "batch",
                        "--input",
                        "examples/recipes/gem.ops.json",
                        *arguments,
                    )
                painter(
                    "--file",
                    "gem.json",
                    "render",
                    "--output",
                    "gem-preview.png",
                    "--scale",
                    "16",
                )
                painter("--file", "gem.json", "inspect", "--json")
                painter(
                    "--file",
                    "gem.json",
                    "draw",
                    "rect",
                    "--x",
                    "0",
                    "--y",
                    "0",
                    "--width",
                    "2",
                    "--height",
                    "2",
                    "--fill",
                    "--color",
                    "#7299BB",
                )
                painter(
                    "--file",
                    "gem.json",
                    "render",
                    "--output",
                    "gem-preview.png",
                    "--scale",
                    "16",
                    "--replace",
                )
                painter("--file", "gem.json", "export", "--output", "gem.png")
                original = painter(
                    "--json", "--file", "examples/soccer-ball.json", "print"
                )
                painter(
                    "--json",
                    "--file",
                    "examples/soccer-ball.json",
                    "export",
                    "--output",
                    "roundtrip.png",
                )
                painter(
                    "--json",
                    "--file",
                    "roundtrip.json",
                    "import",
                    "--input",
                    "roundtrip.png",
                )
                restored = painter("--json", "--file", "roundtrip.json", "print")
                if json.loads(original) != json.loads(restored):
                    raise ReleaseError(
                        "packaged executable failed its exact RGBA round trip"
                    )
        elif info["kind"] == "source":
            if (
                root.name != f"painter-{info['version']}-source"
                or project(root)["version"] != info["version"]
            ):
                raise ReleaseError("source archive layout and version disagree")
            if lock_digest(root / "rust/Cargo.lock") != info["lock_digest"]:
                raise ReleaseError("source lockfile differs from build provenance")
            if offline_build:
                environment = os.environ.copy()
                environment["CARGO_HOME"] = str(base / "empty-cargo-home")
                environment["CARGO_TARGET_DIR"] = str(base / "build")
                for manifest, features in [
                    ("rust/Cargo.toml", []),
                    ("rust/Cargo.toml", ["--no-default-features"]),
                    ("integrations/macroquad/Cargo.toml", []),
                ]:
                    subprocess.run(
                        [
                            "cargo",
                            "test",
                            "--release",
                            "--locked",
                            "--offline",
                            "--manifest-path",
                            manifest,
                            *features,
                        ],
                        cwd=root,
                        env=environment,
                        check=True,
                    )
        else:
            raise ReleaseError("unknown release archive kind")
        return info


def checksums(directory, version):
    expected = {f"painter-{version}-source.tar.gz"}
    expected.update(
        f"painter-{version}-{platform}"
        + (".zip" if platform.startswith("windows-") else ".tar.gz")
        for platform in PLATFORMS
    )
    actual = {path.name for path in directory.iterdir() if path.is_file()}
    if actual != expected:
        raise ReleaseError(
            f"release assets differ: missing={sorted(expected - actual)}, extra={sorted(actual - expected)}"
        )
    candidates = [verify_archive(directory / name) for name in sorted(expected)]
    if (
        any(info["version"] != version for info in candidates)
        or len({(info["commit"], info["lock_digest"]) for info in candidates}) != 1
    ):
        raise ReleaseError(
            "release archives do not share one version, commit, and core lockfile"
        )
    with (directory / "SHA256SUMS").open("x", encoding="utf-8", newline="\n") as output:
        for name in sorted(expected):
            output.write(f"{digest(directory / name)}  {name}\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    commands = parser.add_subparsers(dest="command", required=True)
    candidate = commands.add_parser("prepare")
    candidate.add_argument("--tag", default="")
    candidate.add_argument("--publish", choices=["true", "false"], default="false")
    candidate.add_argument("--github-output", type=Path)
    source = commands.add_parser("source")
    source.add_argument("--output", type=Path, required=True)
    source.add_argument("--notices", type=Path, required=True)
    source.add_argument("--offline", action="store_true")
    binary = commands.add_parser("binary")
    binary.add_argument("--output", type=Path, required=True)
    binary.add_argument("--notices", type=Path, required=True)
    binary.add_argument("--binary", type=Path, required=True)
    binary.add_argument("--platform", choices=PLATFORMS, required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("archive", type=Path)
    verify.add_argument("--smoke", action="store_true")
    verify.add_argument("--offline-build", action="store_true")
    sums = commands.add_parser("checksums")
    sums.add_argument("--directory", type=Path, required=True)
    sums.add_argument("--version", required=True)
    args = parser.parse_args()
    try:
        if args.command == "prepare":
            result = prepare(args.root, args.tag, args.publish == "true")
            if args.github_output:
                with args.github_output.open("a", encoding="utf-8") as output:
                    output.writelines(
                        f"{key}={value}\n" for key, value in result.items()
                    )
            print(json.dumps(result))
        elif args.command == "source":
            print(
                source_archive(
                    args.root,
                    args.output.resolve(),
                    args.notices.resolve(),
                    args.offline,
                )
            )
        elif args.command == "binary":
            print(
                binary_archive(
                    args.root,
                    args.output.resolve(),
                    args.notices.resolve(),
                    args.binary.resolve(),
                    args.platform,
                )
            )
        elif args.command == "verify":
            verify_archive(args.archive.resolve(), args.smoke, args.offline_build)
            print(f"Verified {args.archive.name}")
        else:
            checksums(args.directory.resolve(), args.version)
    except (
        ReleaseError,
        OSError,
        ValueError,
        KeyError,
        subprocess.CalledProcessError,
        tarfile.TarError,
        zipfile.BadZipFile,
    ) as error:
        parser.exit(1, f"release error: {error}\n")


if __name__ == "__main__":
    main()
