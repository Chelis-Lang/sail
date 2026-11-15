#!/usr/bin/env python3
"""Check payload-only installation and no-replacement publication boundaries."""
from pathlib import Path
import hashlib
import importlib.util
import io
import os
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("sail_kani_install", Path(__file__).with_name("kani-install.py"))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


@unittest.skipUnless(sys.platform == "linux", "atomic installer publication is Linux-only")
class InstallationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-kani-install-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.destination = self.root / "installed"
        self.payload = b"original rustc"
        self.rust_archive = self.root / "rust.tar.xz"
        with tarfile.open(self.rust_archive, "w:xz") as package:
            for name, value, mode in [("rust/components", b"rustc\n", 0o644),
                                      ("rust/rustc/bin/rustc", self.payload, 0o755),
                                      ("rust/rustc/manifest.in", b"file:bin/rustc\n", 0o644),
                                      ("rust/install.sh", b"exit 97\n", 0o755)]:
                member = tarfile.TarInfo(name)
                member.size = len(value)
                member.mode = mode
                package.addfile(member, io.BytesIO(value))
        self.row = {"package": "rustc", "component": "rustc", "url": "https://example.invalid/rust.tar.xz",
                    "archive_bytes": self.rust_archive.stat().st_size,
                    "archive_sha256": hashlib.sha256(self.rust_archive.read_bytes()).hexdigest(),
                    "payload_files": 1, "payload_bytes": len(self.payload)}
        self.rust_pin = {"toolchain": "fixture-only", "components": [self.row]}
        self.bundle_archive = self.root / "bundle.tar.gz"
        driver = b"not executed"
        with tarfile.open(self.bundle_archive, "w:gz") as package:
            member = tarfile.TarInfo("kani-0.68.0/bin/kani-driver")
            member.mode = 0o755
            member.size = len(driver)
            package.addfile(member, io.BytesIO(driver))
        self.bundle_pin = {"archive_bytes": self.bundle_archive.stat().st_size,
                           "archive_sha256": hashlib.sha256(self.bundle_archive.read_bytes()).hexdigest(),
                           "files": {"bin/kani-driver": {"bytes": len(driver), "sha256": hashlib.sha256(driver).hexdigest(), "executable": True}},
                           "empty_directories": []}

    def install(self):
        return installer.install(self.bundle_archive, self.bundle_pin, self.root, self.rust_pin, self.destination)

    def test_only_original_payload_is_published_together(self):
        result = self.install()
        self.assertEqual((self.destination / "rust/bin/rustc").read_bytes(), self.payload)
        self.assertEqual((self.destination / "bundle/bin/kani-driver").read_bytes(), b"not executed")
        self.assertFalse((self.destination / "rust/install.sh").exists())
        self.assertFalse((self.destination / "rust/lib/rustlib/components").exists())
        self.assertFalse(result["backend_selected"])
        self.assertFalse(result["proof_authority"])
        self.assertEqual(set(p.name for p in self.destination.iterdir()), {"bundle", "rust"})
        self.assertEqual(self.destination.stat().st_mode & 0o777, 0o700)
        self.assertFalse(list(self.root.glob(".sail-kani-install-*")))

    def test_existing_file_directory_and_dangling_link_stay_intact(self):
        for kind in ["file", "directory", "link"]:
            with self.subTest(kind=kind):
                if kind == "file": self.destination.write_bytes(b"keep")
                elif kind == "directory": self.destination.mkdir()
                else: self.destination.symlink_to(self.root / "absent")
                with self.assertRaises(ValueError): self.install()
                if kind == "file": self.assertEqual(self.destination.read_bytes(), b"keep")
                elif kind == "directory": self.assertEqual(list(self.destination.iterdir()), [])
                else: self.assertEqual(os.readlink(self.destination), str(self.root / "absent"))
                if kind == "directory": self.destination.rmdir()
                else: self.destination.unlink()

    def test_bad_original_archive_never_publishes_either_tree(self):
        self.bundle_pin["archive_sha256"] = "0" * 64
        with self.assertRaises(ValueError): self.install()
        self.assertFalse(self.destination.exists())
        self.assertFalse(list(self.root.glob(".sail-kani-install-*")))

    def test_changed_second_pass_payload_is_not_published(self):
        files = installer.rust.inventory(self.rust_pin, self.root)
        files["bin/rustc"]["sha256"] = "0" * 64
        stage = self.root / "staged-rust"
        stage.mkdir()
        with self.assertRaises(ValueError):
            installer.materialize_component(self.rust_archive, self.row, files, stage, set())
        self.assertFalse(self.destination.exists())

    def test_identical_component_overlay_preserves_exact_payload(self):
        self.rust_pin["components"].append(dict(self.row))
        result = self.install()
        self.assertEqual(result["rust_payload_files"], 1)
        self.assertEqual((self.destination / "rust/bin/rustc").read_bytes(), self.payload)

    def test_destination_appearing_at_final_publication_is_preserved(self):
        original = installer.bundle.publish

        def race(source, destination):
            if destination == self.destination:
                destination.write_bytes(b"concurrent owner")
            return original(source, destination)

        with mock.patch.object(installer.bundle, "publish", side_effect=race):
            with self.assertRaises(FileExistsError): self.install()
        self.assertEqual(self.destination.read_bytes(), b"concurrent owner")
        self.assertFalse(list(self.root.glob(".sail-kani-install-*")))

    def test_public_parent_refuses_before_staging(self):
        self.root.chmod(0o755)
        with self.assertRaises(ValueError): self.install()
        self.assertFalse(self.destination.exists())
        self.assertFalse(list(self.root.glob(".sail-kani-install-*")))


if __name__ == "__main__":
    unittest.main()
