# Supply-chain security controls

This document describes the repository-level controls added for the engineering preview. They reduce risk; they do not constitute a third-party security audit or a guarantee that the application is vulnerability-free.

## Automated controls

- **CodeQL:** scans Rust source on pushes to the implementation/security branches, pull requests targeting the implementation branch, and manual workflow dispatch. It uses Rust's supported `none` build mode, least-privilege token permissions, and SHA-pinned GitHub Actions.
- **RustSec:** resolves the current Cargo dependency graph and checks it against the RustSec advisory database using `cargo-audit`.
- **Dependabot:** checks Cargo dependencies and GitHub Actions weekly. Minor and patch updates are grouped to reduce review noise; major updates remain separate.
- **Reporting:** `SECURITY.md` tells reporters to use private vulnerability reporting and not expose personal habit data.

## Cargo lockfile limitation

The current implementation branch does not contain a committed `Cargo.lock`. The dependency-audit workflow therefore creates a temporary lockfile on the runner before scanning.

This checks the dependency versions resolved at that workflow run, but it does **not** make builds reproducible and does not guarantee that two builds on different dates use identical transitive dependencies. Before a release candidate:

1. Generate and review `Cargo.lock` using the pinned Rust/Cargo toolchain.
2. Commit the reviewed lockfile because this repository builds application binaries.
3. Change engineering and audit commands to use `--locked` so CI fails if dependency resolution would modify the lockfile.
4. Re-run the audit and review every advisory; do not suppress an advisory without a documented, time-bounded rationale.

## Operational review

- Review Dependabot PRs; do not auto-merge dependency updates.
- Investigate CodeQL and RustSec findings before merging feature work.
- Keep workflow permissions read-only unless a specific job needs a narrowly scoped write permission.
- Review the exact source and full commit SHA for every third-party Action.
- Never put user databases, exported habit data, personal labels, or notes into logs or uploaded artifacts.
