#!/usr/bin/env python3
"""Bounded real-process ownership tests for the local repair workload harness."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("repair_bench", Path(__file__).with_name("m7-repair-bench.py"))
bench = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bench)


class Lifecycle(unittest.TestCase):
    def test_observation_failure_interrupts_cli_then_cancels_only_owned_job(self):
        with tempfile.TemporaryDirectory(prefix="m7-lifecycle-", dir="/tmp") as temporary:
            root = Path(temporary)
            cancelled = root / "cancelled"
            ready = root / "ready"
            release = root / "release"
            program = root / "cli.py"
            program.write_text(
                "import pathlib,signal,sys,time\n"
                "def interrupt(sig,frame):\n"
                f" pathlib.Path({str(cancelled)!r}).write_text('cancelled'); sys.exit(130)\n"
                "signal.signal(signal.SIGINT,interrupt)\n"
                f"pathlib.Path({str(ready)!r}).touch()\n"
                f"while not pathlib.Path({str(release)!r}).exists(): time.sleep(0.01)\n"
                f"with pathlib.Path({str(root / 'evidence/stderr.txt')!r}).open('a') as out:\n"
                " print('Repair validation job owned-job',file=out,flush=True)\n"
                "time.sleep(30)\n")

            class Observer:
                def __init__(self, environment):
                    self.job = None
                    self.address = root / "owned-job.sock"
                def poll(self, stderr, directory):
                    if "owned-job" in stderr.read_text():
                        self.job = "owned-job"
                        raise RuntimeError("original observation failure")
                    return False

            env = dict(os.environ, SAIL_DAEMON_SOCKET=str(root / "owned.sock"))
            with mock.patch.object(bench, "ProofObserver", Observer), mock.patch.object(bench.protocol, "call") as call:
                # invoke selects observation only when argv[1] is repair-check.
                cli = root / "repair-check"
                cli.write_text(f"#!{sys.executable}\n" + program.read_text())
                cli.chmod(0o700)
                # This checks interruption after observation fails, rather than
                # Python startup speed under the full workspace gate. Establish
                # the real process and installed handler before the unchanged
                # two-second observation interval, then release it at Popen.
                child = subprocess.Popen([str(cli), "repair-check"], cwd=root, env=env,
                                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                         start_new_session=True)
                try:
                    startup = time.monotonic()
                    while not ready.is_file():
                        self.assertIsNone(child.poll(), "fixture exited before installing its handler")
                        if time.monotonic() - startup > 5:
                            self.fail("fixture startup handshake timed out")
                        time.sleep(0.01)

                    def launch(command, **options):
                        self.assertEqual(Path(command[-2]), cli)
                        self.assertTrue(options["start_new_session"])
                        release.touch()
                        return child

                    with mock.patch.object(bench.subprocess, "Popen", launch):
                        with self.assertRaisesRegex(RuntimeError, "original observation failure"):
                            bench.invoke([str(cli), "repair-check"], root, env, root / "evidence", 2)
                finally:
                    if child.poll() is None:
                        child.kill(); child.wait(timeout=5)
                call.assert_called_once_with(root / "owned-job.sock", {"operation":"cancel", "id":"owned-job", "superseded":False})
            self.assertTrue(cancelled.is_file(), "CLI interrupt cancellation did not execute")
            evidence = json.loads((root / "evidence/interrupted.json").read_text())
            self.assertEqual(evidence["exception"], "RuntimeError")
            self.assertFalse(evidence["timed_out"])
            self.assertFalse(evidence["cleanup_errors"])

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


if __name__ == "__main__":
    unittest.main()
