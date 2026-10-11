#!/usr/bin/env python3
"""Pure controls for the focused wait diagnostic; never starts a proof/service."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import time
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("ordinary_diag", Path(__file__).with_name("m7-ordinary-wait-diagnostic.py"))
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)


class Controls(unittest.TestCase):
    def test_observation_only_environment_does_not_change_input_selector(self):
        base = {"PATH": "/frozen", "SAIL_CACHE_DIR": "/original", "RUSTUP_TOOLCHAIN": "1.98.1"}
        observed = dict(base, SAIL_JOB_EVENTS="/new/events", SAIL_JOB_RPC_TIMINGS="1", SAIL_MANAGED_FORCE="1",
                        SAIL_DAEMON_SOCKET="/original/socket")
        self.assertEqual(DIAG.input_environment(base), DIAG.input_environment(observed))
        self.assertNotEqual(DIAG.input_environment(base), DIAG.input_environment(dict(base, PATH="/other")))

    def test_snapshot_schedule_caps_and_retains_original_deadline(self):
        seen = set()
        labels = []
        for _ in range(6):
            label = DIAG.observation_due(599.3, seen, True, True)
            if label:
                labels.append(label)
                seen.add(label)
        self.assertEqual(labels, ["context-ready", "target-started", "60-seconds", "300-seconds", "before-timeout"])
        self.assertIsNone(DIAG.observation_due(59, set(), False, False))
        self.assertEqual(DIAG.observation_due(60, set(), False, False), "60-seconds")
        self.assertIsNone(DIAG.observation_due(599.29, {"60-seconds", "300-seconds"}, False, False))

    def test_context_binding_rejects_changed_scope_revision_and_completeness(self):
        good = {"kind": "context-ready", "context_sha256": DIAG.CONTEXT,
                "input_revision": DIAG.REVISION, "complete": True}
        self.assertTrue(DIAG.check_context(good))
        for key, value in (("context_sha256", "other"), ("input_revision", "other"), ("complete", False)):
            with self.assertRaises(ValueError):
                DIAG.check_context(dict(good, **{key: value}))
        self.assertFalse(DIAG.check_context({"event": "target-started"}))

    def test_partial_event_tail_is_retained_without_double_consumption(self):
        first = b'{"kind":"other"}\n'
        partial = b'{"event":"target-'
        offset, rows = DIAG.event_rows(first + partial, 0)
        self.assertEqual(offset, len(first))
        self.assertEqual(rows, [{"kind": "other"}])
        joined = first + b'{"event":"target-started"}\n'
        offset, rows = DIAG.event_rows(joined, offset)
        self.assertEqual(rows, [{"event": "target-started"}])
        self.assertEqual(offset, len(joined))
        with self.assertRaises(ValueError):
            DIAG.event_rows(b'[]\n', 0)

    def test_context_archive_excludes_target_and_preserves_original_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "context"
            root.mkdir()
            (root / "target").mkdir()
            (root / "target" / "artifact").write_bytes(b"never-copy")
            (root / "receipts").mkdir()
            (root / "receipts" / "exact.json").write_bytes(b'{"seal":"exact"}')
            destination = Path(directory) / "retained"
            rows = DIAG.context_archive(root, destination)
            self.assertEqual(set(rows), {"receipts/exact.json"})
            self.assertEqual((destination / "receipts/exact.json").read_bytes(), b'{"seal":"exact"}')
            self.assertFalse((destination / "target").exists())

    def test_absolute_deadline_runs_independently_of_observation(self):
        class Actor:
            def poll(self):
                return None
        with patch.object(DIAG, "terminate", return_value=[]) as terminate:
            owner = DIAG.OwnedDeadline(Actor(), time.monotonic(), .02)
            self.assertTrue(owner.expired.wait(.5))
            owner.finish()
            self.assertFalse(owner.worker.is_alive())
            terminate.assert_called_once()

    def test_normal_completion_wakes_and_joins_deadline_owner(self):
        class Actor:
            def poll(self):
                return None
        with patch.object(DIAG, "terminate", return_value=[]) as terminate:
            owner = DIAG.OwnedDeadline(Actor(), time.monotonic())
            owner.finish()
            self.assertFalse(owner.worker.is_alive())
            self.assertFalse(owner.expired.is_set())
            terminate.assert_not_called()

    def test_context_archive_refuses_alias(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "context"
            root.mkdir()
            (root / "alias").symlink_to("missing")
            with self.assertRaises(ValueError):
                DIAG.context_archive(root, Path(directory) / "retained")


if __name__ == "__main__":
    unittest.main()
