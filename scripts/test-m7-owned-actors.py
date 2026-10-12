#!/usr/bin/env python3
"""Owned-actor helper tests: checkpoints, daemon ownership and warm-up state."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("owned_actors", Path(__file__).with_name("m7-owned-actors.py"))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class Lifecycle(unittest.TestCase):
    def test_checkpoint_failure_preserves_previous_complete_snapshot(self):
        with tempfile.TemporaryDirectory(prefix="m7-checkpoint-") as temporary:
            path = Path(temporary) / "results.json"
            original = {"acceptance": {"complete": False}, "samples": [{"round": 0}]}
            bench.write_json(path, original)
            for failure in ("fsync", "replace"):
                with mock.patch.object(bench.os, failure, side_effect=OSError("disk unavailable")):
                    with self.assertRaisesRegex(OSError, "disk unavailable"):
                        bench.write_json(path, {"samples": [{"round": 1}]})
                self.assertEqual(json.loads(path.read_text()), original)
                self.assertEqual(list(path.parent.iterdir()), [path])
            replacement = {"samples": [{"round": 1}], "acceptance": {"complete": False}}
            bench.write_json(path, replacement)
            self.assertEqual(json.loads(path.read_text()), replacement)

    def test_private_daemon_reused_and_cleaned_after_failure_other_session_survives(self):
        with tempfile.TemporaryDirectory(prefix="m7-lifecycle-", dir="/tmp") as temporary:
            root = Path(temporary)
            binary = root / "daemon.py"
            binary.write_text(f"#!{sys.executable}\n"
                "import os,signal,socket,sys,time\n"
                "signal.signal(signal.SIGTERM,lambda sig,frame:sys.exit(0))\n"
                "s=socket.socket(socket.AF_UNIX); s.bind(sys.argv[sys.argv.index('--socket')+1]); s.listen()\n"
                "time.sleep(30)\n")
            binary.chmod(0o700)
            env = dict(os.environ, SAIL_CACHE_DIR=str(root / "cache"), SAIL_DAEMON_SOCKET=str(root / "d.sock"))
            other = subprocess.Popen([sys.executable,"-c","import time; time.sleep(30)"], start_new_session=True)
            actor = None
            try:
                with self.assertRaisesRegex(RuntimeError, "workload failed"):
                    with bench.OwnedDaemons(root) as owned:
                        owned.start("m7", binary, root, env)
                        actor = owned.actors["m7"][0]
                        owned.start("m7", binary, root, env)
                        self.assertEqual(len(owned.actors), 1)
                        raise RuntimeError("workload failed")
                self.assertIsNotNone(actor.poll(), "owned daemon leaked")
                self.assertIsNone(other.poll(), "cleanup killed an unrelated actor")
                records = json.loads((root / "actor-lifecycle.json").read_text())
                self.assertFalse(records["cleanup_errors"])
                self.assertEqual([record.get("stopped") for record in records["actors"]], [None, True])
            finally:
                if actor is not None and actor.poll() is None:
                    actor.kill(); actor.wait(timeout=5)
                other.terminate(); other.wait(timeout=5)

    def test_preexisting_socket_is_never_adopted_or_removed(self):
        with tempfile.TemporaryDirectory(prefix="m7-lifecycle-", dir="/tmp") as temporary:
            root = Path(temporary)
            socket = root / "owned-by-someone-else.sock"
            socket.write_text("untouched")
            env = {"SAIL_DAEMON_SOCKET":str(socket),"SAIL_CACHE_DIR":str(root / "cache")}
            with bench.OwnedDaemons(root) as owned:
                with self.assertRaisesRegex(ValueError, "pre-existing"):
                    owned.start("m7", Path(sys.executable), root, env)
                self.assertFalse(owned.actors)
            self.assertEqual(socket.read_text(), "untouched")

    def test_only_explicit_cold_declarations_allow_one_retry(self):
        doc = {"accepted": False, "selection_error": None, "batch": {
            "batched": True, "manifest_sha256": "manifest", "focus": {},
            "full": {"currentness": "unknown", "exit_code": 0, "error": None,
                     "targets": [{"requested": True, "complete": True}],
                     "notes": ["Build-script input declarations were established during this run"]}}}
        self.assertEqual(bench.warm_repair_state(doc, True), "declarations-established")
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, False)
        doc["batch"]["full"]["notes"] = []
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, True)
        doc["accepted"] = True
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, True)
        doc["batch"]["full"]["currentness"] = "current"
        self.assertEqual(bench.warm_repair_state(doc, False), "ready")
        doc["batch"]["full"]["targets"][0]["complete"] = False
        with self.assertRaises(ValueError):
            bench.warm_repair_state(doc, False)


if __name__ == "__main__":
    unittest.main()
