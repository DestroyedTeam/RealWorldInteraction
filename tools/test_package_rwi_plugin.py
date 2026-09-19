import json
from pathlib import Path
import tempfile
import unittest
from package_rwi_plugin import package, NAME


class PackagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.binary = self.root / "adapter with spaces"
        self.binary.write_text("offline fixture; never executed")
        self.binary.chmod(0o700)
        self.profile = self.root / "private-profile.json"
        self.profile.write_text('{"secret":"DO_NOT_COPY_PROFILE_CONTENTS"}')
        self.output = self.root / NAME

    def test_external_entry_three_skills_and_no_secret_copy(self):
        package(self.binary, self.profile, self.output)
        entry = json.loads((self.output / ".mcp.json").read_text())["mcpServers"]
        self.assertEqual(list(entry), ["real_world_interaction"])
        self.assertEqual(entry["real_world_interaction"]["args"], ["--profile", str(self.profile)])
        self.assertEqual(len(list((self.output / "skills").glob("*/SKILL.md"))), 3)
        for path in self.output.rglob("*"):
            if path.is_file():
                self.assertNotIn(b"DO_NOT_COPY_PROFILE_CONTENTS", path.read_bytes())
        self.assertFalse((self.output / "bin").exists())

    def test_existing_bundle_requires_explicit_replace_and_preserves_backup(self):
        package(self.binary, self.profile, self.output)
        with self.assertRaises(ValueError):
            package(self.binary, self.profile, self.output)
        result = package(self.binary, self.profile, self.output, replace=True)
        self.assertTrue((Path(result["backup"]) / ".mcp.json").is_file())

    def test_private_profile_cannot_be_inside_replaced_bundle(self):
        self.output.mkdir()
        profile = self.output / "private.json"
        profile.write_text("private")
        with self.assertRaises(ValueError):
            package(self.binary, profile, self.output, replace=True)
        self.assertEqual(profile.read_text(), "private")

    def test_symlink_output_rejected(self):
        target = self.root / "unrelated"
        target.mkdir()
        self.output.symlink_to(target, target_is_directory=True)
        with self.assertRaises(ValueError):
            package(self.binary, self.profile, self.output, replace=True)


if __name__ == "__main__":
    unittest.main()
