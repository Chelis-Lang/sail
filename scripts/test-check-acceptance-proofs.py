#!/usr/bin/env python3
"""Failure controls for the direct stock-Verus acceptance-model checker."""

import copy
import hashlib
import importlib.util
import json
import os
import subprocess
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location("acceptance_checker", Path(__file__).with_name("check-acceptance-proofs.py"))
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


class VerificationEvidence(unittest.TestCase):
    """A process exit or plausible result object alone cannot pass the gate."""

    def setUp(self):
        self.pins = {"verus": {"commit": "stock-commit", "release": "stock-release"}}
        self.entry = {"source": "proofs/acceptance/model.rs", "verified": 2, "functions": ["check_model::first", "check_model::second"]}
        self.document = {
            "verus": {"commit": "stock-commit", "version": "stock-release"},
            "verification-results": {"success": True, "verified": 2, "errors": 0, "encountered-error": False, "encountered-vir-error": False, "is-verifying-entire-crate": True},
            "func-details": {"check_model::first": {}, "check_model::second": {}, "vstd::trusted": {}},
        }

    def checked(self, document=None, code=0, broken=False):
        return CHECKER.checked_document(json.dumps(self.document if document is None else document), code, self.entry, self.pins, broken=broken)

    def test_complete_success_and_actual_verification_failure(self):
        self.checked()
        broken = copy.deepcopy(self.document)
        broken["verification-results"].update(success=False, verified=1, errors=1, **{"encountered-error": True})
        self.checked(broken, code=1, broken=True)

    def test_exit_success_does_not_replace_complete_evidence(self):
        for field, value in [("success", False), ("errors", 1), ("errors", False), ("verified", True), ("verified", 1), ("encountered-error", True), ("encountered-vir-error", True), ("is-verifying-entire-crate", False)]:
            with self.subTest(field=field):
                candidate = copy.deepcopy(self.document)
                candidate["verification-results"][field] = value
                with self.assertRaises(CHECKER.Refusal):
                    self.checked(candidate)

    def test_verifier_identity_and_function_omission(self):
        candidate = copy.deepcopy(self.document)
        candidate["verus"]["commit"] = "fork-commit"
        with self.assertRaises(CHECKER.Refusal):
            self.checked(candidate)
        candidate = copy.deepcopy(self.document)
        del candidate["func-details"]["check_model::second"]
        with self.assertRaises(CHECKER.Refusal):
            self.checked(candidate)

    def test_broken_control_cannot_pass_or_merely_fail_parsing(self):
        with self.assertRaises(CHECKER.Refusal):
            self.checked(broken=True)
        candidate = copy.deepcopy(self.document)
        candidate["verification-results"].update(success=False, verified=0, errors=0, **{"encountered-vir-error": True})
        with self.assertRaises(CHECKER.Refusal):
            self.checked(candidate, code=1, broken=True)

    def test_duplicate_and_incomplete_json_refused(self):
        for value in ['{"verification-results": {}, "verification-results": {}}', '{', json.dumps(self.document) + json.dumps(self.document)]:
            with self.subTest(value=value), self.assertRaises(CHECKER.Refusal):
                CHECKER.checked_document(value, 0, self.entry, self.pins, broken=False)

    def test_resource_failure_cannot_pass_a_negative_control(self):
        self.assertTrue(CHECKER.resource_failure("error: Resource limit (rlimit) exceeded; consider rerunning with --profile"))
        self.assertTrue(CHECKER.resource_failure("solver timed out"))
        self.assertFalse(CHECKER.resource_failure("error: assertion failed"))

    def test_signal_omitted_functions_and_bad_negative_counters_refused(self):
        candidate = copy.deepcopy(self.document)
        candidate["verification-results"].update(success=False, verified=1, errors=1, **{"encountered-error": True})
        with self.assertRaises(CHECKER.Refusal):
            self.checked(candidate, code=-9, broken=True)
        changed = copy.deepcopy(candidate)
        changed["func-details"] = {}
        with self.assertRaises(CHECKER.Refusal):
            self.checked(changed, code=1, broken=True)
        for field, value in [("encountered-error", False), ("errors", True), ("verified", 0)]:
            changed = copy.deepcopy(candidate)
            changed["verification-results"][field] = value
            with self.subTest(field=field), self.assertRaises(CHECKER.Refusal):
                self.checked(changed, code=1, broken=True)


class SourceInventory(unittest.TestCase):
    """Deleting an expected component or adding hidden proof sources fails."""

    def test_composition_uses_only_the_complete_captured_closure(self):
        paths = CHECKER.dependency_paths("composition")
        snapshots = {path: ("captured_" + Path(path).stem).encode() for path in paths}
        entry = {"id": "composition", "source": "proofs/acceptance/composition.rs", "dependencies": paths}
        name, driver, files = CHECKER.captured_driver(entry, snapshots, b"mutated_composition")
        self.assertEqual(name, "check_composition.rs")
        self.assertEqual(set(files), {Path(path).name for path in paths})
        self.assertEqual(files["composition.rs"], b"mutated_composition")
        self.assertEqual(files["context.rs"], snapshots[CHECKER.SUPPORT])
        self.assertIn(b'#[path="query_association.rs"] pub mod query_association;', driver)
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.captured_driver({**entry, "dependencies": paths[:-1]}, snapshots, b"candidate")
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.captured_driver(entry, snapshots, b'include!("/untracked.rs");')
        changed = {**snapshots, CHECKER.SUPPORT: b'#[path="/untracked.rs"] mod hidden;'}
        with self.assertRaises(CHECKER.Refusal):
            CHECKER.captured_driver(entry, changed, b"candidate")

    def test_missing_extra_duplicate_and_ambiguous_roots(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            directory = root / "proofs/acceptance"
            directory.mkdir(parents=True)
            manifest = {"schema": "sail-acceptance-proofs-v2", "support_sources": [CHECKER.SUPPORT], "components": []}
            (root / CHECKER.SUPPORT).write_text("shared_context")
            for name in sorted(CHECKER.COMPONENTS):
                source = directory / (CHECKER.COMPONENTS[name] + ".rs")
                source.write_text("required_check")
                manifest["components"].append({"id": name, "source": str(source.relative_to(root)), "dependencies": CHECKER.dependency_paths(name), "verified": 1, "functions": ["lemma"], "negative_controls": [{"name": "remove-check", "before": "required_check", "after": "false"}]})
            CHECKER.inventory(root, manifest)
            for changed in [manifest["components"][:-1], [manifest["components"][0]] * 3]:
                with self.assertRaises(CHECKER.Refusal):
                    CHECKER.inventory(root, {**manifest, "components": changed})
            extra = directory / "hidden.rs"
            extra.write_text("proof")
            with self.assertRaises(CHECKER.Refusal):
                CHECKER.inventory(root, manifest)
            extra.unlink()
            for dependency in ['#[path="/tmp/untracked.rs"] mod helper;', 'include!("/tmp/untracked.rs");', 'use std::include_str as hidden;', 'macro_rules! hidden { () => {} }', 'env!("HOME")', 'option_env!("USER")', 'use std::env as hidden;']:
                source.write_text("required_check\n" + dependency)
                with self.subTest(dependency=dependency), self.assertRaises(CHECKER.Refusal):
                    CHECKER.inventory(root, manifest)
            source.write_text("required_check required_check")
            with self.assertRaises(CHECKER.Refusal):
                CHECKER.inventory(root, manifest)
            source.write_text("required_check")
            mutation = copy.deepcopy(manifest)
            mutation["components"][0]["negative_controls"][0]["after"] = 'include!("/tmp/hidden.rs");'
            with self.assertRaises(CHECKER.Refusal):
                CHECKER.inventory(root, mutation)


class CanonicalGitObjects(unittest.TestCase):
    def test_replace_refs_cannot_change_the_bytes_of_a_selected_commit(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            commits = []
            for content in ("original", "replacement"):
                (root / "proof.rs").write_text(content)
                subprocess.run(["git", "-C", str(root), "add", "proof.rs"], check=True)
                subprocess.run(["git", "-C", str(root), "-c", "user.name=Proof Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "-qm", content], check=True)
                commits.append(CHECKER.git(root, "rev-parse", "HEAD"))
            subprocess.run(["git", "-C", str(root), "replace", commits[0], commits[1]], check=True)
            self.assertEqual(subprocess.check_output(["git", "-C", str(root), "show", commits[0] + ":proof.rs"]), b"replacement")
            self.assertEqual(CHECKER.git_bytes(root, "show", commits[0] + ":proof.rs"), b"original")


class RuntimeIdentity(unittest.TestCase):
    """These fixtures test identity refusal, not proof-checker qualification."""

    def test_changed_verifier_solver_or_unlisted_file_refused(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            install = root / "stock"
            install.mkdir()
            archive = root / "stock.zip"
            files = {"verus": b"launcher", "rust_verify": b"checker", "z3": b"solver"}
            with zipfile.ZipFile(archive, "w") as bundle:
                for name, content in files.items():
                    bundle.writestr("stock/" + name, content)
                    (install / name).write_bytes(content)
            asset = {"sha256": CHECKER.digest(archive), "dir": "stock", "z3_sha256": hashlib.sha256(files["z3"]).hexdigest()}
            CHECKER.validated_runtime(archive, install, asset)
            for name in files:
                (install / name).write_bytes(b"replacement")
                with self.subTest(name=name), self.assertRaises(CHECKER.Refusal):
                    CHECKER.validated_runtime(archive, install, asset)
                (install / name).write_bytes(files[name])
            (install / "extra").write_bytes(b"unlisted")
            with self.assertRaises(CHECKER.Refusal):
                CHECKER.validated_runtime(archive, install, asset)

    def test_injected_arguments_and_loader_state_are_not_inherited(self):
        with patch.dict(os.environ, {"SAIL_VERUS_ARGS": "--no-verify", "RUSTFLAGS": "injected", "RUSTC_WRAPPER": "replacement", "VERUS_Z3_PATH": "shim", "LD_PRELOAD": "replacement"}):
            env = CHECKER.proof_environment(Path("/validated-stock"), "pinned-rust")
        self.assertEqual(env["VERUS_Z3_PATH"], "/validated-stock/z3")
        self.assertEqual(env["RUSTUP_TOOLCHAIN"], "pinned-rust")
        self.assertTrue({"SAIL_VERUS_ARGS", "RUSTFLAGS", "RUSTC_WRAPPER", "LD_PRELOAD"}.isdisjoint(env))


if __name__ == "__main__":
    unittest.main()
