#!/usr/bin/env python3
"""No-proof controls for daemon-only observation and bounded export archival."""
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

spec = importlib.util.spec_from_file_location("rpc_workflow", Path(__file__).with_name("m7-rpc-stage-workflow.py"))
control = importlib.util.module_from_spec(spec)
spec.loader.exec_module(control)


class Controls(unittest.TestCase):
    def test_observation_environment_is_owned_daemon_only(self):
        captured = []
        class Service:
            def start(self, mode, binary, workspace, environment):
                captured.append(environment)
                self.actors = {mode: (SimpleNamespace(pid=23),)}
                return "started"
        helper = SimpleNamespace(Service=Service)
        environment = {"PATH": "/toolchain"}
        service = control.observed_service(helper)()
        self.assertEqual(service.start("reader", None, None, environment), "started")
        self.assertEqual(environment, {"PATH": "/toolchain"})
        self.assertEqual(captured, [{"PATH": "/toolchain", "SAIL_JOB_RPC_TIMINGS": "1"}])
        self.assertEqual(helper.rpc_launch["pid"], 23)

    def fixture(self, root, pid=23, rows=None):
        cache = root / "cache"
        location = cache / "projects/jobs"
        location.mkdir(parents=True)
        payload = {"kind": "diagnostic-job-rpc-stages", "capacity": 256, "daemon_pid": pid,
                   "origin_unix_ms": 100, "started": 301, "dropped": 2, "overwritten": 43,
                   "rows": rows if rows is not None else [{"sequence": 300, "operation": "result"}]}
        (location / "rpc-timings.json").write_text(json.dumps(payload))
        return SimpleNamespace(cache=cache, out=root, daemon=SimpleNamespace(pid=23), summary={}), SimpleNamespace(rpc_launch={"pid": 23})

    def test_complete_raw_export_is_copied_and_bound_to_owned_daemon(self):
        with tempfile.TemporaryDirectory() as directory:
            run, helper = self.fixture(Path(directory))
            before = (run.cache / "projects/jobs/rpc-timings.json").read_bytes()
            control.archive_rpc(run, helper)
            self.assertEqual((run.out / "rpc-timings.json").read_bytes(), before)
            self.assertEqual(run.summary["rpc_stage_archive"]["rows"], 1)
            self.assertEqual(run.summary["rpc_stage_archive"]["dropped"], 2)

    def test_wrong_owner_excess_rows_and_ambiguous_exports_refused(self):
        for fault in ("owner", "rows", "ambiguous"):
            with tempfile.TemporaryDirectory() as directory:
                run, helper = self.fixture(Path(directory), pid=24 if fault == "owner" else 23,
                                           rows=[{}] * 257 if fault == "rows" else None)
                if fault == "ambiguous":
                    alternate = run.cache / "results/projects/jobs"
                    alternate.mkdir(parents=True)
                    (alternate / "rpc-timings.json").write_bytes((run.cache / "projects/jobs/rpc-timings.json").read_bytes())
                with self.assertRaises(AssertionError):
                    control.archive_rpc(run, helper)
                self.assertFalse((run.out / "rpc-timings.json").exists())


if __name__ == "__main__":
    unittest.main()
