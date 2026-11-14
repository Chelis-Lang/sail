#!/usr/bin/env python3
"""Exercise original Rust payload boundaries without executing a toolchain."""
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import tarfile
import tempfile
import subprocess
import sys
import unittest

spec = importlib.util.spec_from_file_location("sail_kani_toolchain", Path(__file__).with_name("kani-toolchain.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class PayloadTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="sail-kani-rust-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def archive(self, extras=()):
        archive = self.root / "component.tar.xz"
        with tarfile.open(archive, "w:xz") as output:
            for name, data in [("rust/components", b"rustc\n"), ("rust/rustc/bin/rustc", b"original rustc"), ("rust/rustc/manifest.in", b"file:bin/rustc\n")]:
                member = tarfile.TarInfo(name)
                member.mode = 0o755 if name.endswith("bin/rustc") else 0o644
                member.size = len(data)
                output.addfile(member, io.BytesIO(data))
            for member, data in extras:
                output.addfile(member, io.BytesIO(data) if data else None)
        with archive.open("rb") as source:
            digest = checker.sha(source)
        return archive, {"component": "rustc", "archive_bytes": archive.stat().st_size, "archive_sha256": digest, "payload_files": 1, "payload_bytes": 14}

    def test_original_payload_and_installed_bytes(self):
        archive, row = self.archive()
        files = checker.component_inventory(archive, row)
        self.assertEqual(set(files), {"bin/rustc"})
        prefix = self.root / "prefix"
        (prefix / "bin").mkdir(parents=True)
        rustc = prefix / "bin/rustc"
        rustc.write_bytes(b"original rustc")
        rustc.chmod(0o755)
        self.assertEqual(checker.verify_installed(prefix, {"components": []}, files), {})
        rustc.write_bytes(b"modified rustc")
        with self.assertRaises(ValueError):
            checker.verify_installed(prefix, {"components": []}, files)

    def test_links_duplicates_and_noncanonical_members_refuse(self):
        for name, kind in [("rust/rustc/bin/../../escape", tarfile.REGTYPE), ("rust/rustc/bin/rustc", tarfile.REGTYPE), ("rust/rustc/lib/link", tarfile.SYMTYPE), ("rust/rustc/lib/link", tarfile.LNKTYPE), ("rust/rustc/lib/fifo", tarfile.FIFOTYPE)]:
            with self.subTest(name=name, kind=kind):
                member = tarfile.TarInfo(name)
                member.type = kind
                member.linkname = "bin/rustc"
                archive, row = self.archive([(member, b"")])
                with self.assertRaises(ValueError):
                    checker.component_inventory(archive, row)

    def test_original_digest_size_and_payload_completeness_refuse(self):
        archive, row = self.archive()
        for key, value in [("archive_sha256", "0" * 64), ("archive_bytes", row["archive_bytes"] + 1), ("payload_files", 2), ("payload_bytes", 15)]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                checker.component_inventory(archive, dict(row, **{key: value}))

    def test_unbound_files_missing_payload_and_installed_links_refuse(self):
        archive, row = self.archive()
        files = checker.component_inventory(archive, row)
        prefix = self.root / "prefix"
        prefix.mkdir()
        with self.assertRaises(ValueError):
            checker.verify_installed(prefix, {"components": []}, files)
        (prefix / "extra").write_bytes(b"unbound")
        with self.assertRaises(ValueError):
            checker.verify_installed(prefix, {"components": []}, files)
        (prefix / "extra").unlink()
        (prefix / "bin").symlink_to(self.root, target_is_directory=True)
        with self.assertRaises(ValueError):
            checker.verify_installed(prefix, {"components": []}, files)

    @unittest.skipUnless(hasattr(os, "geteuid"), "Unix filesystem permission control")
    def test_unreadable_extra_subtree_refuses_as_unprivileged_reader(self):
        self.root.chmod(0o755)
        source = self.root / "checker.py"
        source.write_bytes(Path(__file__).with_name("kani-toolchain.py").read_bytes())
        source.chmod(0o644)
        prefix = self.root / "prefix"
        (prefix / "bin").mkdir(parents=True)
        rustc = prefix / "bin/rustc"
        rustc.write_bytes(b"original rustc")
        rustc.chmod(0o755)
        hidden = prefix / "unreadable-extra"
        hidden.mkdir()
        (hidden / "unbound.so").write_bytes(b"unbound")
        if os.geteuid() == 0:
            for owned in [prefix, prefix / "bin", rustc, hidden, hidden / "unbound.so"]:
                os.chown(owned, 65534, 65534)
        hidden.chmod(0)
        code = """
import hashlib, runpy, sys
from pathlib import Path
checker = runpy.run_path(sys.argv[1])
files = {"bin/rustc": {"bytes":14, "sha256":hashlib.sha256(b"original rustc").hexdigest(), "executable":True}}
try:
    checker["verify_installed"](Path(sys.argv[2]), {"components":[]}, files)
except PermissionError:
    raise SystemExit(0) from None
raise SystemExit(1)
"""
        def unprivileged():
            os.setgroups([])
            os.setgid(65534)
            os.setuid(65534)
        try:
            result = subprocess.run(
                [sys.executable, "-c", code, str(source), str(prefix)],
                preexec_fn=unprivileged if os.geteuid() == 0 else None,
                capture_output=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, b"")
            self.assertEqual(result.stderr, b"")
        finally:
            hidden.chmod(0o755)

    def test_deeply_nested_metadata_has_stable_cli_refusal(self):
        pin = self.root / "nested.json"
        pin.write_text("[" * 16_000 + "0" + "]" * 16_000)
        manifest = self.root / "channel.toml"
        manifest.write_bytes(b"")
        output = self.root / "must-not-exist.json"
        result = subprocess.run(
            [sys.executable, str(Path(__file__).with_name("kani-toolchain.py")),
             "--pin", str(pin), "--manifest", str(manifest),
             "--archives", str(self.root), "--out", str(output)],
            capture_output=True, check=False,
        )
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertTrue(result.stderr.startswith(b"error[SAIL6302]:"))
        self.assertNotIn(b"Traceback", result.stderr)
        self.assertFalse(output.exists())

    def test_noncanonical_paths_and_duplicate_pin_fields_refuse(self):
        for name in ["/absolute", "a/../b", "a/./b", "a//b", "a\\b", "", "."]:
            with self.subTest(name=name), self.assertRaises(ValueError):
                checker.relative(name)
        with self.assertRaises(ValueError):
            checker.object_pairs([("schema_version", 1), ("schema_version", 1)])


if __name__ == "__main__":
    unittest.main()
