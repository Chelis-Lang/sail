#!/usr/bin/env python3
"""No-proof controls for daemon-only observation and bounded export archival."""
import importlib.util
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest

spec = importlib.util.spec_from_file_location("rpc_workflow", Path(__file__).with_name("m7-descriptor-reader-workflow.py"))
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


    def test_inherited_selector_names_are_restricted_without_mutation(self):
        env = {"PATH": "/bin", "SAIL_TEST_Z3": "/solver"}
        self.assertEqual(control.inherited_selectors(env), ["SAIL_TEST_Z3"])
        self.assertEqual(env, {"PATH": "/bin", "SAIL_TEST_Z3": "/solver"})
        for name in ("SAIL_ENGINE", "SAIL_JOB_RPC_TIMINGS", "SAIL_DAEMON_SOCKET", "SAIL_MANAGED_FORCE"):
            with self.assertRaises(AssertionError):
                control.inherited_selectors(dict(env, **{name: "unapproved"}))

    def fake_run(self, root, passed=True, censuses=None, failure=None):
        calls = []
        class Run:
            def __init__(self):
                self.out = root
                self.bin = root / "bin"
                self.cache = root / "cache"
                self.socket = root / "cache.sock"
                self.args = SimpleNamespace(environment_label="component sequencing control")
                self.summary = {"passed": passed,
                    "censuses": censuses if censuses is not None else {"python-census": {}, "rust-census": {}},
                    "proofs": {"baseline": {"job": "actual-baseline-selector"}}}
            def run(self):
                calls.append("original-readers")
                if failure:
                    raise failure
            def save(self):
                calls.append("save")
            def command(self, label, argv, cwd, timeout, json_output):
                calls.append((label, argv, cwd, timeout, json_output))
                retained = self.out / "retained-raw"
                retained.mkdir()
                (retained / "summary.json").write_text('{"passed":true}')
        return Run(), calls

    def test_retained_uses_only_admitted_baseline_after_both_readers(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run, calls = self.fake_run(root)
            control.retained_once(run, SimpleNamespace(scripts_dir=root / "scripts"))
            self.assertEqual(calls[0], "original-readers")
            invocation = calls[2]
            self.assertEqual(invocation[0], "retained-integration")
            self.assertEqual(invocation[1][invocation[1].index("--job") + 1], "actual-baseline-selector")
            self.assertEqual(invocation[3], 145)
            self.assertTrue(run.summary["retained_functional_passed"])
            self.assertEqual(run.summary["retained_harness_sha256"],
                "207b45ef034b6c1e947dbb0efdb87368b62d5ef8a4c42ab5768413670f775a11")

    def test_failed_or_missing_census_never_enters_retained(self):
        for failed, partial in ((True, False), (False, True)):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                run, calls = self.fake_run(root, passed=not failed,
                    censuses={"python-census": {}} if partial else None)
                with self.assertRaises(AssertionError):
                    control.retained_once(run, SimpleNamespace(scripts_dir=root / "scripts"))
                self.assertEqual(calls, ["original-readers"])
                self.assertNotIn("retained_attempted", run.summary)

    def test_original_failure_is_not_retried_or_replaced(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            expected = TimeoutError("original600")
            run, calls = self.fake_run(root, failure=expected)
            with self.assertRaises(TimeoutError) as caught:
                control.retained_once(run, SimpleNamespace(scripts_dir=root / "scripts"))
            self.assertIs(caught.exception, expected)
            self.assertEqual(calls, ["original-readers"])


if __name__ == "__main__":
    unittest.main()
