# Linux desktop integration metadata

This increment adds the format-neutral desktop integration files used by Linux Mint and other freedesktop-compatible desktops.

## Files

- packaging/org.madhab.habittracker.desktop: application launcher with the stable GTK application ID, binary command and menu categories.
- packaging/org.madhab.habittracker.metainfo.xml: AppStream metadata for software-center discovery.
- packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg: scalable application icon.
- .github/workflows/desktop-metadata.yml: validates the desktop entry, AppStream metadata and SVG on pushes and pull requests.

## Packaging boundary

These files do not select a distribution format. They can be consumed by a future Debian package, Flatpak manifest or another supported packaging recipe. The final packaging work must install the launcher into the desktop-entry directory, the AppStream file into the metainfo directory and the icon into the hicolor icon theme.

The metadata describes version 0.1.0 as a development preview, not a stable release. No project license is declared here because the repository owner has not yet selected one. The CC0-1.0 value applies only to this AppStream metadata document.

## Validation

Run the same validators locally on a Linux workstation:

```bash
sudo apt install appstream desktop-file-utils
desktop-file-validate packaging/org.madhab.habittracker.desktop
appstreamcli validate --pedantic packaging/org.madhab.habittracker.metainfo.xml
```

The workflow validates the metadata files only. It does not build an installable package or prove installation, upgrade, uninstall or database-preservation behavior. Those remain release gates in docs/RELEASE_READINESS.md.
