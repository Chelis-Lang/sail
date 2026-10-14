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
                    "features": ["sail-fork:" + artifact.fork_commit(),
                                 "host:x86_64-unknown-linux-gnu", "source-lock:" + artifact.sha(artifact.REPO / "engine/verus/Cargo.lock"),
                                 artifact.VSTD_BUILD]}
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

    def test_stale_fork_commit_is_rejected_even_if_archive_is_valid(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, pin, pin_path = self.fixture(root)
            pin["fork_commit"] = "0" * 40
            pin_path.write_text(json.dumps(pin))
            with self.assertRaises(ValueError):
                artifact.committed_pin(pin_path)

    def test_pin_records_the_runtime_verus_features(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive, pin, pin_path = self.fixture(root)
            self.assertEqual(pin["verus_features"], [])
            claimed = dict(pin, verus_features=["singular"])
            with self.assertRaises(ValueError):
                artifact.install(archive, claimed, root / "cache")
            del pin["verus_features"]
            pin_path.write_text(json.dumps(pin))
            with self.assertRaises(ValueError):
                artifact.committed_pin(pin_path)
        committed = json.loads((artifact.REPO / "engine/verus/artifact-linux.json").read_text())
        self.assertEqual(committed["verus_features"], [])

    def test_runtime_is_fetched_from_the_public_fork_release(self):
        committed = artifact.committed_pin(artifact.REPO / "engine/verus/artifact-linux.json")
        self.assertTrue(artifact.asset_url(committed).startswith(
            "https://github.com/" + artifact.fork_repo() + "/releases/download/engine-runtime-"
            + committed["fork_commit"][:8] + "-"))
        for field, value in [("repo", "Chelis-Lang/sail"), ("repo", "attacker/verus"),
                             ("release_tag", "engine-runtime-00000000-linux"),
                             ("release_tag", "engine-runtime-" + committed["fork_commit"][:8] + "-linux/../x"),
                             ("asset", "../engine.tar.gz"), ("asset", "a?b=c")]:
            with self.assertRaises(ValueError, msg=field + "=" + value):
                artifact.asset_url(dict(committed, **{field: value}))

    def test_pack_names_the_fork_release_for_the_commit(self):
        with tempfile.TemporaryDirectory() as directory:
            _, archive, pin, _ = self.fixture(Path(directory))
            self.assertEqual(pin["repo"], artifact.fork_repo())
            self.assertEqual(pin["asset"], archive.name)
            self.assertEqual(artifact.asset_url(pin), "https://github.com/" + artifact.fork_repo()
                             + "/releases/download/engine-runtime-" + artifact.fork_commit()[:8]
                             + "-linux/" + archive.name)


if __name__ == "__main__":
    unittest.main()
