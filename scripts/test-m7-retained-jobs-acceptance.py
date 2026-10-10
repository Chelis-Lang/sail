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
                  "report": {"crates": [{"functions": [{"function": "demo::f", "verdict": "verified", "tier": "confirmed"}]}]}}
        full = {"currentness": "current", "error": None, "exit_code": 0, "targets": [target]}
        return {"job": {"state": "completed", "currentness": "current"}, "result": full,
                "repair": {"accepted": True, "batch": {"full": full, "batched": True}}}

    def admitted(self, document):
        return harness.selected(document, "demo", "demo", "lib", "demo::f")

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
