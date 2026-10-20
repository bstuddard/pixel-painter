# SPDX-License-Identifier: GPL-3.0-only
"""Regression tests for release safety and distributable contents.

Git writes are confined to disposable fixture repositories, never the project.
No test contacts a registry or creates a GitHub release.
"""

import copy
import io
import json
import os
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

import release


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="painter release tests ")
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / "repository"
        (self.root / "rust").mkdir(parents=True)
        (self.root / "rust/Cargo.toml").write_text(
            '[package]\nname="painter"\nversion="0.2.0"\n'
            'repository="https://github.com/example/painter"\n',
            encoding="utf-8",
        )
        self.lock = self.root / "rust/Cargo.lock"
        self.lock.write_text(
            'version=4\n[[package]]\nname="dep"\nversion="1.0.0"\n'
            'source="registry+https://github.com/rust-lang/crates.io-index"\n'
            'checksum="' + "a" * 64 + '"\n',
            encoding="utf-8",
        )
        self.git("init", "--quiet")
        self.commit()

    def git(self, *arguments):
        # Bind writes to the fixture, without user repository overrides, keys, or hooks.
        environment = {
            name: value
            for name, value in os.environ.items()
            if not name.startswith("GIT_")
        }
        environment.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull)
        return release.run("git", *arguments, cwd=self.root, env=environment)

    def commit(self):
        self.git("add", "--all")
        self.git(
            "-c",
            "user.name=Release Test",
            "-c",
            "user.email=release-test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "fixture",
        )

    def notices(self):
        vendor = self.base / "vendor"
        package = vendor / "dep"
        (package / "LICENSES").mkdir(parents=True)
        (package / "Cargo.toml").write_text(
            '[package]\nname="dep"\nversion="1.0.0"\nlicense="MIT"\n', encoding="utf-8"
        )
        (package / "LICENSES/MIT.txt").write_text(
            "upstream license\n", encoding="utf-8"
        )
        (package / "NOTICE").write_text("upstream copyright\n", encoding="utf-8")
        destination = self.base / "notices"
        release.make_notices(vendor, self.lock, destination)
        return destination

    def archives(self, mixed_commit=False):
        notices = self.notices()
        output = self.base / "downloads"
        output.mkdir()
        for platform in ["source", *release.PLATFORMS]:
            directory = self.base / f"painter-0.2.0-{platform}"
            directory.mkdir()
            release.copy_notices(notices, directory)
            (directory / "LICENSE").write_text("project license\n", encoding="utf-8")
            (directory / "README.md").write_text(
                "[License](LICENSE)\n", encoding="utf-8"
            )
            info = {
                "format_version": 1,
                "version": "0.2.0",
                "package": "painter",
                "commit": "a" * 40,
                "lock_digest": release.lock_digest(self.lock),
            }
            if platform == "source":
                (directory / "rust").mkdir()
                shutil.copy2(
                    self.root / "rust/Cargo.toml", directory / "rust/Cargo.toml"
                )
                shutil.copy2(self.lock, directory / "rust/Cargo.lock")
                info["kind"] = "source"
                if mixed_commit:
                    info["commit"] = "b" * 40
            else:
                executable = directory / (
                    "painter.exe" if platform.startswith("windows-") else "painter"
                )
                executable.write_bytes(b"fixture executable")
                info.update(
                    kind="binary",
                    platform=platform,
                    target=release.PLATFORMS[platform][0],
                    binary_sha256=release.digest(executable),
                )
            release.write_json(directory / "BUILD-INFO.json", info)
            release.pack(directory, output, windows=platform.startswith("windows-"))
        return output

    def test_publication_requires_a_matching_version_tag(self):
        release.validate_tag("0.2.0", "", False)
        release.validate_tag("0.2.0", "v0.2.0", True)
        for version, tag, publish in [
            ("0.2.0", "", True),
            ("0.2.0", "v0.1.1", False),
            ("../../bad", "", False),
        ]:
            with self.subTest(version=version, tag=tag, publish=publish):
                with self.assertRaises(release.ReleaseError):
                    release.validate_tag(version, tag, publish)

    def test_prepare_rejects_untracked_or_modified_candidate_files(self):
        self.assertEqual(
            release.prepare(self.root)["commit"], self.git("rev-parse", "HEAD").strip()
        )
        extra = self.root / "new-feature.rs"
        extra.write_text("uncommitted", encoding="utf-8")
        with self.assertRaisesRegex(release.ReleaseError, "dirty"):
            release.prepare(self.root)
        extra.unlink()
        with self.lock.open("a", encoding="utf-8") as file:
            file.write("\n# changed\n")
        with self.assertRaisesRegex(release.ReleaseError, "dirty"):
            release.prepare(self.root)

    def test_tag_must_reference_the_exact_clean_commit(self):
        self.git("tag", "v0.2.0")
        self.assertEqual(release.prepare(self.root, "v0.2.0", True)["tag"], "v0.2.0")
        (self.root / "new-feature.rs").write_text("new", encoding="utf-8")
        self.commit()
        with self.assertRaisesRegex(release.ReleaseError, "does not point"):
            release.prepare(self.root, "v0.2.0", True)

    def test_prepare_cli_reports_outputs_without_changing_the_checkout(self):
        outputs = self.base / "github-output"
        result = subprocess.run(
            [
                sys.executable,
                str(release.ROOT / "tools/release.py"),
                "--root",
                str(self.root),
                "prepare",
                "--github-output",
                str(outputs),
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        expected = json.loads(result.stdout)
        self.assertEqual(
            dict(line.split("=", 1) for line in outputs.read_text().splitlines()),
            expected,
        )
        self.assertEqual(self.git("status", "--porcelain"), "")

    def test_notice_inventory_preserves_upstream_texts_and_license_directories(self):
        notices = self.notices()
        self.assertEqual(
            (notices / "third-party-licenses/dep-1.0.0/LICENSES/MIT.txt").read_bytes(),
            b"upstream license\n",
        )
        self.assertEqual(
            (notices / "third-party-licenses/dep-1.0.0/NOTICE").read_bytes(),
            b"upstream copyright\n",
        )
        release.verify_notices(notices, release.lock_digest(self.lock))
        release.check_links(notices)

    def test_notice_inventory_rejects_tampering_and_different_lockfiles(self):
        notices = self.notices()
        with self.assertRaisesRegex(release.ReleaseError, "lockfile"):
            release.verify_notices(notices, "b" * 64)
        (notices / "third-party-licenses/dep-1.0.0/NOTICE").write_text(
            "changed", encoding="utf-8"
        )
        with self.assertRaisesRegex(release.ReleaseError, "changed"):
            release.verify_notices(notices, release.lock_digest(self.lock))

    def test_notice_inventory_requires_every_locked_package_exactly_once(self):
        notices = self.notices()
        shutil.rmtree(notices)
        second = self.base / "vendor/second"
        shutil.copytree(self.base / "vendor/dep", second)
        manifest = second / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('name="dep"', 'name="second"'))
        with self.lock.open("a", encoding="utf-8") as lock:
            lock.write(
                '[[package]]\nname="second"\nversion="1.0.0"\n'
                'source="registry+https://github.com/rust-lang/crates.io-index"\n'
                'checksum="' + "b" * 64 + '"\n'
            )
        release.make_notices(self.base / "vendor", self.lock, notices)
        inventory = release.read_json(notices / "dependency-notices.json")
        for mutation in ["missing", "duplicate", "name", "checksum", "dependencies"]:
            changed = copy.deepcopy(inventory)
            if mutation == "missing":
                changed["packages"].pop()
            elif mutation == "duplicate":
                changed["packages"].append(copy.deepcopy(changed["packages"][0]))
            else:
                changed["packages"][0][mutation] = (
                    ["unlocked"] if mutation == "dependencies" else "unlocked"
                )
            release.write_json(notices / "dependency-notices.json", changed)
            with (
                self.subTest(mutation=mutation),
                self.assertRaisesRegex(release.ReleaseError, "lockfile"),
            ):
                release.verify_notices(notices, release.lock_digest(self.lock))

    def test_lock_identity_ignores_checkout_line_endings(self):
        expected = release.lock_digest(self.lock)
        self.lock.write_bytes(self.lock.read_bytes().replace(b"\n", b"\r\n"))
        self.assertEqual(release.lock_digest(self.lock), expected)

    def test_missing_dependency_license_text_is_a_packaging_failure(self):
        notices = self.notices()
        shutil.rmtree(notices)
        shutil.rmtree(self.base / "vendor/dep/LICENSES")
        (self.base / "vendor/dep/NOTICE").unlink()
        with self.assertRaisesRegex(release.ReleaseError, "missing license"):
            release.make_notices(self.base / "vendor", self.lock, notices)

    def test_upstream_authors_file_can_contain_the_license_grant(self):
        notices = self.notices()
        shutil.rmtree(notices)
        package = self.base / "vendor/dep"
        shutil.rmtree(package / "LICENSES")
        (package / "NOTICE").rename(package / "AUTHORS")
        release.make_notices(self.base / "vendor", self.lock, notices)
        self.assertEqual(
            (notices / "third-party-licenses/dep-1.0.0/AUTHORS").read_bytes(),
            b"upstream copyright\n",
        )

    def test_standalone_crate_preserves_the_project_license_and_registry_policy(self):
        self.assertEqual(
            (release.ROOT / "LICENSE").read_bytes(),
            (release.ROOT / "rust/LICENSE").read_bytes(),
        )
        self.assertIs(release.project(release.ROOT)["publish"], False)

    def test_packaged_markdown_cannot_reference_missing_or_external_local_files(self):
        root = self.base / "documentation"
        root.mkdir()
        (root / "LICENSE").write_text("license", encoding="utf-8")
        markdown = root / "README.md"
        markdown.write_text(
            "[license](LICENSE) [web](https://example.com/a) [self](#heading)\n",
            encoding="utf-8",
        )
        release.check_links(root)
        for link in ["missing.md", "../repository/rust/Cargo.toml"]:
            markdown.write_text(f"[bad]({link})\n", encoding="utf-8")
            with (
                self.subTest(link=link),
                self.assertRaisesRegex(release.ReleaseError, "broken packaged link"),
            ):
                release.check_links(root)

    def test_tar_and_zip_extraction_reject_traversal_and_symlinks(self):
        for kind in ["tar", "zip"]:
            for name, symlink in [("../escape", False), ("package/link", True)]:
                archive = self.base / f"bad-{kind}-{symlink}.{kind}"
                if kind == "tar":
                    with tarfile.open(archive, "w") as packed:
                        info = tarfile.TarInfo(name)
                        if symlink:
                            info.type = tarfile.SYMTYPE
                            info.linkname = "/outside"
                            packed.addfile(info)
                        else:
                            info.size = 1
                            packed.addfile(info, io.BytesIO(b"x"))
                else:
                    with zipfile.ZipFile(archive, "w") as packed:
                        info = zipfile.ZipInfo(name)
                        if symlink:
                            info.external_attr = (stat.S_IFLNK | 0o777) << 16
                        packed.writestr(info, b"x")
                with (
                    self.subTest(kind=kind, symlink=symlink),
                    self.assertRaises(release.ReleaseError),
                ):
                    release.unpack(archive, self.base / f"extract-{kind}-{symlink}")
                self.assertFalse((self.base / "escape").exists())

    def test_checksums_require_all_five_consistent_archives(self):
        output = self.archives()
        release.checksums(output, "0.2.0")
        lines = (output / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(lines), 5)
        for line in lines:
            checksum, name = line.split("  ", 1)
            self.assertEqual(release.digest(output / name), checksum)
        with self.assertRaises(release.ReleaseError):
            release.checksums(output, "0.2.0")

    def test_export_tree_uses_the_selected_commit_instead_of_local_files(self):
        commit = self.git("rev-parse", "HEAD").strip()
        original = (self.root / "rust/Cargo.toml").read_bytes()
        (self.root / "rust/Cargo.toml").write_text("local edit\n", encoding="utf-8")
        (self.root / "rust/.env").write_text("local-only data\n", encoding="utf-8")
        destination = self.base / "checkout"
        release.export_tree(self.root, commit, destination, ("rust",))
        self.assertEqual((destination / "rust/Cargo.toml").read_bytes(), original)
        self.assertFalse((destination / "rust/.env").exists())

    def test_export_tree_rejects_committed_symlinks_before_extracting_files(self):
        # Git's index can represent a link even on systems without symlink privileges.
        target = self.base / "link-target"
        target.write_text("../local-only\n", encoding="utf-8")
        blob = self.git("hash-object", "-w", str(target)).strip()
        self.git("update-index", "--add", "--cacheinfo", f"120000,{blob},rust/link")
        self.git(
            "-c",
            "user.name=Release Test",
            "-c",
            "user.email=release-test@example.invalid",
            "commit",
            "--quiet",
            "-m",
            "symlink fixture",
        )
        commit = self.git("rev-parse", "HEAD").strip()
        destination = self.base / "checkout"
        with self.assertRaisesRegex(release.ReleaseError, "symlink"):
            release.export_tree(self.root, commit, destination)
        self.assertEqual(list(destination.iterdir()), [])

    def test_binary_downloads_include_resolved_readmes_guides_and_notices(self):
        notices = self.notices()
        shutil.copytree(
            release.ROOT / "rust/tests/fixtures", self.root / "rust/tests/fixtures"
        )
        for name in [
            "LICENSE",
            "CHANGELOG.md",
            "rust/examples/gallery/README.md",
            "rust/examples/village/README.md",
            "rust/examples/village/scene.json",
        ]:
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text("fixture\n", encoding="utf-8")
        (self.root / "docs").mkdir()
        for name in [
            "cli-and-formats.md",
            "roadmap.md",
            "security-review.md",
            "coherent-assets.md",
        ]:
            (self.root / "docs" / name).write_text(
                "[consumer](../integrations/macroquad/README.md)\n"
                "Recipe: `tests/fixtures/gem.ops.json`.\n"
                "[scene](../rust/examples/village/scene.json)\n",
                encoding="utf-8",
            )
        (self.root / "release").mkdir()
        shutil.copy2(
            release.ROOT / "release/README.md", self.root / "release/README.md"
        )
        binary = self.base / "executable"
        binary.write_bytes(b"fixture executable")
        (self.root / ".gitignore").write_text(".env\n__pycache__/\n", encoding="utf-8")
        self.commit()
        for name in [
            "rust/examples/.env",
            "rust/tests/fixtures/.env",
            "rust/examples/__pycache__/local.pyc",
        ]:
            ignored = self.root / name
            ignored.parent.mkdir(parents=True, exist_ok=True)
            ignored.write_text("local-only fixture data\n", encoding="utf-8")
        for platform in release.PLATFORMS:
            with self.subTest(platform=platform):
                archive = release.binary_archive(
                    self.root, self.base / "packages", notices, binary, platform
                )
                info = release.verify_archive(archive)
                self.assertEqual(info["platform"], platform)
                extracted = release.unpack(archive, self.base / f"extract-{platform}")
                self.assertFalse((extracted / "examples/.env").exists())
                self.assertFalse((extracted / "examples/recipes/.env").exists())
                self.assertFalse((extracted / "examples/__pycache__").exists())
                readme = (extracted / "README.md").read_text(encoding="utf-8")
                self.assertNotIn("${", readme)
                self.assertIn(release.PLATFORMS[platform][1], readme)
                self.assertEqual(
                    (extracted / "examples/recipes/gem.ops.json").read_bytes(),
                    (release.ROOT / "rust/tests/fixtures/gem.ops.json").read_bytes(),
                )
                self.assertIn(
                    "examples/recipes/gem.ops.json",
                    (extracted / "docs/cli-and-formats.md").read_text(encoding="utf-8"),
                )
                self.assertIn(
                    "../examples/village/scene.json",
                    (extracted / "docs/coherent-assets.md").read_text(encoding="utf-8"),
                )
                self.assertIn(
                    f"/blob/{info['commit']}/integrations/macroquad/README.md",
                    (extracted / "docs/security-review.md").read_text(encoding="utf-8"),
                )

    def test_checksums_reject_missing_assets_before_writing_a_manifest(self):
        output = self.archives()
        (output / "painter-0.2.0-windows-x86_64.zip").unlink()
        with self.assertRaisesRegex(release.ReleaseError, "missing"):
            release.checksums(output, "0.2.0")
        self.assertFalse((output / "SHA256SUMS").exists())

    def test_checksums_reject_mixed_source_commits(self):
        output = self.archives(mixed_commit=True)
        with self.assertRaisesRegex(release.ReleaseError, "one version, commit"):
            release.checksums(output, "0.2.0")
        self.assertFalse((output / "SHA256SUMS").exists())

    def test_checksums_reject_archives_with_swapped_platform_filenames(self):
        output = self.archives()
        linux = output / "painter-0.2.0-linux-x86_64.tar.gz"
        macos = output / "painter-0.2.0-macos-x86_64.tar.gz"
        linux_bytes, macos_bytes = linux.read_bytes(), macos.read_bytes()
        linux.write_bytes(macos_bytes)
        macos.write_bytes(linux_bytes)
        with self.assertRaisesRegex(release.ReleaseError, "filename"):
            release.checksums(output, "0.2.0")
        self.assertFalse((output / "SHA256SUMS").exists())


if __name__ == "__main__":
    unittest.main()
