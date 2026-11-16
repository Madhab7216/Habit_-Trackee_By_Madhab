# GTK Accessibility Smoke Test

This test complements the Linux desktop startup check. The existing startup test explicitly disables the AT-SPI bridge because it only needs to verify that the GTK window appears and that the isolated SQLite database is valid. This separate workflow enables GTK's AT-SPI backend and inspects the accessible tree that a screen reader consumes.

## Automated coverage

The test launches the actual GTK application under Xvfb and a private D-Bus session with `GTK_A11Y=atspi`. It then uses the GObject-introspection bindings for AT-SPI to inspect the live application window.

The smoke test verifies that:
- the main window is exposed to the accessibility bus;
- the primary Today actions have their expected accessible names;
- no exposed push button is unnamed;
- the Habit name entry is labelled by its visible label;
- the schedule selector and weekly target spinner expose their associated labels.

The test creates its database under a temporary `XDG_DATA_HOME`, never uses personal habit records, and deletes the temporary directory on exit. It does not run the application as root, use secrets, or upload user data.

## What this test does not prove

A passing AT-SPI snapshot is not a full accessibility certification. It does not validate:
- Orca speech quality, pronunciation, verbosity or announcements;
- keyboard-only task completion, focus order, focus restoration or keyboard traps;
- contrast, colour-only status communication, zoom or large-text clipping;
- Bengali font shaping, translation completeness or visual layout;
- every dialog and every screen added by separate feature branches.

The Today form uses GTK labels with explicit mnemonic targets. GTK automatically updates the target control's `LabelledBy` accessibility relation when a label is associated with its mnemonic widget. The labels also provide keyboard accelerators (Alt+H, Alt+S and Alt+W). The same association is applied in the Edit habit dialog. The CI assertion checks the field names and expected AT-SPI roles; it does not replace checking their announcement order or pronunciation with Orca.

Those behaviours still need manual validation on the target Linux Mint workstation.

## Local run

Install the dependencies on Linux Mint (package names can vary by Mint/Ubuntu base):

```bash
sudo apt update
sudo apt install build-essential pkg-config libgtk-4-dev xvfb xauth dbus-daemon at-spi2-core gir1.2-atspi-2.0 python3-gi
cargo build --bin madhab-habit-tracker
bash scripts/smoke-linux-accessibility.sh
```

## Research basis

- GTK selects its accessibility backend through `GTK_A11Y`; `atspi` enables the Linux accessibility bridge, while `test` is GTK's backend intended for test suites and remote CI.
- GTK's `GtkAccessible` exposes roles, properties, relations and states to assistive technologies. GTK's own accessibility guidance recommends explicitly specifying accessible properties and using the GTK Inspector during development.
- AT-SPI is a D-Bus accessibility interface that exposes accessible object names, roles, states and actions. The libatspi GObject-introspection API is preferred for new clients over pyatspi's legacy wrapper.

References:
- [GTK accessibility](https://docs.gtk.org/gtk4/section-accessibility.html)
- [GTK runtime and GTK_A11Y](https://docs.gtk.org/gtk4/running.html)
- [AT-SPI accessibility infrastructure](https://ubuntu.com/desktop/docs/en/24.04/explanation/accessibility-stack/)
- [AT-SPI GObject-introspection API](https://lazka.github.io/pgi-docs/Atspi-2.0/)
- [pyatspi project note recommending GObject introspection for new clients](https://github.com/GNOME/pyatspi2)
