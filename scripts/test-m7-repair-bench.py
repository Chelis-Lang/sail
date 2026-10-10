#!/usr/bin/env python3
"""Acceptance-oracle tests; these do not substitute for project verification."""

import copy
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("repair_bench", Path(__file__).with_name("m7-repair-bench.py"))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class AcceptanceTests(unittest.TestCase):
    def complete_results(self):
        data = bench.catalogue()
        samples = []
        for project in data["projects"]:
            for round_index in range(3):
                for cycle in project["cycles"]:
                    samples.append({"project": project["id"], "round": round_index,
                                    "cycle": cycle["id"], "same": True, "expected_observed": True,
                                    "m6": {"wall_ms": 2500}, "m7": {"wall_ms": 1000}})
        return {"rounds": 3, "samples": samples}

    def test_missing_or_duplicate_real_cycle_never_passes(self):
        result = self.complete_results()
        self.assertTrue(bench.summarize(result)["passed"])
        result["samples"].pop()
        self.assertFalse(bench.summarize(result)["passed"])
        result = self.complete_results()
        result["samples"][-1] = copy.deepcopy(result["samples"][-2])
        self.assertFalse(bench.summarize(result)["passed"])

    def test_each_project_must_meet_speed_and_correctness(self):
        result = self.complete_results()
        for sample in result["samples"]:
            if sample["project"] == "vest":
                sample["m7"]["wall_ms"] = 2000
        self.assertFalse(bench.summarize(result)["passed"])
        result = self.complete_results()
        result["samples"][0]["same"] = False
        self.assertFalse(bench.summarize(result)["passed"])
        result = self.complete_results()
        result["samples"][0]["expected_observed"] = False
        self.assertFalse(bench.summarize(result)["passed"])

    def test_empty_or_under_sampled_census_never_passes(self):
        self.assertFalse(bench.summarize({"rounds": 0, "samples": []})["passed"])
        result = self.complete_results()
        result["rounds"] = 2
        result["samples"] = [sample for sample in result["samples"] if sample["round"] < 2]
        self.assertFalse(bench.summarize(result)["passed"])

    def test_target_infrastructure_error_is_not_a_failure_verdict(self):
        with self.assertRaises(ValueError):
            bench.selected_report({"exit_code": 2, "error": {"message": "broken engine"}})

    def test_only_explicit_cold_declarations_allow_one_retry(self):
        doc = {"accepted": False, "selection_error": None, "batch": {
            "batched": True, "manifest_sha256": "manifest", "focus": {},
            "full": {"currentness": "unknown", "exit_code": 0, "error": None,
                     "targets": [{"requested": True, "complete": True}],
                     "notes": ["Build-script input declarations were established during this run"]}}}
        self.assertEqual(bench.warm_repair_state(doc, True), "declarations-established")
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, False)
        doc["batch"]["full"]["notes"] = []
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, True)
        doc["accepted"] = True
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, True)
        doc["batch"]["full"]["currentness"] = "current"
        self.assertEqual(bench.warm_repair_state(doc, False), "ready")
        doc["batch"]["full"]["targets"][0]["complete"] = False
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, False)


if __name__ == "__main__":
    unittest.main()
