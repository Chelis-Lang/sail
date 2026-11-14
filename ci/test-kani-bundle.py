#!/usr/bin/env python3
"""Exercise strict Kani artifact admission and atomic fixture publication."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import os
import sys

spec = importlib.util.spec_from_file_location("sail_kani_bundle", Path(__file__).with_name("kani-bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)
PIN = Path(__file__).resolve().parent.parent / "engine/kani/bundle-linux.json"


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-kani-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.expected = {"bin/kani-driver": b"driver", "license-notes.txt": b"original notes"}

    def fixture(self, extras=(), omit=(), replacement=None, mode=0o755):
        archive = self.root / "bundle.tar.gz"
        pin = bundle.load_pin(PIN)
        pin["files"] = {name: {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(), "executable": name.startswith("bin/")} for name, data in self.expected.items()}
        with tarfile.open(archive, "w:gz") as output:
            for name, data in self.expected.items():
                if name in omit:
                    continue
                actual = replacement if replacement is not None and name.startswith("bin/") else data
                member = tarfile.TarInfo("kani-0.68.0/" + name)
                member.size = len(actual)
                member.mode = mode if name.startswith("bin/") else 0o644
                output.addfile(member, io.BytesIO(actual))
            for member, data in extras:
                output.addfile(member, io.BytesIO(data) if data else None)
        pin["archive_bytes"] = archive.stat().st_size
        pin["archive_sha256"] = hashlib.sha256(archive.read_bytes()).hexdigest()
        return archive, pin

    def refused(self, archive, pin):
        destination = self.root / "installed"
        with self.assertRaises((ValueError, OSError)):
            bundle.unpack(archive, pin, destination)
        self.assertFalse(os.path.lexists(destination))
        self.assertEqual(list(self.root.glob(".sail-kani-*")), [])

    def test_original_files_and_modes_exist_before_single_publication(self):
        archive, pin = self.fixture()
        destination = self.root / "installed"
        rename = Path.rename
        seen = []
        def observe(stage, target):
            self.assertEqual(target, destination)
            self.assertFalse(os.path.lexists(target))
            self.assertEqual(stage.stat().st_dev, target.parent.stat().st_dev)
            for name, data in self.expected.items():
                p = stage / name
                self.assertEqual(p.read_bytes(), data)
                self.assertEqual(p.stat().st_mode & 0o777, 0o755 if name.startswith("bin/") else 0o644)
            self.assertEqual((stage / "bin").stat().st_mode & 0o777, 0o755)
            seen.append(True)
            return rename(stage, target)
        previous = os.umask(0o077)
        try:
            with patch.object(Path, "rename", observe):
                bundle.unpack(archive, pin, destination)
        finally:
            os.umask(previous)
        self.assertEqual(seen, [True])

    def test_size_digest_missing_content_and_modes_refuse(self):
        archive, pin = self.fixture()
        for key, bad in [("archive_bytes", pin["archive_bytes"] + 1), ("archive_sha256", "0" * 64)]:
            changed = dict(pin, **{key: bad})
            self.refused(archive, changed)
        for options in [{"omit": ["license-notes.txt"]}, {"replacement": b"change"}, {"mode": 0o644}]:
            archive, pin = self.fixture(**options)
            self.refused(archive, pin)

    def test_alias_traversal_foreign_and_unknown_members_refuse(self):
        for name in ["kani-0.68.0/bin//extra", "kani-0.68.0/bin/./extra", "kani-0.68.0/../extra", "/kani-0.68.0/extra", "other/extra", "kani-0.68.0/extra", "kani-0.68.0/bin/kani-driver"]:
            with self.subTest(name=name):
                member = tarfile.TarInfo(name)
                member.size = 1
                archive, pin = self.fixture(extras=[(member, b"x")])
                self.refused(archive, pin)

    def test_links_fifo_and_devices_refuse(self):
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.FIFOTYPE, tarfile.CHRTYPE, tarfile.BLKTYPE]:
            member = tarfile.TarInfo("kani-0.68.0/toolchain")
            member.type = kind
            member.linkname = "/outside"
            archive, pin = self.fixture(extras=[(member, b"")])
            self.refused(archive, pin)

    def test_existing_directory_file_and_links_are_preserved(self):
        archive, pin = self.fixture()
        for kind in ["directory", "file", "link"]:
            destination = self.root / kind
            if kind == "directory":
                destination.mkdir()
            elif kind == "file":
                destination.write_text("keep")
            else:
                destination.symlink_to(self.root / "missing")
            with self.assertRaises(ValueError):
                bundle.unpack(archive, pin, destination)
            self.assertTrue(os.path.lexists(destination))
        self.assertEqual((self.root / "file").read_text(), "keep")
        self.assertEqual(os.readlink(self.root / "link"), str(self.root / "missing"))

    def test_faulted_mode_or_rename_never_publishes_and_retry_succeeds(self):
        archive, pin = self.fixture()
        for operation in ["chmod", "rename"]:
            with patch.object(Path, operation, side_effect=OSError("injected failure")):
                self.refused(archive, pin)
        bundle.unpack(archive, pin, self.root / "installed")
        self.assertEqual((self.root / "installed/license-notes.txt").read_bytes(), self.expected["license-notes.txt"])

    def test_pin_is_strict_and_bounded(self):
        pin = bundle.load_pin(PIN)
        self.assertEqual(len(pin["files"]), 120)
        path = self.root / "pin.json"
        for change in [{"schema_version": True}, {"archive_bytes": True}, {"version": "0.69.0"}, {"host": "aarch64-apple-darwin"}, {"unknown": 1}]:
            path.write_text(json.dumps(dict(pin, **change)))
            with self.assertRaises(ValueError):
                bundle.load_pin(path)
        path.write_text('{"version":"0.68.0","version":"0.68.0"}')
        with self.assertRaises(ValueError):
            bundle.load_pin(path)
        for name in [".", "a/../b", "a//b", "a/./b", "a\\b", "/a", "/".join(["a"] * 65)]:
            with self.assertRaises(ValueError):
                bundle.relative(name)

    def test_download_stream_uses_same_authentication(self):
        archive, pin = self.fixture()
        path = self.root / "pin.json"
        path.write_text(json.dumps(pin))
        content = archive.read_bytes()
        for suffix, data, succeeds in [("good", content, True), ("short", content[:-1], False), ("long", content + b"x", False)]:
            destination = self.root / suffix
            argv = ["kani-bundle.py", "--pin", str(path), "--destination", str(destination)]
            with patch.object(sys, "argv", argv), patch.object(bundle.urllib.request, "urlopen", return_value=io.BytesIO(data)):
                if succeeds:
                    bundle.main()
                    self.assertEqual((destination / "license-notes.txt").read_bytes(), self.expected["license-notes.txt"])
                else:
                    with self.assertRaises(ValueError):
                        bundle.main()
                    self.assertFalse(destination.exists())


if __name__ == "__main__":
    unittest.main()
