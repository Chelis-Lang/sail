#!/usr/bin/env python3
"""Check the real repair smoke harness only stops its owned process sessions."""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

spec = importlib.util.spec_from_file_location("repair_acceptance", Path(__file__).with_name("test-repair.py"))
repair = importlib.util.module_from_spec(spec)
spec.loader.exec_module(repair)


class OwnedActors(unittest.TestCase):
    def test_mcp_graceful_eof_is_reaped(self):
        actor = subprocess.Popen([sys.executable, "-c", "import sys; sys.stdin.read()"],
                                 stdin=subprocess.PIPE, start_new_session=True)
        repair.stop_owned(actor, 5, close_stdin=True)
        self.assertEqual(actor.returncode, 0)

    def test_stalled_actor_and_child_are_killed_without_touching_other_sessions(self):
        with tempfile.TemporaryDirectory(prefix="repair-lifecycle-") as temporary:
            marker = Path(temporary) / "child-survived"
            ready = Path(temporary) / "ready"
            child = "import pathlib,time; time.sleep(.5); pathlib.Path(" + repr(str(marker)) + ").write_text('alive'); time.sleep(30)"
            parent = ("import pathlib,signal,subprocess,sys,time; "
                      "signal.signal(signal.SIGTERM,signal.SIG_IGN); "
                      "subprocess.Popen([sys.executable,'-c'," + repr(child) + "]); "
                      "pathlib.Path(" + repr(str(ready)) + ").write_text('ready'); time.sleep(30)")
            actor = subprocess.Popen([sys.executable, "-c", parent], start_new_session=True)
            other = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"], start_new_session=True)
            try:
                deadline = time.monotonic() + 5
                while not ready.exists():
                    self.assertLess(time.monotonic(), deadline, "owned fixture did not start")
                    time.sleep(.01)
                repair.stop_owned(actor, .05)
                self.assertIsNotNone(actor.returncode)
                time.sleep(.6)
                self.assertFalse(marker.exists(), "owned child survived group cancellation")
                self.assertIsNone(other.poll(), "cleanup killed a different actor session")
            finally:
                repair.stop_owned(actor, .05)
                repair.stop_owned(other, 5)


if __name__ == "__main__":
    unittest.main()
