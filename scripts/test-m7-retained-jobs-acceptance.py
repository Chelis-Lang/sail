#!/usr/bin/env python3
"""Harness admission regressions; do not substitute for real project acceptance."""

import copy
import importlib.util
import io
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

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


class RpcDeadlineTests(unittest.TestCase):
    """Check the actual collector without waiting or manufacturing proof evidence."""

    def acceptance(self):
        acceptance = harness.Acceptance.__new__(harness.Acceptance)
        acceptance.deadline = 120
        acceptance.request_id = 0
        acceptance.pending = bytearray()
        acceptance.operations = []
        acceptance.mcp = SimpleNamespace(stdin=io.BytesIO(), stdout=SimpleNamespace(fileno=lambda: 0))
        acceptance.retain = lambda *_: None
        return acceptance

    def test_mcp_response_uses_rpc_bound_instead_of_cli_bound(self):
        acceptance = self.acceptance()
        waits = []

        def readable(read, write, errors, timeout):
            waits.append(timeout)
            return read, write, errors

        with patch.object(harness.time, "monotonic", return_value=0), \
                patch.object(harness.select, "select", side_effect=readable), \
                patch.object(harness.os, "read", return_value=b'{"id":1,"result":{"ok":true}}\n'):
            self.assertEqual(acceptance.exchange("tools/call", {}), {"ok": True})
        self.assertEqual(waits, [harness.protocol.DEADLINE])
        self.assertEqual(acceptance.operations[0]["elapsed_s"], 0)

    def test_partial_frame_cannot_restart_the_rpc_deadline(self):
        acceptance = self.acceptance()
        clock = [0]
        waits = []
        reads = [0]

        def readable(read, write, errors, timeout):
            waits.append(timeout)
            return read, write, errors

        def partial(_fd, _size):
            clock[0] = harness.protocol.DEADLINE + .01
            reads[0] += 1
            # A broken collector must fail the regression promptly rather than
            # loop forever on an artificial stream that never advances.
            return b'{"id":1,' if reads[0] == 1 else b'"result":{}}\n'

        with patch.object(harness.time, "monotonic", side_effect=lambda: clock[0]), \
                patch.object(harness.select, "select", side_effect=readable), \
                patch.object(harness.os, "read", side_effect=partial):
            with self.assertRaisesRegex(AssertionError, "five-second RPC bound"):
                acceptance.exchange("tools/call", {})
        self.assertEqual(waits, [harness.protocol.DEADLINE])

    def test_decoding_is_inside_the_same_rpc_deadline(self):
        acceptance = self.acceptance()
        clock = [0]
        original_loads = harness.json.loads

        def delayed_decode(raw):
            clock[0] = harness.protocol.DEADLINE + .01
            return original_loads(raw)

        with patch.object(harness.time, "monotonic", side_effect=lambda: clock[0]), \
                patch.object(harness.select, "select", side_effect=lambda read, write, errors, _timeout: (read, write, errors)), \
                patch.object(harness.os, "read", return_value=b'{"id":1,"result":{}}\n'), \
                patch.object(harness.json, "loads", side_effect=delayed_decode):
            with self.assertRaisesRegex(AssertionError, "decoding exceeded original five-second RPC bound"):
                acceptance.exchange("tools/call", {})


if __name__ == "__main__":
    unittest.main()
