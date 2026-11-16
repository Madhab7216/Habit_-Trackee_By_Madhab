#!/usr/bin/env python3
"""Validate that the desktop launcher, AppStream metadata and app build agree."""

from __future__ import annotations

import configparser
import re
import shlex
import sys
import tomllib
import xml.etree.ElementTree as ET
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DESKTOP_FILE = ROOT / "packaging/org.madhab.habittracker.desktop"
METAINFO_FILE = ROOT / "packaging/org.madhab.habittracker.metainfo.xml"
CARGO_FILE = ROOT / "Cargo.toml"
MAIN_FILE = ROOT / "src/main.rs"
ICON_DIR = ROOT / "packaging/icons/hicolor/scalable/apps"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(f"desktop metadata validation failed: {message}")


def main() -> int:
    cargo = tomllib.loads(CARGO_FILE.read_text(encoding="utf-8"))
    source = MAIN_FILE.read_text(encoding="utf-8")
    match = re.search(r'const APP_ID: &str = "([^"]+)";', source)
    require(match is not None, "could not find GTK APP_ID in src/main.rs")
    app_id = match.group(1)

    desktop = configparser.ConfigParser(interpolation=None, strict=True)
    desktop.optionxform = str
    desktop.read_string(DESKTOP_FILE.read_text(encoding="utf-8"))
    require("Desktop Entry" in desktop, "desktop launcher has no [Desktop Entry] group")
    entry = desktop["Desktop Entry"]

    metainfo = ET.parse(METAINFO_FILE).getroot()
    metadata_id = metainfo.findtext("id", default="").strip()
    metadata_name = metainfo.findtext("name", default="").strip()
    metadata_summary = metainfo.findtext("summary", default="").strip()
    metadata_license = metainfo.findtext("metadata_license", default="").strip()
    launchable = metainfo.find("./launchable[@type='desktop-id']")
    stock_icon = metainfo.find("./icon[@type='stock']")
    binary = metainfo.findtext("./provides/binary", default="").strip()
    release = metainfo.find("./releases/release")

    require(DESKTOP_FILE.name == f"{app_id}.desktop", "desktop filename does not match GTK APP_ID")
    require(metadata_id == app_id, "AppStream component ID does not match GTK APP_ID")
    require(metadata_name == entry.get("Name", ""), "desktop and AppStream names differ")
    require(bool(metadata_summary), "AppStream summary is empty")
    require(metadata_license == "CC0-1.0", "AppStream metadata license must remain explicit")
    require(launchable is not None, "AppStream desktop-id launchable is missing")
    require(launchable.text == DESKTOP_FILE.name, "AppStream launchable does not match desktop filename")
    require(stock_icon is not None, "AppStream stock icon reference is missing")

    icon_name = entry.get("Icon", "")
    require(bool(icon_name), "desktop launcher has no Icon value")
    require(stock_icon.text == icon_name, "desktop and AppStream icon names differ")
    icon_path = ICON_DIR / f"{icon_name}.svg"
    require(icon_path.is_file(), f"referenced hicolor SVG icon is missing: {icon_path}")
    svg = ET.parse(icon_path).getroot()
    require(svg.tag == "{http://www.w3.org/2000/svg}svg", "referenced icon is not an SVG document")
    require(svg.attrib.get("viewBox") == "0 0 512 512", "icon viewBox must be 0 0 512 512")

    executable = shlex.split(entry.get("Exec", ""))
    require(bool(executable), "desktop Exec command is empty")
    require(binary == executable[0], "desktop Exec and AppStream provided binary differ")
    declared_bins = {item["name"] for item in cargo.get("bin", [])}
    require(binary in declared_bins, f"desktop executable {binary!r} is not declared in Cargo.toml")

    require(release is not None, "AppStream release entry is missing")
    require(release.get("version") == cargo["package"]["version"],
            "AppStream development version does not match Cargo.toml")

    print(
        "Desktop metadata references are consistent: "
        f"{app_id}, {binary}, {icon_name}, version {release.get('version')}"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
