# M7 — Linux desktop launch smoke test

## What this validates

The GitHub Actions desktop smoke job builds the native GTK application, launches it under a virtual X11 display, waits for the expected main window, and checks that startup creates a valid SQLite database under a temporary `XDG_DATA_HOME`.

The test verifies:

- The application process stays alive long enough to present its main window.
- GTK can initialize and create the expected `Habit Trackers by Madhab` window.
- Application data is written beneath the test's temporary XDG data directory, not the runner's real home directory.
- The created SQLite database passes `PRAGMA integrity_check`.

## Run locally on Linux Mint

Install the GTK development files and smoke-test utilities:

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev xvfb xauth xdotool dbus-daemon sqlite3
```

Then build and run:

```bash
cargo build --bin madhab-habit-tracker
bash scripts/smoke-linux-desktop.sh
```

The script creates a temporary data directory and deletes it after the test. It does not use or modify the user's normal habit database.

## What this does not prove

A virtual display is a CI smoke test, not a substitute for testing on the supported physical Linux Mint workstation. It does not validate installer integration, real Wayland/compositor behavior, screen-reader accessibility, keyboard navigation, upgrade/uninstall behavior, or actual user workflows. Issue #2 remains the place to record the target-machine validation.

The smoke job uses a read-only GitHub token and does not upload application data or test database artifacts.
