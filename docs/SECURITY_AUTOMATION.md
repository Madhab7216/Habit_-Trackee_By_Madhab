# Security automation

## What is automated

The repository uses two independent CodeQL configurations:

- **Rust** source scanning with the `security-extended` query suite.
- **GitHub Actions** workflow scanning with the same suite.

Both analyses run for pushes to any branch, pull requests, a weekly scheduled scan, and a maintainer-triggered workflow dispatch. Rust uses CodeQL's supported `build-mode: none`, so it does not run the application build script just to create the database. This is a good fit for the current Rust codebase; generated Rust source, if introduced later, must be reviewed and the scanning mode revisited.

The analysis action is pinned to the immutable CodeQL Action v4.39.3 commit. Dependabot tracks the GitHub Actions ecosystem weekly so the SHA pin can be advanced through review rather than silently floating to a new action revision.

Each analysis job has only `contents: read`, `actions: read` and `security-events: write`. Checkout does not persist Git credentials. `pull_request_target` is not used. Fork pull requests are not uploaded to the repository's code-scanning results; their analysis jobs are skipped to avoid granting write permissions to untrusted fork code.

## Dependency update policy

Dependabot checks Cargo dependencies and GitHub Actions every Monday. It groups SemVer minor/patch version updates to reduce PR noise; major updates remain separate for deliberate compatibility review. A seven-day cooldown applies to ordinary version updates so new releases have time to surface regressions. The cooldown does not delay Dependabot security updates.

**Staged branch caveat:** the application currently lives on `work/linux-mint-habit-tracker-foundation`, while the GitHub default branch `main` remains a bootstrap README. The Dependabot entries therefore set `target-branch` to the foundation branch so version updates are useful now. GitHub does not create Dependabot security-update PRs for non-default target branches. Before calling vulnerability auto-remediation complete, the owner must:
1. Enable the dependency graph and Dependabot alerts in repository Settings → Security & analysis.
2. After the foundation is approved and made part of the default branch, remove `target-branch` from the Dependabot entries (or move the project to its intended default branch after review).
3. Confirm that Dependabot security updates are enabled and verify that a test vulnerable dependency produces an alert/update PR in a controlled branch.

Do not interpret a quiet Dependabot dashboard as proof that all dependencies are current or vulnerability-free.

## What this does not prove

- CodeQL alerts are findings requiring triage, not automatic proof of exploitability.
- `build-mode: none` does not analyze generated code and may have less dependency/build context than a manual build.
- CodeQL does not replace Rust tests, Clippy, dependency advisories, manual code review, Linux Mint GUI acceptance, or runtime security tests.
- No code scanning alert is considered resolved merely because the workflow is green.
- The initial workflow run is the first scan of the branch; this PR does not claim that the whole repository is clean.

## Maintainer checks

After merging this PR into the foundation branch:
1. Open Security → Code scanning and inspect Rust and Actions alerts.
2. Triage each finding; document confirmed false positives and create issues for real risks.
3. Check the CodeQL tool status page for a successful analysis of both languages.
4. When the project is promoted to the default branch, revisit the Dependabot target-branch caveat above.
