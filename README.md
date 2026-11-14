# Habit Trackers by Madhab

A privacy-first, offline-first habit and personal-goal tracker for Linux Mint. The intended product is a polished native desktop application with Bengali and English interfaces, flexible habit schedules, measurable goals, transparent progress scoring, and user-owned data.

> **Status (14 November 2026):** initial engineering foundation and M2 starter slice are on the feature branch. This is not yet a finished app or a verified Linux Mint release.

## Product decisions
- **Platform:** Linux Mint desktop first.
- **Design:** hybrid native desktop UI with a modern dashboard.
- **Languages:** Bengali and English (localization work remains).
- **Scoring:** hybrid; adherence, habit-strength estimate, and streak shown separately.
- **Storage:** local-first SQLite; no cloud account or network dependency for core tracking.
- **Privacy:** no telemetry or remote AI in the MVP.

## Current M2 prototype
The first GTK 4 + Rust slice includes:
- A native Today screen.
- Add a daily binary habit.
- Mark today's habit complete and undo it.
- Persist data to SQLite under the user's local data directory.
- A constrained initial schema with version tracking.
- A dependency-free Python SQLite schema test suite.

It is deliberately a narrow starter. Numeric habits, flexible schedule evaluation, full editing and archive flows, analytics, backup/restore, Bengali/English localization, accessibility review, and packaging remain future milestones.

## Build on Linux Mint
Install the GTK 4 development prerequisites and Rust toolchain required by rust-toolchain.toml. Then run:

~~~bash
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo run
~~~

Run the read-only environment audit:

~~~bash
bash scripts/audit-linux-mint.sh | tee madhab-habit-environment-audit.txt
~~~

The audit script does not install packages or change system settings. Review its output before sharing it.

## Tests
~~~bash
python3 -m unittest discover -s tests -p 'test_*.py' -v
bash -n scripts/audit-linux-mint.sh
~~~

## Engineering documentation
- [Engineering status and scope freeze](docs/ENGINEERING_STATUS.md)
- [Engineering blueprint](docs/ENGINEERING_BLUEPRINT.md)
- [M2 prototype scope and validation boundary](docs/M2_PROTOTYPE.md)

## License
No license has been selected. Do not assume public visibility means permission to reuse or redistribute code. A license decision is required before inviting external contributions.
