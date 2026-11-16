# Branch protection setup (P0)

This setup protects only:

- `main`
- `work/linux-mint-habit-tracker-foundation`

It does not change feature branches, GitHub Actions secrets, repository visibility, or merge settings.

## Preview first

Requirements: GitHub CLI (`gh`), `jq`, and a GitHub account with repository administration permission.

From a local clone:

```bash
gh auth login
bash scripts/apply-branch-protection.sh
```

The default is read-only. It checks the current settings and previews the intended policy. If either branch already has a different protection rule, it stops rather than overwriting that rule.

## Apply after reviewing

```bash
bash scripts/apply-branch-protection.sh --apply
```

The apply mode requires an interactive terminal and the exact confirmation `APPLY`. It re-checks both branches before writing, applies only to branches that are currently unprotected, and reads the settings back to verify the result. It is safe to re-run: branches already matching the policy are left unchanged.

The script uses the GitHub REST branch-protection endpoint. The authenticated token must have repository **Administration: write** permission. A 403 means the current token or GitHub App cannot administer branch settings; it does not mean protection was applied.

## Policy

| Control | Setting |
|---|---|
| Pull request required | Yes |
| Required approving reviews | 0 (single-owner repository) |
| Stale approvals dismissed | Yes |
| Required checks | `rust-check`, `schema-tests`, `audit-script`, `Backup, restore and CSV export` |
| Branch must be up to date | Yes |
| Enforce for administrators | Yes |
| Force pushes | Blocked |
| Branch deletion | Blocked |
| Conversation resolution | Required |
| Push restrictions | None; feature development remains possible |

The check names are the job contexts in the repository's current workflows. Update the script and this table if those job names change.

## Important limitations

- This repository change only adds the guarded setup script and instructions. It does **not** apply settings when committed, during CI, or when a PR is opened.
- The repository owner must run the script with an authorized GitHub CLI session and verify the resulting branch settings.
- The script deliberately refuses to replace an existing non-matching protection policy. Review and reconcile such a policy manually.
- After applying, verify that a failing or pending required check blocks a test PR into both target branches.

Official API reference: [GitHub REST API — Update branch protection](https://docs.github.com/en/rest/branches/branch-protection?apiVersion=2026-03-10).
