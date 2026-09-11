"""Deterministic negative gates for the engineering-only architecture checker."""
import tempfile
from pathlib import Path
import unittest
from check_architecture import audit


class ArchitectureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "core/src").mkdir(parents=True)
        (self.root / "Cargo.toml").write_text('[workspace]\nmembers=["core"]\n')
        self.manifest = self.root / "core/Cargo.toml"
        self.manifest.write_text('[package]\nname="core"\n')
        self.source = self.root / "core/src/lib.rs"
        self.source.write_text("// small\n")
        self.rules = {"allowed_production_dependencies": {"core": []},
                      "existing_hotspot_ceilings": {}, "new_file_line_limit": 2}

    def test_small_leaf_passes(self):
        self.assertTrue(audit(self.root, self.rules)["passed"])

    def test_platform_dependency_fails(self):
        with self.manifest.open("a") as stream:
            stream.write('[target.\'cfg(windows)\'.dependencies]\nplatform={path="../platform"}\n')
        self.assertFalse(audit(self.root, self.rules)["passed"])

    def test_new_god_file_and_obsolete_workspace_fail(self):
        self.source.write_text("// line\n" * 3)
        self.assertFalse(audit(self.root, self.rules)["passed"])
        self.source.write_text("// small\n")
        self.assertTrue(audit(self.root, self.rules)["passed"])
        self.rules["allowed_production_dependencies"]["missing-native-link"] = []
        self.assertFalse(audit(self.root, self.rules)["passed"])

    def test_alias_and_workspace_dependencies_cannot_hide_reverse_edge(self):
        with self.manifest.open("a") as stream:
            stream.write('[dependencies]\ninnocent={package="platform",path="../platform"}\n')
        self.assertIn("platform", str(audit(self.root, self.rules)["violations"]))
        self.manifest.write_text('[package]\nname="core"\n[dependencies]\ninnocent={workspace=true}\n')
        with (self.root / "Cargo.toml").open("a") as stream:
            stream.write('[workspace.dependencies]\ninnocent={package="platform",path="platform"}\n')
        self.assertIn("platform", str(audit(self.root, self.rules)["violations"]))


if __name__ == "__main__":
    unittest.main()
