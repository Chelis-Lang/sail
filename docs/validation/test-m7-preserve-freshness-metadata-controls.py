"""Pure controller predicate checks; no compiler or verification actor."""
import importlib.util
from pathlib import Path
import unittest
import signal
import subprocess
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("controls", Path(__file__).with_name("m7-preserve-freshness-metadata-controls.py"))
CONTROLS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CONTROLS)


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


class Destination:
    def __init__(self):
        self.writes = []

    def write_bytes(self, value):
        self.writes.append(value)


class Controls(unittest.TestCase):
    def test_normal_zero_and_assertion_exit_allow_next_phase_and_restore(self):
        for code in (0, 101):
            owner = CONTROLS.OwnedCommands()
            child = Child([code])
            killed = []
            self.assertEqual(owner.wait(child, 180, lambda *args: killed.append(args)), code)
            owner.require_closed()
            destination = Destination()
            owner.restore(destination, b"original")
            self.assertEqual(destination.writes, [b"original"])
            self.assertEqual(child.waits, [180])
            self.assertEqual(killed, [])

    def test_timeout_reaped_leader_still_refuses_restore_and_next_actor(self):
        owner = CONTROLS.OwnedCommands()
        child = Child([subprocess.TimeoutExpired("fake", 600), 0])
        killed = []
        with self.assertRaises(subprocess.TimeoutExpired):
            owner.wait(child, 600, lambda *args: killed.append(args))
        self.assertEqual(child.waits, [600, 2])
        self.assertEqual(killed, [(123, signal.SIGTERM)])
        self.assertEqual(child.returncode, 0)
        destination = Destination()
        with self.assertRaises(RuntimeError):
            owner.restore(destination, b"original")
        with self.assertRaises(RuntimeError):
            owner.execute([], None, {}, None, 600)
        self.assertEqual(destination.writes, [])

    def test_interrupted_wait_keeps_term_kill_graces_and_refuses_restore(self):
        owner = CONTROLS.OwnedCommands()
        child = Child([InterruptedError("fake"), subprocess.TimeoutExpired("fake", 2), -9])
        killed = []
        with self.assertRaises(InterruptedError):
            owner.wait(child, 180, lambda *args: killed.append(args))
        self.assertEqual(child.waits, [180, 2, 2])
        self.assertEqual(killed, [(123, signal.SIGTERM), (123, signal.SIGKILL)])
        with self.assertRaises(RuntimeError):
            owner.restore(Destination(), b"original")

    def test_wait_timeout_racing_exit_remains_uncertain(self):
        class Exiting(Child):
            def wait(self, timeout):
                self.returncode = 0
                raise subprocess.TimeoutExpired("fake", timeout)
        owner = CONTROLS.OwnedCommands()
        killed = []
        with self.assertRaises(subprocess.TimeoutExpired):
            owner.wait(Exiting([]), 180, lambda *args: killed.append(args))
        self.assertEqual(killed, [])
        with self.assertRaises(RuntimeError):
            owner.require_closed()

    def test_failed_termination_does_not_clear_uncertainty(self):
        owner = CONTROLS.OwnedCommands()
        child = Child([subprocess.TimeoutExpired("fake", 180)])
        def failed_kill(*_args):
            raise OSError("fake termination failure")
        with self.assertRaises(OSError):
            owner.wait(child, 180, failed_kill)
        with self.assertRaises(RuntimeError):
            owner.require_closed()

    def test_spawn_interruption_never_authorizes_restore(self):
        owner = CONTROLS.OwnedCommands()
        with patch.object(CONTROLS.subprocess, "Popen", side_effect=InterruptedError("fake")):
            with self.assertRaises(InterruptedError):
                owner.execute([], None, {}, None, 600)
        with self.assertRaises(RuntimeError):
            owner.restore(Destination(), b"original")

    def test_actual_execute_uses_owned_session_and_original_cap(self):
        owner = CONTROLS.OwnedCommands()
        child = Child([0])
        with patch.object(CONTROLS.subprocess, "Popen", return_value=child) as spawn:
            self.assertEqual(owner.execute(["fake"], "source", {}, None, 600), 0)
        self.assertTrue(spawn.call_args.kwargs["start_new_session"])
        self.assertEqual(child.waits, [600])
        owner.require_closed()
        self.assertIsNone(owner.active)
        self.assertFalse(owner.spawning)

    def test_interruption_before_wait_cleans_known_child_but_never_restores(self):
        owner = CONTROLS.OwnedCommands()
        child = Child([0])
        killed = []
        cleanup = owner.cleanup
        with patch.object(CONTROLS.subprocess, "Popen", return_value=child), \
                patch.object(owner, "wait", side_effect=InterruptedError("fake pre-wait")), \
                patch.object(owner, "cleanup", side_effect=lambda value: cleanup(value, lambda *args: killed.append(args))):
            with self.assertRaises(InterruptedError):
                owner.execute(["fake"], "source", {}, None, 600)
        self.assertEqual(killed, [(123, signal.SIGTERM)])
        self.assertEqual(child.waits, [2])
        self.assertIsNone(owner.active)
        with self.assertRaises(RuntimeError):
            owner.restore(Destination(), b"original")

    def test_only_exact_unchanged_owner_assertion_failure_is_sensitivity(self):
        text = ("running 1 test\ntest " + CONTROLS.CASE + " ... FAILED\n"
                "assertion failed: " + CONTROLS.ASSERTION + "\nfailures:\n    "
                + CONTROLS.CASE + "\ntest result: FAILED. 0 passed; 1 failed; 0 ignored\n")
        self.assertTrue(CONTROLS.expected_negative(101, text))
        for code, candidate in [(0, text), (101, text.replace(CONTROLS.ASSERTION, "other")),
                                (101, text.replace(CONTROLS.CASE, "other")),
                                (101, text.replace("1 failed", "2 failed")),
                                (101, text.replace("1 failed", "11 failed")),
                                (101, "error: compilation failed")]:
            self.assertFalse(CONTROLS.expected_negative(code, candidate))


    def test_actual_interleaved_libtest_failure_and_refusal_variants(self):
        path = Path(__file__).with_name("m7-preserve-freshness-metadata-libtest-failure.txt")
        text = path.read_text()
        self.assertTrue(CONTROLS.expected_negative(101, text))
        for candidate in [text.replace("\nFAILED\n", "\nok\n"),
                          text.replace("running 1 test", "running 2 tests"),
                          text.replace(" panicked at ", " succeeded at "),
                          text.replace("    " + CONTROLS.CASE, "    another_case"),
                          text.replace("0 passed; 1 failed;", "1 passed; 0 failed;"),
                          text.replace(CONTROLS.ASSERTION, "different assertion")]:
            self.assertFalse(CONTROLS.expected_negative(101, candidate))


if __name__ == "__main__":
    unittest.main()
