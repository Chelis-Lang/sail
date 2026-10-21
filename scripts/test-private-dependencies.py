#!/usr/bin/env python3
"""Check ephemeral Git credentials without network access or real secrets."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
WRAPPER = ROOT / "scripts/with-private-dependencies.sh"


class Credentials(unittest.TestCase):
    def test_scope_and_no_persistence(self):
        with tempfile.TemporaryDirectory() as directory:
            env = {**os.environ, "HOME": directory, "GIT_CONFIG_GLOBAL": directory + "/global",
                   "GIT_CONFIG_NOSYSTEM": "1", "GIT_TERMINAL_PROMPT": "0",
                   "SAIL_DEPENDENCY_READ_TOKEN": "synthetic-test-credential"}
            for name in list(env):
                if name.startswith("GIT_CONFIG_") and name not in {
                    "GIT_CONFIG_GLOBAL", "GIT_CONFIG_NOSYSTEM"
                }:
                    del env[name]
            for repository in ("sail", "dive"):
                for suffix in ("", ".git"):
                    result = subprocess.run([str(WRAPPER), "git", "credential", "fill"],
                        input=f"protocol=https\nhost=github.com\npath=Chelis-Lang/{repository}{suffix}\n\n",
                        text=True, capture_output=True, env=env, check=True)
                    self.assertIn("password=synthetic-test-credential", result.stdout)
                    self.assertEqual(result.stderr, "")
            for protocol, host, path in (("https", "elsewhere.example", "Chelis-Lang/dive"),
                                         ("http", "github.com", "Chelis-Lang/dive"),
                                         ("https", "github.com", "Chelis-Lang/other")):
                result = subprocess.run([str(WRAPPER), "git", "credential", "fill"],
                    input=f"protocol={protocol}\nhost={host}\npath={path}\n\n",
                    text=True, capture_output=True, env=env)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("synthetic-test-credential", result.stdout + result.stderr)
            result = subprocess.run([str(WRAPPER), "git", "config", "--list"],
                                    text=True, capture_output=True, env=env, check=True)
            self.assertNotIn("synthetic-test-credential", result.stdout + result.stderr)
            self.assertFalse(Path(directory, "global").exists())

    def test_missing_command(self):
        result = subprocess.run([str(WRAPPER)], capture_output=True)
        self.assertEqual(result.returncode, 2)


if __name__ == "__main__":
    unittest.main()
