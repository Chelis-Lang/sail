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
        for field in ("job", "result"):
            changed = copy.deepcopy(result)
            changed["value"][field]["input_revision"] = "other"
            self.assertFalse(bench.valid_established(changed, "proof", "r"))
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
        compiler = {"pid": 22, "parent": 21, "name": "rust_verify", "state": "R", "started_ticks": 50,
                    "crate_name": "human_eval_verus"}
        table = {20: owner, 21: {"pid": 21, "parent": 20, "name": "cargo",
                                "state": "S", "started_ticks": 45}, 22: compiler}
        def live(values=table, started=40):
            return bench.active_verifiers(values, 20, started, "human_eval_verus")
        self.assertEqual(live(), [compiler])
        self.assertEqual(live(started=41), [])
        self.assertEqual(live({20: owner}), [])
        for name in (None, "verus_builtin_macros", "vstd", "other_project"):
            compiler["crate_name"] = name
            self.assertEqual(live(), [])
        compiler["crate_name"] = "human_eval_verus"
        for state in ("Z", "T", "t"):
            compiler["state"] = state
            self.assertEqual(live(), [])
        compiler["state"] = "R"
        table[21]["parent"] = 1
        self.assertEqual(live(), [])
        table[21]["parent"] = 20
        table[21]["started_ticks"] = 51
        self.assertEqual(live(), [])

    def test_same_compiler_must_span_rpc_but_state_can_change(self):
        before = {"pid": 22, "parent": 21, "name": "rust_verify", "state": "R",
                  "started_ticks": 50, "crate_name": "human_eval_verus"}
        after = dict(before, state="S")
        self.assertTrue(bench.continuing_verifier([before], [after]))
        for changed in (dict(after, pid=23), dict(after, started_ticks=51), dict(after, parent=24),
                        dict(after, crate_name="vstd")):
            self.assertFalse(bench.continuing_verifier([before], [changed]))
        self.assertFalse(bench.continuing_verifier([before], []))

    def test_exact_crate_selector_rejects_ambiguous_or_missing_arguments(self):
        self.assertEqual(bench.proof_crate([b"rust_verify", b"--crate-name", b"human_eval_verus"]), "human_eval_verus")
        self.assertEqual(bench.proof_crate([b"--crate-name=vest_lib"]), "vest_lib")
        self.assertEqual(bench.proof_crate([b"--crate-name=vest_lib", b"--", b"--crate-name=other"]), "vest_lib")
        for command in ([b"--crate-name"], [], [b"--crate-name="],
                        [b"--crate-name", b"one", b"--crate-name=two"],
                        [b"--crate-name", b"not-a-crate"], [b"--crate-name", b"\xff"],
                        [b"--", b"--crate-name=human_eval_verus"]):
            self.assertIsNone(bench.proof_crate(command))


if __name__ == "__main__":
    unittest.main()
