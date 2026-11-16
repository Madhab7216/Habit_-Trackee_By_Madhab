import unittest

from scripts.validate_sbom import validate_document


def valid_bom():
    return {
        "bomFormat": "CycloneDX",
        "specVersion": "1.7",
        "metadata": {
            "component": {
                "name": "habit-trackers-by-madhab",
                "version": "0.1.0",
            }
        },
        "components": [
            {"name": "gtk4", "version": "0.11.5"},
            {"name": "rusqlite", "version": "0.42.0"},
        ],
        "dependencies": [
            {"ref": "pkg:cargo/habit-trackers-by-madhab@0.1.0", "dependsOn": []}
        ],
    }


class SbomValidatorTests(unittest.TestCase):
    def test_accepts_valid_project_bom(self):
        self.assertEqual(validate_document(valid_bom()), [])

    def test_rejects_non_cyclonedx_document(self):
        document = valid_bom()
        document["bomFormat"] = "SPDX"
        self.assertIn("bomFormat must be 'CycloneDX'", validate_document(document))

    def test_rejects_unsupported_specification_version(self):
        document = valid_bom()
        document["specVersion"] = "1.6"
        self.assertIn("specVersion must be '1.7'", validate_document(document))

    def test_rejects_wrong_root_component(self):
        document = valid_bom()
        document["metadata"]["component"]["name"] = "unrelated-crate"
        self.assertIn(
            "root component name must be 'habit-trackers-by-madhab'",
            validate_document(document),
        )

    def test_requires_expected_direct_dependencies(self):
        document = valid_bom()
        document["components"] = [{"name": "gtk4", "version": "0.11.5"}]
        self.assertIn(
            "missing expected direct dependency components: rusqlite",
            validate_document(document),
        )

    def test_requires_dependency_relationships(self):
        document = valid_bom()
        document["dependencies"] = []
        self.assertIn(
            "dependencies must contain the resolved dependency relationships",
            validate_document(document),
        )

    def test_rejects_non_object_document(self):
        self.assertEqual(
            validate_document([]),
            ["SBOM root must be a JSON object"],
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
