from __future__ import annotations

import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = (ROOT / ".github/workflows/codeql.yml").read_text(encoding="utf-8")
DEPENDABOT = (ROOT / ".github/dependabot.yml").read_text(encoding="utf-8")
DOC = (ROOT / "docs/SECURITY_AUTOMATION.md").read_text(encoding="utf-8")

CODEQL_SHA = "33d7b85da1a04171e211befe2c739303b9a4c066"
CHECKOUT_SHA = "3d3c42e5aac5ba805825da76410c181273ba90b1"


class SecurityAutomationContractTests(unittest.TestCase):
    def test_codeql_actions_are_immutable_and_currently_pinned(self) -> None:
        self.assertEqual(WORKFLOW.count("github/codeql-action/init@" + CODEQL_SHA), 2)
        self.assertEqual(WORKFLOW.count("github/codeql-action/analyze@" + CODEQL_SHA), 2)
        self.assertEqual(WORKFLOW.count("actions/checkout@" + CHECKOUT_SHA), 2)
        self.assertRegex(CODEQL_SHA, r"^[0-9a-f]{40}$")

    def test_rust_and_workflow_languages_have_separate_analyses(self) -> None:
        self.assertIn("languages: rust", WORKFLOW)
        self.assertIn("build-mode: none", WORKFLOW)
        self.assertIn('category: "/language:rust"', WORKFLOW)
        self.assertIn("languages: actions", WORKFLOW)
        self.assertIn('category: "/language:actions"', WORKFLOW)

    def test_security_extended_queries_are_enabled_for_both_languages(self) -> None:
        self.assertEqual(WORKFLOW.count("queries: security-extended"), 2)

    def test_scans_pushes_pull_requests_and_weekly_schedule(self) -> None:
        self.assertIn("  push:", WORKFLOW)
        self.assertIn('      - "**"', WORKFLOW)
        self.assertIn("  pull_request:", WORKFLOW)
        self.assertIn("  schedule:", WORKFLOW)
        self.assertIn("  workflow_dispatch:", WORKFLOW)

    def test_only_minimum_code_scanning_permissions_are_granted(self) -> None:
        self.assertEqual(WORKFLOW.count("security-events: write"), 2)
        self.assertEqual(WORKFLOW.count("contents: read"), 3)
        self.assertEqual(WORKFLOW.count("actions: read"), 2)
        self.assertNotIn("write-all", WORKFLOW)
        self.assertNotIn("pull_request_target", WORKFLOW)
        self.assertIn("persist-credentials: false", WORKFLOW)

    def test_fork_pull_requests_do_not_run_write_enabled_analysis(self) -> None:
        self.assertEqual(
            WORKFLOW.count(
                "if: ${{ github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository }}"
            ),
            2,
        )

    def test_dependabot_tracks_cargo_and_github_actions_on_foundation(self) -> None:
        self.assertEqual(DEPENDABOT.count("package-ecosystem: cargo"), 1)
        self.assertEqual(DEPENDABOT.count("package-ecosystem: github-actions"), 1)
        self.assertEqual(DEPENDABOT.count("target-branch: work/linux-mint-habit-tracker-foundation"), 2)
        self.assertEqual(DEPENDABOT.count("interval: weekly"), 2)

    def test_dependabot_groups_only_nonbreaking_version_updates(self) -> None:
        self.assertEqual(DEPENDABOT.count("- minor"), 2)
        self.assertEqual(DEPENDABOT.count("- patch"), 2)
        self.assertNotIn("- major", DEPENDABOT)

    def test_dependabot_limits_noise_and_applies_cooldown(self) -> None:
        self.assertEqual(DEPENDABOT.count("open-pull-requests-limit: 5"), 2)
        self.assertEqual(DEPENDABOT.count("default-days: 7"), 2)
        self.assertIn("security updates", DOC.lower())
        self.assertIn("default branch", DOC.lower())

    def test_staged_branch_security_update_limitation_is_documented(self) -> None:
        self.assertIn("target-branch", DOC)
        self.assertIn("does not create Dependabot security-update PRs", DOC)
        self.assertIn("Enable the dependency graph and Dependabot alerts", DOC)


if __name__ == "__main__":
    unittest.main()
