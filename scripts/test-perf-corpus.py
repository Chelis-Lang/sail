#!/usr/bin/env python3
"""Evaluation controls: failures cannot turn into successful speed samples."""

import importlib.util
import json
from pathlib import Path
import tempfile
import tomllib
import unittest
from unittest.mock import patch
import subprocess


def module(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"),
                                                Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


suite = module("perf-corpus")
scaling = module("perf-scaling")
work = module("perf-work")


class EvaluationControls(unittest.TestCase):
    def test_every_catalogue_selection_has_a_full_source_pin(self):
        entries = suite.workloads()
        self.assertGreater(len(entries), 4)
        for entry in entries:
            manifest = tomllib.loads(suite.manifest(entry).read_text())
            project = next(project for project in manifest["project"]
                           if project["name"] == entry["project"])
            self.assertRegex(project["commit"], r"^[0-9a-f]{40}$")
        self.assertEqual({entry["project"] for entry in entries if entry.get("held_out")},
                         {"semi-persistent", "hekate-math"})

    def test_matching_plain_failure_is_not_a_passing_compatibility_baseline(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "report.json"
            report = {"summary": {"units": 1, "same": 1, "projects_skipped": 0},
                      "units": [{"plain": {"exit_code": 1, "verified": 12, "errors": 1}}]}
            path.write_text(json.dumps(report))
            self.assertEqual(suite.conformance_status(path)[1:], (True, False))
            report["units"][0]["plain"].update(exit_code=0, errors=0)
            path.write_text(json.dumps(report))
            self.assertEqual(suite.conformance_status(path)[1:], (True, True))
            report["summary"]["projects_skipped"] = 1
            path.write_text(json.dumps(report))
            self.assertEqual(suite.conformance_status(path)[1:], (False, False))

    def test_git_warning_cannot_certify_a_clean_frozen_source(self):
        result = subprocess.CompletedProcess(["git"], 0, b"", b"warning: unreadable directory")
        with patch.object(suite.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(ValueError, "incomplete Git source inventory"):
                suite.source_state()

    def test_changed_or_removed_measurement_input_refuses_the_capture(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "tool"
            path.write_bytes(b"original")
            expected = suite.file_digests([path])
            self.assertTrue(suite.inputs_intact(expected))
            replacement = Path(tmp) / "replacement"
            replacement.write_bytes(b"changed")
            replacement.replace(path)
            self.assertFalse(suite.inputs_intact(expected))
            path.unlink()
            self.assertFalse(suite.inputs_intact(expected))

    def test_missing_executable_retains_a_failed_command_and_log(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "command.log"
            result = suite.execute([str(Path(tmp) / "missing")], path, {})
            self.assertEqual(result["exit"], 127)
            self.assertTrue(path.read_text())
            self.assertRegex(result["log_sha256"], r"^[0-9a-f]{64}$")

    def test_benchmark_refusals_and_incomplete_rounds_cannot_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / "warm.json"
            unit = {"same": 3, "different": 0, "unstable": 0, "infra_errors": 0,
                    "plain": {"wall_ms": [1, 2, 3]}, "sail": {"wall_ms": [1, 2]}}
            path.write_text(json.dumps({"units": [unit]}))
            self.assertTrue(suite.benchmark_complete(path, "warm", 2))
            unit["unstable"] = 1
            path.write_text(json.dumps({"units": [unit]}))
            self.assertFalse(suite.benchmark_complete(path, "warm", 2))
            unit["unstable"] = 0
            unit["plain"]["wall_ms"].pop()
            path.write_text(json.dumps({"units": [unit]}))
            self.assertFalse(suite.benchmark_complete(path, "warm", 2))

    def test_scaling_workspaces_are_reproducible_and_cover_the_complete_graph(self):
        for shape in ("chain", "diamond", "shared"):
            with self.subTest(shape=shape), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                vstd = root / "vstd"
                vstd.mkdir()
                (vstd / "Cargo.toml").write_text('[package]\nname="vstd"\n')
                for name in ("a", "b"):
                    scaling.generate(root / name, shape, 16, 3, 4, 64, vstd)
                files = lambda name: {path.relative_to(root / name): path.read_bytes()
                                      for path in (root / name).rglob("*") if path.is_file()}
                self.assertEqual(files("a"), files("b"))
                graph = json.loads((root / "a/sail-scaling.json").read_text())["dependencies"]
                seen = set()

                def visit(name, ancestors=()):
                    self.assertNotIn(name, ancestors)
                    seen.add(name)
                    for dependency in graph[name]:
                        visit(dependency, (*ancestors, name))

                visit("sail-scale-root")
                self.assertEqual(seen, set(graph))
                for path in (root / "a").rglob("Cargo.toml"):
                    tomllib.loads(path.read_text())

    def test_syscall_errors_and_unknown_rows_remain_visible(self):
        sample = ("% time seconds usecs/call calls errors syscall\n"
                  "------ ----------- ----------- --------- --------- ----------------\n"
                  " 50.00 0.000100 1 100 3 openat\n"
                  " 50.00 0.000100 2 50 read\n"
                  "100.00 0.000200 1 150 3 total\n")
        result = work.parse_summary(sample)
        self.assertEqual(result["openat"]["errors"], 3)
        self.assertEqual(result["read"]["calls"], 50)
        with self.assertRaises(ValueError):
            work.parse_summary(sample + "unknown row\n")


if __name__ == "__main__":
    unittest.main()
