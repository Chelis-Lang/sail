#!/usr/bin/env python3
"""Pure preparation controls; no compiler, solver, endpoint or proof launch."""

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("stock_diagnostic", Path(__file__).with_name("m7-stock-smt-process-diagnostic.py"))
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)


class PreparationControls(unittest.TestCase):
    def test_exact_extraction_preserves_newline_and_non_utf8(self):
        commands = b"(push)\n;\xff\n(get-info :all-statistics)\n"
        self.assertEqual(DIAG.extract_commands(DIAG.OPEN + commands + DIAG.CLOSE), commands)

    def test_ambiguous_incomplete_and_response_inputs_refused(self):
        for invalid in (b"(push)\n", DIAG.OPEN + b"(push)\n", DIAG.OPEN + DIAG.OPEN + DIAG.CLOSE,
                        DIAG.OPEN + b";;;>>> RESPONSE\n" + DIAG.CLOSE,
                        DIAG.OPEN + DIAG.CLOSE + b"extra\n"):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                DIAG.extract_commands(invalid)

    def test_changed_stock_boundary_refused(self):
        with self.assertRaises(ValueError):
            DIAG.instrument("modified stock source")
        with self.assertRaises(ValueError):
            DIAG.replace_once("two two", "two", "marker")

    def test_independent_boundaries_identify_injected_loss_only(self):
        for before, after in ((1, 2), (4, 5), (13, 14), (16, 17)):
            rows = [{"stage": before, "dropped": 0}]
            self.assertTrue(DIAG.sensitivity(rows, before, True)["passed"])
            self.assertFalse(DIAG.sensitivity(rows, before, False)["passed"])
            self.assertFalse(DIAG.sensitivity(rows + [{"stage": after, "dropped": 0}], before, True)["passed"])
            self.assertFalse(DIAG.sensitivity([], before, True)["passed"])

    def test_loss_and_invalid_observations_never_certify_boundary(self):
        self.assertFalse(DIAG.sensitivity([{"stage": 1, "dropped": 1}], 1, True)["passed"])
        self.assertFalse(DIAG.sensitivity([{"stage": 99, "dropped": 0}], 1, True)["passed"])

    def test_complete_delivery_requires_every_named_stage(self):
        rows = [{"stage": stage, "dropped": 0} for stage in (2, 5, 7, 9, 11, 15, 17, 19, 21)]
        self.assertTrue(DIAG.sensitivity(rows, 0, False)["passed"])
        self.assertFalse(DIAG.sensitivity(rows[:-1], 0, False)["passed"])
        self.assertFalse(DIAG.sensitivity(rows, 0, True)["passed"])

    def test_fake_peer_and_main_are_static_parseable_sources(self):
        compile(DIAG.FAKE_SOLVER, "fake-solver", "exec")
        self.assertIn('b\'(echo "<<DONE>>")\\n\'', DIAG.FAKE_SOLVER)
        self.assertIn("#![forbid(unsafe_code)]", DIAG.MAIN)
        self.assertIn("sock.set_nonblocking(true)?", DIAG.MAIN)
        self.assertIn("drop(process)", DIAG.MAIN)

    def test_build_timeout_retains_terminal_and_owns_cleanup(self):
        class Actor:
            pid = 123456
            calls = 0

            def wait(self, timeout):
                self.calls += 1
                if self.calls == 1:
                    raise subprocess.TimeoutExpired("owned diagnostic compiler", timeout)
                return -2

            def poll(self):
                return None if self.calls == 1 else -2

        actor = Actor()
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            with patch.object(DIAG, "validate_prepared", return_value={"rustc": "sealed-rustc", "yansi": "sealed-yansi"}), \
                    patch.object(DIAG.subprocess, "Popen", return_value=actor), \
                    patch.object(DIAG.os, "killpg") as kill:
                with self.assertRaises(RuntimeError):
                    DIAG.build(SimpleNamespace(out=out))
            record = json.loads((out / "build.json").read_text())
            self.assertTrue(record["timed_out"])
            self.assertEqual(record["cleanup_errors"], [])
            self.assertIsNone(record["binary"])
            kill.assert_called_once_with(actor.pid, DIAG.signal.SIGINT)


if __name__ == "__main__":
    unittest.main()
