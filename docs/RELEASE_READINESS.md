# Release Readiness and Quality Gates

**Status:** planning contract; no release gate is claimed as passed by this document  
**Baseline reviewed:** 2026-11-17  
**Target:** first stable Linux Mint desktop release

This document turns the remaining release blockers into testable engineering work. It is intentionally separate from the active M4 scoring, M5 calendar/goals/routines, M6 localization/data-management and M7 virtual-desktop smoke-test branches. It does not replace those implementation PRs, authorize a merge, select a software license, or claim that the target Linux Mint workstation has been tested.

## 1. Current baseline and release decision

The feature branch is a native Rust + GTK 4 + SQLite application. Its core product boundary is local-first/offline-first: no account, telemetry, cloud sync, ads or remote AI is required. The current repository already has automated formatting, build, Rust test, Clippy, SQLite schema, shell syntax, data-portability and Xvfb startup checks.

The following are distinct evidence types and must not be conflated:

- **Hosted CI:** proves the exact tested commit passed the jobs that ran.
- **Virtual desktop smoke test:** proves the app can start in the CI X server and create a valid isolated database.
- **Target Linux Mint acceptance:** proves installation, launch, input, rendering, persistence, accessibility and upgrade behaviour on the supported real desktop.
- **Release approval:** requires all release gates below, a selected license and an owner-approved release candidate.

Keep PR #1 and its stacked feature PRs as drafts until their latest heads and target-machine validation are reviewed. The main branch must remain unchanged until that review is complete.

## 2. Gate A — stable local-date and timezone semantics

### A1. Keep date-only habit rules separate from instants

A habit schedule such as “every weekday” is a civil-calendar rule, not a fixed 24-hour UTC timer. Continue to evaluate date-only recurrence using a validated civil date and an explicit IANA time-zone identifier. Do not calculate a recurring local day by adding 86,400 seconds to a UTC instant.

Persist the zone identifier (for example, Asia/Kolkata), not only the current UTC offset. Offsets can change when governments revise civil-time rules. The IANA Time Zone Database records historical and changing local-time rules and is updated periodically.

### A2. Make the zone-change behaviour explicit

- Default new installations to the operating-system's current IANA zone when it can be identified.
- Show the active zone in settings and provide an explicit user override.
- If the OS zone changes, do not silently re-date saved check-ins or rewrite historical schedule periods. Ask whether the user wants the new zone to apply from a chosen effective local date.
- Preserve saved local_date values as historical facts. Recalculate only future schedule evaluation under the new effective zone.
- Store the timezone database version used for timed reminder expansion when reproducibility is needed; update future occurrences after tzdb updates, never historical check-in dates.

### A3. Treat DST gaps and folds as real domain cases

A wall-clock reminder time can map to:
- one instant (Single);
- two instants when clocks move backwards (Ambiguous); or
- no instant when clocks move forwards (None).

Do not use unwrap() or silently pick an offset for these cases. Before timed reminders are implemented, choose and document a user-visible policy for ambiguous and nonexistent times. A suitable UI must explain the choice and let the user preview the next occurrence. Habit check-in dates remain date-only and must not be shifted by a DST transition.

### A4. Schedule history must be effective-dated

Schedule edits must not retroactively reinterpret history. Before the first stable release, introduce an immutable schedule-revision history (or an equivalent audited representation) with:

- a per-habit revision identifier;
- an inclusive effective local date and a clearly defined exclusive end boundary;
- the recurrence rule and relevant parameters for that revision;
- the IANA zone governing that revision;
- creation metadata sufficient to audit the change.

When a user edits a schedule, default the revision's effective date to today or a future date. If the user requests a past effective date, show the affected historical period and require explicit confirmation. Never overwrite old revision rows. The scoring replay must select the revision that was effective for each historical period and must retain the scoring-policy version independently from the schedule revision.

The migration must be additive and transactional. Backfill one initial revision from each existing habit's stored schedule and start date; keep the original data until the new representation has been verified. Run integrity, foreign-key and before/after record-count checks. Test migration idempotency and backup/restore across the migration.

### A5. Required deterministic timezone test matrix

| Scenario | Expected result |
|---|---|
| Asia/Kolkata (no seasonal clock change) | Local date is stable across UTC midnight; no assumed DST |
| America/New_York spring-forward gap | A nonexistent reminder time is detected and handled by the chosen policy |
| America/New_York fall-back fold | Two possible instants are detected; policy is explicit and deterministic |
| Zone changed while the app is closed | Past check-ins keep their saved local dates; new zone applies only from its effective date |
| Week spanning a year boundary | Weekly-frequency period uses the documented ISO-week convention |
| Leap day and skipped civil dates | Date validation and migration/replay remain deterministic |
| tzdb rule update | Future timed reminders are recomputed; saved historical records are not rewritten |

Use a maintained IANA-aware library or system tzdb integration. If using Chrono, handle MappedLocalTime/LocalResult explicitly; Chrono documents the single, ambiguous and nonexistent cases. Pin dependency versions through the committed lockfile and rerun the matrix after timezone-data updates.

## 3. Gate B — accessible GTK interaction

GTK's standard controls expose accessibility interfaces, but an application still has to provide useful names, labels, descriptions and correct semantics. Any custom widget must implement its accessibility contract rather than merely claiming a role.

Required manual acceptance on the target Linux Mint desktop:

- Complete the main flows with keyboard only: add, edit, complete, undo, skip, pause/resume, archive/restore, open goals, routine workflows, calendar and backup/restore.
- Verify visible focus, logical tab order, no keyboard traps, predictable Escape/Enter behaviour and sensible focus restoration after dialogs close.
- Run Orca and confirm each control announces its purpose, state and value; dynamic status changes are announced where appropriate.
- Check GTK Inspector's accessibility view for missing names, invalid roles and broken label relations.
- Test Bengali and English strings at larger text sizes; labels must not clip, overlap or hide critical status.
- Verify colour is not the only way to distinguish complete, skipped, paused, partial or error states.
- Test at the supported display scaling and a narrow window size.

Automated checks can verify the presence of accessible metadata and exercise deterministic widget state, but they do not replace a real Orca/keyboard session.

## 4. Gate C — packaging, install, upgrade and uninstall

Choose the first distribution format only after a successful target-machine build. Keep the packaging definition reproducible and source-based.

Minimum packaging deliverables:

- a stable application ID and .desktop launcher following the freedesktop Desktop Entry Specification;
- AppStream metainfo with name, summary, description, developer/project URL, supported desktop category, release notes, icon and screenshots;
- versioned package metadata and an auditable build recipe;
- icons at the required sizes and a clear uninstall path;
- runtime dependency declarations, including GTK 4 and the SQLite strategy;
- an installation guide that does not require running the app as root.

If Flatpak is selected, request only necessary sandbox permissions. Prefer XDG portals for user-selected files and notifications; do not grant broad home-directory or network access merely for convenience. The app is designed to work offline.

Test from clean Linux Mint snapshots:
1. fresh install and launch from the desktop menu;
2. create data, close and reopen the app;
3. upgrade from the previous candidate and verify the database migration;
4. interrupt an upgrade and verify recoverability;
5. uninstall and confirm the user's database and backups are preserved unless the user explicitly chooses data removal;
6. reinstall and reopen the retained database;
7. verify the package does not overwrite an existing user backup or export.

The choice between a native Debian package and Flatpak remains an owner/product decision; this document does not choose it.

## 5. Gate D — reproducible builds and dependency hygiene

The current foundation tree has no committed Cargo.lock, while the product is a binary desktop application. Add the lockfile generated by Cargo, review its dependency changes, and make CI use locked dependency resolution (cargo check --locked, cargo test --locked, and cargo clippy --locked). Do not hand-edit the lockfile. This prevents CI and release builds from silently resolving a different transitive dependency graph.

Before release:
- review RustSec advisories for the resolved dependency graph;
- record the license and source of each direct and transitive dependency;
- review pinned GitHub Actions by immutable commit SHA and keep persist-credentials: false unless a job demonstrably needs write access;
- generate and retain an SBOM for each release candidate;
- publish build provenance and checksums for distributable artifacts;
- never place user database files, private habit labels or real notes in CI artifacts.

The package manifest currently declares license = "UNLICENSED" and the repository has no selected project license. Public visibility is not a license grant. The repository owner must choose and approve a license before accepting external contributions or presenting the project as open source. Do not silently substitute MIT, Apache-2.0 or GPL.

## 6. Gate E — local data recovery and privacy

- Test backup → verify → restore → open the restored copy with an older supported schema and with the release candidate.
- Verify the active application database is never overwritten by restore.
- Verify backup/export permissions are private to the current user on Unix-like systems.
- Verify malformed, truncated, read-only and low-disk-space destinations fail safely without damaging the original database.
- Verify schema migration failure leaves the prior database recoverable.
- Keep diagnostics free of habit names, notes, check-in content and absolute home paths by default.
- Keep all export, restore and deletion actions user-initiated; communicate which files are created.

## 7. Final release checklist

| Gate | Required evidence | Status at baseline review |
|---|---|---|
| Feature PR integration | Exact latest-head CI and reviewed diffs | Pending review; drafts remain open |
| Timezone and schedule history | Effective-dated model and DST test matrix | Not implemented |
| Accessibility | Keyboard + Orca + GTK Inspector on target desktop | Manual validation pending |
| Packaging | Install/upgrade/uninstall evidence on Linux Mint | Not implemented |
| Reproducible Rust build | Committed Cargo.lock, locked CI and dependency review | Lockfile absent |
| License | Owner-approved license decision and matching repository metadata | Owner decision required |
| Data recovery | Migration + backup/restore + failure-injection evidence | Partial CLI/UI work exists; release matrix pending |
| Target machine | Issue #2 audit, build, launch and persistence evidence | Pending user workstation |
| Release candidate | Versioned artifacts, checksums, SBOM, notes and rollback plan | Not started |

A release candidate is **not ready** while any required gate is pending. Do not infer readiness from a green hosted CI badge alone.

## 8. References

- [IANA Time Zone Database](https://www.iana.org/time-zones) — authoritative timezone-rule source and update history.
- [Chrono: MappedLocalTime](https://docs.rs/chrono/latest/chrono/offset/type.MappedLocalTime.html) and [chrono-tz: Tz](https://docs.rs/chrono-tz/latest/chrono_tz/enum.Tz.html) — handling of IANA zones and ambiguous/nonexistent local times.
- [GTK 4 Accessibility](https://docs.gtk.org/gtk4/section-accessibility.html) — accessible roles, properties, labels and GTK Inspector guidance.
- [Freedesktop Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry/latest-single/) — launcher metadata.
- [AppStream desktop application metadata](https://www.freedesktop.org/software/appstream/docs/sect-Metadata-Application.html) — software-center metadata for desktop applications.
- [Flatpak sandbox permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html) and [Flathub requirements](https://docs.flathub.org/docs/for-app-authors/requirements) — least-privilege permissions and portal guidance.
- [Cargo.toml vs Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html) and [Cargo build --locked](https://doc.rust-lang.org/cargo/commands/cargo-build.html) — exact dependency resolution for repeatable builds.
- [Draft PR #1](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/pull/1) and [Issue #2](https://github.com/Madhab7216/Habit_-Trackee_By_Madhab/issues/2) — current foundation and target-workstation acceptance tracking.
