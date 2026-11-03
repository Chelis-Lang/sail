#!/usr/bin/env python3
"""Bounded publication controls; no builds, registry requests or real tokens."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
from urllib.error import HTTPError


SPEC = importlib.util.spec_from_file_location("publish_crates", Path(__file__).with_name("publish-crates.py"))
PUBLISH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISH)


def metadata():
    packages = [
        {"id": "cli", "name": "sail-solve", "version": "0.2.0-rc.1", "publish": ["crates-io"]},
        {"id": "library", "name": "sail-api", "version": "0.2.0-rc.1", "publish": ["crates-io"]},
        {"id": "tool", "name": "sail-bench", "version": "0.2.0-rc.1", "publish": []},
        {"id": "external", "name": "third-party", "version": "1.0.0", "publish": None},
    ]
    return {"packages": packages, "workspace_members": ["cli", "library", "tool"], "target_directory": "/unused"}


class PublicationControls(unittest.TestCase):
    def test_selection_excludes_tooling_and_external_packages(self):
        packages, version = PUBLISH.select_packages(metadata(), "0.2.0-rc.1")
        self.assertEqual([p["name"] for p in packages], ["sail-api", "sail-solve"])
        self.assertEqual(version, "0.2.0-rc.1")

    def test_selection_requires_root_uniform_version_and_explicit_allowlist(self):
        for field, value in [("name", "sail-other"), ("version", "0.3.0"), ("publish", None)]:
            with self.subTest(field=field):
                data = metadata()
                data["packages"][0][field] = value
                with self.assertRaises(ValueError):
                    PUBLISH.select_packages(data)
        with self.assertRaises(ValueError):
            PUBLISH.select_packages(metadata(), "0.2.0")

    def test_preparation_does_not_receive_registry_tokens(self):
        with patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": "fake-secret", "CARGO_REGISTRIES_CRATES_IO_TOKEN": "other-fake-secret"}):
            with patch.object(PUBLISH.subprocess, "run", return_value=Mock(stdout="ok")) as run:
                PUBLISH.run(["cargo", "metadata"], capture=True)
        environment = run.call_args.kwargs["env"]
        self.assertNotIn("CARGO_REGISTRY_TOKEN", environment)
        self.assertNotIn("CARGO_REGISTRIES_CRATES_IO_TOKEN", environment)
        self.assertIs(run.call_args.kwargs["stdin"], PUBLISH.subprocess.DEVNULL)

    def test_dirty_source_is_rejected(self):
        with patch.object(PUBLISH, "run", return_value=" M Cargo.toml\n"):
            with self.assertRaisesRegex(ValueError, "clean Git"):
                PUBLISH.clean_commit()

    def test_dry_run_verifies_without_reading_token_file(self):
        def run(arguments, capture=False):
            return json.dumps(metadata()) if arguments[:2] == ["cargo", "metadata"] else ""

        packages, _ = PUBLISH.select_packages(metadata())
        with patch.object(PUBLISH, "run", side_effect=run) as commands, \
             patch.object(PUBLISH, "clean_commit", return_value="same-commit"), \
             patch.object(PUBLISH, "pending_packages", return_value=packages), \
             patch.object(PUBLISH, "upload") as upload, \
             patch.object(Path, "read_text", side_effect=AssertionError("no token reads")), \
             contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(PUBLISH.main(["--dry-run", "--token-file", "/nonexistent", "--version", "0.2.0-rc.1"]), 0)
        upload.assert_not_called()
        arguments = [call.args[0] for call in commands.call_args_list]
        self.assertEqual(arguments[0][-2:], ["scripts/crate-resources.py", "check"])
        package_command = next(command for command in arguments if command[:2] == ["cargo", "package"])
        self.assertIn("--locked", package_command)
        self.assertIn("sail-solve", package_command)
        self.assertIn("sail-api", package_command)
        self.assertNotIn("--no-verify", package_command)
        self.assertNotIn("--allow-dirty", package_command)

    def test_resume_requires_matching_archive_checksum(self):
        package = {"name": "sail-api", "version": "0.2.0-rc.1"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "sail-api-0.2.0-rc.1.crate").write_bytes(b"verified archive")
            digest = PUBLISH.hashlib.sha256(b"verified archive").hexdigest()
            entry = {"name": "sail-api", "cksum": digest, "yanked": False}
            with patch.object(PUBLISH, "registry_version", return_value=entry), \
                 patch.object(PUBLISH.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(PUBLISH.pending_packages([package], root), [])
                entry["cksum"] = "different"
                with self.assertRaisesRegex(ValueError, "different package bytes"):
                    PUBLISH.pending_packages([package], root)
                entry["cksum"] = digest
                entry["yanked"] = True
                with self.assertRaisesRegex(ValueError, "yanked"):
                    PUBLISH.pending_packages([package], root)

    def test_registry_throttling_honors_retry_after(self):
        entry = {"name": "sail-api", "vers": "0.2.0-rc.1", "cksum": "hash", "yanked": False}
        error = HTTPError("https://index.crates.io/sa/il/sail-api", 429, "rate limit", {"Retry-After": "7"}, None)
        response = io.BytesIO(json.dumps(entry).encode() + b"\n")
        with patch.object(PUBLISH, "urlopen", side_effect=[error, response]) as open_url, \
             patch.object(PUBLISH.time, "sleep") as sleep, contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(PUBLISH.registry_version("sail-api", "0.2.0-rc.1"), entry)
        sleep.assert_called_once_with(7)
        self.assertNotIn("Authorization", dict(open_url.call_args.args[0].header_items()))

    def test_missing_registry_package_is_pending(self):
        error = HTTPError("https://index.crates.io/sa/il/sail-api", 404, "missing", {}, None)
        with patch.object(PUBLISH, "urlopen", side_effect=error):
            self.assertIsNone(PUBLISH.registry_version("sail-api", "0.2.0-rc.1"))

    def test_upload_token_is_only_in_environment_and_output_is_redacted(self):
        token = "fake-publish-secret"
        process = Mock()
        process.stdout = [f"unexpected diagnostic containing {token}\n"]
        process.wait.return_value = 0
        popen = Mock()
        popen.return_value.__enter__ = Mock(return_value=process)
        popen.return_value.__exit__ = Mock(return_value=False)
        output = io.StringIO()
        with patch.dict(os.environ, {"CARGO_REGISTRY_TOKEN": token}), \
             patch.object(PUBLISH.subprocess, "Popen", popen), contextlib.redirect_stdout(output):
            PUBLISH.upload(["cargo", "publish", "-p", "sail-api"], None)
        self.assertNotIn(token, repr(popen.call_args.args))
        self.assertEqual(popen.call_args.kwargs["env"]["CARGO_REGISTRY_TOKEN"], token)
        self.assertNotIn(token, output.getvalue())
        self.assertIn("[REDACTED]", output.getvalue())


if __name__ == "__main__":
    unittest.main()
