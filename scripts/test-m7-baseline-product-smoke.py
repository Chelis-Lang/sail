#!/usr/bin/env python3
"""Pure binding and full-evidence controls; no service, compiler or proof actor."""

import copy
import json
from pathlib import Path
import types
import unittest

PATH = Path(__file__).with_name("m7-baseline-product-smoke.py")
smoke = types.ModuleType("baseline_product_smoke")
smoke.__file__ = str(PATH)
exec(compile(PATH.read_bytes(), str(PATH), "exec"), smoke.__dict__)


def fixture():
    result = {"kind": "sail-project-result", "schema_version": 1,
              "currentness": "current", "run_id": "run", "workspace": "workspace",
              "context_sha256": "context", "input_revision": "input", "exit_code": 0,
              "targets": [{"complete": True, "establishing_run": "run", "report": {"provenance": "exact"}}],
              "execution": {"compiler_invocations": 1}, "error": None}
    good = {"result": result, "repair": {"accepted": True}}
    historical = copy.deepcopy(result)
    historical["currentness"] = "historical"
    document = {"response": "baseline", "value": {"version": 1, "name": smoke.NAME,
                "job_id": "job", "result": historical}}
    return good, document


class Bindings(unittest.TestCase):
    def test_exact_original_restored_by_removing_one_hook(self):
        original = (smoke.REPO / "ci/test-repair.py").read_bytes()
        derived = smoke.derive(original)
        self.assertEqual(derived.count(smoke.HOOK), 1)
        self.assertEqual(derived.replace(smoke.HOOK, b"", 1), original)
        compile(derived, "derived-test-repair.py", "exec")

    def test_changed_source_is_refused(self):
        original = (smoke.REPO / "ci/test-repair.py").read_bytes()
        with self.assertRaisesRegex(AssertionError, "bytes changed"):
            smoke.derive(original + b"\n")

    def test_missing_or_duplicate_anchor_is_refused(self):
        original = (smoke.REPO / "ci/test-repair.py").read_bytes()
        for changed in (original.replace(smoke.ANCHOR, b""), original + smoke.ANCHOR):
            with self.assertRaisesRegex(AssertionError, "anchor changed"):
                smoke.derive(changed)

    def test_entire_provenance_and_complete_historical_evidence_required(self):
        good, document = fixture()
        self.assertEqual(smoke.baseline(document, "job", good), document)
        for field, value in (("currentness", "current"), ("input_revision", "other"),
                             ("targets", [{"complete": False}]),
                             ("targets", [{"complete": True, "report": {"provenance": "other"}}])):
            changed = copy.deepcopy(document)
            changed["value"]["result"][field] = value
            with self.assertRaises(AssertionError):
                smoke.baseline(changed, "job", good)

    def test_wrong_job_or_name_is_refused(self):
        good, document = fixture()
        for field in ("job_id", "name"):
            changed = copy.deepcopy(document)
            changed["value"][field] = "other"
            with self.assertRaises(AssertionError):
                smoke.baseline(changed, "job", good)

    def test_structured_and_full_text_must_match(self):
        _, document = fixture()
        tool = {"isError": False, "resultType": "complete", "structuredContent": document,
                "content": [{"text": "summary"}, {"text": json.dumps(document)}]}
        self.assertEqual(smoke.full_tool(tool), document)
        tool["content"][1]["text"] = "{}"
        with self.assertRaisesRegex(AssertionError, "payload mismatch"):
            smoke.full_tool(tool)

    def test_overwrite_success_and_wrong_error_are_refused(self):
        refusal = {"isError": True, "content": [{"text": "SAIL0905: baseline already exists and is immutable"}]}
        self.assertIsNone(smoke.full_tool(refusal, expected_error=True))
        for wrong in ({"isError": False}, {"isError": True, "content": [{"text": "SAIL0905: invalid name"}]}):
            with self.assertRaises(AssertionError):
                smoke.full_tool(wrong, expected_error=True)


if __name__ == "__main__":
    unittest.main()
