#!/usr/bin/env python3
"""Fail-closed preflight for a tagged Habit Trackers by Madhab release candidate."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

REQUIRED_FILES = (
    ".github/workflows/engineering-checks.yml",
    ".github/workflows/data-portability.yml",
    ".github/workflows/rust-sbom.yml",
    ".github/workflows/release-candidate-gate.yml",
    ".github/workflows/release-attestation-contract.yml",
    "docs/RELEASE_CANDIDATE_GATE.md",
    "tests/test_release_attestations.py",
    "rust-toolchain.toml",
    "SECURITY.md",
    "docs/RELEASE_READINESS.md",
    "scripts/validate_sbom.py",
    "tests/test_sbom_validator.py",
    "packaging/org.madhab.habittracker.desktop",
    "packaging/org.madhab.habittracker.metainfo.xml",
    "packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg",
    "scripts/validate-desktop-metadata.py",
)


def _license_evidence(root: Path, package: dict[str, Any]) -> tuple[bool, str]:
    expression = str(package.get("license", "")).strip()
    license_file = str(package.get("license-file", "")).strip()

    if license_file:
        candidate = root / license_file
        if candidate.is_file():
            return True, ""
        return False, f"license-file does not exist: {license_file}"

    if not expression or expression.upper() == "UNLICENSED":
        return False, "Cargo.toml must contain an owner-selected SPDX license expression; UNLICENSED is not releasable"

    generic_files = ("LICENSE", "LICENSE.txt", "LICENSE.md", "COPYING")
    if any((root / name).is_file() for name in generic_files):
        return True, ""

    identifiers = [
        token
        for token in re.findall(r"[A-Za-z0-9.+-]+", expression)
        if token.upper() not in {"OR", "AND", "WITH"}
    ]
    if not identifiers:
        return False, "license expression has no recognizable SPDX identifier"

    aliases = {
        "Apache-2.0": ("LICENSE-APACHE", "LICENSE-Apache-2.0"),
        "MIT": ("LICENSE-MIT",),
    }
    missing = []
    for identifier in identifiers:
        candidates = aliases.get(identifier, (f"LICENSE-{identifier}",))
        if not any((root / candidate).is_file() for candidate in candidates):
            missing.append(identifier)
    if missing:
        return False, "missing license text for: " + ", ".join(missing)
    return True, ""


def _cargo_lock_errors(root: Path, package_name: str, package_version: str) -> list[str]:
    errors: list[str] = []
    lock_path = root / "Cargo.lock"
    if not lock_path.is_file() or lock_path.stat().st_size == 0:
        return ["Cargo.lock is missing or empty; generate and review it using the pinned Rust toolchain"]

    try:
        with lock_path.open("rb") as source:
            lock_data = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"Cargo.lock is not valid TOML: {error}"]

    packages = lock_data.get("package")
    if not isinstance(packages, list) or not packages:
        errors.append("Cargo.lock has no package records")
    elif not any(
        item.get("name") == package_name and item.get("version") == package_version
        for item in packages
        if isinstance(item, dict)
    ):
        errors.append(
            f"Cargo.lock does not contain the root package {package_name} {package_version}"
        )

    if (root / ".git").exists():
        tracked = subprocess.run(
            ["git", "ls-files", "--error-unmatch", "Cargo.lock"],
            cwd=root,
            check=False,
            capture_output=True,
            text=True,
        )
        if tracked.returncode != 0:
            errors.append("Cargo.lock exists locally but is not tracked in the release commit")

    return errors


def inspect_release(root: Path, candidate_ref: str) -> list[str]:
    """Return all release blockers without changing files or executing build scripts."""
    errors: list[str] = []
    manifest_path = root / "Cargo.toml"
    try:
        with manifest_path.open("rb") as source:
            manifest = tomllib.load(source)
    except (OSError, tomllib.TOMLDecodeError) as error:
        return [f"Cargo.toml cannot be read: {error}"]

    package = manifest.get("package")
    if not isinstance(package, dict):
        return ["Cargo.toml has no [package] table"]

    package_name = str(package.get("name", "")).strip()
    package_version = str(package.get("version", "")).strip()
    if not package_name or not package_version:
        return ["Cargo.toml must declare a package name and version"]

    tag = candidate_ref.removeprefix("refs/tags/").strip()
    expected_tag = re.compile(
        rf"^v{re.escape(package_version)}(?:-(?:alpha|beta|rc)\.\d+)?$"
    )
    if not expected_tag.fullmatch(tag):
        errors.append(
            f"candidate tag {tag!r} must match Cargo.toml version {package_version!r} "
            "(for example v0.1.0 or v0.1.0-rc.1)"
        )

    errors.extend(_cargo_lock_errors(root, package_name, package_version))

    license_ok, license_error = _license_evidence(root, package)
    if not license_ok:
        errors.append(license_error)

    for relative in REQUIRED_FILES:
        if not (root / relative).is_file():
            errors.append(f"required release file is missing: {relative}")

    sbom_validator = root / "scripts/validate_sbom.py"
    if sbom_validator.is_file():
        validator_text = sbom_validator.read_text(encoding="utf-8")
        expected_version = re.search(
            r"""^EXPECTED_VERSION\s*=\s*["']([^"']+)["']""",
            validator_text,
            re.MULTILINE,
        )
        if expected_version is None:
            errors.append("SBOM validator must declare EXPECTED_VERSION")
        elif expected_version.group(1) != package_version:
            errors.append(
                "SBOM validator EXPECTED_VERSION does not match Cargo.toml "
                f"({expected_version.group(1)!r} != {package_version!r})"
            )

    metainfo_path = root / "packaging/org.madhab.habittracker.metainfo.xml"
    if metainfo_path.is_file():
        try:
            metainfo = ET.parse(metainfo_path).getroot()
            release = metainfo.find("./releases/release")
            if release is None:
                errors.append("AppStream metainfo must include a release entry")
            elif release.get("version") != package_version:
                errors.append(
                    "AppStream release version does not match Cargo.toml "
                    f"({release.get('version')!r} != {package_version!r})"
                )
        except (OSError, ET.ParseError) as error:
            errors.append(f"AppStream metainfo is invalid XML: {error}")

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--ref", required=True, help="release candidate tag, e.g. v0.1.0-rc.1")
    args = parser.parse_args(argv)

    root = args.root.resolve()
    errors = inspect_release(root, args.ref)
    if errors:
        print("RELEASE CANDIDATE: BLOCKED")
        for error in errors:
            print(f" - {error}")
        print(f"\n{len(errors)} release blocker(s) must be resolved before building.")
        return 1

    print("RELEASE CANDIDATE: PREFLIGHT PASSED")
    print(f"Package manifest, lockfile, license, security policy and release metadata are present for {args.ref}.")
    print("The locked build, test, Clippy, packaging and SBOM jobs must still pass.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
