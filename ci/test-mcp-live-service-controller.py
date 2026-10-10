#!/usr/bin/env python3
"""Negative shutdown controls for the live-service test controller only.

These owned Python children are controller fixtures, not verification or MCP
acceptance. The actual live-service scenario always runs installed Sail.
"""

import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SPEC = importlib.util.spec_from_file_location(
    "live_service", Path(__file__).with_name("test-mcp-live-service.py"))
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ShutdownControls(unittest.TestCase):
    def child(self, root, after_eof):
        executable = root / "shutdown-fixture"
        executable.write_text(
            f"#!{sys.executable}\nimport sys, time\n"
            "sys.stdin.buffer.read()\n" + after_eof + "\n")
        executable.chmod(0o700)
        return MODULE.Mcp(executable, root, None, root)

    def test_normal_eof_is_joined_and_complete(self):
        with tempfile.TemporaryDirectory() as temporary:
            child = self.child(Path(temporary), 'print("{\\"complete\\": true}")')
            try:
                child.finish()
                self.assertEqual(child.process.returncode, 0)
                self.assertEqual(child.process.stdout.read(), b'{"complete": true}\n')
            finally:
                child.close()
                child.process.stdout.close()

    def test_nonzero_exit_cannot_establish_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            child = self.child(Path(temporary), "sys.exit(17)")
            try:
                with self.assertRaisesRegex(AssertionError, "normally after EOF: 17"):
                    child.finish()
                self.assertEqual(child.process.returncode, 17)
            finally:
                child.close()
                child.process.stdout.close()

    def test_eof_ignoring_process_cannot_pass_via_forced_cleanup(self):
        with tempfile.TemporaryDirectory() as temporary:
            child = self.child(Path(temporary), "time.sleep(30)")
            try:
                # Exercise the real, unchanged five-second shutdown bound.
                with self.assertRaises(subprocess.TimeoutExpired):
                    child.finish()
                self.assertIsNone(child.process.poll())
            finally:
                child.close()
                child.process.stdout.close()
            self.assertIsNotNone(child.process.returncode)
            self.assertNotEqual(child.process.returncode, 0)


if __name__ == "__main__":
    unittest.main()
