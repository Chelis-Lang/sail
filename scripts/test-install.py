#!/usr/bin/env python3
"""Installation integrity, extraction and rollback controls; no verifier claim."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
import urllib.error
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("sail_install", Path(__file__).with_name("install.py"))
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)
TARGET = "x86_64-unknown-linux-gnu"
NAME = "sail-0.1.0-test-" + TARGET


class InstallTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-install-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / (NAME + ".tar.gz")

    def archive_with(self, extra=None):
        with tarfile.open(self.archive, "w:gz") as archive:
            for path in ("bin/sail", "bin/sail-z3", "engine/runtime/sail-build.json"):
                member = tarfile.TarInfo(NAME + "/" + path)
                member.mode = 0o755 if path.startswith("bin/") else 0o644
                member.size = 7
                archive.addfile(member, io.BytesIO(b"fixture"))
            if extra:
                archive.addfile(extra)
        checksum = hashlib.sha256(self.archive.read_bytes()).hexdigest()
        Path(str(self.archive) + ".sha256").write_text(checksum + "  " + self.archive.name + "\n")

    def mock_run(self, args, **kwargs):
        if args[1:] == ["--version"]:
            return subprocess.CompletedProcess(args, 0, stdout="sail 0.1.0-test\n")
        return subprocess.CompletedProcess(args, 0)

    def mock_status(self, args, **kwargs):
        install = Path(args[0]).parent.parent
        self.assertNotIn("SAIL_VERUS_BUILD", kwargs["env"])
        return json.dumps({"build": str(install / "engine/runtime"), "rust_toolchain": "1.99.1"})

    def test_install_preserves_layout_and_existing_install(self):
        self.archive_with()
        with patch.object(installer.subprocess, "run", side_effect=self.mock_run), \
                patch.object(installer.subprocess, "check_output", side_effect=self.mock_status):
            result = installer.install(self.archive, self.root / "prefix", TARGET)
            self.assertEqual((result / "bin/sail").read_bytes(), b"fixture")
            with self.assertRaisesRegex(ValueError, "already exists"):
                installer.install(self.archive, self.root / "prefix", TARGET)
            self.assertEqual((result / "bin/sail").read_bytes(), b"fixture")

    def test_corruption_and_other_platform_do_not_install(self):
        self.archive_with()
        with self.archive.open("ab") as archive:
            archive.write(b"corrupt")
        with self.assertRaisesRegex(ValueError, "checksum"):
            installer.install(self.archive, self.root / "prefix", TARGET)
        with self.assertRaisesRegex(ValueError, "platform"):
            installer.install(self.archive, self.root / "prefix", "aarch64-apple-darwin")
        self.assertFalse((self.root / "prefix").exists())

    def test_unavailable_download_explains_private_access(self):
        with patch.object(installer.shutil, "which", return_value=None), \
                patch.object(installer.urllib.request, "urlopen",
                             side_effect=urllib.error.HTTPError("https://github.com/", 404, "missing", {}, None)):
            with self.assertRaisesRegex(ValueError, "authenticated GitHub CLI"):
                installer.download("0.1.0", NAME + ".tar.gz", self.root)

    def test_traversal_links_and_special_files_are_rejected(self):
        for name, kind in ((NAME + "/../../escape", tarfile.REGTYPE),
                           (NAME + "/link", tarfile.SYMTYPE),
                           (NAME + "/hard", tarfile.LNKTYPE),
                           (NAME + "/pipe", tarfile.FIFOTYPE)):
            with self.subTest(name=name):
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = "/tmp"
                self.archive_with(member)
                with self.assertRaisesRegex(ValueError, "unsafe"):
                    installer.install(self.archive, self.root / "prefix", TARGET)
                self.assertFalse((self.root / "prefix" / NAME).exists())
        self.assertFalse((self.root / "escape").exists())

    def test_failed_runtime_validation_cleans_staging(self):
        self.archive_with()
        with patch.object(installer.subprocess, "run", side_effect=self.mock_run), \
                patch.object(installer.subprocess, "check_output", side_effect=ValueError("bad engine")):
            with self.assertRaisesRegex(ValueError, "bad engine"):
                installer.install(self.archive, self.root / "prefix", TARGET)
        self.assertEqual(list((self.root / "prefix").iterdir()), [])

    def test_toolchain_is_explicit_and_failure_does_not_publish(self):
        self.archive_with()
        calls = []

        def run(args, **kwargs):
            calls.append(args)
            if args[0] == "rustup":
                raise subprocess.CalledProcessError(1, args)
            return self.mock_run(args, **kwargs)

        with patch.object(installer.subprocess, "run", side_effect=run), \
                patch.object(installer.subprocess, "check_output", side_effect=self.mock_status):
            with self.assertRaises(subprocess.CalledProcessError):
                installer.install(self.archive, self.root / "prefix", TARGET, True)
        self.assertEqual(calls[-1], ["rustup", "toolchain", "install", "1.99.1", "--profile",
                                    "minimal", "--component", "rust-src"])
        self.assertFalse((self.root / "prefix" / NAME).exists())


if __name__ == "__main__":
    unittest.main()
