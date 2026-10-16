#!/usr/bin/env python3
"""Check performance harness command selection without building or solving."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class Commands(unittest.TestCase):
    def run_command(self, mode, selector, baseline=""):
        text = Path(__file__).with_name("perf-real.sh").read_text()
        command = text.split("partial=0\n", 1)[1].split("\nmode_path()", 1)[0]
        with tempfile.TemporaryDirectory(prefix="sail-perf-command-") as tmp:
            root = Path(tmp)
            cargo = root / "cargo"
            cargo.write_text(
                "#!/usr/bin/env python3\nimport json,os,sys\n"
                "print(json.dumps({'args':sys.argv[1:],'shim':os.getenv('VERUS_Z3_PATH'),"
                "'solver':os.getenv('SAIL_Z3_PATH'),'socket':os.getenv('SAIL_DAEMON_SOCKET'),"
                "'runtime':os.getenv('SAIL_CARGO_VERUS')}))\n"
            )
            cargo.chmod(0o755)
            sail = root / "sail"
            sail.write_bytes(cargo.read_bytes())
            sail.chmod(0o755)
            shim = root / "sail-z3"
            shim.write_text("#!/bin/sh\nexit 0\n")
            shim.chmod(0o755)
            env = {key: val for key, val in os.environ.items()
                   if not key.startswith(("SAIL_", "VERUS_"))}
            env.update(PATH=f"{root}:{env['PATH']}", MODE=mode, SELECTOR=selector,
                       FORK=str(root / "fork"), BASELINE=baseline, SAIL_DAEMON_SOCKET=str(root / "d.sock"))
            script = ('cargs=(--lib); vargs=(); [ -z "$SELECTOR" ] || vargs=("$SELECTOR"); '
                      'fork="$FORK"; baseline="$BASELINE"; partial=0\n' + command +
                      '\nverus_cmd "$MODE" --time-expanded\n')
            result = subprocess.run(["bash", "-c", script], env=env, text=True,
                                    capture_output=True, timeout=10)
            output = json.loads(result.stdout) if result.stdout else None
            return result, output, str(shim), str(root / "fork/z3")

    def test_focused_shim_uses_fork_solver_and_owned_daemon(self):
        result, output, shim, solver = self.run_command("fork-shim", "--verify-module=module")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output["args"], ["verus", "focus", "--lib", "--",
                                         "--verify-module=module", "--time-expanded"])
        self.assertEqual(output["shim"], shim)
        self.assertEqual(output["solver"], solver)
        self.assertTrue(output["socket"].endswith("/d.sock"))

    def test_full_shim_keeps_verify_subcommand(self):
        result, output, _, _ = self.run_command("fork-shim", "")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output["args"], ["verus", "verify", "--lib", "--", "--time-expanded"])

    def test_plain_focus_stays_plain(self):
        result, output, _, _ = self.run_command("fork-plain", "--verify-root")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output["args"][1], "focus")
        self.assertIsNone(output["shim"])
        self.assertIsNone(output["solver"])

    def test_unknown_mode_refuses_instead_of_recording_a_success(self):
        result, output, _, _ = self.run_command("unknown", "")
        self.assertEqual(result.returncode, 2)
        self.assertIsNone(output)

    def test_sail_binds_baseline_runtime(self):
        result, output, _, _ = self.run_command("sail", "", "/owned/baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output["runtime"], "/owned/baseline/cargo-verus")

    def test_fork_sail_binds_fork_despite_baseline(self):
        result, output, _, solver = self.run_command("fork-sail", "", "/owned/baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(output["runtime"], str(Path(solver).with_name("cargo-verus")))

    def test_plain_does_not_use_sail_runtime_override(self):
        result, output, _, _ = self.run_command("plain", "", "/owned/baseline")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIsNone(output["runtime"])

    def modes(self, catalogue="", explicit="", fork="/owned/fork"):
        text = Path(__file__).with_name("perf-real.sh").read_text()
        marker = 'if [ "$cmd" = run ]; then\n'
        self.assertEqual(text.count(marker), 1)
        block = marker + text.split(marker, 1)[1].split("\nnow()", 1)[0]
        env = {key: val for key, val in os.environ.items() if not key.startswith("SAIL_")}
        env.update(SAIL_PERF_CATALOGUE=catalogue, SAIL_PERF_MODES=explicit, FORK=fork)
        return subprocess.run(["bash", "-c", 'cmd=run; fork="$FORK";\n' + block +
                               '\nprintf "%s\n" "${modes[@]}"'], env=env,
                              text=True, capture_output=True, timeout=10)

    def test_default_modes_keep_both_original_arms(self):
        result = self.modes()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["plain", "sail"])

    def test_incidental_default_keeps_three_runtime_arms(self):
        result = self.modes(catalogue="incidental")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["plain", "sail", "fork-sail"])

    def test_explicit_shim_and_unknown_mode_admission(self):
        result = self.modes(catalogue="incidental", explicit="fork-shim")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), ["fork-shim"])
        self.assertEqual(self.modes(explicit="fork-shim", fork="").returncode, 2)
        self.assertEqual(self.modes(explicit="unknown").returncode, 2)


if __name__ == "__main__":
    unittest.main()
