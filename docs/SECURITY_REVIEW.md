# Security review and dependency-audit baseline

Audit date: 2026-11-17

## Review scope

This is a repository-level review of the current integration foundation and its engineering workflow. It is not a penetration test, source-code security certification, or desktop threat-model sign-off.

Reviewed baseline: `work/linux-mint-habit-tracker-foundation` at `d8e17b60c39908451088f375ba3a553e057c7faf`.

## Controls observed

- Essential tracking is designed to work locally without an account, cloud sync, telemetry, ads, or remote AI.
- Habit data is stored in SQLite under the user's data directory.
- Backup, restore, and export are explicit local operations; restore writes a separate destination rather than replacing the active database.
- Existing CI workflows declare read-only `contents: read` permissions and pin the checkout and Rust toolchain actions to full commit SHAs.
- The application documentation warns against publishing live databases, habit labels, notes, or exported records.

These controls reduce exposure, but do not prove that every code path or operating-system integration is safe.

## Added in this security increment

### Rust dependency advisory scan

`.github/workflows/dependency-security-audit.yml` runs RustSec `cargo-audit` 0.23.0 on pull requests to the integration branches, pushes to this work branch, manual dispatch, and a weekly schedule.

The workflow:
- Uses the repository's pinned Rust 1.100.0 toolchain.
- Pins GitHub Actions to full commit SHAs.
- Grants only read access to repository contents.
- Does not receive deployment secrets or write repository contents.
- Audits the Rust dependency graph against the RustSec advisory database.
- Does not automatically upgrade dependencies or suppress advisories.

The weekly schedule becomes active only after this workflow is present on the repository's default branch. Pull-request and manual runs can validate it earlier.

### Vulnerability disclosure

The root `SECURITY.md` describes private reporting, the project scope, and safe reproduction practices. It specifically asks reporters not to attach real habit data, backups, CSV exports, credentials, or private home-directory information.

## Important release limitation: no committed Cargo.lock

The reviewed foundation tree does not contain a committed `Cargo.lock`. When the audit runs without one, Cargo/RustSec resolves a dependency graph for that run. This can identify known advisories in the resolved graph, but it does **not** make builds reproducible and different runs can resolve different transitive versions.

Before a v1.0 release:
1. Generate and review `Cargo.lock` using the pinned project toolchain.
2. Commit it for this binary application.
3. Change the audit command to `cargo audit --locked` so CI checks the committed graph rather than silently resolving a new one.
4. Run the full build, test, Clippy, and advisory checks against that exact lockfile.

This increment deliberately does not fabricate a lockfile or claim reproducible dependency resolution without running Cargo against the project.

## Coverage gaps still requiring work

- RustSec checks Rust crate advisories; it does not replace review of GTK/GLib system libraries supplied by the host or Flatpak runtime.
- No source-level static analysis, fuzzing campaign, penetration test, or independent security review has been completed here.
- GitHub private vulnerability reporting and branch-protection/ruleset settings must be verified in repository settings; this workflow cannot enable those settings.
- Release packaging, install/update/uninstall tests, and desktop permission review remain outstanding.
- No open-source licence has been selected. That is an owner/legal decision and must not be inferred from repository visibility or dependency licences.

## Advisory handling

Do not add an advisory ignore merely to make CI green. For any exception, record the advisory ID, affected dependency/version, reachability or platform rationale, mitigation, owner, and review date. Prefer upgrading or removing the affected dependency.
