#!/usr/bin/env python3
"""Pure controller controls: fake child states and in-memory logs; no actors."""
import importlib.util
import io
from pathlib import Path
import signal
import subprocess
import unittest


ROOT = Path(__file__).resolve().parent


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


runner = load("http_control_runner", "run.py")
analyzer = load("http_control_analyzer", "analyze.py")


class Child:
    pid = 123

    def __init__(self, outcomes):
        self.outcomes = list(outcomes)
        self.returncode = None
        self.waits = []

    def wait(self, timeout):
        self.waits.append(timeout)
        outcome = self.outcomes.pop(0)
        if isinstance(outcome, BaseException):
            raise outcome
        self.returncode = outcome
        return outcome

    def poll(self):
        return self.returncode


class ControllerTests(unittest.TestCase):
    def test_normal_completion_keeps_original_wait_and_allows_next_phase(self):
        for code in (0, 101):
            child = Child([code])
            closure = runner.ProcessClosure()
            kills = []
            runner.wait_child(child, 600, closure, lambda *args: kills.append(args))
            closure.require_normal_completion()
            self.assertEqual(child.waits, [600])
            self.assertEqual(kills, [])
            self.assertEqual(child.returncode, code)

    def test_timeout_reaped_leader_still_refuses_restore_and_next_phase(self):
        child = Child([subprocess.TimeoutExpired("fake", 30), 0])
        closure = runner.ProcessClosure()
        kills = []
        with self.assertRaises(subprocess.TimeoutExpired):
            runner.wait_child(child, 30, closure, lambda *args: kills.append(args))
        self.assertEqual(child.waits, [30, 2])
        self.assertEqual(kills, [(123, signal.SIGTERM)])
        self.assertEqual(child.returncode, 0)
        for _ in range(2):
            with self.assertRaisesRegex(RuntimeError, "explicit lead audit required"):
                closure.require_normal_completion()

    def test_timeout_racing_leader_exit_is_sticky_without_termination(self):
        class ExitingChild(Child):
            def wait(self, timeout):
                self.returncode = 0
                raise subprocess.TimeoutExpired("fake", timeout)
        child = ExitingChild([])
        closure = runner.ProcessClosure()
        kills = []
        with self.assertRaises(subprocess.TimeoutExpired):
            runner.wait_child(child, 180, closure, lambda *args: kills.append(args))
        self.assertEqual(kills, [])
        with self.assertRaises(RuntimeError):
            closure.require_normal_completion()

    def test_signal_termination_and_kill_keep_original_graces(self):
        child = Child([InterruptedError("fake signal"), subprocess.TimeoutExpired("fake", 2), -9])
        closure = runner.ProcessClosure()
        kills = []
        with self.assertRaises(InterruptedError):
            runner.wait_child(child, 30, closure, lambda *args: kills.append(args))
        self.assertEqual(child.waits, [30, 2, 2])
        self.assertEqual(kills, [(123, signal.SIGTERM), (123, signal.SIGKILL)])
        with self.assertRaises(RuntimeError):
            closure.require_normal_completion()

    def test_termination_failure_does_not_clear_uncertainty(self):
        child = Child([subprocess.TimeoutExpired("fake", 30)])
        closure = runner.ProcessClosure()
        def failed_kill(_pid, _signal):
            raise OSError("fake kill failure")
        with self.assertRaises(OSError):
            runner.wait_child(child, 30, closure, failed_kill)
        self.assertIsNone(child.poll())
        with self.assertRaises(RuntimeError):
            closure.require_normal_completion()

    def test_uncertainty_is_not_cleared_by_later_completed_leader(self):
        closure = runner.ProcessClosure()
        closure.uncertain("signal during active command or spawn")
        closure.uncertain("signal during active command or spawn")
        runner.wait_child(Child([0]), 10, closure, lambda *_args: None)
        self.assertEqual(len(closure.reasons), 1)
        with self.assertRaises(RuntimeError):
            closure.require_normal_completion()

    def test_log_read_is_bounded_before_allocation(self):
        requested = []
        class Reader(io.BytesIO):
            def read(self, size=-1):
                requested.append(size)
                # Simulate excess data without materializing a whole file.
                return b"x" * size
        class FakePath:
            def __truediv__(self, _name):
                return self
            def open(self, mode):
                self.mode = mode
                return Reader()
        result = analyzer.arm(FakePath(), "baseline")
        self.assertEqual(requested, [32 * 1024 * 1024 + 1])
        self.assertEqual(result, {"status": "unavailable", "reason": "raw interpretation bound exceeded"})

    def test_small_log_still_uses_original_interpretation(self):
        class FakePath:
            def __truediv__(self, _name):
                return self
            def open(self, _mode):
                return io.BytesIO(b"coalesced_followers=7\n")
        self.assertEqual(analyzer.arm(FakePath(), "candidate")["coalesced_followers"], [7])

    def test_incomplete_sampler_never_supplies_cpu_authority(self):
        sample = {"complete": False, "rows": [], "start_us": 0}
        result = analyzer.interval(sample, sample)
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["reason"], "bounded traversal incomplete")

    def test_both_sampler_overlays_refuse_missing_comm_and_final_overrun(self):
        # Static overlay control only: actual Rust/proc execution remains pending.
        for arm in ("baseline", "candidate"):
            text = (ROOT / (arm + "-observation.patch")).read_text()
            self.assertIn('let Some(name) = owner_field(&path.join("comm")) else {\n+                complete = false;\n+                continue;', text)
            self.assertIn('if ended.duration_since(started) >= std::time::Duration::from_millis(10) {\n+        complete = false;', text)


if __name__ == "__main__":
    unittest.main()
