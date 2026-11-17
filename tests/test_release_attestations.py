from __future__ import annotations

import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
WORKFLOW_PATH = ROOT / ".github" / "workflows" / "release-candidate-gate.yml"
WORKFLOW = WORKFLOW_PATH.read_text(encoding="utf-8")
ACTION = "actions/attest@1e69f48acb82d1966a394da916b4c1698aa569d6 # v4.2.2"
APP_BINARY = "target/release/madhab-habit-tracker"
DATA_BINARY = "target/release/madhab-data-tool"


class ReleaseAttestationContractTests(unittest.TestCase):
    def test_attestation_action_is_pinned_to_verified_release_commit(self) -> None:
        self.assertEqual(WORKFLOW.count("uses: " + ACTION), 2)
        self.assertNotIn("uses: actions/attest@v4", WORKFLOW)

    def test_release_binaries_are_built_with_locked_dependencies(self) -> None:
        self.assertIn(
            "cargo build --locked --release --bin madhab-habit-tracker --bin madhab-data-tool",
            WORKFLOW,
        )
        self.assertIn(APP_BINARY, WORKFLOW)
        self.assertIn(DATA_BINARY, WORKFLOW)

    def test_both_binaries_receive_provenance_and_cyclonedx_attestations(self) -> None:
        self.assertGreaterEqual(WORKFLOW.count(APP_BINARY), 4)
        self.assertGreaterEqual(WORKFLOW.count(DATA_BINARY), 4)
        self.assertIn("sbom-path: sbom.json", WORKFLOW)
        self.assertIn("https://cyclonedx.org/bom", WORKFLOW)

    def test_attestations_are_verified_before_evidence_upload(self) -> None:
        verify = WORKFLOW.index("gh attestation verify")
        upload = WORKFLOW.index("- name: Upload release candidate evidence")
        self.assertLess(verify, upload)
        self.assertIn("GH_TOKEN: ${{ github.token }}", WORKFLOW)

    def test_signing_permissions_are_job_scoped_and_not_write_all(self) -> None:
        for permission in (
            "id-token: write",
            "attestations: write",
            "artifact-metadata: write",
            "contents: read",
        ):
            self.assertIn(permission, WORKFLOW)
        self.assertNotIn("write-all", WORKFLOW)

    def test_attestations_only_run_for_release_tags_or_manual_dispatch(self) -> None:
        self.assertIn('      - "v*"', WORKFLOW)
        self.assertIn("  workflow_dispatch:", WORKFLOW)
        self.assertNotIn("  pull_request:", WORKFLOW)

    def test_uploaded_evidence_contains_binaries_and_checksums(self) -> None:
        for path in (
            APP_BINARY,
            DATA_BINARY,
            "madhab-habit-tracker.sha256",
            "madhab-data-tool.sha256",
            "sbom.json.sha256",
            "candidate-metadata.json.sha256",
        ):
            self.assertIn(path, WORKFLOW)


if __name__ == "__main__":
    unittest.main()
