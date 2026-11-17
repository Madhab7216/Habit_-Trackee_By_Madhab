# Linux desktop integration metadata

This increment adds the format-neutral desktop integration files used by Linux Mint and other freedesktop-compatible desktops.

## Files

- packaging/org.madhab.habittracker.desktop: application launcher with the stable GTK application ID, binary command and menu categories.
- packaging/org.madhab.habittracker.metainfo.xml: AppStream metadata for software-center discovery.
- packaging/icons/hicolor/scalable/apps/org.madhab.habittracker.svg: scalable application icon.
- .github/workflows/desktop-metadata.yml: validates the desktop entry, AppStream metadata and SVG on pushes and pull requests.

## Packaging

The Debian-family package recipe is implemented in `scripts/build-deb.sh` and currently targets Linux Mint x86_64 (amd64). It installs:

- the GTK executable and local data CLI into `/usr/bin`;
- the desktop launcher into `/usr/share/applications`;
- AppStream metainfo into `/usr/share/metainfo`;
- the scalable icon into the hicolor icon theme;
- project documentation into `/usr/share/doc/madhab-habit-tracker`.

See `docs/DEBIAN_PACKAGE.md` for prerequisites, build and installation commands, architecture limits and the current preview boundaries. The dedicated Debian package workflow tests package contents plus a real install, versioned upgrade and purge. It also checks that package removal does not delete user data.

The repository still needs a committed `Cargo.lock` and an explicit application license before a reproducible, redistributable stable package can be claimed. This preview does not yet provide a signed APT repository or automatic updates.

## Validation

Run the same validators locally on a Linux workstation:

```bash
sudo apt install appstream desktop-file-utils
desktop-file-validate packaging/org.madhab.habittracker.desktop
appstreamcli validate --pedantic packaging/org.madhab.habittracker.metainfo.xml
```

The desktop-metadata workflow validates the desktop entry, AppStream metadata and SVG. The Debian package workflow separately builds the installable package and tests install, upgrade, purge and user-data preservation. Target Linux Mint workstation validation remains a release gate.
