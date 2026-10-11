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

    @staticmethod
    def rows(pause):
        return [{"sequence": index, "stage": stage, "dropped": 0}
                for index, stage in enumerate(DIAG.fixed_stages(pause))]

    def test_independent_boundaries_require_exact_fixed_prefix(self):
        self.assertEqual([len(DIAG.fixed_stages(stage)) for stage in (1, 4, 13, 16)], [1, 7, 14, 19])
        for before, after in ((1, 2), (4, 5), (13, 14), (16, 17)):
            rows = self.rows(before)
            self.assertTrue(DIAG.sensitivity(rows, before, True)["passed"])
            self.assertFalse(DIAG.sensitivity(rows, before, False)["passed"])
            self.assertFalse(DIAG.sensitivity(rows + [{"sequence": len(rows), "stage": after, "dropped": 0}], before, True)["passed"])
            for index in range(len(rows)):
                missing = rows[:index] + rows[index + 1:]
                # Even renumbered loss cannot pass the exact cardinality check.
                missing = [dict(row, sequence=n) for n, row in enumerate(missing)]
                self.assertFalse(DIAG.sensitivity(missing, before, True)["passed"])

    def test_loss_and_invalid_observations_never_certify_boundary(self):
        self.assertFalse(DIAG.sensitivity([{"sequence": 0, "stage": 1, "dropped": 1}], 1, True)["passed"])
        self.assertFalse(DIAG.sensitivity([{"sequence": 0, "stage": 99, "dropped": 0}], 1, True)["passed"])
        self.assertFalse(DIAG.sensitivity([{"stage": 1, "dropped": 0}], 1, True)["passed"])
        rows = self.rows(16)
        self.assertFalse(DIAG.sensitivity(rows[:-1] + [rows[0]], 16, True)["passed"])

    def test_complete_delivery_requires_exact_multiplicity(self):
        rows = self.rows(0)
        self.assertEqual(len(rows), 23)
        self.assertTrue(DIAG.sensitivity(rows, 0, False)["passed"])
        self.assertFalse(DIAG.sensitivity(rows[:-1], 0, False)["passed"])
        self.assertFalse(DIAG.sensitivity(rows, 0, True)["passed"])
        changed = [dict(row) for row in rows]
        changed[-1]["stage"] = 13
        self.assertFalse(DIAG.sensitivity(changed, 0, False)["passed"])
        # Thread interleaving need not follow numeric stage order.
        reverse = [dict(row, sequence=n) for n, row in enumerate(reversed(rows))]
        self.assertTrue(DIAG.sensitivity(reverse, 0, False)["passed"])

    def test_completed_loss_accounting_requires_every_counter(self):
        value = {"responses": 1, "observations": 23, "dropped": 0, "queue_dropped": 0,
                 "queue_full": 0, "queue_disconnected": 0, "send_dropped": 0,
                 "sender_errors": 0, "deadline_dropped": 0}
        self.assertEqual(DIAG.completed_counters(value), value)
        for key in value:
            invalid = dict(value)
            invalid.pop(key)
            with self.assertRaises(ValueError):
                DIAG.completed_counters(invalid)
        for key in ("queue_full", "sender_errors", "dropped"):
            with self.assertRaises(ValueError):
                DIAG.completed_counters(dict(value, **{key: 1}))
        with self.assertRaises(ValueError):
            DIAG.completed_counters(dict(value, dropped=True))
        loss = dict(value, dropped=3, queue_dropped=1, queue_full=1,
                    send_dropped=2, sender_errors=1, deadline_dropped=1)
        self.assertEqual(DIAG.completed_counters(loss), loss)

    def test_fake_peer_and_main_are_static_parseable_sources(self):
        compile(DIAG.FAKE_SOLVER, "fake-solver", "exec")
        self.assertIn('b\'(echo "<<DONE>>")\\n\'', DIAG.FAKE_SOLVER)
        self.assertIn("#![forbid(unsafe_code)]", DIAG.MAIN)
        self.assertIn("sender.try_send(row)", DIAG.MAIN)
        self.assertIn("sync_channel::<[u8; 32]>(64)", DIAG.MAIN)
        self.assertIn("remaining.min(Duration::from_millis(25))", DIAG.MAIN)
        self.assertIn("writer.join()", DIAG.MAIN)
        self.assertLess(DIAG.MAIN.index("drop(process)"), DIAG.MAIN.index("metadata.finish()?"))

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
