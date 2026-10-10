#!/usr/bin/env python3
"""Preparation/admission/lifecycle regressions, not substitutes for real reader acceptance."""
import copy
import importlib.util
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
        return document

    def runner(self, root):
        run = harness.Run.__new__(harness.Run)
        run.out = root
        run.bin = root / "bin"
        run.env = {}
        run.owned = []
        run.args = SimpleNamespace(package="demo", target="demo", kind="lib", function="demo::f", setup_timeout=600)
        run.summary = {"setup": [], "commands": [], "proofs": {}}
        return run

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
