# Security and Supply-Chain Baseline

Audit date: 17 November 2026

This document records the security boundary for the current Linux Mint engineering preview. It is a baseline and release checklist, not a claim that the application is secure against a compromised operating-system account or a malicious local administrator.

## 1. Data and trust boundary

- Habit records are stored locally in SQLite under the user's application data directory.
- The MVP does not require a remote account, cloud sync, telemetry, advertising SDK, or remote AI service.
- Backups and CSV exports are deliberate user actions. They can contain sensitive habit labels, goal names, dates, and notes.
- The desktop application runs with the current user's operating-system permissions. It cannot protect records from malware or another process already running with equivalent or higher privileges.
- Do not use real personal records in tests, issue attachments, CI artifacts, screenshots, or public pull requests.

## 2. Local-data safeguards

Before v1.0, verify all of the following on a real Linux Mint installation:

- Application data is created only in an absolute, user-owned data directory.
- New database, backup, and export files receive restrictive permissions where the platform supports them.
- Backup verification checks SQLite integrity and foreign-key consistency before restore is offered.
- Restore writes to a new destination and never silently replaces the active database.
- Export and restore reject unsafe paths and existing destinations rather than overwriting.
- Error messages do not print habit names, goal names, notes, SQL parameters, or full database contents.
- A user can understand where data is stored and how to make, verify, restore, and remove their own copies.

Do not describe SQLite files or CSV exports as encrypted unless application-level encryption is deliberately implemented and verified. Users should protect the device account, enable full-disk encryption where appropriate, and keep exported copies in a trusted location.

## 3. Dependency and build provenance

The repository currently pins direct Rust dependencies in Cargo.toml and pins GitHub Actions to full commit SHAs. However, the feature branch does not yet contain a committed Cargo.lock.

The new dependency audit workflow generates a temporary lockfile and checks the resolved graph against the RustSec advisory database. This provides a useful vulnerability signal, but it is not a reproducible record of the exact transitive dependency graph used for a shipped binary.

Before a release candidate:

1. Generate Cargo.lock from the approved source tree using the pinned Rust toolchain.
2. Review and commit the lockfile; do not hand-edit it.
3. Require cargo check --locked, cargo test --locked, cargo clippy --locked -- -D warnings, and cargo audit --locked in release CI.
4. Review every Dependabot update before merging. A green advisory scan does not replace compatibility tests or source review.
5. Keep third-party GitHub Actions pinned to full reviewed commit SHAs and review their permission requirements.
6. Generate a software bill of materials (SBOM) for each release candidate and retain it with release provenance.

Dependabot configuration is read from the repository's default branch. The configuration in this increment targets the active engineering branch and becomes operational after it is merged into the default branch.

The GitHub Actions schedule trigger is also default-branch-only. The push-triggered run on this feature branch validates the new workflow now; the weekly scheduled scan becomes active after the workflow file reaches the default branch. Manual dispatch is likewise best treated as a post-merge operation.

## 4. GitHub workflow security

- Workflows use permissions: contents: read unless a documented job needs more.
- Checkout uses persist-credentials: false so the checkout token is not left in Git configuration.
- Do not expose write-capable tokens to pull-request code from untrusted forks.
- Do not print repository secrets, authentication headers, personal home paths, or application data in workflow logs.
- Scheduled scans and manually triggered scans should remain read-only.
- Review new actions by publisher, repository, immutable commit SHA, and required permissions before adoption.

## 5. Pre-release security review

| Area | Required evidence | Release gate |
|---|---|---|
| Local storage | Real-machine permission and path checks | Must pass |
| Backup/restore | Integrity check and restore-to-new-file test | Must pass |
| Export privacy | Synthetic-data test and output inspection | Must pass |
| Dependency graph | Committed Cargo.lock and clean RustSec audit | Must pass |
| GitHub Actions | Pinned actions and least-privilege permissions | Must pass |
| Network boundary | Confirm normal tracking works offline and emits no telemetry | Must pass |
| Error handling | Logs contain no habit names, notes, or database contents | Must pass |
| Vulnerability intake | Private reporting route or documented private contact | Must pass |
| Licensing | Explicit project license selected by the owner | Must pass before outside contributions |

## 6. Known limitations

- No application-level database encryption is currently documented.
- A compromised user account can read or alter that user's local habit database.
- A temporary lockfile audit is weaker than auditing a committed lockfile.
- No stable release/support window has been declared.
- GitHub private vulnerability reporting must be enabled in repository settings separately; adding SECURITY.md alone does not enable it.
- Real Linux Mint desktop behavior, accessibility, timezone/DST behavior, packaging, and upgrade/rollback still require target-machine validation.

## 7. References

- GitHub security policy guidance: https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/add-security-policy
- GitHub private vulnerability reporting: https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/report-privately
- GitHub Dependabot configuration: https://docs.github.com/en/code-security/concepts/supply-chain-security/about-the-dependabot-yml-file
- RustSec cargo-audit: https://github.com/rustsec/rustsec/tree/main/cargo-audit
- SQLite online backup API: https://sqlite.org/backup.html
