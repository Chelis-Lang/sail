#!/usr/bin/env python3
"""Harness admission regressions; do not substitute for real project acceptance."""

import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("retained_acceptance", Path(__file__).with_name("m7-retained-jobs-acceptance.py"))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)


class EvidenceAdmissionTests(unittest.TestCase):
    def document(self):
        target = {"requested": True, "target": "demo", "kind": ["lib"], "complete": True,
                  "package_id": "path+file:///fixture#demo@0.0.0",
                  "report": {"error": None, "exit_code": 0, "crates": [{"verus": {"errors": 0, "success": True, "encountered_vir_error": False}, "functions": [{"function": "demo::f", "verdict": "verified", "tier": "confirmed"}]}]}}
        full = {"input_revision": "revision", "currentness": "current", "error": None, "exit_code": 0, "targets": [target]}
        return {"job": {"state": "completed", "currentness": "current", "error": None, "input_revision": "revision"}, "result": full,
                "repair": {"accepted": True, "batch": {"full": full, "batched": True}}}

    def admitted(self, document):
        return harness.selected(document, "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")

    def test_requires_independent_current_full_target(self):
        original = self.document()
        self.assertIs(self.admitted(original)[0], original["result"])
        mutations = [("state", "cancelled"), ("currentness", "historical")]
        for key, value in mutations:
            document = copy.deepcopy(original)
            document["job"][key] = value
            with self.assertRaises(AssertionError):
                self.admitted(document)
        for key, value in [("currentness", "unknown"), ("exit_code", 1), ("error", {"message": "broken"})]:
            document = copy.deepcopy(original)
            document["result"][key] = value
            with self.assertRaises(AssertionError):
                self.admitted(document)
        document = copy.deepcopy(original)
        document["result"]["targets"][0]["report"]["crates"][0]["functions"][0]["tier"] = "provisional"
        with self.assertRaises(AssertionError):
            self.admitted(document)
        document = copy.deepcopy(original)
        document["repair"]["accepted"] = False
        with self.assertRaises(AssertionError):
            self.admitted(document)

    def test_wrong_package_revision_and_job_errors_never_qualify(self):
        original = self.document()
        for key, value in [("input_revision", "different"), ("error", "SAIL0808: worker failed")]:
            document = copy.deepcopy(original)
            document["job"][key] = value
            with self.assertRaises(AssertionError):
                self.admitted(document)
        document = copy.deepcopy(original)
        document["result"]["targets"][0]["package_id"] = "path+file:///other#demo@0.0.0"
        with self.assertRaises(AssertionError):
            self.admitted(document)

    def test_incomplete_or_errorful_dependencies_never_qualify(self):
        original = self.document()
        for mutation in ("partial", "infra", "exit", "proof"):
            document = copy.deepcopy(original)
            dependency = copy.deepcopy(document["result"]["targets"][0])
            dependency["requested"] = False
            dependency["package_id"] = "dependency"
            if mutation == "partial":
                dependency["complete"] = False
            elif mutation == "infra":
                dependency["report"]["error"] = {"message": "broken"}
            elif mutation == "exit":
                dependency["report"]["exit_code"] = 1
            else:
                dependency["report"]["crates"][0]["verus"]["errors"] = 1
            document["result"]["targets"].append(dependency)
            with self.assertRaises(AssertionError):
                self.admitted(document)

    def test_package_identity_comes_from_actual_unique_metadata_scope(self):
        metadata = {"packages": [{"name": "demo", "id": "exact", "targets": [{"name": "demo", "kind": ["lib"]}]}]}
        self.assertEqual(harness.package_identity(metadata, "demo", "demo", "lib"), "exact")
        self.assertEqual(harness.package_identity(metadata, "exact", "demo", "lib"), "exact")
        for package, target, kind in [("other", "demo", "lib"), ("demo", "other", "lib"), ("demo", "demo", "bin")]:
            with self.assertRaises(AssertionError):
                harness.package_identity(metadata, package, target, kind)
        duplicate = copy.deepcopy(metadata)
        duplicate["packages"].append(dict(duplicate["packages"][0], id="different-source"))
        with self.assertRaises(AssertionError):
            harness.package_identity(duplicate, "demo", "demo", "lib")

    def test_same_name_lib_bin_and_partial_scopes_are_distinct(self):
        original = self.document()
        for key, value in [("kind", ["bin"]), ("target", "other"), ("complete", False)]:
            document = copy.deepcopy(original)
            document["result"]["targets"][0][key] = value
            with self.assertRaises(AssertionError):
                self.admitted(document)
        document = copy.deepcopy(original)
        document["result"]["targets"].append(copy.deepcopy(document["result"]["targets"][0]))
        with self.assertRaises(AssertionError):
            self.admitted(document)

    def test_exact_function_inventory_cannot_be_missing_or_ambiguous(self):
        original = self.document()
        for functions in ([], [{"function": "demo::other"}], [{"function": "demo::f", "verdict": "verified", "tier": "confirmed"}] * 2):
            document = copy.deepcopy(original)
            document["result"]["targets"][0]["report"]["crates"][0]["functions"] = functions
            with self.assertRaises(AssertionError):
                self.admitted(document)


if __name__ == "__main__":
    unittest.main()
