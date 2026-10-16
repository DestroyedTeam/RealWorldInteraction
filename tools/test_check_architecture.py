"""Deterministic negative gates for the engineering-only architecture checker."""
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch
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

    def test_utf8_source_and_manifests_ignore_windows_ansi_default(self):
        self.source.write_text("// \u9f20\u6807\u60ac\u505c \u2192 \U0001f680\n", encoding="utf-8")
        self.manifest.write_text('[package]\nname="core"\n# \u63a7\u4ef6\n', encoding="utf-8")
        (self.root / "Cargo.toml").write_text(
            '[workspace]\nmembers=["core"]\n# \u7a97\u53e3\n', encoding="utf-8")
        original = Path.read_text

        def ansi_default(path, *args, **kwargs):
            if not args and kwargs.get("encoding") is None:
                kwargs["encoding"] = "cp1252"
            return original(path, *args, **kwargs)

        with patch.object(Path, "read_text", ansi_default):
            self.assertTrue(audit(self.root, self.rules)["passed"])

    def test_invalid_utf8_is_not_ignored_replaced_or_ansi_decoded(self):
        self.source.write_bytes(b"// invalid UTF-8: \xff\n")
        original = Path.read_text

        def ansi_default(path, *args, **kwargs):
            if not args and kwargs.get("encoding") is None:
                kwargs["encoding"] = "cp1252"
            return original(path, *args, **kwargs)

        with patch.object(Path, "read_text", ansi_default):
            with self.assertRaises(UnicodeDecodeError):
                audit(self.root, self.rules)

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

    def presentation_rules(self):
        self.rules["presentation_only_packages"] = {"core": {"external_dependencies": ["serde"]}}

    def test_presentation_leaf_allows_only_declared_serde_and_ignores_dev_edges(self):
        self.presentation_rules()
        with self.manifest.open("a") as stream:
            stream.write('[dependencies]\nserde="1"\n[dev-dependencies]\nimage={path="../image"}\n')
        self.assertTrue(audit(self.root, self.rules)["passed"])

    def test_presentation_registry_alias_and_target_build_edges_remain_forbidden(self):
        self.presentation_rules()
        for fragment in [
            '[dependencies]\ninnocent={package="windows",version="1",optional=true}\n',
            '[target.\'cfg(windows)\'.dependencies]\nwindows="1"\n',
            '[build-dependencies]\nlibloading="1"\n',
        ]:
            with self.subTest(fragment=fragment):
                self.manifest.write_text('[package]\nname="core"\n' + fragment)
                self.assertFalse(audit(self.root, self.rules)["passed"])

    def test_presentation_workspace_alias_cannot_hide_external_platform_package(self):
        self.presentation_rules()
        self.manifest.write_text('[package]\nname="core"\n[dependencies]\ninnocent={workspace=true}\n')
        with (self.root / "Cargo.toml").open("a") as stream:
            stream.write('[workspace.dependencies]\ninnocent={package="windows",version="1"}\n')
        self.assertIn("windows", str(audit(self.root, self.rules)["violations"]))

    def test_presentation_implicit_or_explicit_build_script_is_refused(self):
        self.presentation_rules()
        (self.root / "core/build.rs").write_text("fn main() {}\n")
        self.assertFalse(audit(self.root, self.rules)["passed"])
        (self.root / "core/build.rs").unlink()
        self.manifest.write_text('[package]\nname="core"\nbuild="elsewhere.rs"\n')
        self.assertFalse(audit(self.root, self.rules)["passed"])


if __name__ == "__main__":
    unittest.main()
