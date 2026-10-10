#!/usr/bin/env python3
"""Corrupt or stale optional-engine artifacts must never become available."""

import argparse
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("artifact", Path(__file__).with_name("engine-artifact.py"))
artifact = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifact)


class ArtifactTests(unittest.TestCase):
    def fixture(self, root):
        build = root / "source-variant"
        runtime = build / artifact.RUNTIME
        runtime.mkdir(parents=True)
        files = {}
        for name in artifact.REQUIRED:
            path = runtime / name
            path.write_bytes(name.encode())
            files[str(artifact.RUNTIME / name)] = artifact.sha(path)
        manifest = {"schema_version": 1, "commit": "pinned", "rust_toolchain": "pinned",
                    "z3_sha256": artifact.sha(runtime / "z3"), "artifacts": files,
                    "features": ["sail-interactive:" + artifact.sha(artifact.REPO / "engine/verus/interactive.patch"),
                                 "host:x86-linux", "source-lock:" + artifact.sha(artifact.REPO / "engine/verus/Cargo.lock")]}
        (build / "sail-build.json").write_text(json.dumps(manifest))
        archive = root / "runtime.tar.gz"
        pin_path = root / "pin.json"
        with contextlib.redirect_stdout(io.StringIO()):
            artifact.pack(argparse.Namespace(build=build, archive=archive, pin=pin_path))
        return build, archive, json.loads(pin_path.read_text()), pin_path

    def test_corrupt_archive_or_runtime_is_never_installed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build, archive, pin, _ = self.fixture(root)
            installed = artifact.install(archive, pin, root / "cache")
            self.assertTrue(installed.is_dir())
            (installed / artifact.RUNTIME / "rust_verify").write_bytes(b"forged executable")
            with self.assertRaises(ValueError):
                artifact.install(archive, pin, root / "cache")
            with archive.open("ab") as file:
                file.write(b"changed")
            with self.assertRaises(ValueError):
                artifact.install(archive, pin, root / "different-cache")

    def test_archive_path_escape_is_rejected_even_with_matching_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive, pin, _ = self.fixture(root)
            archive.unlink()
            with tarfile.open(archive, "w:gz") as output:
                member = tarfile.TarInfo("../../escaped")
                member.size = 1
                output.addfile(member, io.BytesIO(b"x"))
            pin["archive_sha256"] = artifact.sha(archive)
            with self.assertRaises(ValueError):
                artifact.install(archive, pin, root / "cache")
            self.assertFalse((root / "escaped").exists())

    def test_stale_source_patch_is_rejected_even_if_archive_is_valid(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, pin, pin_path = self.fixture(root)
            pin["patch_sha256"] = "older-patch"
            pin_path.write_text(json.dumps(pin))
            with self.assertRaises(ValueError):
                artifact.committed_pin(pin_path)


if __name__ == "__main__":
    unittest.main()
