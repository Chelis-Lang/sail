#!/usr/bin/env python3
"""No-proof controls for the external original-reader lifecycle wrapper."""
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("current_reader", Path(__file__).with_name("m7-current-reader-workflow.py"))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)


class OwnershipControls(unittest.TestCase):
    def simulate(self, fault=None, finalization=None):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            class Actor:
                pid = 23
                returncode = None
                def poll(self):
                    return self.returncode
                def wait(self, timeout):
                    self.returncode = 0
                    calls.append("join")
                    if finalization == "cleanup":
                        raise RuntimeError("cleanup failed")
                    return self.returncode
            class Run:
                out = root
                bin = root
                cache = root / "cache"
                socket = root / "cache.sock"
                daemon = Actor()
                args = SimpleNamespace(environment_label="test")
                summary = {"passed": False, "proofs": {"baseline": {"job": "new-baseline"}},
                           "censuses": {}, "retained_attempted": False}
                def run(self):
                    for stage in ("preparation", "python-census", "rust-census"):
                        calls.append(stage)
                        if fault == stage:
                            raise TimeoutError("original " + stage)
                        if stage.endswith("census"):
                            self.summary["censuses"][stage] = {"passed": True}
                    self.summary["passed"] = True
                def save(self):
                    pass
                def command(self, label, argv, cwd, timeout, json_output):
                    calls.append(label)
                    assert timeout == 145 and json_output is False
                    assert "--job" in argv and "new-baseline" in argv
                    destination = root / "retained-raw"
                    destination.mkdir()
                    (destination / "summary.json").write_text(json.dumps({"passed": fault != "retained"}))
            run = Run()
            args = SimpleNamespace(scripts_dir=root)
            def signal(actor, number):
                self.assertIs(actor, run.daemon)
                calls.append("signal")
            def timers(*args):
                calls.append("timers")
                if finalization == "timers":
                    raise OSError("archive failed")
            def after():
                calls.append("seal")
                return {"seal": "changed" if finalization == "seal" else "original"}
            helper = SimpleNamespace(bench=SimpleNamespace(signal_owned=signal))
            error = None
            with patch.object(control.support, "archive_timers", timers):
                try:
                    control.execute(run, args, helper, {"seal": "original"}, 0, after)
                except (TimeoutError, AssertionError, RuntimeError) as failure:
                    error = failure
            return calls, run.summary, error

    def test_original_sequence_and_owned_normal_shutdown(self):
        calls, summary, error = self.simulate()
        self.assertIsNone(error)
        self.assertEqual(calls, ["preparation", "python-census", "rust-census", "retained-integration",
                                 "signal", "join", "timers", "seal"])
        self.assertTrue(summary["passed"])
        self.assertTrue(summary["immutable_inputs_unchanged"])
        self.assertTrue(summary["retained_functional_passed"])

    def test_preparation_failure_stops_without_collectors(self):
        calls, summary, error = self.simulate("preparation")
        self.assertIsInstance(error, TimeoutError)
        self.assertNotIn("python-census", calls)
        self.assertFalse(summary["retained_attempted"])
        self.assertIn("join", calls)

    def test_python_failure_leaves_rust_and_retained_unattempted(self):
        calls, summary, error = self.simulate("python-census")
        self.assertIsInstance(error, TimeoutError)
        self.assertNotIn("rust-census", calls)
        self.assertFalse(summary["retained_attempted"])

    def test_rust_failure_leaves_retained_unattempted(self):
        calls, summary, error = self.simulate("rust-census")
        self.assertIsInstance(error, TimeoutError)
        self.assertNotIn("retained-integration", calls)
        self.assertFalse(summary["retained_attempted"])

    def test_retained_failure_does_not_report_workflow_pass(self):
        calls, summary, error = self.simulate("retained")
        self.assertIsInstance(error, AssertionError)
        self.assertTrue(summary["original_reader_passed"])
        self.assertFalse(summary["passed"])
        self.assertIn("join", calls)

    def test_archival_error_never_masks_original_timeout(self):
        calls, summary, error = self.simulate("python-census", "timers")
        self.assertIsInstance(error, TimeoutError)
        self.assertEqual(summary["outer_finalization_errors"][0]["stage"], "worker_timers")
        self.assertIn("seal", calls)

    def test_seal_change_refuses_success(self):
        _, summary, error = self.simulate(finalization="seal")
        self.assertIsInstance(error, RuntimeError)
        self.assertFalse(summary["passed"])
        self.assertFalse(summary["immutable_inputs_unchanged"])

    def test_cleanup_error_still_archives_and_checks_seals(self):
        calls, summary, error = self.simulate(finalization="cleanup")
        self.assertIsInstance(error, RuntimeError)
        self.assertFalse(summary["passed"])
        self.assertEqual(calls[-2:], ["timers", "seal"])

    def test_duplicate_and_oversize_lifecycle_documents_refused(self):
        with self.assertRaises(AssertionError):
            control.support_original_json(io.BytesIO(b'{"passed":true,"passed":false}'))
        class Oversize:
            def read(self, count):
                return b" " * count
        with self.assertRaises(AssertionError):
            control.support_original_json(Oversize())

    def test_forced_shutdown_is_failure_with_original_five_second_waits(self):
        calls = []
        class Child:
            pid, returncode = 29, None
            def poll(self):
                return None
            def wait(self, timeout):
                calls.append(timeout)
                if len(calls) == 1:
                    raise subprocess.TimeoutExpired("owned", 5)
                self.returncode = -9
        run = SimpleNamespace(daemon=Child(), summary={})
        helper = SimpleNamespace(bench=SimpleNamespace(signal_owned=lambda *args: None))
        with self.assertRaises(AssertionError):
            control.close_owned(run, helper)
        self.assertEqual(calls, [5, 5])
        self.assertTrue(run.summary["retained_service_cleanup"]["forced"])


if __name__ == "__main__":
    unittest.main()
