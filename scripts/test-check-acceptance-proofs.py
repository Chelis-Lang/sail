#!/usr/bin/env python3
"""Failure controls for the direct stock-Verus acceptance-model checker."""

import copy
import hashlib
import importlib.util
import json
import os
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
        self.entry = {"source": "proofs/acceptance/model.rs", "verified": 2, "functions": ["model::first", "model::second"]}
        self.document = {
            "verus": {"commit": "stock-commit", "version": "stock-release"},
            "verification-results": {"success": True, "verified": 2, "errors": 0, "encountered-error": False, "encountered-vir-error": False, "is-verifying-entire-crate": True},
            "func-details": {"model::first": {}, "model::second": {}, "vstd::trusted": {}},
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
        del candidate["func-details"]["model::second"]
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

    def test_missing_extra_duplicate_and_ambiguous_roots(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            directory = root / "proofs/acceptance"
            directory.mkdir(parents=True)
            manifest = {"schema": "sail-acceptance-proofs-v1", "components": []}
            for name in sorted(CHECKER.COMPONENTS):
                source = directory / (name + ".rs")
                source.write_text("required_check")
                manifest["components"].append({"id": name, "source": str(source.relative_to(root)), "verified": 1, "functions": ["lemma"], "negative_controls": [{"name": "remove-check", "before": "required_check", "after": "false"}]})
            CHECKER.inventory(root, manifest)
            for changed in [manifest["components"][:-1], [manifest["components"][0]] * 3]:
                with self.assertRaises(CHECKER.Refusal):
                    CHECKER.inventory(root, {**manifest, "components": changed})
            extra = directory / "hidden.rs"
            extra.write_text("proof")
            with self.assertRaises(CHECKER.Refusal):
                CHECKER.inventory(root, manifest)
            extra.unlink()
            for dependency in ['#[path="/tmp/untracked.rs"] mod helper;', 'include!("/tmp/untracked.rs");', 'use std::include_str as hidden;', 'macro_rules! hidden { () => {} }']:
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
