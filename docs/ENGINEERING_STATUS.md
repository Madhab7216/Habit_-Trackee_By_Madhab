# Habit Trackers by Madhab — Engineering Status

Audit date: 2026-11-14 (latest repository review)

## Live GitHub state

- Repository: `Madhab7216/Habit_-Trackee_By_Madhab`
- Default branch: `main` (the original README-only baseline remains unchanged).
- Active implementation branch: `work/linux-mint-habit-tracker-foundation`
- Current feature-branch head at this review: `4abc792c26c5379bf072c83ec420fcaee372d308`
- Pull request: [#1 — M0–M2 foundation: native GTK4 habit tracker starter](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1) — open, draft, not merged.
- Target-machine validation issue: [#2 — validate the GTK/Rust build on the target Linux Mint workstation](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/2) — open.
- PR #1 currently contains 24 commits across 14 files. Keep it in draft until CI is green and Linux Mint validation is recorded.

## Implemented in the feature branch

- Rust + GTK 4 native Linux desktop starter.
- Local SQLite database under XDG data directory / user data directory.
- Versioned initial schema with constraints, indexes, and foreign-key enforcement.
- Pure Rust domain module for civil dates, daily/weekday/every-N-days/weekly schedules, adherence, EMA habit-strength estimate, and streak rules.
- Basic Today screen with add daily binary habit and complete/undo.
- Python SQLite schema tests and GitHub Actions workflow.
- Read-only Linux Mint environment audit script.
- Product blueprint, scope freeze, prototype boundary, and roadmap.

## Additional code-review fixes committed during this review

1. Weekly targets are now constrained to 1–7 completions per ISO week. A target of 8 or higher is rejected; regression coverage was added.
2. Data-directory resolution now accepts only absolute `XDG_DATA_HOME` / `HOME` paths. If neither is usable, startup fails with an explicit storage error rather than writing private app data into the current working directory. Three path-resolution regression tests were added.

The two code changes were committed separately to the feature branch; neither modifies `main`.

## CI evidence and current gate

- Earlier workflow run #7 (push) and run #9 (pull request) completed successfully on earlier feature-branch commits.
- Some intervening runs were cancelled because newer commits superseded them. One earlier run failed at `cargo fmt --all -- --check`; a later run passed all three workflow jobs.
- Latest observed run #16 for head `4abc792c`: SQLite schema tests and audit-script syntax checks passed; Rust format/build/test/Clippy job was still in progress at the time of this audit.
- Do not describe the current PR as fully CI-green until run #16 or a newer run completes successfully.

## Remaining release blockers

1. Confirm the latest Rust job passes formatting, compile, unit tests, and Clippy.
2. Run the read-only audit and actual build/launch smoke test on the target Linux Mint workstation (Issue #2).
3. Wire the pure schedule/domain rules into the SQLite repository and GTK flows; the current visible UI still creates daily binary habits only.
4. Implement full habit editing, pause/resume, archive/restore, numeric/duration/count/limit tracking, and partial/skipped entry states.
5. Add Bengali/English localization and keyboard/screen-reader accessibility review.
6. Implement 7-day analytics and goals/routine workflows.
7. Add user-initiated CSV export and consistent SQLite backup/restore tests.
8. Finalize package/install/update testing on the supported Linux Mint release.
9. Choose and add an explicit open-source license before inviting outside contributions.

## Product and safety boundaries

- The app is separate from Madhab OS.
- Core tracking remains offline-first and local-only; there is no cloud, telemetry, or remote AI in the MVP.
- Habit descriptions and notes may be sensitive. Do not upload the live SQLite database or private records into issues, PR comments, or diagnostics.
- Keep PR #1 as draft until the stated gates have passed. Do not merge into `main` solely because a hosted schema check succeeded.
