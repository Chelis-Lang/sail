#!/usr/bin/env python3
"""Bounded publication controls; no builds, registry requests or real tokens."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import textwrap
import unittest
from unittest.mock import Mock, patch
from urllib.error import HTTPError


SPEC = importlib.util.spec_from_file_location("publish_crates", Path(__file__).with_name("publish-crates.py"))
PUBLISH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PUBLISH)
WORKFLOW = Path(__file__).resolve().parents[1] / ".github/workflows/publish-crates.yml"


def workflow_code(name):
    """Execute the workflow's actual bounded Python controls without YAML dependencies."""
    source = WORKFLOW.read_text().split(f"# BEGIN {name}\n", 1)[1].split(f"# END {name}", 1)[0]
    return compile(textwrap.dedent(source), str(WORKFLOW) + ":" + name, "exec")


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


class ReleaseSourceControls(unittest.TestCase):
    sha = "1" * 40
    other = "2" * 40
    tag = "v0.2.0-rc.1"

    def resolve(self, event="release", commit=None, target=None, release=None, reference=None, tags=None):
        """Run pre-checkout code against an exact mocked GitHub response sequence."""
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "output"
            environment = {
                "SAIL_RELEASE_TAG": self.tag, "GITHUB_EVENT_NAME": event,
                "GITHUB_SHA": self.sha, "GITHUB_REPOSITORY": "Chelis-Lang/sail",
                "GITHUB_OUTPUT": str(output),
            }
            if commit is not None:
                environment["SAIL_QUALIFIED_COMMIT"] = commit
            responses = [
                release if release is not None else {"tagName": self.tag, "isDraft": False, "publishedAt": "2026-11-03T00:00:00Z"},
                {"ref": reference or "refs/tags/" + self.tag,
                 "object": target or {"type": "commit", "sha": self.sha}},
            ] + [{"object": item} for item in (tags or [])]
            with patch.dict(os.environ, environment, clear=True), \
                 patch("subprocess.run", side_effect=[Mock(stdout=json.dumps(item)) for item in responses]) as run:
                self.requests = run
                exec(workflow_code("crate-release-source"), {})
            return output.read_text()

    def test_release_uses_event_commit_and_ignores_dispatch_input(self):
        self.assertEqual(self.resolve(commit=self.other), "sha=" + self.sha + "\n")
        self.assertEqual(self.requests.call_args_list[1].args[0][-1],
                         "repos/Chelis-Lang/sail/git/ref/tags/" + self.tag)

    def test_dispatch_requires_matching_explicit_qualified_commit(self):
        self.assertEqual(self.resolve(event="workflow_dispatch", commit=self.sha), "sha=" + self.sha + "\n")
        with self.assertRaisesRegex(SystemExit, "tag and qualified commit differ"):
            self.resolve(event="workflow_dispatch", commit=self.other)

    def test_missing_or_malformed_dispatch_commit_fails_before_network(self):
        for commit in [None, "", "1" * 39, "1" * 41, "main", "refs/tags/" + self.tag, "g" * 40, " " + self.sha]:
            with self.subTest(commit=commit), self.assertRaisesRegex(SystemExit, "full qualified commit"):
                self.resolve(event="workflow_dispatch", commit=commit)
            self.requests.assert_not_called()

    def test_retargeted_release_tag_is_rejected(self):
        with self.assertRaisesRegex(SystemExit, "tag and qualified commit differ"):
            self.resolve(target={"type": "commit", "sha": self.other})

    def test_annotated_and_nested_tags_resolve_to_qualified_commit(self):
        self.assertEqual(self.resolve(target={"type": "tag", "sha": self.other},
                                     tags=[{"type": "commit", "sha": self.sha}]), "sha=" + self.sha + "\n")
        self.assertEqual(self.resolve(target={"type": "tag", "sha": self.other},
                                     tags=[{"type": "tag", "sha": "3" * 40},
                                           {"type": "commit", "sha": self.sha}]), "sha=" + self.sha + "\n")

    def test_invalid_tag_targets_are_rejected(self):
        for target in [{"type": "tree", "sha": self.sha}, {"type": "commit", "sha": "bad"}]:
            with self.subTest(target=target), self.assertRaises(SystemExit):
                self.resolve(target=target)
        with self.assertRaisesRegex(SystemExit, "cyclic"):
            self.resolve(target={"type": "tag", "sha": self.other},
                         tags=[{"type": "tag", "sha": self.other}])
        with self.assertRaisesRegex(SystemExit, "nesting"):
            self.resolve(target={"type": "tag", "sha": f"{2:040x}"},
                         tags=[{"type": "tag", "sha": f"{number:040x}"} for number in range(3, 19)])

    def test_draft_unpublished_or_wrong_release_is_rejected(self):
        for release in [{"tagName": self.tag, "isDraft": True, "publishedAt": "date"},
                        {"tagName": self.tag, "isDraft": False, "publishedAt": None},
                        {"tagName": "v0.2.0", "isDraft": False, "publishedAt": "date"}]:
            with self.subTest(release=release), self.assertRaisesRegex(SystemExit, "published release"):
                self.resolve(release=release)
        with self.assertRaisesRegex(SystemExit, "selected release tag"):
            self.resolve(reference="refs/heads/main")

    def checkout(self, head=None, version="0.2.0-rc.1"):
        with tempfile.TemporaryDirectory() as directory, contextlib.chdir(directory):
            Path("Cargo.toml").write_text('[workspace.package]\nversion = "' + version + '"\n')
            Path("rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.0"\nprofile = "minimal"\ncomponents = ["clippy"]\n')
            with patch.dict(os.environ, {"SAIL_SOURCE_SHA": self.sha, "SAIL_RELEASE_TAG": self.tag}, clear=True), \
                 patch("subprocess.check_output", return_value=(head or self.sha) + "\n"), \
                 patch("subprocess.run") as run:
                self.toolchain = run
                exec(workflow_code("crate-release-checkout"), {})

    def test_checkout_matches_source_and_prerelease_version_before_rustup(self):
        self.checkout()
        self.toolchain.assert_called_once_with(["rustup", "toolchain", "install", "1.99.0", "--profile", "minimal", "--component", "clippy"], check=True)
        for arguments, message in [({"head": self.other}, "checkout and qualified commit"),
                                   ({"version": "0.2.0"}, "workspace version")]:
            with self.subTest(arguments=arguments), self.assertRaisesRegex(SystemExit, message):
                self.checkout(**arguments)
            self.toolchain.assert_not_called()

    def test_workflow_checks_out_and_rechecks_resolved_commit(self):
        source = WORKFLOW.read_text()
        self.assertIn("ref: ${{ steps.source.outputs.sha }}", source)
        self.assertIn("SAIL_SOURCE_SHA: ${{ steps.source.outputs.sha }}", source)
        self.assertNotIn("ref: refs/tags/", source)


if __name__ == "__main__":
    unittest.main()
