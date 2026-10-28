#!/usr/bin/env python3
"""Corrupt or stale optional-engine artifacts must never become available."""

import argparse
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tarfile
import tempfile
import subprocess
import sys
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("artifact", Path(__file__).with_name("engine-artifact.py"))
artifact = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifact)


class ArtifactTests(unittest.TestCase):
    def test_runtime_scan_rejects_early_late_and_cross_chunk_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / "runtime"
            for payload in [b"/Users/probe\n" + b"ordinary\n" * 400000,
                            b"x" * 65534 + b"/home/probe",
                            b"x" * 131072 + b"private-host"]:
                with self.subTest(length=len(payload)):
                    file.write_bytes(payload)
                    with self.assertRaisesRegex(ValueError, "forbidden runtime token"):
                        artifact.scan_paths(root, [b"/Users/", b"/home/", b"private-host"])

    def test_runtime_scan_accepts_clean_files_and_refuses_unknown_reads(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "nested").mkdir()
            (root / "nested/runtime").write_bytes(b"ordinary runtime\x00bytes")
            artifact.scan_paths(root, [b"/Users/", b"/home/"])
            with mock.patch.object(Path, "open", side_effect=OSError("unreadable file")):
                with self.assertRaises(OSError):
                    artifact.scan_paths(root, [b"/Users/"])
            with mock.patch.object(os, "scandir", side_effect=OSError("unreadable directory")):
                with self.assertRaises(OSError):
                    artifact.scan_paths(root, [b"/Users/"])
            (root / "link").symlink_to(root / "nested/runtime")
            with self.assertRaisesRegex(ValueError, "nonregular"):
                artifact.scan_paths(root, [b"/Users/"])
            with self.assertRaises(OSError):
                artifact.scan_paths(root / "absent", [b"/Users/"])

    def test_runtime_scan_cli_rejects_large_matches_and_configured_tokens(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / "runtime"
            command = [sys.executable, str(Path(__file__).with_name("engine-artifact.py")),
                       "scan", "--root", str(root)]
            for token in [b"/Users/probe", b"private-host"]:
                file.write_bytes(token + b"\n" + b"ordinary\n" * 400000)
                with mock.patch.dict(os.environ, {"SAIL_SCAN_TOKENS": "private-host"}):
                    output = subprocess.run(command, capture_output=True)
                self.assertEqual(output.returncode, 1)
                self.assertIn(b"SAIL0944", output.stderr)
                self.assertIn(b"forbidden runtime token", output.stderr)

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

    def test_local_rust_src_paths_in_vstd_are_never_packed(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build, _, _, _ = self.fixture(root)
            vir = build / artifact.RUNTIME / "vstd.vir"
            vir.write_bytes(b"/rustc/58cf8f9c35b355199727627d07ae34a4e2846d67/library/core/src/ops/bit.rs"
                            b"/opt/rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library/core/src/ops/bit.rs")
            manifest = json.loads((build / "sail-build.json").read_text())
            manifest["artifacts"][str(artifact.RUNTIME / "vstd.vir")] = artifact.sha(vir)
            (build / "sail-build.json").write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, "rust-src"), contextlib.redirect_stdout(io.StringIO()):
                artifact.pack(argparse.Namespace(build=build, archive=root / "again.tar.gz", pin=root / "again.json"))

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

    def test_pin_binds_the_runtime_manifest_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            build, archive, pin, pin_path = self.fixture(root)
            self.assertEqual(pin["manifest_sha256"], artifact.sha(build / "sail-build.json"))
            with self.assertRaises(ValueError):
                artifact.install(archive, dict(pin, manifest_sha256="0" * 64), root / "cache")
            del pin["manifest_sha256"]
            pin_path.write_text(json.dumps(pin))
            with self.assertRaises(ValueError):
                artifact.committed_pin(pin_path)
        for name in ("linux", "macos"):
            artifact.committed_pin(artifact.REPO / ("engine/verus/artifact-" + name + ".json"))

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

    def test_a_runtime_is_bound_to_its_platform_host(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive, pin, _ = self.fixture(root)
            self.assertEqual(pin["release_tag"], "engine-runtime-" + artifact.fork_commit()[:8] + "-linux")
            # The Linux runtime cannot be installed under a macOS pin.
            with self.assertRaises(ValueError):
                artifact.install(archive, dict(pin, release_tag=pin["release_tag"][:-5] + "macos"), root / "cache")
        for name, host in (("linux", "x86_64-unknown-linux-gnu"), ("macos", "aarch64-apple-darwin")):
            committed = json.loads((artifact.REPO / ("engine/verus/artifact-" + name + ".json")).read_text())
            self.assertEqual(artifact.pin_host(committed), host)


if __name__ == "__main__":
    unittest.main()
