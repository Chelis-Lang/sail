#!/usr/bin/env python3
"""Preparation/admission/lifecycle regressions, not substitutes for real reader acceptance."""
import copy
import importlib.util
import json
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("warm_reader", Path(__file__).with_name("m7-reader-acceptance.py"))
harness = importlib.util.module_from_spec(spec)
spec.loader.exec_module(harness)
fixtures = harness.load("retained_admission_fixtures", Path(__file__).with_name("test-m7-retained-jobs-acceptance.py"))


class WarmReaderTests(unittest.TestCase):
    def document(self):
        document = fixtures.EvidenceAdmissionTests().document()
        document["repair"]["batch"].update(manifest_sha256="manifest", focus={"currentness": "unknown"}, selection_error=None)
        document["result"]["notes"] = []
        document["job"].update(version=1, id="owned", project_dir="/fixture", args=["--package", "demo", "--lib"])
        return document

    def runner(self, root):
        run = harness.Run.__new__(harness.Run)
        run.out = root
        run.bin = root / "bin"
        run.env = {}
        run.cache = root / "cache"
        run.socket = root / "cache.sock"
        run.job_started = {}
        run.job_scopes = {}
        run.owned = []
        run.args = SimpleNamespace(package="demo", target="demo", kind="lib", function="demo::f", setup_timeout=600, prepare_contexts=False)
        run.summary = {"setup": [], "commands": [], "proofs": {}, "worker_archives": {}, "archival_errors": [], "cleanup_errors": [], "unmeasured_preparation": {"enabled": False, "phases": []}}
        return run

    def frozen_runner(self, root):
        run = self.runner(root)
        run.bin.mkdir()
        run.probe_dir = root / "probes"
        run.probe_dir.mkdir()
        run.args.python_probe = run.probe_dir / "python-reader-probe.py"
        for directory, names in [(run.bin, ("sail", "sail-z3")),
                                 (run.probe_dir, ("job_read_bench", "python-reader-probe.py"))]:
            files = {}
            for name in names:
                (directory / name).write_bytes(name.encode())
                files[name] = {"sha256": harness.bench.sha(directory / name)}
            (directory / "freeze.json").write_text(json.dumps({"files": files}))
        return run

    def test_product_ownership_failure_is_retained_before_any_setup(self):
        with tempfile.TemporaryDirectory() as temporary:
            run = self.frozen_runner(Path(temporary))
            with patch.object(harness.native_product, "identity", return_value={"fsuid": os.geteuid() + 1, "cap_lease": False}):
                with self.assertRaisesRegex(ValueError, "SAIL0948"):
                    run.freeze()
            self.assertEqual(run.summary["commands"], [])
            record = run.summary["product_ownership_prerequisite"]
            self.assertFalse(record["ownership_prerequisite_met"])
            self.assertFalse(record["currentness_certified"])
            self.assertEqual(record["supplied_bindings"], 2)

    def test_optional_complete_inventory_audit_does_not_claim_current(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = self.frozen_runner(root)
            source = root / "runtime-input"
            source.write_bytes(b"runtime")
            metadata = source.stat()
            rows = [{"path": list(os.fsencode(source)), "canonical": list(os.fsencode(source.resolve())),
                     "device": metadata.st_dev, "inode": metadata.st_ino}]
            run.args.lease_inventory = root / "guard.json"
            run.args.lease_inventory.write_text(json.dumps(rows))
            with patch.object(harness.native_product, "identity", return_value={"fsuid": os.geteuid(), "cap_lease": False}):
                run.freeze()
            record = run.summary["supplied_guard_ownership_prerequisite"]
            self.assertTrue(record["ownership_prerequisite_met"])
            self.assertFalse(record["currentness_certified"])
            self.assertEqual(record["inventory_sha256"], harness.bench.sha(run.args.lease_inventory))
            self.assertIn("m7-native-product.py", run.summary["controller_scripts"])
            rows[0]["inode"] += 1
            run.args.lease_inventory.write_text(json.dumps(rows))
            with patch.object(harness.native_product, "identity", return_value={"fsuid": os.geteuid(), "cap_lease": False}):
                with self.assertRaisesRegex(ValueError, "binding changed"):
                    run.freeze()

    def test_current_independent_full_proof_and_exact_manifest_are_required(self):
        document = self.document()
        self.assertEqual(harness.setup_state(document["repair"], "manifest", False), "ready")
        self.assertIs(harness.admit(document, "manifest", "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f"), document["result"])
        mutations = [lambda d: d["repair"]["batch"].update(manifest_sha256="foreign"),
                     lambda d: d["repair"]["batch"].update(selection_error={"code": "SAIL3122"}),
                     lambda d: d["job"].update(input_revision="other"),
                     lambda d: d["result"].update(currentness="unknown"),
                     lambda d: d["result"]["targets"][0].update(kind=["bin"])]
        for mutation in mutations:
            changed = copy.deepcopy(document)
            mutation(changed)
            with self.assertRaises(AssertionError):
                harness.admit(changed, "manifest", "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")

    def test_declaration_establishment_is_one_explicit_transition_not_generic_retry(self):
        document = self.document()["repair"]
        document["accepted"] = False
        document["batch"]["full"]["currentness"] = "unknown"
        with self.assertRaises(ValueError):
            harness.setup_state(document, "manifest", True)
        document["batch"]["full"]["notes"] = ["Build-script input declarations were established during this run"]
        self.assertEqual(harness.setup_state(document, "manifest", True), "declarations-established")
        with self.assertRaises(ValueError):
            harness.setup_state(document, "manifest", False)
        dependency = copy.deepcopy(document["batch"]["full"]["targets"][0])
        dependency.update(requested=False, complete=False)
        document["batch"]["full"]["targets"].append(dependency)
        with self.assertRaises(AssertionError):
            harness.setup_state(document, "manifest", True)

    def test_full_dependency_setup_precedes_exact_context_and_generic_unknown_stops(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = self.runner(root)
            metadata = {"packages": [{"name": "demo", "id": "path+file:///fixture#demo@0.0.0", "targets": [{"name": "demo", "kind": ["lib"]}]}]}
            current = self.document()
            cold = copy.deepcopy(current)
            cold["repair"]["accepted"] = False
            cold["result"].update(currentness="unknown", notes=["Build-script input declarations were established during this run"])
            calls = []
            def command(label, argv, workspace, env, timeout=30):
                calls.append((label, timeout))
                return metadata if label.endswith("metadata") else current["result"]
            def submit(label, workspace):
                calls.append((label, "submit"))
                return label
            with patch.object(run, "command", side_effect=command), patch.object(run, "submit", side_effect=submit), patch.object(run, "wait", side_effect=[cold, current]):
                job = run.warm("baseline", root, "manifest")
            self.assertEqual(job, "baseline-repair-context-1")
            self.assertEqual(calls, [("baseline-metadata", 30), ("baseline-ordinary-full", 600),
                                     ("baseline-repair-context-0", "submit"), ("baseline-repair-context-1", "submit")])
            self.assertTrue(run.summary["proofs"]["baseline"]["complete_current"])
            unknown = copy.deepcopy(current)
            unknown["repair"]["accepted"] = False
            unknown["result"]["currentness"] = "unknown"
            with patch.object(run, "command", side_effect=command), patch.object(run, "submit", side_effect=submit) as submissions, patch.object(run, "wait", return_value=unknown):
                with self.assertRaises(ValueError):
                    run.warm("active", root, "manifest")
                self.assertEqual(submissions.call_count, 1)

    def test_explicit_preparation_is_separate_and_restores_the_original_budget(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = self.runner(root)
            observed = []
            def warm(label, workspace, manifest):
                observed.append((label, workspace, manifest, run.args.setup_timeout))
                return label + "-job"
            workspaces = {"baseline": root / "baseline", "active": root / "active"}
            with patch.object(run, "warm", side_effect=warm):
                run.args.prepare_contexts = True
                self.assertEqual(run.establish_workspaces(workspaces, "manifest"), "baseline-job")
            self.assertEqual(observed, [
                ("baseline-unmeasured-preparation", workspaces["baseline"], "manifest", 0),
                ("active-unmeasured-preparation", workspaces["active"], "manifest", 0),
                ("baseline", workspaces["baseline"], "manifest", 600),
                ("active", workspaces["active"], "manifest", 600)])
            self.assertEqual(run.args.setup_timeout, 600)
            self.assertTrue(all(phase["completed"] and phase["subsequent_setup_budget_s"] == 600
                                and phase["deadline_s"] is None and not phase["measured_reader_latency"]
                                for phase in run.summary["unmeasured_preparation"]["phases"]))
            observed.clear()
            run.args.prepare_contexts = False
            with patch.object(run, "warm", side_effect=warm):
                run.establish_workspaces(workspaces, "manifest")
            self.assertEqual([entry[0] for entry in observed], ["baseline", "active"])
            self.assertTrue(all(entry[3] == 600 for entry in observed))

    def test_preparation_interruption_keeps_failure_and_restores_bounded_policy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = self.runner(root)
            with patch.object(run, "warm", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    run.prepare("baseline", root, "manifest")
            self.assertEqual(run.args.setup_timeout, 600)
            phase = run.summary["unmeasured_preparation"]["phases"][0]
            self.assertFalse(phase["completed"])
            self.assertEqual(phase["failure"]["type"], "KeyboardInterrupt")
            self.assertGreaterEqual(phase["wall_ms"], 0)
            self.assertFalse(run.summary["proofs"])
            run.args.setup_timeout = 0
            with self.assertRaises(AssertionError):
                run.prepare("baseline", root, "manifest")

    def projection(self):
        original = self.document()["repair"]
        projected = self.document()
        projected["job"]["currentness"] = "unknown"
        projected["result"].update(currentness="unknown", notes=[harness.PROJECTION_NOTE])
        projected["repair"]["accepted"] = False
        return original, projected

    def pending(self, public, original):
        return harness.pending_projection(public, original, "manifest",
            "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")

    def test_only_exact_owned_current_evidence_authorizes_projection_wait_not_admission(self):
        original, projected = self.projection()
        self.assertTrue(self.pending(projected, original))
        with self.assertRaises(AssertionError):
            harness.admit(projected, "manifest", "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")
        for currentness in ("unknown", "historical"):
            changed = copy.deepcopy(original)
            changed["batch"]["full"]["currentness"] = currentness
            self.assertFalse(self.pending(projected, changed))
        for mutation in (lambda d: d["job"].update(input_revision="other"),
                         lambda d: d["result"].update(notes=[]),
                         lambda d: d["result"]["targets"][0].update(package_id="another")):
            changed = copy.deepcopy(projected)
            mutation(changed)
            with self.assertRaises(AssertionError):
                self.pending(changed, original)
        for mutation in (lambda d: d["batch"].update(manifest_sha256="other"),
                         lambda d: d["batch"]["full"]["targets"][0].update(complete=False),
                         lambda d: d["batch"]["full"]["targets"][0]["report"]["crates"][0]["functions"][0].update(tier="provisional")):
            changed = copy.deepcopy(original)
            mutation(changed)
            with self.assertRaises(AssertionError):
                self.pending(projected, changed)

    def projection_runner(self, root, responses, layout="results/projects/jobs"):
        run = self.runner(root)
        run.job_started["owned"] = time.monotonic()
        original, _ = self.projection()
        self.write_job(run.cache, layout, original)
        calls = []
        def call(address, request):
            calls.append(request)
            return next(responses), 1.0, 100
        run.probe = SimpleNamespace(endpoint=lambda socket: "owned-endpoint", call=call)
        return run, calls

    def write_job(self, cache, layout, original=None, state="completed"):
        original = original or self.document()["repair"]
        directory = cache / layout / "owned"
        directory.mkdir(parents=True)
        payload = json.dumps(original).encode()
        (directory / "stdout").write_bytes(payload)
        (directory / "stderr").write_text("actual verifier diagnostic")
        status = dict(self.document()["job"], state=state)
        record = {"status": status,
                  "submission": {"project_dir": status["project_dir"], "args": status["args"],
                                 "environment": {"DO_NOT_ARCHIVE": "secret-marker"}},
                  "projection": {"stdout_sha256": hashlib.sha256(payload).hexdigest()}}
        (directory / "record.json").write_text(json.dumps(record))
        return directory, record

    def test_both_actual_layouts_retain_exact_owned_files_without_environment(self):
        for layout in ("projects/jobs", "results/projects/jobs"):
            with self.subTest(layout=layout), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                directory, _ = self.write_job(root / "cache", layout)
                original = harness.owned_original(root / "cache", "owned", self.document()["job"])
                self.assertEqual(original, self.document()["repair"])
                evidence = harness.archive_owned_job(root / "cache", "owned", root / "archive")
                self.assertEqual(evidence["layout"], layout)
                self.assertEqual(evidence["missing"], ["events.jsonl", "proof-source.jsonl"])
                self.assertEqual((root / "archive/stdout").read_bytes(), (directory / "stdout").read_bytes())
                self.assertEqual(evidence["files"]["stdout"]["sha256"], hashlib.sha256((directory / "stdout").read_bytes()).hexdigest())
                self.assertFalse(evidence["files"]["stdout"]["changed_during_copy"])
                self.assertFalse((root / "archive/record.json").exists())
                self.assertNotIn("secret-marker", (root / "archive/archive.json").read_text())

    def test_ambiguous_or_missing_layouts_cannot_silently_select_or_omit_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_job(root / "cache", "projects/jobs")
            self.write_job(root / "cache", "results/projects/jobs")
            with self.assertRaisesRegex(AssertionError, "missing or ambiguous"):
                harness.archive_owned_job(root / "cache", "owned", root / "archive")
            self.assertFalse((root / "archive").exists())
            with self.assertRaisesRegex(AssertionError, "missing or ambiguous"):
                harness.owned_original(root / "cache", "absent", self.document()["job"])
            shutil = harness.shutil
            shutil.rmtree(root / "cache/results/projects/jobs/owned")
            (root / "cache/projects/jobs/owned/stdout").unlink()
            with self.assertRaisesRegex(AssertionError, "completed owned job output is missing"):
                harness.archive_owned_job(root / "cache", "owned", root / "missing-output")
            self.assertIn("stdout", json.loads((root / "missing-output/archive.json").read_text())["missing"])

    def test_mismatched_record_scope_revision_seal_and_duplicate_fields_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory, original = self.write_job(root / "cache", "projects/jobs")
            for mutation in (lambda d: d["status"].update(id="foreign"),
                             lambda d: d["submission"].update(project_dir="/different"),
                             lambda d: d["submission"].update(args=[]),
                             lambda d: d["status"].update(version=2)):
                record = copy.deepcopy(original)
                mutation(record)
                (directory / "record.json").write_text(json.dumps(record))
                with self.assertRaises(AssertionError):
                    harness.archive_owned_job(root / "cache", "owned", root / "not-created")
                self.assertFalse((root / "not-created").exists())
            (directory / "record.json").write_text(json.dumps(original))
            changed = copy.deepcopy(original)
            changed["status"]["project_dir"] = changed["submission"]["project_dir"] = "/foreign"
            (directory / "record.json").write_text(json.dumps(changed))
            with self.assertRaisesRegex(AssertionError, "project_dir"):
                harness.archive_owned_job(root / "cache", "owned", root / "not-created", self.document()["job"])
            self.assertFalse((root / "not-created").exists())
            (directory / "record.json").write_text(json.dumps(original))
            expected = dict(self.document()["job"], input_revision="another")
            with self.assertRaisesRegex(AssertionError, "input_revision"):
                harness.owned_original(root / "cache", "owned", expected)
            (directory / "stdout").write_text('{"accepted": false}')
            with self.assertRaisesRegex(AssertionError, "seal mismatch"):
                harness.owned_original(root / "cache", "owned", self.document()["job"])
            (directory / "record.json").write_text('{"status": {}, "status": {}}')
            with self.assertRaisesRegex(AssertionError, "duplicate owned"):
                harness.archive_owned_job(root / "cache", "owned", root / "not-created")

    def test_unsafe_ids_symlink_directories_files_and_fifo_never_supply_owned_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory, _ = self.write_job(root / "cache", "projects/jobs")
            for identifier in ("../owned", "owned/child", "/owned", "owned..", "", "owned\\child"):
                with self.assertRaisesRegex(AssertionError, "unsafe owned job ID"):
                    harness.archive_owned_job(root / "cache", identifier, root / "not-created")
            for name in ("record.json", "stdout"):
                path = directory / name
                original = path.read_bytes()
                path.unlink()
                foreign = root / ("foreign-" + name)
                foreign.write_bytes(original)
                path.symlink_to(foreign)
                with self.assertRaises(OSError):
                    harness.owned_original(root / "cache", "owned", self.document()["job"])
                path.unlink()
                path.write_bytes(original)
            stdout = directory / "stdout"
            stdout.unlink()
            os.mkfifo(stdout)
            with self.assertRaisesRegex(AssertionError, "not a regular file"):
                harness.owned_original(root / "cache", "owned", self.document()["job"])
            stdout.unlink()
            for relative in ("projects/jobs/owned", "projects/jobs", "projects"):
                path = root / "cache" / relative
                moved = path.with_name(path.name + "-moved")
                path.rename(moved)
                path.symlink_to(moved, target_is_directory=True)
                with self.assertRaises(OSError):
                    harness.archive_owned_job(root / "cache", "owned", root / "not-created")
                self.assertFalse((root / "not-created").exists())
                path.unlink()
                moved.rename(path)

    def test_archival_failure_is_explicit_and_cannot_skip_exact_owned_cancellation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            run = self.runner(root)
            run.owned = ["owned"]
            run.job_scopes["owned"] = {field: self.document()["job"][field] for field in ("id", "project_dir", "args")}
            self.write_job(run.cache, "projects/jobs")
            self.write_job(run.cache, "results/projects/jobs")
            calls = []
            def call(address, request):
                calls.append(request)
                state = "running" if request["operation"] == "status" else "cancelled"
                return {"response": "status", "value": dict(self.document()["job"], state=state)}, 1.0, 100
            run.probe = SimpleNamespace(endpoint=lambda socket: "owned-endpoint", call=call)
            run.finish_jobs()
            self.assertEqual(calls, [{"operation": "status", "id": "owned"},
                                     {"operation": "cancel", "id": "owned", "superseded": False}])
            self.assertEqual(run.summary["worker_archives"], {})
            self.assertEqual(len(run.summary["archival_errors"]), 1)
            self.assertIn("missing or ambiguous", run.summary["archival_errors"][0]["message"])
            self.assertEqual(run.summary["cleanup_errors"], [])
            self.assertEqual(json.loads((root / "summary.json").read_text())["archival_errors"], run.summary["archival_errors"])

    def test_directory_replacement_cannot_redirect_open_owned_evidence(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            directory, _ = self.write_job(root / "cache", "projects/jobs")
            with harness.owned_job(root / "cache", "owned", self.document()["job"]) as (descriptor, _, _):
                directory.rename(directory.with_name("retired"))
                directory.mkdir()
                (directory / "stdout").write_text("substituted")
                with harness.owned_file(descriptor, "stdout") as stream:
                    self.assertEqual(json.load(stream), self.document()["repair"])

    def test_same_job_establishment_retains_unknown_then_requires_public_current(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, projected = self.projection()
            current = self.document()
            unknown_status = {"response": "status", "value": projected["job"]}
            ready_status = {"response": "status", "value": current["job"]}
            responses = iter([unknown_status, {"response": "result", "value": projected},
                              unknown_status, ready_status, {"response": "result", "value": current}])
            run, calls = self.projection_runner(root, responses)
            with patch.object(harness.time, "sleep"):
                result = run.wait("establish", "owned", "manifest", "path+file:///fixture#demo@0.0.0")
            self.assertEqual(result, current)
            self.assertEqual([request["operation"] for request in calls], ["status", "result", "status", "status", "result"])
            self.assertTrue(all(request["id"] == "owned" for request in calls))
            self.assertEqual(json.loads((root / "establish-result-1.json").read_text())["value"]["result"]["currentness"], "unknown")
            self.assertTrue(run.summary["setup"][0]["awaited_original_current_projection"])
            self.assertEqual(run.summary["setup"][0]["result_reads"], 2)
            harness.admit(result, "manifest", "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")

    def test_legacy_layout_still_requires_final_public_current_on_the_same_job(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, projected = self.projection()
            current = self.document()
            responses = iter([{"response": "status", "value": projected["job"]},
                              {"response": "result", "value": projected},
                              {"response": "status", "value": current["job"]},
                              {"response": "result", "value": current}])
            run, calls = self.projection_runner(root, responses, "projects/jobs")
            with patch.object(harness.time, "sleep"):
                result = run.wait("legacy", "owned", "manifest", "path+file:///fixture#demo@0.0.0")
            self.assertEqual(result, current)
            self.assertTrue(all(request["id"] == "owned" for request in calls))
            self.assertTrue(run.summary["setup"][0]["awaited_original_current_projection"])
            self.assertEqual(json.loads((root / "legacy-result-1.json").read_text())["value"]["result"]["currentness"], "unknown")
            harness.admit(result, "manifest", "path+file:///fixture#demo@0.0.0", "demo", "lib", "demo::f")

    def test_cancelled_job_during_projection_wait_returns_failure_and_budget_never_resets(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            _, projected = self.projection()
            cancelled = copy.deepcopy(projected)
            cancelled["job"]["state"] = "cancelled"
            cancelled["result"] = None
            responses = iter([{"response": "status", "value": projected["job"]},
                              {"response": "result", "value": projected},
                              {"response": "status", "value": cancelled["job"]},
                              {"response": "result", "value": cancelled}])
            run, calls = self.projection_runner(root, responses)
            with patch.object(harness.time, "sleep"):
                self.assertEqual(run.wait("cancelled", "owned", "manifest", "path+file:///fixture#demo@0.0.0"), cancelled)
            self.assertEqual(len(calls), 4)
            run.job_started["owned"] = 0
            with patch.object(harness.time, "monotonic", return_value=600):
                with self.assertRaisesRegex(AssertionError, "preparation budget exceeded"):
                    run.wait("exhausted", "owned", "manifest", "path+file:///fixture#demo@0.0.0")
            self.assertEqual(len(calls), 4, "expired per-job budget cannot start another RPC")

    def test_source_copy_excludes_actual_output_and_preserves_nested_target_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            for directory in [source / "target", source / "src/target", source / ".git"]:
                directory.mkdir(parents=True)
            (source / "target/artifact").write_text("compiled")
            (source / "src/target/mod.rs").write_text("legitimate proof source")
            (source / ".git/config").write_text("git metadata")
            harness.copy_source(source, root / "copy", source / "target")
            self.assertFalse((root / "copy/target").exists())
            self.assertFalse((root / "copy/.git").exists())
            self.assertEqual((root / "copy/src/target/mod.rs").read_text(), "legitimate proof source")

    def test_collector_admission_preserves_both_schemas_and_exact_bounds(self):
        compiler = {"pid": 17, "parent": 16, "started_ticks": 7, "crate_name": "demo"}
        value = {"passed": True, "job": "baseline", "active_job": "active", "input_revision": "revision",
                 "active_crate": "demo", "clients": 4, "deadline_seconds": 5,
                 "summary": {operation: {"count": 100, "p95_ms": 199} for operation in ("status", "result")},
                 "samples": [{"operation": "status" if index % 2 == 0 else "result", "current": True,
                              "active_before": "running", "active_after": "running", "verifier_continued": True}
                             for index in range(200)]}
        harness.admit_census(value, "baseline", "active", "revision", "demo")
        rust = copy.deepcopy(value)
        for sample in rust["samples"]:
            del sample["verifier_continued"]
            sample.update(active_before=True, active_after=True, error=None,
                          verifiers_before=[compiler], verifiers_after=[compiler])
        harness.admit_census(rust, "baseline", "active", "revision", "demo")
        mutations = [lambda d: d.update(passed=False), lambda d: d.update(input_revision="other"),
                     lambda d: d.update(active_crate="dependency"), lambda d: d.update(deadline_seconds=6),
                     lambda d: d["summary"]["status"].update(p95_ms=200),
                     lambda d: d["samples"][0].update(current=False),
                     lambda d: d["samples"][0].update(verifiers_after=[])]
        for mutation in mutations:
            changed = copy.deepcopy(rust)
            mutation(changed)
            with self.assertRaises(AssertionError):
                harness.admit_census(changed, "baseline", "active", "revision", "demo")

    def test_command_timeout_interrupts_only_real_owned_ready_process_and_retains_failure(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "ready"
            process = subprocess.Popen([sys.executable, "-c", "import pathlib,time,sys; pathlib.Path(sys.argv[1]).write_text('ready'); time.sleep(30)", str(marker)], start_new_session=True,
                                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                started = time.monotonic()
                while not marker.exists():
                    self.assertIsNone(process.poll())
                    self.assertLess(time.monotonic() - started, 2, "separate real child startup handshake")
                    time.sleep(.01)
                run = self.runner(root)
                began = time.monotonic()
                actual_interrupt = harness.bench.interrupt_owned
                def cleanup_failure(child):
                    actual_interrupt(child)
                    raise OSError("retained secondary cleanup observation")
                with patch.object(harness.subprocess, "Popen", return_value=process), patch.object(harness.bench, "interrupt_owned", side_effect=cleanup_failure):
                    with self.assertRaises(subprocess.TimeoutExpired):
                        run.command("interrupted", ["already-ready-owned-child"], root, timeout=.1)
                self.assertLess(time.monotonic() - began, 2, "observation failure must cancel promptly")
                self.assertIsNotNone(process.poll())
                record = run.summary["setup"][0]
                self.assertEqual(record["failure"]["type"], "TimeoutExpired")
                self.assertEqual(record["failure"]["cleanup_error"]["type"], "OSError")
                self.assertFalse(record["measured_reader_latency"])
                self.assertTrue((root / "interrupted/command.json").is_file())
            finally:
                if process.poll() is None:
                    harness.bench.interrupt_owned(process)


if __name__ == "__main__":
    unittest.main()
