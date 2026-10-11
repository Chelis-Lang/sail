#!/usr/bin/env python3
"""No-proof lifecycle/refusal controls for the separately granted wrapper."""
from contextlib import contextmanager
import importlib.util
import os
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

spec = importlib.util.spec_from_file_location("sysroot_diagnostic", Path(__file__).with_name("m7-sysroot-proof-diagnostic.py"))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)


class DiagnosticControls(unittest.TestCase):
    def run_control(self, fault=None):
        calls = []
        with tempfile.TemporaryDirectory() as directory:
            class Service:
                def __init__(self, output, keep):
                    self.actors = {"diagnostic": [object()]}
                    assert keep is False
                def __enter__(self):
                    return self
                def start(self, *args):
                    calls.append("start")
                def __exit__(self, *args):
                    calls.append("stop")
            full = {"context_sha256": "new-runtime", "input_revision": "source-revision",
                    "currentness": "unknown", "notes": ["original Unknown note"],
                    "execution": {"verifier_invocations": 1}, "targets": []}
            document = {"job": {"id": "owned"}, "repair": {"accepted": False,
                        "batch": {"manifest_sha256": control.MANIFEST, "full": full,
                                  "batched": True, "compiler_invocations": 1}}}
            def submit(*args):
                calls.append("submit")
                return "owned"
            def wait(*args):
                calls.append("wait")
                if fault == "timeout":
                    raise TimeoutError("original deadline")
                if fault == "manifest":
                    document["repair"]["batch"]["manifest_sha256"] = "wrong"
                return document
            def finish():
                calls.append("archive-cancel")
            run = SimpleNamespace(args=SimpleNamespace(setup_timeout=600), owned=[],
                out=Path(directory), bin=Path(directory), source=Path(directory), env={}, summary={
                    "diagnostic_completed": False, "cleanup_errors": [], "archival_errors": []},
                submit=submit, wait=wait, finish_jobs=finish)
            helper = SimpleNamespace(Service=Service, bench=SimpleNamespace(write_json=lambda *args: None),
                                     same_scope=lambda *args: None)
            try:
                control.proof_once(run, helper, control.MANIFEST, "package")
            except (TimeoutError, AssertionError) as error:
                return calls, run.summary, error
            return calls, run.summary, None

    def test_unknown_is_retained_without_retry_or_acceptance(self):
        calls, summary, error = self.run_control()
        self.assertIsNone(error)
        self.assertEqual(calls, ["start", "submit", "wait", "archive-cancel", "stop"])
        self.assertTrue(summary["diagnostic_completed"])
        self.assertFalse(summary["observation"]["accepted"])
        self.assertEqual(summary["observation"]["notes"], ["original Unknown note"])

    def test_timeout_archives_and_stops_without_another_submission(self):
        calls, summary, error = self.run_control("timeout")
        self.assertIsInstance(error, TimeoutError)
        self.assertEqual(calls, ["start", "submit", "wait", "archive-cancel", "stop"])
        self.assertFalse(summary["diagnostic_completed"])

    def test_changed_manifest_refuses_and_cleans_owned_actor(self):
        calls, summary, error = self.run_control("manifest")
        self.assertIsInstance(error, AssertionError)
        self.assertEqual(calls, ["start", "submit", "wait", "archive-cancel", "stop"])
        self.assertFalse(summary["diagnostic_completed"])

    def test_source_seal_excludes_caller_target_and_refuses_aliases(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "target").mkdir()
            (root / "target/generated").write_bytes(b"artifact")
            (root / "source.rs").write_bytes(b"original")
            self.assertEqual(set(control.source_inventory(root)), {"source.rs"})
            (root / "alias").symlink_to(root / "source.rs")
            with self.assertRaises(OSError):
                control.source_inventory(root)

    def test_only_the_explicit_existing_generated_lockfile_can_extend_template(self):
        original = {"src/lib.rs": {"bytes": 1, "sha256": "original"}}
        actual = dict(original, **{"Cargo.lock": control.GENERATED_LOCK})
        control.bind_source(actual, original)
        actual["new.rs"] = {"bytes": 1, "sha256": "extra"}
        with self.assertRaises(AssertionError):
            control.bind_source(actual, original)
        actual.pop("new.rs")
        actual["Cargo.lock"] = {"bytes": 3007, "sha256": "another"}
        with self.assertRaises(AssertionError):
            control.bind_source(actual, original)

    def test_timer_archive_selects_only_owned_bounded_regular_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            job = root / "job"
            job.mkdir()
            (job / "events.setup-12-0.jsonl").write_bytes(b'{"phase":"actual"}\n')
            (job / "record.json").write_bytes(b"private environment")
            @contextmanager
            def owned(*args):
                descriptor = os.open(job, os.O_RDONLY | os.O_DIRECTORY)
                try:
                    yield descriptor, {}, "projects/jobs"
                finally:
                    os.close(descriptor)
            def regular(descriptor, name):
                fd = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=descriptor)
                return os.fdopen(fd, "rb")
            run = SimpleNamespace(owned=["owned"], cache=root, job_scopes={"owned": {}},
                                  out=root / "out", summary={})
            helper = SimpleNamespace(owned_job=owned, owned_file=regular)
            control.archive_timers(run, helper)
            self.assertEqual(set(run.summary["worker_timer_archives"]), {"owned/events.setup-12-0.jsonl"})
            self.assertFalse((root / "out/worker-timers/owned/record.json").exists())
            (job / "events.setup-12-1.jsonl").write_bytes(b"x" * (256 * 1024 + 1))
            run.out = root / "oversize"
            with self.assertRaisesRegex(AssertionError, "timer bytes"):
                control.archive_timers(run, helper)

    def test_nonregular_input_never_blocks_for_contents(self):
        import os
        with tempfile.TemporaryDirectory() as directory:
            fifo = Path(directory) / "pipe"
            os.mkfifo(fifo)
            with self.assertRaises(AssertionError):
                control.identity(fifo)


if __name__ == "__main__":
    unittest.main()
