#!/usr/bin/env python3
"""Pure retention/lifecycle controls; no subprocess, verifier or daemon actors."""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location(
    "diagnostic", Path(__file__).with_name("m7-publication-timing-diagnostic.py"))
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


def rpc():
    row = dict.fromkeys(diagnostic.NUMERIC, None)
    row.update(sequence=0, started_us=0, dispatch_delay_us=0, total_us=0,
               operation="worker_publication", frame_hit=None, currentness=None,
               write_succeeded=False)
    return {"kind": "diagnostic-job-rpc-stages", "daemon_pid": 1,
            "origin_unix_ms": 0, "capacity": 256, "started": 1,
            "dropped": 0, "overwritten": 0, "rows": [row]}


def setup():
    return {"kind": "managed-setup-phase", "phase": "receipt-publication",
            "pid": 1, "started_ms": 1.0, "elapsed_ms": 2.0}


def populate(root):
    jobs = root / "cache/projects/jobs"
    job = jobs / "123-4-0"
    job.mkdir(parents=True)
    rpc_bytes = json.dumps(rpc()).encode()
    setup_bytes = json.dumps(setup()).encode() + b"\n"
    (jobs / "rpc-timings.json").write_bytes(rpc_bytes)
    (job / "events.setup-4-0.jsonl").write_bytes(setup_bytes)
    for name in ("record.json", "stdout", "stderr", "events.jsonl", "proof-source.jsonl"):
        (job / name).write_bytes(b'{"private_environment":"never copy"}')
    return jobs, job, rpc_bytes, setup_bytes


class Controls(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self):
        self.temporary.cleanup()

    def test_exact_public_bytes_only_and_private_modes(self):
        _, _, rpc_bytes, setup_bytes = populate(self.root)
        destination = self.root / "retained"
        result = diagnostic.retain_timings(self.root, destination)
        self.assertFalse(result["partial"])
        self.assertEqual(len(result["files"]), 2)
        self.assertEqual((destination / "rpc-timings.json").read_bytes(), rpc_bytes)
        self.assertEqual((destination / "123-4-0/events.setup-4-0.jsonl").read_bytes(), setup_bytes)
        self.assertEqual(len(list(destination.rglob("*.json*"))), 2)
        for row in result["files"]:
            target = destination / row["member"]
            self.assertEqual(target.stat().st_mode & 0o777, 0o600)
            self.assertEqual(hashlib.sha256(target.read_bytes()).hexdigest(), row["sha256"])

    def test_schema_refuses_private_payload_duplicates_nonfinite_and_boolean_numbers(self):
        cases = []
        value = rpc()
        value["rows"][0]["environment"] = "private"
        cases.append((json.dumps(value).encode(), True))
        cases.append((b'{"kind":"a","kind":"b"}', True))
        value = rpc()
        value["rows"][0]["sequence"] = True
        cases.append((json.dumps(value).encode(), True))
        value = setup()
        value["elapsed_ms"] = float("nan")
        cases.append((json.dumps(value).encode(), False))
        value = setup()
        value["phase"] = "unrecognized-private-path"
        cases.append((json.dumps(value).encode(), False))
        cases.append((json.dumps(setup()).encode() + b"\n" + b"private", False))
        for data, is_rpc in cases:
            with self.assertRaises((ValueError, AssertionError)):
                diagnostic.validate(data, is_rpc)

    def test_overwritten_or_absent_ring_is_explicitly_incomplete(self):
        jobs, _, _, _ = populate(self.root)
        value = rpc()
        value.update(started=377, overwritten=121)
        (jobs / "rpc-timings.json").write_bytes(json.dumps(value).encode())
        result = diagnostic.retain_timings(self.root, self.root / "overwritten")
        self.assertTrue(result["partial"])
        self.assertFalse(result["rpc_coverage_complete"])
        self.assertEqual(result["rpc_coverage"]["overwritten"], 121)
        self.assertTrue(result["rpc_present"])
        (jobs / "rpc-timings.json").unlink()
        result = diagnostic.retain_timings(self.root, self.root / "missing")
        self.assertTrue(result["partial"])
        self.assertFalse(result["rpc_present"])

    def test_invalid_data_still_consumes_aggregate_read_budget(self):
        jobs, job, _, _ = populate(self.root)
        (jobs / "rpc-timings.json").unlink()
        for index in range(3):
            (job / f"events.setup-4-{index}.jsonl").write_bytes(b"x" * 40)
        with mock.patch.object(diagnostic, "MAX_TOTAL", 50):
            result = diagnostic.retain_timings(self.root, self.root / "retained")
        self.assertEqual(result["inspected_bytes"], 40)
        self.assertTrue(result["partial"])
        self.assertEqual(result["files"], [])

    def test_symlink_fifo_and_oversize_are_not_read_or_retained(self):
        jobs, job, _, _ = populate(self.root)
        (job / "events.setup-4-0.jsonl").unlink()
        os.symlink(jobs / "rpc-timings.json", job / "events.setup-4-0.jsonl")
        os.mkfifo(job / "events.setup-4-1.jsonl")
        (job / "events.setup-4-2.jsonl").write_bytes(b"x" * (16384 + 1))
        result = diagnostic.retain_timings(self.root, self.root / "retained")
        self.assertTrue(result["partial"])
        self.assertEqual([r["member"] for r in result["files"]], ["rpc-timings.json"])
        self.assertEqual(len(result["rejected"]), 3)

    def test_directory_alias_does_not_open_foreign_records(self):
        other = self.root / "other"
        other.mkdir()
        populate(other)
        os.symlink(other / "cache", self.root / "cache")
        result = diagnostic.retain_timings(self.root, self.root / "retained")
        self.assertTrue(result["partial"])
        self.assertEqual(result["files"], [])

    def test_shared_byte_scan_and_row_bounds_report_partial(self):
        populate(self.root)
        with mock.patch.object(diagnostic, "MAX_TOTAL", 1):
            result = diagnostic.retain_timings(self.root, self.root / "tiny")
        self.assertTrue(result["partial"])
        self.assertLessEqual(result["inspected_bytes"], 1)
        with mock.patch.object(diagnostic, "MAX_SCAN", 1):
            result = diagnostic.retain_timings(self.root, self.root / "scan")
        self.assertTrue(result["partial"])
        self.assertLessEqual(result["scan_entries"], 1)
        with self.assertRaises(AssertionError):
            diagnostic.validate((json.dumps(setup()).encode() + b"\n") * 65, False)

    def test_original_exception_and_cleanup_survive_retention_failure(self):
        captures = []
        factory = diagnostic.temporary_factory(tempfile.TemporaryDirectory,
                                               self.root / "retained", captures)
        error = ValueError("original fixture assertion")
        with mock.patch.object(diagnostic, "retain_timings", side_effect=OSError("disk failure")):
            try:
                with factory(prefix="sr-", dir="/tmp") as path:
                    raise error
            except ValueError as actual:
                self.assertIs(actual, error)
        self.assertFalse(Path(path).exists())
        self.assertEqual(captures, [{"partial": True, "unavailable": "OSError", "files": []}])

    def test_retention_is_after_body_finally_and_before_real_cleanup(self):
        captures = []
        events = []
        def capture(root, _destination):
            self.assertEqual(events, ["original-owned-cleanup"])
            self.assertTrue(root.is_dir())
            return {"partial": False, "files": []}
        factory = diagnostic.temporary_factory(tempfile.TemporaryDirectory,
                                               self.root / "retained", captures)
        with mock.patch.object(diagnostic, "retain_timings", side_effect=capture):
            with factory(prefix="sr-", dir="/tmp") as path:
                try:
                    pass
                finally:
                    events.append("original-owned-cleanup")
        self.assertFalse(Path(path).exists())

    def arguments(self):
        fixture = Path(__file__).resolve().parents[1] / "ci/test-repair.py"
        product = self.root / "product"
        product.write_bytes(b"fixture bytes, never executed")
        return argparse.Namespace(fixture=str(fixture), sail=str(product),
                                  sail_sha256=diagnostic.sha_file(product), engine_build="engine",
                                  verus_source="source", out=str(self.root / "output"),
                                  environment_label="pure-control")

    def test_runpy_original_namespace_and_environment_restored(self):
        args = self.arguments()
        def original(arguments):
            self.assertEqual(arguments.timeout, 120)
            self.assertEqual(arguments.engine_build, args.engine_build)
            self.assertEqual(arguments.verus_source, args.verus_source)
            self.assertEqual(os.environ["SAIL_JOB_RPC_TIMINGS"], "1")
            with tempfile.TemporaryDirectory(prefix="sr-", dir="/tmp") as path:
                populate(Path(path))
        original_factory = tempfile.TemporaryDirectory
        with mock.patch.dict(os.environ, {"SAIL_JOB_RPC_TIMINGS": "old"}):
            with mock.patch.object(diagnostic.runpy, "run_path", return_value={"run": original}) as load:
                self.assertEqual(diagnostic.run(args), 0)
                load.assert_called_once_with(args.fixture, run_name="original_repair_fixture")
            self.assertEqual(os.environ["SAIL_JOB_RPC_TIMINGS"], "old")
        self.assertIs(tempfile.TemporaryDirectory, original_factory)
        self.assertFalse(json.loads((Path(args.out) / "diagnostic.json").read_text())["qualification_credit"])

    def test_original_error_survives_final_record_io_failure(self):
        args = self.arguments()
        error = ValueError("original assertion")
        def original(_):
            with tempfile.TemporaryDirectory(prefix="sr-", dir="/tmp"):
                raise error
        with mock.patch.object(diagnostic.runpy, "run_path", return_value={"run": original}):
            with mock.patch.object(Path, "write_text", side_effect=OSError("record failure")):
                with self.assertRaises(ValueError) as failure:
                    diagnostic.run(args)
        self.assertIs(failure.exception, error)

    def test_fixture_digest_refusal_precedes_original_run(self):
        args = self.arguments()
        args.fixture = str(self.root / "wrong-fixture")
        Path(args.fixture).write_text("raise RuntimeError('never run')")
        with mock.patch.object(diagnostic.runpy, "run_path") as load:
            with self.assertRaises(AssertionError):
                diagnostic.run(args)
        load.assert_not_called()
        self.assertFalse(Path(args.out).exists())


if __name__ == "__main__":
    unittest.main()
