#!/usr/bin/env python3
"""Owned timeout diagnostics regressions; no verifier or fixture deadline changes."""

import ast
import importlib.util
import json
import os
from pathlib import Path
import select
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

    def test_previously_bound_root_and_queued_child_reuse_are_refused(self):
        capture = evidence.Capture()
        with patch.object(capture, "read", return_value=raw_stat(1, 200)):
            self.assertEqual(evidence.owned_processes(100, capture, Path("/missing"), 100), [])
        self.assertTrue(capture.limited)
        self.assertEqual(capture.errors[-1]["operation"], "process-identity")
        with tempfile.TemporaryDirectory() as temporary:
            proc = Path(temporary)
            for pid, parent, children in [(100, 1, b"101"), (101, 100, b"")]:
                directory = proc / str(pid)
                (directory / "task" / str(pid)).mkdir(parents=True)
                (directory / "stat").write_bytes(raw_stat(parent, 100))
                (directory / "task" / str(pid) / "children").write_bytes(children)
            capture = evidence.Capture()
            original = capture.read
            child_reads = 0
            def changed_child(path, limit=65536):
                nonlocal child_reads
                value = original(path, limit)
                if path == proc / "101/stat":
                    child_reads += 1
                    return raw_stat(100, 100 if child_reads == 1 else 200)
                return value
            with patch.object(capture, "read", side_effect=changed_child):
                rows = evidence.owned_processes(100, capture, proc, 100)
            self.assertEqual([row["pid"] for row in rows], [100])
            self.assertTrue(capture.limited)

    @unittest.skipUnless(sys.platform == "linux", "actual Linux per-task child ownership")
    def test_worker_thread_child_is_absent_from_main_task_but_observed(self):
        # The worker thread stays alive until its owned child exits normally.
        code = """
import json, os, subprocess, sys, threading
release = threading.Event()
def worker():
    child = subprocess.Popen([sys.executable, '-c', 'import sys;sys.stdin.read()'], stdin=subprocess.PIPE)
    print(json.dumps({'child': child.pid, 'task': threading.get_native_id()}), flush=True)
    release.wait()
    child.communicate(b'closed', timeout=3)
thread = threading.Thread(target=worker)
thread.start()
sys.stdin.readline()
release.set()
thread.join()
"""
        child = subprocess.Popen([sys.executable, "-u", "-c", code], stdin=subprocess.PIPE,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            self.assertTrue(select.select([child.stdout], [], [], 3)[0], "owned worker readiness")
            made = json.loads(child.stdout.readline())
            self.assertNotEqual(made["task"], child.pid)
            proc = Path("/proc") / str(child.pid)
            main = (proc / "task" / str(child.pid) / "children").read_text().split()
            self.assertNotIn(str(made["child"]), main, "main-task-only observer must miss this child")
            worker = (proc / "task" / str(made["task"]) / "children").read_text().split()
            self.assertIn(str(made["child"]), worker)
            identity = evidence.process_stat((proc / "stat").read_bytes())
            capture = evidence.Capture()
            rows = evidence.owned_processes(child.pid, capture,
                                           expected_start_ticks=identity["start_ticks"])
            by_pid = {row["pid"]: row for row in rows}
            self.assertIn(made["child"], by_pid)
            self.assertTrue(all(row["identity_stable"] for row in rows))
            self.assertEqual(by_pid[made["child"]]["parent"], child.pid)
            self.assertGreaterEqual(by_pid[made["child"]]["start_ticks"], identity["start_ticks"])
            self.assertFalse(capture.limited)
            self.assertLessEqual(evidence.MAX_BYTES - capture.remaining, evidence.MAX_BYTES)
        finally:
            try:
                _, stderr = child.communicate(b"close\n", timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, __import__("signal").SIGKILL)
                _, stderr = child.communicate(timeout=5)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(stderr, b"", "owned fixture thread/child cleanup failed")

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
