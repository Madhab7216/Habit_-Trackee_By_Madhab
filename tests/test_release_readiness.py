from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from check_release_readiness import inspect_release  # noqa: E402


class ReleaseReadinessTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text(
            '[package]\nname = "habit-trackers-by-madhab"\nversion = "0.1.0"\nedition = "2024"\nlicense = "MIT"\n',
            encoding="utf-8",
        )
        (self.root / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "habit-trackers-by-madhab"\nversion = "0.1.0"\n',
            encoding="utf-8",
        )
        (self.root / "LICENSE").write_text("Approved license text\n", encoding="utf-8")
        for relative in (
            ".github/workflows/engineering-checks.yml",
            ".github/workflows/data-portability.yml",
            ".github/workflows/rust-sbom.yml",
            "rust-toolchain.toml",
            "SECURITY.md",
            "docs/RELEASE_READINESS.md",
            "scripts/validate_sbom.py",
            "tests/test_sbom_validator.py",
            "packaging/org.madhab.habittracker.desktop",
            "packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg",
            "scripts/validate-desktop-metadata.py",
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("fixture\n", encoding="utf-8")
        metainfo = self.root / "packaging/org.madhab.habittracker.metainfo.xml"
        metainfo.write_text(
            '<component><releases><release version="0.1.0"/></releases></component>',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temp.cleanup()

    def test_complete_release_candidate_passes(self) -> None:
        self.assertEqual(inspect_release(self.root, "v0.1.0-rc.1"), [])

    def test_missing_lockfile_blocks_release(self) -> None:
        (self.root / "Cargo.lock").unlink()
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("Cargo.lock is missing" in error for error in errors))

    def test_unlicensed_project_blocks_release(self) -> None:
        manifest = self.root / "Cargo.toml"
        manifest.write_text(
            '[package]\nname = "habit-trackers-by-madhab"\nversion = "0.1.0"\nlicense = "UNLICENSED"\n',
            encoding="utf-8",
        )
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("owner-selected SPDX license" in error for error in errors))

    def test_missing_license_text_blocks_release(self) -> None:
        (self.root / "LICENSE").unlink()
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("missing license text" in error for error in errors))

    def test_lockfile_must_contain_the_root_package(self) -> None:
        (self.root / "Cargo.lock").write_text(
            'version = 4\n\n[[package]]\nname = "other-package"\nversion = "1.0.0"\n',
            encoding="utf-8",
        )
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("does not contain the root package" in error for error in errors))

    def test_tag_must_match_package_version(self) -> None:
        errors = inspect_release(self.root, "v0.2.0")
        self.assertTrue(any("must match Cargo.toml version" in error for error in errors))

    def test_appstream_version_must_match_manifest(self) -> None:
        metainfo = self.root / "packaging/org.madhab.habittracker.metainfo.xml"
        metainfo.write_text(
            '<component><releases><release version="0.0.9"/></releases></component>',
            encoding="utf-8",
        )
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("AppStream release version" in error for error in errors))

    def test_missing_release_file_is_reported(self) -> None:
        (self.root / "SECURITY.md").unlink()
        errors = inspect_release(self.root, "v0.1.0")
        self.assertTrue(any("required release file is missing: SECURITY.md" in error for error in errors))

    def test_license_file_field_is_respected(self) -> None:
        manifest = self.root / "Cargo.toml"
        manifest.write_text(
            '[package]\nname = "habit-trackers-by-madhab"\nversion = "0.1.0"\nlicense = "MIT"\nlicense-file = "licenses/APP-LICENSE.txt"\n',
            encoding="utf-8",
        )
        (self.root / "LICENSE").unlink()
        (self.root / "licenses/APP-LICENSE.txt").parent.mkdir(parents=True, exist_ok=True)
        (self.root / "licenses/APP-LICENSE.txt").write_text("license\n", encoding="utf-8")
        self.assertEqual(inspect_release(self.root, "v0.1.0"), [])


if __name__ == "__main__":
    unittest.main()
