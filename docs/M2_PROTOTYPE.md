# M2 starter implementation — prototype scope

## Included in this slice
- Native GTK 4 application window with a minimal Today view.
- Add a basic daily binary habit.
- Mark today's habit complete and undo that action.
- Local SQLite file under `$XDG_DATA_HOME/madhab-habit-tracker/` or `$HOME/.local/share/madhab-habit-tracker/`.
- Transactional initial schema version and foreign-key enforcement.
- SQLite constraints, indexes, and a dependency-free Python schema test suite.

## Deliberately not implemented yet
- Bengali/English gettext integration.
- Numeric, duration, limit, frequency-goal forms and flexible schedule evaluation.
- Habit edit/archive screens, calendar, analytics, reminders, backup/restore, or packaging.
- IANA time-zone database integration. The prototype uses SQLite's system-local date for its Today screen; travel/time-zone-sensitive scheduling must be implemented and tested before release.

## Validation boundary
Run `python3 -m unittest discover -s tests -p 'test_*.py' -v` for schema checks.
Run `cargo fmt --all -- --check`, `cargo check --all-targets`, `cargo test --all-targets`, and `cargo clippy --all-targets -- -D warnings` on Linux Mint with the pinned Rust toolchain and GTK 4 development libraries.

GitHub Actions has passed formatting and compilation on Ubuntu 24.04; 7 SQLite tests and shell syntax validation passed. Rust tests and Clippy still need their final CI result, and application launch still needs to be checked on the target Linux Mint workstation.
