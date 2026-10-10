#!/usr/bin/env python3
"""Read-latency acceptance must retain the proof freshness contract."""

import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("read_bench", Path(__file__).with_name("m7-job-read-bench.py"))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class ReadAcceptanceTests(unittest.TestCase):
    def result(self):
        return {"response": "result", "value": {
            "job": {"id": "proof", "state": "completed", "currentness": "current", "input_revision": "r"},
            "result": {"currentness": "current", "input_revision": "r", "error": None}}}

    def test_fast_unknown_or_historical_result_never_qualifies(self):
        result = self.result()
        self.assertTrue(bench.valid_established(result, "proof", "r"))
        for field in ("job", "result"):
            for currentness in ("unknown", "historical"):
                changed = copy.deepcopy(result)
                changed["value"][field]["currentness"] = currentness
                self.assertFalse(bench.valid_established(changed, "proof", "r"))

    def test_another_revision_or_incomplete_worker_never_qualifies(self):
        result = self.result()
        self.assertFalse(bench.valid_established(result, "other", "r"))
        self.assertFalse(bench.valid_established(result, "proof", "new"))
        for state in ("running", "cancelled", "infrastructure_error"):
            changed = copy.deepcopy(result)
            changed["value"]["job"]["state"] = state
            self.assertFalse(bench.valid_established(changed, "proof", "r"))

    def test_unavailable_result_is_not_latency_success(self):
        result = self.result()
        result["value"]["result"] = None
        self.assertFalse(bench.valid_established(result, "proof", "r"))

    def test_running_job_without_live_descendant_verifier_does_not_qualify(self):
        owner = {"pid": 20, "parent": 1, "name": "sail", "state": "S", "started_ticks": 40}
        compiler = {"pid": 22, "parent": 21, "name": "rust_verify", "state": "R", "started_ticks": 50}
        table = {20: owner, 21: {"pid": 21, "parent": 20, "name": "cargo",
                                "state": "S", "started_ticks": 45}, 22: compiler}
        self.assertEqual(bench.active_verifiers(table, 20, 40), [compiler])
        self.assertEqual(bench.active_verifiers(table, 20, 41), [])
        self.assertEqual(bench.active_verifiers({20: owner}, 20, 40), [])
        compiler["state"] = "Z"
        self.assertEqual(bench.active_verifiers(table, 20, 40), [])
        compiler["state"] = "R"
        table[21]["parent"] = 1
        self.assertEqual(bench.active_verifiers(table, 20, 40), [])


if __name__ == "__main__":
    unittest.main()
