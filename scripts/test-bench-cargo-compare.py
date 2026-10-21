#!/usr/bin/env python3
"""Behavioral tests for Cargo verifier benchmarks."""

import copy
import itertools
import json
import math
import os
import select
from pathlib import Path
import runpy
import signal
import subprocess
import sys
import tempfile
import unittest

import bench_cargo_compare_report as report


def sail_document():
    crates = []
    for name, size in (("vstd", 2), ("demo", 1)):
        crates.append({"name": name, "verus": {"verified": 10 * size, "errors": 0,
                       "success": True, "encountered_vir_error": False},
                       "functions": [{"function": name + "::f%d" % i, "module": "",
                                      "fn_mode": "proof", "verdict": "verified",
                                      "establishment": "confirmed", "verus_success": True}
                                     for i in range(size)]})
    inner = {"mode": "default", "exit_code": 0, "error": None, "crates": crates,
             "summary": {"functions": 3, "verified": 3, "failed": 0, "unknown": 0, "provisional": 0}}
    return {"kind": "sail-project-result", "exit_code": 0, "error": None,
            "execution": {"verifier_invocations": 1, "reused_targets": 0},
            "no_reuse": {"verified_as": "plain-cargo-verus"},
            "targets": [{"reused": False, "complete": False, "report": inner}]}


def proof(mode):
    if mode == "sail":
        return report.parse_proof(mode, json.dumps(sail_document()))
    return report.parse_proof(mode, "verification results:: 100 verified, 0 errors\n"
                             "verification results:: 400 verified, 0 errors\n")


def sample(phase, rnd, mode, wall, counter):
    before = dict(pid=123, sessions=counter, gets=counter * 10,
                  hits=counter * 3, puts=counter * 7, memory_hits=0)
    delta = dict(sessions=1, gets=10, hits=3, puts=7, memory_hits=0) if mode == "sail" else dict.fromkeys(
        ("sessions", "gets", "hits", "puts", "memory_hits"), 0)
    after = {key: before[key] + delta[key] for key in delta}
    after["pid"] = before["pid"]
    before["state"] = dict(memory_entries=counter, disk_entries=counter * 7, pending_writes=0)
    after["state"] = dict(memory_entries=after["sessions"], disk_entries=after["puts"], pending_writes=0)
    return {"scenario": "fresh", "source_state": "base",
            "source_checks": {"before": [], "after": []},
            "phase": phase, "round": rnd, "mode": mode, "exit_code": 0,
            "wall_seconds": wall, "client_cpu_seconds": wall * 2,
            "proof": proof(mode), "cache_before": before, "cache_after": after,
            "cache_delta": delta, "problems": [],
            "stdout": "logs/%s-%s-%s.stdout" % (phase, rnd, mode),
            "stderr": "logs/%s-%s-%s.stderr" % (phase, rnd, mode)}


def complete(rounds=3):
    rows = [sample("warmup", 0, mode, 15, 0) for mode in report.MODES]
    times = {"stock": [10, 100, 30], "fork": [5, 50, 15], "sail": [1, 50, 20]}
    for rnd in range(1, rounds + 1):
        rows.extend(sample("measured", rnd, mode, times[mode][rnd - 1], rnd)
                    for mode in report.trial_order(rnd - 1))
    return {"schema_version": 2, "scenarios": ["fresh"], "rounds": rounds,
            "modes": list(report.MODES), "problems": [], "root_package_id": None,
            "root_crate": None, "edit": None}, rows


class TrialOrders(unittest.TestCase):
    def test_six_orders_and_position_balance(self):
        orders = [report.trial_order(i) for i in range(6)]
        self.assertEqual(set(orders), set(itertools.permutations(report.MODES)))
        for start in (0, 3):
            for position in range(3):
                self.assertEqual(
                    {orders[i][position] for i in range(start, start + 3)},
                    set(report.MODES))
        for i in range(12):
            self.assertEqual(report.trial_order(i), report.trial_order(i + 6))


class ProofParsing(unittest.TestCase):
    def test_native_requires_positive_complete_proof_work(self):
        for text in ("Finished dev target", "verification results:: 0 verified, 0 errors",
                     "verification results:: 2 verified, 1 errors",
                     "verification results:: 4 verified, 0 errors\nverification results:: 0 verified, 0 errors"):
            with self.subTest(text=text):
                self.assertTrue(report.parse_proof("stock", text)["problems"])
        self.assertEqual(proof("stock")["counts"], [100, 400])
        self.assertEqual(proof("stock")["problems"], [])

    def test_sail_separate_count_schema_and_order_independent_scope(self):
        doc = sail_document()
        parsed = report.parse_proof("sail", json.dumps(doc))
        self.assertEqual(parsed["problems"], [])
        self.assertEqual(parsed["verified"], 3)
        self.assertEqual(parsed["counts"], [1, 2])
        inner = doc["targets"][0]["report"]
        inner["crates"].reverse()
        for crate in inner["crates"]:
            crate["functions"].reverse()
        self.assertEqual(report.parse_proof("sail", json.dumps(doc)), parsed)

    def test_confirmed_log_only_functions_keep_valid_statistics(self):
        metadata, rows = complete()
        document = sail_document()
        function = document["targets"][0]["report"]["crates"][0]["functions"][0]
        function["fn_mode"] = None
        function["verus_success"] = None
        for row in rows:
            if row["mode"] == "sail":
                row["proof"] = report.parse_proof("sail", json.dumps(document))
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        self.assertEqual(summary["scenarios"]["fresh"]["aggregates"]["sail"]["samples"], [1, 50, 20])

    def test_friendly_name_multiplicity_remains_part_of_scope(self):
        metadata, rows = complete()
        document = sail_document()
        inner = document["targets"][0]["report"]
        crate = inner["crates"][-1]
        crate["functions"].append(copy.deepcopy(crate["functions"][0]))
        inner["summary"]["functions"] += 1
        inner["summary"]["verified"] += 1
        parsed = report.parse_proof("sail", json.dumps(document))
        self.assertEqual(parsed["verified"], 4)
        self.assertNotEqual(parsed["scope_sha256"], proof("sail")["scope_sha256"])
        for row in rows:
            if row["mode"] == "sail":
                row["proof"] = parsed
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])

    def test_sail_rejects_skipped_and_reused_work(self):
        for field, value in (("verifier_invocations", 0), ("reused_targets", 1)):
            doc = sail_document()
            doc["execution"][field] = value
            self.assertTrue(report.parse_proof("sail", json.dumps(doc))["problems"])
        for field, value in (("reused", True), ("dependency_store", "key"),
                             ("dependency_build", {"kind": "release"})):
            doc = sail_document()
            doc["targets"][0][field] = value
            self.assertTrue(report.parse_proof("sail", json.dumps(doc))["problems"])

    def test_sail_rejects_bad_verdicts_and_inconsistent_totals(self):
        for field, value in (("verdict", "failed"), ("verdict", "unknown"),
                             ("establishment", "provisional"), ("verus_success", False)):
            doc = sail_document()
            doc["targets"][0]["report"]["crates"][0]["functions"][0][field] = value
            self.assertTrue(report.parse_proof("sail", json.dumps(doc))["problems"])
        for field in ("functions", "verified", "failed", "unknown", "provisional"):
            doc = sail_document()
            doc["targets"][0]["report"]["summary"][field] += 1
            self.assertTrue(report.parse_proof("sail", json.dumps(doc))["problems"])
        for text in ("", "null", "[]", "{}", '{"targets": [null], "kind": "sail-project-result"}'):
            self.assertTrue(report.parse_proof("sail", text)["problems"])

    def test_empty_or_incomplete_reports_are_visible_errors(self):
        for change in ("empty", "crate", "error", "mode"):
            doc = sail_document()
            inner = doc["targets"][0]["report"]
            if change == "empty":
                inner["crates"] = []
            elif change == "crate":
                inner["crates"][0]["verus"]["success"] = False
            elif change == "error":
                inner["error"] = {"message": "Refused proof"}
            else:
                inner["mode"] = "experimental"
            with self.subTest(change=change):
                self.assertTrue(report.parse_proof("sail", json.dumps(doc))["problems"])


class Summaries(unittest.TestCase):
    def assert_invalid(self, metadata, rows):
        summary = report.summarize(metadata, rows)
        self.assertFalse(summary["valid"])
        self.assertIsNone(summary["scenarios"]["fresh"]["aggregates"])
        self.assertIsNone(summary["scenarios"]["fresh"]["paired_speedups"])
        self.assertEqual(len(summary["samples"]), len(rows))
        self.assertTrue(summary["problems"])
        return summary

    def test_complete_set_has_statistics_and_paired_ratios(self):
        metadata, rows = complete()
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        self.assertEqual(summary["expected_samples"], 12)
        self.assertEqual(summary["observed_samples"], 12)
        stock = summary["scenarios"]["fresh"]["aggregates"]["stock"]
        self.assertEqual(stock["median_seconds"], 30)
        self.assertEqual(stock["min_seconds"], 10)
        self.assertEqual(stock["max_seconds"], 100)
        self.assertAlmostEqual(stock["stdev_seconds"], math.sqrt(6700 / 3))
        pair = summary["scenarios"]["fresh"]["paired_speedups"]["stock_over_sail"]
        self.assertEqual(pair["rounds"], [{"round": 1, "ratio": 10},
                                         {"round": 2, "ratio": 2}, {"round": 3, "ratio": 1.5}])
        self.assertEqual(pair["median_ratio"], 2)
        self.assertNotEqual(pair["median_ratio"], stock["median_seconds"] /
                            summary["scenarios"]["fresh"]["aggregates"]["sail"]["median_seconds"])
        self.assertEqual(summary["scenarios"]["fresh"]["paired_speedups"]["stock_over_fork"]["median_ratio"], 2)
        self.assertEqual(summary["scenarios"]["fresh"]["paired_speedups"]["fork_over_sail"]["median_ratio"], 1)


    def test_missing_duplicate_and_unexpected_samples(self):
        metadata, rows = complete()
        self.assert_invalid(metadata, rows[:-1])
        self.assert_invalid(metadata, rows + [copy.deepcopy(rows[-1])])
        rows[-1]["round"] = 9
        self.assert_invalid(metadata, rows)

    def test_fast_failed_samples_never_produce_statistics(self):
        for failure in ("exit", "row", "proof", "time", "interrupt", "metadata"):
            metadata, rows = complete()
            rows[-1]["wall_seconds"] = 0.0001
            if failure == "exit":
                rows[-1]["exit_code"] = 1
            elif failure == "row":
                rows[-1]["problems"].append("Refused sample")
            elif failure == "proof":
                rows[-1]["proof"]["problems"].append("Proof failed")
            elif failure == "time":
                rows[-1]["wall_seconds"] = float("nan")
            elif failure == "interrupt":
                rows[-1]["exit_code"] = None
            else:
                metadata["problems"].append("Interrupted run")
            with self.subTest(failure=failure):
                summary = self.assert_invalid(metadata, rows)
                if failure != "metadata":
                    self.assertTrue(summary["samples"][-1]["problems"])

    def test_native_scope_uses_stock_warmup_multiset(self):
        metadata, rows = complete()
        for row in rows:
            if row["mode"] != "sail":
                row["proof"]["counts"].reverse()
        self.assertTrue(report.summarize(metadata, rows)["valid"])
        rows[1]["proof"]["counts"] = [99, 401]
        self.assert_invalid(metadata, rows)

    def test_sail_scope_and_crate_count_use_their_own_reference(self):
        for field, value in (("function", "vstd::renamed"), ("module", "changed"),
                             ("fn_mode", "exec"), ("verdict", "unknown")):
            metadata, rows = complete()
            doc = sail_document()
            doc["targets"][0]["report"]["crates"][0]["functions"][0][field] = value
            sail = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
            sail["proof"] = report.parse_proof("sail", json.dumps(doc))
            self.assert_invalid(metadata, rows)
        metadata, rows = complete()
        for row in rows:
            if row["mode"] != "sail":
                row["proof"]["counts"] = [500]
        self.assert_invalid(metadata, rows)

    def test_cache_state_and_native_requests(self):
        for failure in ("pid", "lookups", "counter", "cold", "native", "warmup", "reset"):
            metadata, rows = complete()
            sail = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
            if failure == "pid":
                sail["cache_before"]["pid"] = sail["cache_after"]["pid"] = 999
            elif failure == "lookups":
                sail["cache_delta"]["gets"] = 0
                sail["cache_after"]["gets"] = sail["cache_before"]["gets"]
            elif failure == "counter":
                sail["cache_delta"]["hits"] += 1
            elif failure == "cold":
                rows[2]["cache_before"]["gets"] = 1
                rows[2]["cache_after"]["gets"] += 1
            elif failure == "native":
                rows[0]["cache_delta"]["gets"] = 1
                rows[0]["cache_after"]["gets"] = 1
            elif failure == "warmup":
                rows[2]["exit_code"] = 1
            else:
                for key in ("sessions", "gets", "hits", "puts"):
                    sail["cache_before"][key] = 0
                    sail["cache_after"][key] = sail["cache_delta"][key]
            with self.subTest(failure=failure):
                self.assert_invalid(metadata, rows)

    def test_no_cache_hit_rate_threshold(self):
        metadata, rows = complete()
        for row in rows:
            row["cache_before"]["hits"] = 0
            row["cache_after"]["hits"] = 0
            row["cache_delta"]["hits"] = 0
        self.assertTrue(report.summarize(metadata, rows)["valid"])



ROOT_PACKAGE = "path+file:///benchmark/source#demo@0.1.0"
ROOT_CRATE = "demo"


def native_workflow(fresh=False, counts=(), package=ROOT_PACKAGE, crate=ROOT_CRATE):
    lines = ["verification results:: %d verified, 0 errors" % count for count in counts]
    lines.extend([json.dumps({"reason": "compiler-artifact", "package_id": package,
                             "target": {"name": crate, "kind": ["lib"]}, "fresh": fresh}),
                  json.dumps({"reason": "build-finished", "success": True})])
    return "\n".join(lines)


def managed_document(reused=False, dependency_reused=True, root_function="demo::f0"):
    original = sail_document()["targets"][0]["report"]
    targets = []
    for crate in original["crates"]:
        is_root = crate["name"] == ROOT_CRATE
        if is_root:
            crate["functions"][0]["function"] = root_function
        total = len(crate["functions"])
        targets.append({"package_id": ROOT_PACKAGE if is_root else "registry+vstd@1.0.0",
                        "target": crate["name"], "kind": ["lib"], "complete": True,
                        "requested": is_root, "reused": reused if is_root else dependency_reused,
                        "establishing_run": ("confirmed-baseline" if (reused if is_root else dependency_reused)
                                             else "current-run"),
                        "report": {"mode": "default", "exit_code": 0, "error": None,
                                   "crates": [crate], "summary": {
                                       "functions": total, "verified": total, "failed": 0,
                                       "unknown": 0, "provisional": 0}}})
    return {"kind": "sail-project-result", "schema_version": 2, "exit_code": 0,
            "run_id": "current-run",
            "currentness": "current", "error": None, "targets": targets,
            "execution": {"verifier_invocations": int(not reused) + int(not dependency_reused),
                          "reused_targets": int(reused) + int(dependency_reused)}}


def workflow_proof(mode, outcome="verified", root_function="demo::f0", counts=(100, 400)):
    stdout = (json.dumps(managed_document(outcome == "reused", root_function=root_function))
              if mode == "sail" else native_workflow(outcome == "cargo-noop",
                                                     () if outcome == "cargo-noop" else counts))
    return report.parse_workflow_proof(mode, stdout, "", ROOT_PACKAGE, ROOT_CRATE)


def workflow_complete(name="unchanged", rounds=2):
    metadata, _ = complete(rounds)
    metadata.update(scenarios=[name], root_package_id=ROOT_PACKAGE, root_crate=ROOT_CRATE)
    if name == "proof-edit":
        metadata["edit"] = {"path": "src/lib.rs", "before_sha256": "a" * 64,
                            "after_sha256": "b" * 64,
                            "spec": {"path": "src/lib.rs", "before": "assert(true)",
                                     "after": "assert(1 == 1)"}}
    rows = []
    sail_counter = 0
    for rnd in (range(1, rounds + 1) if name == "proof-edit" else [0]):
        rows.extend(sample("warmup", rnd, mode, 15, sail_counter) for mode in report.MODES)
        sail_counter += 1
        if name == "proof-edit":
            rows.extend(sample("measured", rnd, mode, 3, sail_counter) for mode in report.MODES)
            sail_counter += 1
    if name == "unchanged":
        for rnd in range(1, rounds + 1):
            rows.extend(sample("measured", rnd, mode, 3, sail_counter) for mode in report.MODES)
            sail_counter += 1
    for row in rows:
        measured = row["phase"] == "measured"
        row["scenario"] = name
        row["source_state"] = "edited" if name == "proof-edit" and measured else "base"
        row["artifact_context"] = "%s-%s-%s" % (name, row["round"] if name == "proof-edit" else 0,
                                                  row["mode"])
        if name == "proof-edit":
            row["edit_sha256"] = ("b" if measured else "a") * 64
        outcome = "verified"
        if name == "unchanged" and measured:
            outcome = "reused" if row["mode"] == "sail" else "cargo-noop"
        row["proof"] = workflow_proof(row["mode"], outcome,
                                      counts=(400,) if name == "proof-edit" and measured else (100, 400))
        if name == "unchanged" and measured and row["mode"] == "sail":
            # A receipt-only command can make no solver requests.
            for key in row["cache_delta"]:
                row["cache_delta"][key] = 0
                row["cache_after"][key] = row["cache_before"][key]
            row["cache_after"]["state"] = copy.deepcopy(row["cache_before"]["state"])
    return metadata, rows


def restart_complete(rounds=2):
    metadata, rows = complete(rounds)
    metadata["scenarios"] = ["daemon-restart"]
    previous = None
    for row in rows:
        row["scenario"] = "daemon-restart"
        if row["mode"] != "sail":
            continue
        if row["phase"] == "measured":
            before = row["cache_before"]
            old_disk = previous["cache_after"]["state"]["disk_entries"]
            row["restart"] = {"old_pid": previous["cache_after"]["pid"], "old_disk_entries": old_disk,
                              "old_snapshot": copy.deepcopy(previous["cache_after"])}
            row["startup_seconds"] = 0.01
            before.update(dict.fromkeys(row["cache_delta"], 0))
            before["pid"] = 200 + row["round"]
            before["state"] = dict(memory_entries=0, disk_entries=old_disk, pending_writes=0)
            after = row["cache_after"]
            after.update(row["cache_delta"])
            after["pid"] = before["pid"]
            after["state"] = dict(memory_entries=1, disk_entries=old_disk + 7, pending_writes=0)
        previous = row
    return metadata, rows


class WorkflowParsing(unittest.TestCase):
    def parse(self, mode, stdout, stderr=""):
        return report.parse_workflow_proof(mode, stdout, stderr, ROOT_PACKAGE, ROOT_CRATE)

    def test_native_noop_needs_positive_selected_library_artifact(self):
        valid = self.parse("stock", native_workflow(True))
        self.assertEqual(valid["problems"], [])
        self.assertEqual(valid["outcome"], "cargo-noop")
        self.assertEqual(valid["verified"], 0)
        self.assertEqual(valid["root_verified"], 0)
        self.assertEqual(valid["execution"]["verifier_invocations"], 0)
        for text in ("", "Finished dev target", native_workflow(False),
                     native_workflow(True, package="other"), native_workflow(True, crate="other"),
                     json.dumps({"reason": "build-finished", "success": True})):
            with self.subTest(text=text):
                self.assertTrue(self.parse("stock", text)["problems"])

    def test_native_artifact_requires_lib_identity_and_success(self):
        artifact, finish = [json.loads(line) for line in native_workflow(True).splitlines()]
        for field, value in (("kind", ["bin"]), ("name", "other")):
            changed = copy.deepcopy(artifact)
            changed["target"][field] = value
            self.assertTrue(self.parse("fork", json.dumps(changed) + "\n" + json.dumps(finish))["problems"])
        for value in (False, None, 1):
            changed = dict(finish, success=value)
            self.assertTrue(self.parse("fork", json.dumps(artifact) + "\n" + json.dumps(changed))["problems"])
        self.assertTrue(self.parse("fork", json.dumps(artifact))["problems"])
        self.assertTrue(self.parse("fork", native_workflow(True) + "\n" + json.dumps(artifact))["problems"])

    def test_native_root_count_is_last_summary_and_dependencies_can_be_warm(self):
        for counts in ((100, 400), (400,)):
            result = self.parse("stock", native_workflow(False, counts))
            self.assertEqual(result["problems"], [])
            self.assertEqual(result["outcome"], "verified")
            self.assertEqual(result["root_verified"], 400)
            self.assertEqual(result["verified"], sum(counts))
        self.assertTrue(self.parse("stock", native_workflow(True, (400,)))["problems"])
        self.assertTrue(self.parse("stock", native_workflow(False, (0,)))["problems"])
        self.assertTrue(self.parse("stock", native_workflow(False, (400,)).replace("0 errors", "1 errors"))["problems"])

    def test_native_evidence_can_use_both_output_streams(self):
        artifact = native_workflow(False)
        result = self.parse("fork", artifact, "verification results:: 400 verified, 0 errors")
        self.assertEqual(result["problems"], [])
        self.assertEqual(result["root_verified"], 400)
        result = self.parse("fork", "", native_workflow(True))
        self.assertEqual(result["outcome"], "cargo-noop")
        self.assertEqual(result["problems"], [])

    def test_supported_library_kinds_keep_root_evidence(self):
        for kind in ("lib", "rlib", "dylib", "cdylib", "staticlib"):
            with self.subTest(kind=kind):
                artifact, finish = [json.loads(line) for line in native_workflow(True).splitlines()]
                artifact["target"]["kind"] = [kind]
                native = self.parse("stock", json.dumps(artifact) + "\n" + json.dumps(finish))
                self.assertEqual(native["problems"], [])
                self.assertEqual(native["outcome"], "cargo-noop")
                document = managed_document()
                document["targets"][-1]["kind"] = [kind]
                sail = self.parse("sail", json.dumps(document))
                self.assertEqual(sail["problems"], [])
                self.assertEqual(sail["outcome"], "verified")

    def test_sail_reuse_is_not_new_proof_work(self):
        fresh = workflow_proof("sail")
        reused = workflow_proof("sail", "reused")
        self.assertEqual(fresh["problems"], [])
        self.assertEqual(reused["problems"], [])
        self.assertEqual(fresh["outcome"], "verified")
        self.assertEqual(fresh["verified"], 1)
        self.assertEqual(fresh["counts"], [1])
        self.assertEqual(reused["outcome"], "reused")
        self.assertEqual(reused["verified"], 0)
        self.assertEqual(reused["counts"], [])
        self.assertEqual(reused["root_verified"], 1)
        self.assertEqual(reused["root_scope_sha256"], fresh["root_scope_sha256"])
        self.assertTrue(report.parse_proof("sail", json.dumps(managed_document(True)))["problems"])

    def test_dependency_replay_does_not_count_as_new_work(self):
        document = managed_document(False, False)
        dependency = document["targets"][0]
        dependency["dependency_build"] = {"kind": "machine-local", "identity": "cache-key", "establishing_run": "old"}
        result = self.parse("sail", json.dumps(document))
        self.assertEqual(result["problems"], [])
        self.assertEqual(result["counts"], [1])
        self.assertEqual(result["verified"], 1)

    def test_sail_requires_complete_confirmed_root_and_dependency_reports(self):
        changes = ("root-missing", "package", "crate", "requested", "partial", "duplicate",
                   "provisional", "dependency-error", "counter", "currentness", "provenance", "execution")
        for change in changes:
            document = managed_document(True)
            root = document["targets"][-1]
            if change == "root-missing":
                document["targets"].pop()
            elif change == "package":
                root["package_id"] = "other"
            elif change == "crate":
                root["report"]["crates"][0]["name"] = "other"
            elif change == "requested":
                root["requested"] = False
            elif change == "partial":
                root["complete"] = False
            elif change == "duplicate":
                document["targets"].append(copy.deepcopy(root))
            elif change == "provisional":
                root["report"]["crates"][0]["functions"][0]["establishment"] = "provisional"
            elif change == "dependency-error":
                document["targets"][0]["report"]["error"] = {"message": "failed"}
            elif change == "counter":
                document["execution"]["reused_targets"] = 0
            elif change == "currentness":
                document["currentness"] = "historical"
            elif change == "provenance":
                root["establishing_run"] = ""
            else:
                document["execution"]["verifier_invocations"] = -1
            with self.subTest(change=change):
                self.assertTrue(self.parse("sail", json.dumps(document))["problems"])


class WorkflowSummaries(unittest.TestCase):
    def assert_invalid(self, metadata, rows, scenario):
        summary = report.summarize(metadata, rows)
        self.assertFalse(summary["valid"])
        result = summary["scenarios"][scenario]
        self.assertFalse(result["valid"])
        self.assertIsNone(result["aggregates"])
        self.assertIsNone(result["paired_speedups"])
        self.assertEqual(summary["observed_samples"], len(rows))
        self.assertEqual(len(summary["samples"]), len(rows))
        return summary

    def test_stale_root_provenance_suppresses_edit_statistics(self):
        metadata, rows = workflow_complete("proof-edit")
        document = managed_document()
        document["targets"][-1]["establishing_run"] = "old-edit-run"
        edited = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
        edited["proof"] = report.parse_workflow_proof(
            "sail", json.dumps(document), "", ROOT_PACKAGE, ROOT_CRATE)
        self.assert_invalid(metadata, rows, "proof-edit")

    def test_missing_current_run_identity_suppresses_edit_statistics(self):
        for run_id in (None, "", 42):
            with self.subTest(run_id=run_id):
                metadata, rows = workflow_complete("proof-edit")
                document = managed_document()
                document["run_id"] = run_id
                edited = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
                edited["proof"] = report.parse_workflow_proof(
                    "sail", json.dumps(document), "", ROOT_PACKAGE, ROOT_CRATE)
                self.assert_invalid(metadata, rows, "proof-edit")

    def test_unchanged_accepts_noop_receipts_and_full_root_verification(self):
        metadata, rows = workflow_complete()
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        self.assertEqual(summary["schema_version"], 2)
        self.assertNotIn("aggregates", summary)
        self.assertNotIn("paired_speedups", summary)
        self.assertEqual(summary["scenarios"]["unchanged"]["expected_samples"], 9)
        for row in rows:
            if row["phase"] == "measured":
                row["proof"] = workflow_proof(row["mode"], counts=(400,))
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])

    def test_proof_edit_uses_each_own_baseline_and_not_dependency_counts(self):
        metadata, rows = workflow_complete("proof-edit")
        for row in rows:
            if row["round"] == 2:
                row["proof"] = workflow_proof(row["mode"], root_function="demo::different_round",
                                              counts=(500,) if row["phase"] == "measured" else (88, 500))
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        self.assertEqual(summary["expected_samples"], 12)
        self.assertEqual(summary["scenarios"]["proof-edit"]["aggregates"]["stock"]["samples"], [3, 3])

    def test_proof_edit_allows_new_obligations_but_requires_native_agreement(self):
        metadata, rows = workflow_complete("proof-edit")
        for row in rows:
            if row["phase"] == "measured" and row["mode"] != "sail":
                row["proof"] = workflow_proof(row["mode"], counts=(401,))
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        edited = next(row for row in rows
                      if row["phase"] == "measured" and row["mode"] == "fork")
        edited["proof"] = workflow_proof("fork", counts=(402,))
        self.assert_invalid(metadata, rows, "proof-edit")

    def test_proof_edit_rejects_reuse_and_cargo_noop(self):
        for mode in report.MODES:
            metadata, rows = workflow_complete("proof-edit")
            edited = next(row for row in rows if row["phase"] == "measured" and row["mode"] == mode)
            edited["proof"] = workflow_proof(mode, "reused" if mode == "sail" else "cargo-noop")
            with self.subTest(mode=mode):
                self.assert_invalid(metadata, rows, "proof-edit")

    def test_proof_edit_rejects_root_count_and_function_scope_changes(self):
        for mode in report.MODES:
            metadata, rows = workflow_complete("proof-edit")
            edited = next(row for row in rows if row["phase"] == "measured" and row["mode"] == mode)
            edited["proof"] = workflow_proof(mode, root_function="demo::changed", counts=(399,))
            with self.subTest(mode=mode):
                summary = self.assert_invalid(metadata, rows, "proof-edit")
                invalid = next(row for row in summary["samples"] if row["phase"] == "measured" and row["mode"] == mode)
                self.assertTrue(invalid["problems"])

    def test_invalid_baseline_transitions_remain_visible(self):
        for failure in ("missing", "duplicate", "exit", "noop", "source", "context", "edit", "cache"):
            metadata, rows = workflow_complete("proof-edit")
            baseline = next(row for row in rows if row["phase"] == "warmup" and row["mode"] == "sail")
            measured = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
            if failure == "missing":
                rows.remove(baseline)
            elif failure == "duplicate":
                rows.insert(0, copy.deepcopy(baseline))
            elif failure == "exit":
                baseline["exit_code"] = 1
            elif failure == "noop":
                baseline["proof"] = workflow_proof("sail", "reused")
            elif failure == "source":
                baseline["source_state"] = "edited"
            elif failure == "context":
                measured["artifact_context"] = "another-target"
            elif failure == "edit":
                measured["edit_sha256"] = "c" * 64
            else:
                baseline["cache_before"]["state"]["memory_entries"] = 1
            with self.subTest(failure=failure):
                self.assert_invalid(metadata, rows, "proof-edit")

    def test_baseline_does_not_cross_rounds_or_modes(self):
        metadata, rows = workflow_complete("proof-edit")
        rows[:] = [row for row in rows if not (row["phase"] == "warmup" and row["round"] == 2 and row["mode"] == "fork")]
        summary = self.assert_invalid(metadata, rows, "proof-edit")
        edited = next(row for row in summary["samples"] if row["phase"] == "measured" and row["round"] == 2 and row["mode"] == "fork")
        self.assertTrue(edited["problems"])

    def test_artifact_context_cannot_reuse_an_earlier_edit_receipt(self):
        metadata, rows = workflow_complete("proof-edit")
        for row in rows:
            if row["mode"] == "sail":
                row["artifact_context"] = "shared-target"
        self.assert_invalid(metadata, rows, "proof-edit")

    def test_cache_continuity_and_zero_native_requests(self):
        for failure in ("pid", "regression", "native", "state", "delta-state", "memory-hits"):
            metadata, rows = workflow_complete("proof-edit")
            edited = next(row for row in rows if row["phase"] == "measured" and row["mode"] == "sail")
            if failure == "pid":
                edited["cache_before"]["pid"] = edited["cache_after"]["pid"] = 456
            elif failure == "regression":
                for key in edited["cache_delta"]:
                    edited["cache_before"][key] = 0
                    edited["cache_after"][key] = edited["cache_delta"][key]
            elif failure == "native":
                native = next(row for row in rows if row["mode"] == "stock")
                native["cache_delta"]["gets"] = native["cache_after"]["gets"] = 1
            elif failure == "state":
                del edited["cache_before"]["state"]
            elif failure == "delta-state":
                edited["cache_delta"]["state"] = {}
            else:
                edited["cache_delta"]["memory_hits"] = 4
                edited["cache_after"]["memory_hits"] = 4
            with self.subTest(failure=failure):
                self.assert_invalid(metadata, rows, "proof-edit")

    def test_scenarios_are_not_pooled_and_local_failure_does_not_hide_valid_timings(self):
        _, fresh = complete(2)
        metadata, unchanged = workflow_complete()
        metadata["scenarios"] = ["fresh", "unchanged"]
        unchanged[-1]["exit_code"] = 1
        summary = report.summarize(metadata, fresh + unchanged)
        self.assertFalse(summary["valid"])
        self.assertTrue(summary["scenarios"]["fresh"]["valid"])
        self.assertEqual(summary["scenarios"]["fresh"]["aggregates"]["stock"]["samples"], [10, 100])
        self.assertIsNone(summary["scenarios"]["unchanged"]["aggregates"])
        self.assertEqual(summary["expected_samples"], 18)
        self.assertEqual(summary["observed_samples"], 18)
        metadata["problems"].append("Runtime identity changed")
        summary = report.summarize(metadata, fresh + unchanged)
        self.assertTrue(all(not scenario["valid"] and scenario["aggregates"] is None
                            for scenario in summary["scenarios"].values()))

    def test_scenario_setup_failure_does_not_invalidate_other_scenarios(self):
        _, fresh = complete(2)
        metadata, unchanged = workflow_complete()
        metadata["scenarios"] = ["fresh", "unchanged"]
        metadata["scenario_problems"] = {"unchanged": ["Baseline setup failed"]}
        summary = report.summarize(metadata, fresh + unchanged)
        self.assertFalse(summary["valid"])
        self.assertTrue(summary["scenarios"]["fresh"]["valid"])
        self.assertFalse(summary["scenarios"]["unchanged"]["valid"])
        self.assertIsNone(summary["scenarios"]["unchanged"]["aggregates"])
        self.assertIsNone(summary["scenarios"]["unchanged"]["paired_speedups"])

    def test_edit_hash_identifies_each_declared_source_state(self):
        edit_metadata, _ = workflow_complete("proof-edit")
        cases = (("proof-edit", "warmup", "b"), ("proof-edit", "measured", "a"),
                 ("unchanged", "measured", "b"), ("fresh", "measured", "b"),
                 ("daemon-restart", "measured", "b"))
        for scenario, phase, wrong_hash in cases:
            if scenario == "fresh":
                metadata, rows = complete(2)
            elif scenario == "daemon-restart":
                metadata, rows = restart_complete()
            else:
                metadata, rows = workflow_complete(scenario)
            metadata["edit"] = copy.deepcopy(edit_metadata["edit"])
            for row in rows:
                row["edit_sha256"] = ("b" if row["source_state"] == "edited" else "a") * 64
            summary = report.summarize(metadata, rows)
            self.assertTrue(summary["valid"], summary["problems"])
            changed = next(row for row in rows if row["phase"] == phase and row["mode"] == "sail")
            changed["edit_sha256"] = wrong_hash * 64
            with self.subTest(scenario=scenario, phase=phase):
                self.assert_invalid(metadata, rows, scenario)

    def test_source_checks_and_metadata_identity_fail_closed(self):
        for failure in ("before", "after", "missing-check", "package", "crate", "edit", "schema"):
            metadata, rows = workflow_complete("proof-edit")
            if failure in ("before", "after"):
                rows[-1]["source_checks"][failure] = ["Source mutation"]
            elif failure == "missing-check":
                rows[-1].pop("source_checks")
            elif failure == "package":
                metadata["root_package_id"] = None
            elif failure == "crate":
                metadata["root_crate"] = None
            elif failure == "edit":
                metadata["edit"] = None
            else:
                metadata["schema_version"] = 1
            with self.subTest(failure=failure):
                self.assert_invalid(metadata, rows, "proof-edit")

    def test_missing_and_unexpected_samples_are_not_discarded(self):
        metadata, rows = workflow_complete()
        removed = rows.pop()
        unexpected = copy.deepcopy(removed)
        unexpected["scenario"] = "unknown"
        rows.append(unexpected)
        summary = self.assert_invalid(metadata, rows, "unchanged")
        self.assertEqual(summary["samples"][-1]["scenario"], "unknown")
        self.assertTrue(summary["samples"][-1]["problems"])
        self.assertEqual(summary["expected_samples"], 9)

    def test_noncurrent_fresh_baseline_blocks_unchanged_results(self):
        for currentness in ("historical", "unknown"):
            metadata, rows = workflow_complete()
            baseline = next(row for row in rows
                            if row["phase"] == "warmup" and row["mode"] == "sail")
            document = managed_document(reused=False, dependency_reused=False)
            document["currentness"] = currentness
            baseline["proof"] = report.parse_workflow_proof(
                "sail", json.dumps(document), "", ROOT_PACKAGE, ROOT_CRATE)
            with self.subTest(currentness=currentness):
                self.assert_invalid(metadata, rows, "unchanged")



class RestartSummaries(unittest.TestCase):
    def test_restart_retains_disk_and_counts_only_nonmemory_hits(self):
        metadata, rows = restart_complete()
        summary = report.summarize(metadata, rows)
        self.assertTrue(summary["valid"], summary["problems"])
        self.assertEqual(summary["scenarios"]["daemon-restart"]["observed_samples"], 9)
        for row in rows:
            if row["mode"] == "sail" and row["phase"] == "measured":
                self.assertEqual(row["cache_before"]["state"]["disk_entries"], row["restart"]["old_disk_entries"])
                self.assertNotEqual(row["cache_before"]["pid"], row["restart"]["old_pid"])

    def test_restart_rejects_invalid_daemon_and_disk_transitions(self):
        for failure in ("same-pid", "wrong-old-pid", "during-command", "empty-disk", "lost-disk", "nonempty-memory",
                        "pending", "counters", "old-pending", "old-disk", "memory-only", "no-startup",
                        "startup-excluded", "proof", "source"):
            metadata, rows = restart_complete()
            row = next(row for row in rows if row["mode"] == "sail" and row["phase"] == "measured")
            before, after = row["cache_before"], row["cache_after"]
            if failure == "same-pid":
                before["pid"] = after["pid"] = row["restart"]["old_pid"]
            elif failure == "wrong-old-pid":
                row["restart"]["old_pid"] = 999
            elif failure == "during-command":
                after["pid"] += 1
            elif failure == "empty-disk":
                row["restart"]["old_disk_entries"] = before["state"]["disk_entries"] = 0
            elif failure == "lost-disk":
                before["state"]["disk_entries"] -= 1
            elif failure == "nonempty-memory":
                before["state"]["memory_entries"] = 1
            elif failure == "pending":
                before["state"]["pending_writes"] = 1
            elif failure == "old-pending":
                row["restart"]["old_snapshot"]["state"]["pending_writes"] = 1
            elif failure == "old-disk":
                row["restart"]["old_snapshot"]["state"]["disk_entries"] += 1
            elif failure == "counters":
                before["sessions"] = 1
                after["sessions"] += 1
            elif failure == "memory-only":
                row["cache_delta"]["memory_hits"] = row["cache_delta"]["hits"]
                after["memory_hits"] = after["hits"]
            elif failure == "no-startup":
                row.pop("startup_seconds")
            elif failure == "startup-excluded":
                row["startup_seconds"] = row["wall_seconds"] + 1
            elif failure == "proof":
                row["proof"] = workflow_proof("sail", "reused")
            else:
                row["source_state"] = "edited"
            with self.subTest(failure=failure):
                summary = report.summarize(metadata, rows)
                scenario = summary["scenarios"]["daemon-restart"]
                self.assertFalse(scenario["valid"])
                self.assertIsNone(scenario["aggregates"])
                self.assertIsNone(scenario["paired_speedups"])
                self.assertEqual(len(summary["samples"]), len(rows))


class SourceEditBoundaries(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        runner = runpy.run_path(str(Path(__file__).with_name("bench-cargo-compare.py")))
        cls.source_edit = runner["SourceEdit"]

    def source(self, text="fn proof() { assert(true); }\n"):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        project = Path(directory.name) / "private"
        (project / "src").mkdir(parents=True)
        path = project / "src/lib.rs"
        path.write_text(text, encoding="utf-8")
        return project, path

    @staticmethod
    def spec(**changes):
        result = {"path": "src/lib.rs", "before": "assert(true);", "after": "assert(1 == 1);"}
        result.update(changes)
        return result


    def test_edit_rejects_invalid_paths_matches_and_equal_replacements(self):
        project, path = self.source()
        for changes in ({"path": "/tmp/lib.rs"}, {"path": "../lib.rs"}, {"path": "src/../lib.rs"},
                        {"path": "./src/lib.rs"}, {"path": "src//lib.rs"}, {"path": "src/lib.txt"},
                        {"before": ""}, {"before": "absent"}, {"after": "assert(true);"},
                        {"before": None}, {"path": ""}):
            with self.subTest(changes=changes):
                with self.assertRaises((ValueError, OSError)):
                    self.source_edit(project, project, {"src/lib.rs"}, self.spec(**changes))
        with self.assertRaises(ValueError):
            self.source_edit(project, project, set(), self.spec())
        path.write_text("assert(true); assert(true);", encoding="utf-8")
        with self.assertRaises(ValueError):
            self.source_edit(project, project, {"src/lib.rs"}, self.spec())
        path.write_text("aaa", encoding="utf-8")
        with self.assertRaises(ValueError):
            self.source_edit(project, project, {"src/lib.rs"}, self.spec(before="aa", after="a"))

    def test_edit_allows_empty_replacement_but_not_repeated_or_unknown_states(self):
        project, path = self.source()
        edit = self.source_edit(project, project, {"src/lib.rs"}, self.spec(after=""))
        with self.assertRaises(ValueError):
            edit.transition("base")
        with self.assertRaises(ValueError):
            edit.transition("unknown")
        edit.transition("edited")
        self.assertNotIn(b"assert(true);", path.read_bytes())
        with self.assertRaises(ValueError):
            edit.transition("edited")
        edit.transition("base")
        self.assertEqual(edit.problems("base"), [])
        self.assertTrue(edit.problems("unknown"))

    def test_edit_refuses_source_mutation_and_symlink_escape(self):
        project, path = self.source()
        edit = self.source_edit(project, project, {"src/lib.rs"}, self.spec())
        path.write_text("changed", encoding="utf-8")
        self.assertTrue(edit.problems("base"))
        with self.assertRaises(RuntimeError):
            edit.transition("edited")
        outside = project.parent / "outside.rs"
        outside.write_text("assert(true);", encoding="utf-8")
        path.unlink()
        path.symlink_to(outside)
        self.assertTrue(edit.problems("base"))
        with self.assertRaises(ValueError):
            self.source_edit(project, project, {"src/lib.rs"}, self.spec())
        self.assertEqual(outside.read_text(encoding="utf-8"), "assert(true);")




class PinnedInputs(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        runner = runpy.run_path(str(Path(__file__).with_name("bench-cargo-compare.py")))
        cls.runner_type = runner["Runner"]
        cls.identity = staticmethod(runner["identity"])
        cls.local_sources = staticmethod(runner["local_source_problems"])

    def setUp(self):
        directory = tempfile.TemporaryDirectory(dir="/tmp")
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name).resolve()
        self.runner = self.runner_type.__new__(self.runner_type)
        self.runner.scratch = self.root / "owned"
        self.runner.scratch.mkdir()
        tools = {}
        for name in ("sail-z3", "toolchain-cargo", "toolchain-rustc"):
            path = self.root / name
            path.write_bytes(b"runtime-v1")
            tools[name] = self.identity(path)
        manifest = self.root / "version.json"
        manifest.write_text("{}", encoding="utf-8")
        self.runner.metadata = {
            "tools": tools, "implementation": {}, "runtimes": {},
            "stock_version_manifest": {"identity": self.identity(manifest)},
        }

    def assert_rejected(self, problems):
        metadata, rows = complete()
        metadata["problems"] = problems
        summary = report.summarize(metadata, rows)
        self.assertFalse(summary["valid"])
        self.assertIsNone(summary["scenarios"]["fresh"]["aggregates"])
        self.assertIsNone(summary["scenarios"]["fresh"]["paired_speedups"])

    def test_runtime_replacements_suppress_timing_statistics(self):
        initial = set(self.runner.runtime_problems())
        for name in self.runner.metadata["tools"]:
            with self.subTest(name=name):
                path = self.root / name
                path.write_bytes(b"runtime-v2")
                self.assert_rejected([
                    issue for issue in self.runner.runtime_problems() if issue not in initial])
                path.write_bytes(b"runtime-v1")

    def test_late_ancestor_configuration_suppresses_timing_statistics(self):
        initial = set(self.runner.runtime_problems())
        cargo = self.root / ".cargo"
        cargo.mkdir()
        for name in ("config", "config.toml"):
            with self.subTest(name=name):
                path = cargo / name
                path.write_text("[build]\njobs=1\n", encoding="utf-8")
                self.assert_rejected([
                    issue for issue in self.runner.runtime_problems() if issue not in initial])
                path.unlink()

    def test_external_and_symlink_local_dependencies_suppress_statistics(self):
        owned = self.runner.scratch / "Cargo.toml"
        owned.write_text("[package]\nname='demo'\nversion='0.1.0'\n", encoding="utf-8")
        external = self.root / "external.toml"
        external.write_text("[package]\nname='external'\nversion='0.1.0'\n", encoding="utf-8")
        packages = [
            {"manifest_path": str(owned), "source": None},
            {"manifest_path": str(external), "source": "registry+https://example.invalid"},
        ]
        self.assertEqual(self.local_sources({"packages": packages}, self.runner.scratch), [])
        link = self.runner.scratch / "linked.toml"
        link.symlink_to(external)
        for manifest in (external, link):
            with self.subTest(manifest=manifest):
                self.assert_rejected(self.local_sources(
                    {"packages": packages + [{"manifest_path": str(manifest), "source": None}]},
                    self.runner.scratch))


class ProcessCleanup(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        runner = runpy.run_path(str(Path(__file__).with_name("bench-cargo-compare.py")))
        cls.stop_group = staticmethod(runner["stop_group"])

    def child(self, ignore_term=False):
        code = "import signal, time\n"
        if ignore_term:
            code += "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
        code += "print('ready', flush=True)\ntime.sleep(60)\n"
        process = subprocess.Popen(
            [sys.executable, "-c", code], stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL, text=True, start_new_session=True)
        self.addCleanup(self.finish_child, process)
        process.stdout.readline()
        return process

    @staticmethod
    def finish_child(process):
        if process.poll() is None:
            process.kill()
        process.wait(timeout=5)
        process.stdout.close()

    def test_stops_owned_group_without_an_unrelated_child(self):
        owned = self.child()
        unrelated = self.child()
        self.stop_group(owned)
        self.assertEqual(owned.returncode, -signal.SIGTERM)
        self.assertIsNone(unrelated.poll())

    def test_escalates_when_the_owned_group_ignores_termination(self):
        owned = self.child(ignore_term=True)
        self.stop_group(owned)
        self.assertEqual(owned.returncode, -signal.SIGKILL)

    @staticmethod
    def finish_descendant(pid):
        try:
            os.kill(pid, signal.SIGKILL)
        except ProcessLookupError:
            pass

    def test_descendants_stop_after_the_group_leader_exits(self):
        worker = ("import signal,time\n"
                  "signal.signal(signal.SIGTERM, signal.SIG_IGN)\n"
                  "print('ready', flush=True)\n"
                  "time.sleep(60)\n")
        for leader_done in (False, True):
            with self.subTest(leader_done=leader_done):
                read_fd, write_fd = os.pipe()
                self.addCleanup(os.close, read_fd)
                leader = ("import os,subprocess,sys,time\n"
                          "child=subprocess.Popen([sys.executable,'-c',%r],stdout=subprocess.PIPE,"
                          "pass_fds=(%d,))\n"
                          "os.close(%d)\n"
                          "print(child.pid,flush=True)\n"
                          "print(child.stdout.readline().decode().strip(),flush=True)\n"
                          "%s\n" % (worker, write_fd, write_fd,
                                     "" if leader_done else "time.sleep(60)"))
                owned = subprocess.Popen(
                    [sys.executable, "-c", leader], stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL, text=True, start_new_session=True,
                    pass_fds=(write_fd,))
                os.close(write_fd)
                self.addCleanup(self.finish_child, owned)
                descendant = int(owned.stdout.readline())
                self.addCleanup(self.finish_descendant, descendant)
                self.assertEqual(owned.stdout.readline().strip(), "ready")
                if leader_done:
                    owned.wait(timeout=5)
                self.stop_group(owned)
                self.assertEqual(select.select([read_fd], [], [], 1)[0], [read_fd])
                self.assertEqual(os.read(read_fd, 1), b"")


if __name__ == "__main__":
    unittest.main()
