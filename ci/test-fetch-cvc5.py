#!/usr/bin/env python3
"""Digest, member and publication controls for the contributor cvc5 fixture."""
import hashlib
import importlib.util
from pathlib import Path
import stat
import tempfile
import unittest
import warnings
import zipfile

spec = importlib.util.spec_from_file_location("fetch_cvc5", Path(__file__).with_name("fetch-cvc5.py"))
fetch = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fetch)


class Fixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-cvc5-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.archive = self.root / "runtime.zip"
        self.pins = self.root / "pins.toml"
        self.out = self.root / "installed"
        self.body = b"fixture bytes, not a solver\x00\xff"

    def prepare(self, extra=(), binary=None):
        prefix = "cvc5-Linux-x86_64-static/"
        entries = [(prefix + "bin/cvc5", self.body), (prefix + "COPYING", b"notice"),
                   (prefix + "AUTHORS", b"authors"), (prefix + "licenses/runtime.txt", b"runtime notice")]
        with warnings.catch_warnings():
            warnings.simplefilter("ignore", UserWarning)
            with zipfile.ZipFile(self.archive, "w") as output:
                for name, data in entries + list(extra):
                    if name == prefix + "bin/cvc5" and binary is not None:
                        info = zipfile.ZipInfo(name)
                        info.external_attr = binary << 16
                        output.writestr(info, data)
                    else:
                        output.writestr(name, data)
        self.pins.write_text(
            '[cvc5]\nversion="1.4.2"\ntag="cvc5-1.4.2"\n'
            '[cvc5.assets.x86_64-unknown-linux-gnu]\n'
            'url="https://github.com/cvc5/cvc5/releases/download/cvc5-1.4.2/cvc5-Linux-x86_64-static.zip"\n'
            'bin="cvc5-Linux-x86_64-static/bin/cvc5"\n'
            f'sha256="{fetch.sha(self.archive)}"\n'
            f'bin_sha256="{hashlib.sha256(self.body).hexdigest()}"\n')

    def test_exact_binary_and_notices_are_published(self):
        self.prepare()
        executable = fetch.install(self.pins, self.out, self.archive)
        self.assertEqual(executable.read_bytes(), self.body)
        self.assertEqual(stat.S_IMODE(executable.stat().st_mode), 0o755)
        self.assertEqual((self.out / "COPYING").read_bytes(), b"notice")
        self.assertEqual((self.out / "licenses/runtime.txt").read_bytes(), b"runtime notice")
        with self.assertRaises(ValueError):
            fetch.install(self.pins, self.out, self.archive)

    def test_tampered_archive_and_binary_never_publish(self):
        self.prepare()
        self.archive.write_bytes(self.archive.read_bytes() + b"tamper")
        with self.assertRaisesRegex(ValueError, "archive digest"):
            fetch.install(self.pins, self.out, self.archive)
        self.assertFalse(self.out.exists())
        self.prepare()
        self.pins.write_text(self.pins.read_text().replace(hashlib.sha256(self.body).hexdigest(), "0" * 64))
        with self.assertRaisesRegex(ValueError, "executable digest"):
            fetch.install(self.pins, self.out, self.archive)
        self.assertFalse(self.out.exists())

    def test_duplicate_redirected_and_special_members_refuse(self):
        prefix = "cvc5-Linux-x86_64-static/"
        for extra, binary in [([(prefix + "COPYING", b"duplicate")], None),
                              ([(prefix + "licenses/../../escape", b"bad")], None),
                              ([], stat.S_IFLNK | 0o777)]:
            with self.subTest(extra=extra, binary=binary):
                self.prepare(extra, binary)
                with self.assertRaises(ValueError):
                    fetch.install(self.pins, self.out, self.archive)
                self.assertFalse(self.out.exists())
                self.assertFalse((self.root / "escape").exists())

    def test_unknown_host_and_noncanonical_digest_refuse(self):
        self.prepare()
        with self.assertRaises(KeyError):
            fetch.install(self.pins, self.out, self.archive, host="unqualified-host")
        self.pins.write_text(self.pins.read_text().replace(fetch.sha(self.archive), "G" * 64))
        with self.assertRaisesRegex(ValueError, "digest pin"):
            fetch.install(self.pins, self.out, self.archive)
        self.assertFalse(self.out.exists())


if __name__ == "__main__":
    unittest.main()
