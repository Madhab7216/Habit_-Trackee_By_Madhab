# Debian package and Linux Mint installation

This is an engineering-preview package recipe for Debian-family Linux distributions. It currently targets **Linux Mint x86_64 (Debian architecture amd64)**. It is not a stable release or a repository-signing/update channel.

## Build prerequisites

On Linux Mint, install the native build and packaging dependencies:

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev dpkg-dev python3
```

Install the repository's pinned Rust toolchain (Rust 1.100.0), then run:

```bash
cargo generate-lockfile
scripts/build-deb.sh
```

The output is written to `target/packages/madhab-habit-tracker_VERSION_amd64.deb`. The package contains the GTK application, the local data CLI, desktop launcher, AppStream metadata, scalable icon and readme. The builder uses Cargo's lockfile in locked mode; while the repository currently has no committed `Cargo.lock`, it generates a local lockfile when absent. Commit and review that generated lockfile before declaring production builds reproducible.

## Install and remove

```bash
sudo apt install ./target/packages/madhab-habit-tracker_0.1.0_amd64.deb
```

The application database remains under the user's XDG data directory (by default `~/.local/share/madhab-habit-tracker/habit-tracker.sqlite3`). Package removal does not delete or migrate that directory. Back up important data before upgrading any preview build.

```bash
sudo apt remove madhab-habit-tracker
```

The CI workflow installs version `0.1.0`, upgrades to `0.1.0+ci1`, then purges the package and verifies that packaged executables are removed while a sentinel user-data file remains untouched.

## Package boundaries

- No maintainer scripts run during install, upgrade or removal.
- No habit database, user note, cache, home-directory file or mutable application state is shipped in the package.
- The package declares GTK 4 and glibc runtime dependencies; SQLite is linked through the application's bundled rusqlite configuration.
- This preview does not yet provide a signed APT repository, automatic updates, Flatpak, ARM64 packages, or a stable-release guarantee.
- The repository has not yet selected an application license. Do not treat a public repository or this preview package as granting redistribution rights.

## Validation

```bash
desktop-file-validate packaging/org.madhab.habittracker.desktop
appstreamcli validate --pedantic packaging/org.madhab.habittracker.metainfo.xml
scripts/build-deb.sh
tests/test-deb-package.sh target/packages/madhab-habit-tracker_0.1.0_amd64.deb
```

The dedicated GitHub Actions workflow also performs a real dpkg install, versioned upgrade and purge on Ubuntu 24.04, while checking that user data remains outside the package lifecycle.
