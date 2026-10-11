#!/usr/bin/env python3
"""Owned timeout diagnostics regressions; no verifier or fixture deadline changes."""

import ast
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

root = Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("managed_evidence", root / "ci/managed-evidence.py")
evidence = importlib.util.module_from_spec(spec)
spec.loader.exec_module(evidence)


def raw_stat(parent, start=100):
    fields = ["S", str(parent), "100", "0", "0", "0", "0", "0", "0", "0", "0",
              "20", "30", "0", "0", "0", "0", "1", "0", str(start), "4096", "2"]
    return b"100 (name ) with spaces) " + " ".join(fields).encode()


def load_bounded(capture):
    # Execute only the actual harness function, avoiding its real-tool main.
    parsed = ast.parse((root / "ci/test-managed.py").read_text())
    function = next(node for node in parsed.body
                    if isinstance(node, ast.FunctionDef) and node.name == "bounded_verification")
    namespace = {"subprocess": subprocess, "os": os, "signal": __import__("signal"),
                 "json": json, "timeout_evidence": SimpleNamespace(capture_timeout=capture)}
    exec(compile(ast.Module(body=[function], type_ignores=[]), "bounded_verification", "exec"), namespace)
    return namespace["bounded_verification"]


class TimeoutEvidenceTests(unittest.TestCase):
    def test_stat_comm_boundary_and_incomplete_records(self):
        self.assertEqual(evidence.process_stat(raw_stat(7)), {
            "state": "S", "parent": 7, "process_group": 100, "user_ticks": 20,
            "system_ticks": 30, "start_ticks": 100, "resident_pages": 2})
        for raw in [None, b"garbage", raw_stat(7)[:20]]:
            self.assertIsNone(evidence.process_stat(raw))

    def test_only_bound_descendants_are_followed_and_limits_are_explicit(self):
        with tempfile.TemporaryDirectory() as temporary:
            proc = Path(temporary)
            for pid, parent, children in [(100, 1, b"101 102"), (101, 100, b""),
                                          (102, 55, b""), (999, 1, b"")]:
                directory = proc / str(pid)
                (directory / "task" / str(pid)).mkdir(parents=True)
                (directory / "stat").write_bytes(raw_stat(parent))
                (directory / "task" / str(pid) / "children").write_bytes(children)
                (directory / "comm").write_text("owned")
                (directory / "wchan").write_text("wait")
            observed = evidence.owned_processes(100, evidence.Capture(), proc)
            self.assertEqual([row["pid"] for row in observed], [100, 101])
            self.assertTrue(all(row["identity_stable"] for row in observed))
            capture = evidence.Capture()
            capture.remaining = 0
            self.assertEqual(evidence.owned_processes(100, capture, proc), [])
            self.assertTrue(capture.limited)

    def test_observation_loss_does_not_follow_reused_parent_identity(self):
        first = raw_stat(1, 100)
        changed = raw_stat(1, 200)
        capture = evidence.Capture()
        with patch.object(capture, "read", side_effect=[first, b"", b"", b"", changed]):
            rows = evidence.owned_processes(100, capture, Path("/missing"))
        self.assertFalse(rows[0]["identity_stable"])

    def test_compiler_selector_excludes_forwarded_tokens_and_full_arguments(self):
        result = evidence.compiler_selection(b"rust_verify\0--crate-name\0vstd\0--is-vstd\0--\0--crate-name\0fake\0secret\0")
        self.assertEqual(result, {"crate_name": "vstd", "literal_is_vstd": True,
                                  "literal_verus_crate": False})

    def test_run_inventory_excludes_environment_artifacts_and_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            cache = Path(temporary)
            context = cache / "projects/project/context"
            run = context / "runs/run"
            (run / "invocations").mkdir(parents=True)
            (run / "invocations/100.started").write_text('{"target":"vstd"}')
            (context / "runtime.json").write_text("private environment")
            (context / "target").mkdir()
            (context / "target/large.rlib").write_text("artifact")
            (run / "alias").symlink_to(context / "runtime.json")
            rows = evidence.scratch_inventory(cache, evidence.Capture())
            self.assertEqual([row["relative_path"] for row in rows],
                             ["projects/project/context/runs/run/invocations/100.started"])
            self.assertIsNone(evidence.Capture().read(run / "alias"))

    def test_capture_preserves_progress_without_claiming_a_cause(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            events = directory / "events"
            events.write_text('{"kind":"context-ready"}\n')
            output = directory / "timeout.json"
            evidence.capture_timeout(99999999, directory, events, output)
            document = json.loads(output.read_text())
            self.assertFalse(document["capture_is_proof_authority"])
            self.assertEqual(document["progress"]["records"], ['{"kind":"context-ready"}'])
            self.assertEqual(document["acceptance_timeout_seconds"], 240)
            self.assertLessEqual(document["bytes_read"], evidence.MAX_BYTES)

    def test_special_file_cannot_block_a_diagnostic_read(self):
        with tempfile.TemporaryDirectory() as temporary:
            pipe = Path(temporary) / "fifo"
            os.mkfifo(pipe)
            capture = evidence.Capture()
            self.assertIsNone(capture.read(pipe))
            self.assertEqual(capture.errors[-1]["error"], "not a regular diagnostic file")

    def test_failed_diagnostics_cannot_prevent_original_owned_kill_and_reap(self):
        def failed(*args):
            raise OSError("diagnostic failed")
        bounded = load_bounded(failed)
        made = []
        original = subprocess.Popen
        def owned(*args, **kwargs):
            process = original(*args, **kwargs)
            made.append(process)
            return process
        with tempfile.TemporaryDirectory() as temporary, patch.object(subprocess, "Popen", side_effect=owned):
            path = Path(temporary)
            with self.assertRaises(subprocess.TimeoutExpired):
                bounded(
                    [sys.executable, "-c", "import time;time.sleep(20)"], path, os.environ.copy(), .05,
                    (path, path / "events", path / "absent/diagnostic.json"))
        self.assertEqual(len(made), 1)
        self.assertIsNotNone(made[0].poll())
        self.assertEqual(made[0].returncode, -__import__("signal").SIGKILL)

    def test_late_complete_bytes_remain_the_original_timeout_deterministically(self):
        def capture(*args):
            pass
        bounded = load_bounded(capture)
        process = SimpleNamespace(pid=100, communicate=Mock(
            side_effect=[subprocess.TimeoutExpired("original", 240), (b"complete late report", b"")]))
        with patch.object(subprocess, "Popen", return_value=process), patch.object(os, "killpg") as kill:
            with self.assertRaises(subprocess.TimeoutExpired) as expired:
                bounded(["original"], Path("."), {}, 240, (Path("."), Path("."), Path(".")))
        self.assertEqual(expired.exception.timeout, 240)
        self.assertEqual(expired.exception.stdout, b"complete late report")
        kill.assert_called_once_with(100, __import__("signal").SIGKILL)

    @unittest.skipUnless(sys.platform == "linux", "actual Linux group lifecycle; macOS completed-group EPERM retained separately")
    def test_linux_completion_during_evidence_cleanup_does_not_admit_a_late_report(self):
        def paused_capture(*args):
            time.sleep(.15)
        bounded = load_bounded(paused_capture)
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)
            with self.assertRaises(subprocess.TimeoutExpired):
                bounded([sys.executable, "-c", "import time;time.sleep(.03);print('late report')"],
                        path, os.environ.copy(), .01, (path, path, path))


if __name__ == "__main__":
    unittest.main()
