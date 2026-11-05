"""Tests for the offline RPM source archive contract, without fetching crates."""
import importlib.util
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("prepare_rpm", Path(__file__).with_name("prepare-rpm.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class PrepareRpmTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.project = Path(self.temporary.name) / "project"
        self.project.mkdir()
        (self.project / "Cargo.toml").write_text('[package]\nname="thermalprint"\nversion="1.2.3"\nlicense="MIT"\n')
        for name in ("Cargo.lock", "LICENSE", "README.md"):
            (self.project / name).write_text(name)
        for name in ("src", "data", "assets", "tests"):
            (self.project / name).mkdir()
            (self.project / name / "fixture").write_text(name)
        (self.project / "packaging/rpm").mkdir(parents=True)
        (self.project / "packaging/rpm/thermalprint.spec.in").write_text(
            'Version: @VERSION@\nLicense: @LICENSE@\n@BUNDLED_PROVIDES@\n')
        self.output = Path(self.temporary.name) / "output"

    def vendor(self, command, *, cwd, **kwargs):
        self.assertIn("--locked", command)
        self.assertIn("--offline", command)
        crate = cwd / "vendor/example-1.0.0"
        crate.mkdir(parents=True)
        (crate / "Cargo.toml").write_text('[package]\nname="example"\nversion="1.0.0"\nlicense="MIT/Apache-2.0"\n')
        (crate / "LICENSE-MIT").write_text("Example license")
        return subprocess.CompletedProcess(command, 0, '[source.crates-io]\nreplace-with="vendored-sources"\n[source.vendored-sources]\ndirectory="vendor"\n')

    def test_archive_contains_assets_offline_config_and_licenses(self):
        with patch.object(module.subprocess, "run", side_effect=self.vendor):
            result = module.prepare(self.project, self.output, offline=True)
        generated = result.read_text()
        self.assertIn("Version: 1.2.3", generated)
        self.assertIn("Bitstream-Vera", generated)
        self.assertIn("MIT OR Apache-2.0", generated)
        self.assertIn("bundled(crate(example)) = 1.0.0", generated)
        with tarfile.open(self.output / "SOURCES/thermalprint-1.2.3.tar.xz") as archive:
            for name in ("assets/fixture", "src/fixture", ".cargo/config.toml", "vendor/example-1.0.0/Cargo.toml", "vendor-licenses/example-1.0.0/LICENSE-MIT", "VENDORED-LICENSES.txt"):
                self.assertIsNotNone(archive.getmember(f"thermalprint-1.2.3/{name}"))
            self.assertTrue(all(entry.uid == 0 and entry.gid == 0 for entry in archive.getmembers()))

    def test_unreviewed_dependency_license_is_rejected(self):
        def unlicensed(command, *, cwd, **kwargs):
            result = self.vendor(command, cwd=cwd, **kwargs)
            (cwd / "vendor/example-1.0.0/Cargo.toml").write_text('[package]\nname="example"\nversion="1.0.0"\n')
            return result
        with patch.object(module.subprocess, "run", side_effect=unlicensed):
            with self.assertRaisesRegex(ValueError, "no license expression"):
                module.prepare(self.project, self.output, offline=True)

    def test_invalid_rpm_version_is_rejected_before_vendoring(self):
        (self.project / "Cargo.toml").write_text('[package]\nversion="1.2.3-beta"\n')
        with patch.object(module.subprocess, "run") as cargo:
            with self.assertRaisesRegex(ValueError, "numeric"):
                module.prepare(self.project, self.output)
            cargo.assert_not_called()


if __name__ == "__main__":
    unittest.main()
