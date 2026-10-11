#!/usr/bin/env python3
"""Installation and exact-inventory prerequisite controls; no proof acceptance."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("native_product", Path(__file__).with_name("m7-native-product.py"))
native = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native)


class NativeProductTests(unittest.TestCase):
    def sources(self, root):
        source = root / "source"
        source.mkdir()
        expected = {}
        for name in native.NAMES:
            data = (name + "\n").encode() * 20
            (source / name).write_bytes(data)
            expected[name] = {"bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        return source, expected

    def credentials(self):
        return {"fsuid": os.geteuid(), "cap_lease": False}

    def binding(self, path):
        metadata = path.stat()
        return {"path": list(os.fsencode(path)), "canonical": list(os.fsencode(path.resolve())),
                "device": metadata.st_dev, "inode": metadata.st_ino}

    def test_fresh_destination_exact_bytes_modes_and_owner(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            before = {name: (source / name).read_bytes() for name in native.NAMES}
            with patch.object(native, "identity", return_value=self.credentials()):
                result = native.install(source, root / "installed", expected)
            self.assertEqual(result["authority"], "installation-only")
            self.assertEqual((root / "installed").stat().st_mode & 0o777, 0o700)
            for name in native.NAMES:
                installed = root / "installed" / name
                self.assertEqual(installed.read_bytes(), before[name])
                self.assertEqual((source / name).read_bytes(), before[name])
                self.assertEqual(installed.stat().st_uid, os.geteuid())
                self.assertEqual(installed.stat().st_mode & 0o777, 0o700)
                self.assertEqual(result["files"][name]["sha256"], expected[name]["sha256"])

    def test_existing_destination_and_symlink_refused_without_replacement(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            existing = root / "installed"
            existing.mkdir()
            (existing / "keep").write_bytes(b"unchanged")
            with patch.object(native, "identity", return_value=self.credentials()):
                with self.assertRaises(FileExistsError):
                    native.install(source, existing, expected)
                alias = root / "alias"
                alias.symlink_to(existing, target_is_directory=True)
                with self.assertRaises(FileExistsError):
                    native.install(source, alias, expected)
            self.assertEqual((existing / "keep").read_bytes(), b"unchanged")
            self.assertTrue(alias.is_symlink())

    def test_hash_size_failure_removes_only_new_partial_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            for key, value in [("sha256", "0" * 64), ("bytes", 1)]:
                broken = {name: dict(row) for name, row in expected.items()}
                broken["sail-z3"][key] = value
                with patch.object(native, "identity", return_value=self.credentials()):
                    with self.assertRaises(ValueError):
                        native.install(source, root / "partial", broken)
                self.assertFalse((root / "partial").exists())
                self.assertEqual(hashlib.sha256((source / "sail").read_bytes()).hexdigest(), expected["sail"]["sha256"])

    def test_nonregular_source_fails_promptly_in_owned_subprocess(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            grant = root / "grant.json"
            grant.write_text(json.dumps(expected))
            (source / "sail").unlink()
            os.mkfifo(source / "sail")
            # Mock identity in the child for portable host control; FIFO is real.
            code = "import runpy,sys; m=runpy.run_path(sys.argv[1]); m['install'].__globals__['identity']=lambda: {'fsuid':__import__('os').geteuid(),'cap_lease':False}; m['install'](__import__('pathlib').Path(sys.argv[2]),__import__('pathlib').Path(sys.argv[3]),__import__('json').loads(open(sys.argv[4]).read()))"
            result = subprocess.run([sys.executable, "-c", code, str(Path(native.__file__)), str(source), str(root / "new"), str(grant)], capture_output=True, timeout=2)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn(b"expected regular file", result.stderr)
            self.assertFalse((root / "new").exists())
            (source / "sail").unlink()
            (source / "sail").symlink_to(source / "sail-z3")
            with patch.object(native, "identity", return_value=self.credentials()):
                with self.assertRaises(OSError):
                    native.install(source, root / "new", expected)

    def test_all_bindings_audited_foreign_uid_and_capability_not_current(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, _ = self.sources(root)
            rows = [self.binding(source / name) for name in native.NAMES]
            absent = native.audit_bindings(rows, {"fsuid": os.geteuid() + 1, "cap_lease": False})
            self.assertFalse(absent["ownership_prerequisite_met"])
            self.assertEqual(len(absent["foreign_owned"]), 2)
            capable = native.audit_bindings(rows, {"fsuid": os.geteuid() + 1, "cap_lease": True})
            self.assertTrue(capable["ownership_prerequisite_met"])
            self.assertFalse(capable["currentness_certified"])
            self.assertEqual(capable["supplied_bindings"], 2)

    def test_replacement_alias_duplicate_and_invalid_path_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, _ = self.sources(root)
            path = source / "sail"
            row = self.binding(path)
            with self.assertRaises(ValueError):
                native.audit_bindings([row, row], self.credentials())
            stale = dict(row, inode=row["inode"] + 1)
            with self.assertRaises(ValueError):
                native.audit_bindings([stale], self.credentials())
            alias = root / "alias"
            alias.symlink_to(path)
            aliased = self.binding(alias)
            alias.unlink()
            alias.symlink_to(source / "sail-z3")
            with self.assertRaises(ValueError):
                native.audit_bindings([aliased], self.credentials())
            with self.assertRaises(ValueError):
                native.audit_bindings([dict(row, path=[47, 0])], self.credentials())

    def test_grants_inventory_and_json_bounds_are_strict(self):
        for value in ({}, {"sail": {}}, {name: {"bytes": True, "sha256": "0" * 64} for name in native.NAMES}):
            with self.assertRaises(ValueError):
                native.expected_files(value)
        with self.assertRaises(ValueError):
            native.audit_bindings([{}] * (native.MAX_ROWS + 1), self.credentials())
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "input"
            path.write_text('{"same":1,"same":2}')
            with self.assertRaises(ValueError):
                native.load_json(path)
            with self.assertRaises(ValueError):
                native.load_json(path, 2)

    @unittest.skipUnless(sys.platform.startswith("linux") and os.geteuid() == 0, "owned Linux root fixture")
    def test_foreign_owned_fixture_refuses_lease_new_byte_copy_accepts(self):
        import errno
        import fcntl
        if native.identity()["cap_lease"]:
            self.skipTest("this negative control requires CAP_LEASE absent")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            for name in native.NAMES:
                os.chown(source / name, 501, 0)  # Only fresh fixture files, never sealed products.
            refused = native.audit_products(source)
            self.assertFalse(refused["ownership_prerequisite_met"])
            with (source / "sail").open("rb") as stream:
                with self.assertRaises(OSError) as error:
                    fcntl.fcntl(stream.fileno(), fcntl.F_SETLEASE, fcntl.F_RDLCK)
                self.assertEqual(error.exception.errno, errno.EACCES)
            installed = native.install(source, root / "new", expected)
            self.assertTrue(native.audit_products(root / "new")["ownership_prerequisite_met"])
            with (root / "new" / "sail").open("rb") as stream:
                fcntl.fcntl(stream.fileno(), fcntl.F_SETLEASE, fcntl.F_RDLCK)
                self.assertEqual(fcntl.fcntl(stream.fileno(), fcntl.F_GETLEASE), fcntl.F_RDLCK)
                fcntl.fcntl(stream.fileno(), fcntl.F_SETLEASE, fcntl.F_UNLCK)
            for name in native.NAMES:
                self.assertEqual((source / name).stat().st_uid, 501)
                self.assertEqual(hashlib.sha256((source / name).read_bytes()).hexdigest(), expected[name]["sha256"])
            print(json.dumps({"scope": "fresh owned Linux fixture only; no sealed product or proof",
                              "credentials": native.identity(), "source_uid": 501, "refusal_errno": errno.EACCES,
                              "source_content_before_and_after": expected, "installed": installed,
                              "destination_read_lease_confirmed": True, "lease_released": True}, sort_keys=True))

    @unittest.skipUnless(sys.platform.startswith("linux"), "actual Linux credentials")
    def test_actual_linux_identity_and_installed_binding(self):
        who = native.identity()
        self.assertEqual(who["uids"][1], os.geteuid())
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, expected = self.sources(root)
            native.install(source, root / "new", expected)
            result = native.audit_products(root / "new")
            self.assertTrue(result["ownership_prerequisite_met"])
            self.assertFalse(result["currentness_certified"])


if __name__ == "__main__":
    unittest.main()
