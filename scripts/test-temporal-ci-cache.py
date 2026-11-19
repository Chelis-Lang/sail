#!/usr/bin/env python3
"""Regression controls for signed temporal receipt transport."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

MODULE = Path(__file__).resolve().parents[1] / "action/temporal-cache/receipts.py"
SPEC = importlib.util.spec_from_file_location("receipts", MODULE)
receipts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(receipts)


class Transport(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        key = self.root / "key"
        subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(key)], check=True)
        self.public = " ".join(key.with_suffix(".pub").read_text().split()[:2])
        self.secret = key.read_text()
        self.entry = Path("ab") / ("ab" * 32)
        self.cache = self.root / "cache/runtime/temporal-v1/v2"
        (self.cache / self.entry.parent).mkdir(parents=True)
        (self.cache / self.entry).write_bytes(b"complete worker receipt")
        self.stage = self.root / "stage"
        self.stage.mkdir()
        self.dest = self.root / "restored/runtime/temporal-v1/v2"
        self.assertTrue(receipts.export_bundle(self.cache, self.stage, "101", self.public, self.secret))

    def test_restore(self):
        self.assertEqual(receipts.import_bundle(self.dest, self.stage, "101", self.public), 1)
        self.assertEqual((self.dest / self.entry).read_bytes(), (self.cache / self.entry).read_bytes())

    def test_group_umask_keeps_signed_files_private(self):
        mask = os.umask(0o002)
        try:
            receipts.export_bundle(self.cache, self.stage, "101", self.public, self.secret)
            for name in ("bundle", "signature"):
                self.assertEqual((self.stage / name).stat().st_mode & 0o777, 0o600)
            self.assertEqual(receipts.import_bundle(self.dest, self.stage, "101", self.public), 1)
        finally:
            os.umask(mask)

    def test_tamper(self):
        path = self.stage / "bundle"
        path.write_bytes(path.read_bytes() + b"x")
        with self.assertRaises(ValueError):
            receipts.import_bundle(self.dest, self.stage, "101", self.public)
        self.assertFalse(self.dest.exists())

    def test_repository_binding(self):
        with self.assertRaises(ValueError):
            receipts.import_bundle(self.dest, self.stage, "102", self.public)
        self.assertFalse(self.dest.exists())

    def test_foreign_key(self):
        with self.assertRaises(ValueError):
            receipts.import_bundle(self.dest, self.stage, "101", "ssh-ed25519 AAAA")
        self.assertFalse(self.dest.exists())

    def test_symlink_omitted(self):
        path = self.cache / self.entry
        path.unlink()
        path.symlink_to(self.root / "key")
        self.assertIsNone(receipts.pack(self.cache, "101"))

    def test_writable_source_never_signed(self):
        (self.cache / self.entry).chmod(0o666)
        self.assertIsNone(receipts.pack(self.cache, "101"))
        (self.cache / self.entry).chmod(0o600)
        self.cache.parent.chmod(0o777)
        with self.assertRaises(ValueError):
            receipts.pack(self.cache, "101")

    def test_import_rejects_symlinked_runtime(self):
        target = self.root / "elsewhere"
        target.mkdir()
        self.dest.parents[2].mkdir()
        self.dest.parents[1].symlink_to(target, target_is_directory=True)
        with self.assertRaises(ValueError):
            receipts.import_bundle(self.dest, self.stage, "101", self.public)
        self.assertEqual(list(target.iterdir()), [])

    def test_partial_record_rejected(self):
        with self.assertRaises(ValueError):
            receipts.records((self.stage / "bundle").read_bytes()[:-1], "101")


if __name__ == "__main__":
    unittest.main()
