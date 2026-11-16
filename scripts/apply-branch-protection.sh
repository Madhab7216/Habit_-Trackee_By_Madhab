#!/usr/bin/env bash
# Apply only the repository's two intended branch-protection rules.
# Default mode is read-only preview. A terminal confirmation is required for --apply.
set -Eeuo pipefail

readonly REPO="Madhab7216/Habit_-Trackee_By_Madhab"
readonly API_VERSION="2026-03-10"
readonly BRANCH_MAIN="main"
readonly BRANCH_FOUNDATION="work/linux-mint-habit-tracker-foundation"
readonly REQUIRED_CHECKS='["rust-check","schema-tests","audit-script","Backup, restore and CSV export"]'

apply=0
if [[ "$#" -gt 1 ]]; then
  printf 'Usage: %s [--apply]\n' "$0" >&2
  exit 2
fi
if [[ "$#" -eq 1 ]]; then
  if [[ "$1" != "--apply" ]]; then
    printf 'Unknown option: %s\nUsage: %s [--apply]\n' "$1" "$0" >&2
    exit 2
  fi
  apply=1
fi

for command_name in gh jq; do
  if ! command -v "$command_name" >/dev/null 2>&1; then
    printf 'Required command not found: %s\n' "$command_name" >&2
    exit 1
  fi
done

if ! gh auth status >/dev/null 2>&1; then
  printf 'Authenticate first with: gh auth login\n' >&2
  exit 1
fi
if ! gh repo view "$REPO" --json nameWithOwner >/dev/null 2>&1; then
  printf 'Cannot access %s with the current GitHub CLI account.\n' "$REPO" >&2
  exit 1
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT
payload_file="$tmp_dir/protection.json"

cat >"$payload_file" <<'JSON'
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "rust-check",
      "schema-tests",
      "audit-script",
      "Backup, restore and CSV export"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": {
    "dismiss_stale_reviews": true,
    "require_code_owner_reviews": false,
    "required_approving_review_count": 0,
    "require_last_push_approval": false
  },
  "restrictions": null,
  "required_linear_history": false,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": false
}
JSON

declare -A branch_state
for branch in "$BRANCH_MAIN" "$BRANCH_FOUNDATION"; do
  encoded_branch="$(jq -rn --arg branch "$branch" '$branch | @uri')"
  endpoint="repos/$REPO/branches/$encoded_branch/protection"

  if gh api "$endpoint" >"$tmp_dir/current.json" 2>"$tmp_dir/error.log"; then
    if jq -e --argjson required_checks "$REQUIRED_CHECKS" '
      .required_status_checks.strict == true
      and ((.required_status_checks.contexts // []) | sort) == ($required_checks | sort)
      and (.enforce_admins.enabled // false) == true
      and (.required_pull_request_reviews.required_approving_review_count // -1) == 0
      and (.required_pull_request_reviews.dismiss_stale_reviews // false) == true
      and (.required_pull_request_reviews.require_code_owner_reviews // true) == false
      and (.allow_force_pushes.enabled // false) == false
      and (.allow_deletions.enabled // false) == false
      and (.required_conversation_resolution.enabled // false) == true
      and (
        .restrictions == null
        or (
          ((.restrictions.users // []) | length) == 0
          and ((.restrictions.teams // []) | length) == 0
          and ((.restrictions.apps // []) | length) == 0
        )
      )
    ' "$tmp_dir/current.json" >/dev/null; then
      branch_state["$branch"]="compliant"
      printf '[OK] %s already matches the intended protection.\n' "$branch"
    else
      printf '[STOP] %s already has a protection rule that differs from this plan.\n' "$branch" >&2
      printf 'Review it manually; this script will not overwrite existing rules.\n' >&2
      exit 1
    fi
  else
    if grep -Eq 'HTTP 404|404 Not Found|Not Found \(HTTP 404\)' "$tmp_dir/error.log"; then
      branch_state["$branch"]="unprotected"
      printf '[PLAN] %s has no branch-protection rule yet.\n' "$branch"
    else
      cat "$tmp_dir/error.log" >&2
      printf 'Could not inspect protection for %s. No changes have been made.\n' "$branch" >&2
      printf 'A 403 usually means the GitHub token lacks Administration: write permission.\n' >&2
      exit 1
    fi
  fi
done

pending=0
for branch in "$BRANCH_MAIN" "$BRANCH_FOUNDATION"; do
  if [[ "${branch_state[$branch]}" == "unprotected" ]]; then
    pending=1
  fi
done

if [[ "$pending" -eq 0 ]]; then
  printf '\nBoth branches already match the intended protection policy.\n'
  exit 0
fi

printf '\nProtection policy to be applied to the unprotected target branch(es):\n'
printf '%s\n' \
  '- Require a pull request; require zero approving reviews (single-owner repository).' \
  '- Require current, passing rust-check, schema-tests, audit-script and portability checks.' \
  '- Enforce the same checks for administrators.' \
  '- Block force pushes and branch deletion.' \
  '- Require all review conversations to be resolved.' \
  '- Do not restrict who may create feature branches or push to unprotected feature branches.'

if [[ "$apply" -eq 0 ]]; then
  printf '\nDry run only. No repository settings were changed.\n'
  printf 'After reviewing this plan, run: bash scripts/apply-branch-protection.sh --apply\n'
  exit 0
fi

if [[ ! -t 0 ]]; then
  printf 'The --apply mode requires an interactive terminal confirmation.\n' >&2
  exit 1
fi
printf '\nThis writes repository branch-protection settings for %s.\n' "$REPO"
read -r -p 'Type APPLY to continue: ' confirmation
if [[ "$confirmation" != "APPLY" ]]; then
  printf 'Cancelled. No repository settings were changed.\n'
  exit 1
fi

for branch in "$BRANCH_MAIN" "$BRANCH_FOUNDATION"; do
  if [[ "${branch_state[$branch]}" == "unprotected" ]]; then
    encoded_branch="$(jq -rn --arg branch "$branch" '$branch | @uri')"
    endpoint="repos/$REPO/branches/$encoded_branch/protection"
    printf '\nApplying protection to %s...\n' "$branch"
    gh api --method PUT \
      --header "Accept: application/vnd.github+json" \
      --header "X-GitHub-Api-Version: $API_VERSION" \
      --input "$payload_file" \
      "$endpoint" >/dev/null
    gh api "$endpoint" >"$tmp_dir/verified.json"
    if ! jq -e --argjson required_checks "$REQUIRED_CHECKS" '
      .required_status_checks.strict == true
      and ((.required_status_checks.contexts // []) | sort) == ($required_checks | sort)
      and (.enforce_admins.enabled // false) == true
      and (.required_pull_request_reviews.required_approving_review_count // -1) == 0
      and (.allow_force_pushes.enabled // false) == false
      and (.allow_deletions.enabled // false) == false
      and (.required_conversation_resolution.enabled // false) == true
    ' "$tmp_dir/verified.json" >/dev/null; then
      printf '[ERROR] Verification did not match for %s. Inspect GitHub settings before retrying.\n' "$branch" >&2
      exit 1
    fi
    printf '[OK] Protection applied and verified for %s.\n' "$branch"
  fi
done

printf '\nBranch-protection setup finished. Verify both branches in GitHub Settings.\n'
