#!/usr/bin/env python3
"""Validate the Habit Trackers by Madhab CycloneDX dependency inventory."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

EXPECTED_PROJECT = "habit-trackers-by-madhab"
EXPECTED_VERSION = "0.1.0"
EXPECTED_SPEC_VERSION = "1.7"
REQUIRED_DIRECT_DEPENDENCIES = {"gtk4", "rusqlite"}


def validate_document(document: Any) -> list[str]:
    """Return validation errors; never modify or normalize the SBOM."""
    errors: list[str] = []

    if not isinstance(document, dict):
        return ["SBOM root must be a JSON object"]

    if document.get("bomFormat") != "CycloneDX":
        errors.append("bomFormat must be 'CycloneDX'")

    if document.get("specVersion") != EXPECTED_SPEC_VERSION:
        errors.append(f"specVersion must be '{EXPECTED_SPEC_VERSION}'")

    metadata = document.get("metadata")
    if not isinstance(metadata, dict):
        errors.append("metadata must be an object")
        metadata = {}

    root_component = metadata.get("component")
    if not isinstance(root_component, dict):
        errors.append("metadata.component must identify the application")
    else:
        if root_component.get("name") != EXPECTED_PROJECT:
            errors.append(f"root component name must be '{EXPECTED_PROJECT}'")
        if root_component.get("version") != EXPECTED_VERSION:
            errors.append(f"root component version must be '{EXPECTED_VERSION}'")

    components = document.get("components")
    if not isinstance(components, list):
        errors.append("components must be an array")
        components = []

    names = {
        component.get("name")
        for component in components
        if isinstance(component, dict) and isinstance(component.get("name"), str)
    }
    missing = sorted(REQUIRED_DIRECT_DEPENDENCIES - names)
    if missing:
        errors.append("missing expected direct dependency components: " + ", ".join(missing))

    dependencies = document.get("dependencies")
    if not isinstance(dependencies, list) or not dependencies:
        errors.append("dependencies must contain the resolved dependency relationships")

    return errors


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(f"Usage: {Path(argv[0]).name} <sbom.json>", file=sys.stderr)
        return 2

    path = Path(argv[1])
    try:
        with path.open("r", encoding="utf-8") as source:
            document = json.load(source)
    except OSError as error:
        print(f"Cannot read SBOM: {error}", file=sys.stderr)
        return 2
    except json.JSONDecodeError as error:
        print(f"SBOM is not valid JSON: {error}", file=sys.stderr)
        return 1

    errors = validate_document(document)
    if errors:
        for error in errors:
            print(f"SBOM validation error: {error}", file=sys.stderr)
        return 1

    count = len(document["components"])
    print(
        f"Validated CycloneDX {document['specVersion']} SBOM for "
        f"{EXPECTED_PROJECT} {EXPECTED_VERSION} ({count} dependency components)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
