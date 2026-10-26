#!/usr/bin/env python3
"""Installation integrity, extraction and rollback controls; no verifier claim."""
import contextlib
import hashlib
import importlib.util
import io
import json
import os
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

    def fake_gh(self, verify_status=0, auth_status=0):
        """A `gh` on PATH that records its arguments and exits as configured."""
        directory = self.root / "fake-bin"
        directory.mkdir()
        log = self.root / "gh-calls"
        script = directory / "gh"
        script.write_text("#!/bin/sh\n"
                          "printf '%s\\n' \"$*\" >>" + str(log) + "\n"
                          "case \"$1 $2 $3\" in\n"
                          "  'auth status '*) exit " + str(auth_status) + " ;;\n"
                          "  'attestation verify --help') exit 0 ;;\n"
                          "  'attestation verify '*) echo 'fake gh: attestation result'; exit "
                          + str(verify_status) + " ;;\n"
                          "esac\n"
                          "exit 2\n")
        script.chmod(0o755)
        path = patch.dict(os.environ, {"PATH": str(directory)})
        path.start()
        self.addCleanup(path.stop)
        return log

    def test_attestation_runs_gh_verify_for_this_repository(self):
        self.archive_with()
        log = self.fake_gh()
        with contextlib.redirect_stdout(io.StringIO()) as output:
            self.assertTrue(installer.attest(self.archive, required=True))
        self.assertIn("Verified build-provenance attestation", output.getvalue())
        self.assertEqual(log.read_text().splitlines()[-1],
                         "attestation verify " + str(self.archive) + " --repo Chelis-Lang/sail")

    def test_failed_attestation_refuses_even_when_not_required(self):
        self.archive_with()
        self.fake_gh(verify_status=1)
        for required in (False, True):
            with self.subTest(required=required):
                with self.assertRaisesRegex(ValueError, "(?s)does not verify.*fake gh"):
                    installer.attest(self.archive, required)

    def assert_unavailable_gh(self):
        with contextlib.redirect_stderr(io.StringIO()) as note:
            self.assertFalse(installer.attest(self.archive))
        self.assertIn("only its SHA-256 checksum", note.getvalue())
        with self.assertRaisesRegex(ValueError, "--require-attestation needs GitHub CLI"):
            installer.attest(self.archive, required=True)

    def test_missing_gh_notes_or_refuses_when_required(self):
        self.archive_with()
        with patch.dict(os.environ, {"PATH": str(self.root / "empty")}):
            self.assert_unavailable_gh()

    def test_signed_out_gh_notes_or_refuses_when_required(self):
        self.archive_with()
        log = self.fake_gh(auth_status=1)
        self.assert_unavailable_gh()
        self.assertNotIn("attestation verify " + str(self.archive), log.read_text())

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
