#!/usr/bin/env python3
"""Check performance harness command selection without building or solving."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class Commands(unittest.TestCase):
    def test_run_one_records_wrapped_failure_and_disabled_memory(self):
        text = Path(__file__).with_name('perf-real.sh').read_text()
        commands = 'measured()' + text.split('measured()', 1)[1].split('\nstats()', 1)[0]
        runner = 'run_one()' + text.split('run_one()', 1)[1].split('\nshim_pid=', 1)[0]
        for enabled in ['1', '0']:
            with self.subTest(memory=enabled), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                (root / 'project').mkdir()
                executable = root / 'cargo'
                executable.write_text('#!/bin/sh\nprintf "child-output\\n"\nexit 7\n')
                executable.chmod(0o755)
                env = {k: v for k, v in os.environ.items() if not k.startswith('SAIL_')}
                env.update(PATH=f"{root}:{env['PATH']}", SAIL_PERF_MEMORY=enabled,
                           SAIL_PERF_SETTLE='0', ROOT=str(root),
                           SCRIPTS=str(Path(__file__).resolve().parent))
                script = ('out="$ROOT/out"; results="$out/results.tsv"; pkg=.; partial=0; '
                          'fork=""; baseline=""; cargs=(); vargs=(); time_args=(); '
                          'here="$SCRIPTS"; sample_failures=0\n' + commands + '\n' +
                          'stats() { echo "0 0 0 0"; }; '
                          'times_of() { printf "NA\\tNA\\tNA\\tNA\\tNA\\tNA"; }; '
                          'now() { python3 -c "import time; print(time.time())"; };\n' +
                          runner + '\nrun_one 1 plain "$ROOT/project" cold\n')
                result = subprocess.run(['bash', '-c', script], env=env,
                                        capture_output=True, text=True, timeout=15)
                self.assertEqual(result.returncode, 0, result.stderr)
                row = (root / 'out/results.tsv').read_text().strip().split('\t')
                self.assertEqual(len(row), 22)
                self.assertEqual(row[4], '7')
                self.assertEqual((root / 'out/r1/plain.cold.log').read_text(), 'child-output\n')
                self.assertFalse(list((root / 'out/r1').glob('*.go')))
                if enabled == '0':
                    self.assertEqual(row[-3:], ['NA', 'NA', 'NA'])

    def run_command(self, mode, selector, baseline="", sample=False):
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
            setup = ''
            if sample:
                setup = ('memory_file="$MEMORY_OUTPUT"; memory_state="$MEMORY_STATE"; '
                         'daemon_status="$DAEMON_STATUS"; here="$SCRIPTS"; ')
                env.update(MEMORY_OUTPUT=str(root / 'memory.json'),
                           MEMORY_STATE=str(root / 'state.json'),
                           DAEMON_STATUS=str(root / 'status.json'),
                           SCRIPTS=str(Path(__file__).resolve().parent))
            script = (setup + 'cargs=(--lib); vargs=(); [ -z "$SELECTOR" ] || vargs=("$SELECTOR"); '
                      'fork="$FORK"; baseline="$BASELINE"; partial=0\n' + command +
                      '\nverus_cmd "$MODE" --time-expanded\n')
            result = subprocess.run(["bash", "-c", script], env=env, text=True,
                                    capture_output=True, timeout=10)
            output = json.loads(result.stdout) if result.stdout else None
            if sample:
                self.assertEqual(json.loads((root / 'memory.json').read_text())['exit'], 0)
            return result, output, str(shim), str(root / "fork/z3")

    def test_sampler_preserves_all_mode_commands_and_environment(self):
        for mode in ['plain', 'sail', 'incremental', 'fork-plain', 'fork-sail', 'fork-shim']:
            with self.subTest(mode=mode):
                result, output, shim, solver = self.run_command(mode, '--verify-root',
                                                                '/owned/baseline', sample=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn('--verify-root', output['args'])
                if mode == 'fork-shim':
                    self.assertEqual((output['shim'], output['solver']), (shim, solver))
                elif mode == 'sail':
                    self.assertEqual(output['runtime'], '/owned/baseline/cargo-verus')
                elif mode == 'fork-sail':
                    self.assertEqual(output['runtime'], str(Path(solver).with_name('cargo-verus')))

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

    def test_partial_sail_modes_use_public_json_before_separator(self):
        for mode in ["sail", "fork-sail", "incremental"]:
            with self.subTest(mode=mode):
                result, output, _, _ = self.run_command(mode, "--verify-module=selected")
                self.assertEqual(result.returncode, 0, result.stderr)
                expected = ["verify"] + (["--incremental"] if mode == "incremental" else [])
                self.assertEqual(output["args"], expected + ["--output", "json", "--lib", "--",
                                                            "--verify-module=selected", "--time-expanded"])

    def test_whole_sail_modes_keep_original_output_scope(self):
        for mode in ["sail", "fork-sail", "incremental"]:
            with self.subTest(mode=mode):
                result, output, _, _ = self.run_command(mode, "")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertNotIn("--output", output["args"])

    def test_partial_stage_totals_are_unavailable_instead_of_zero(self):
        text = Path(__file__).with_name("perf-real.sh").read_text()
        function = "times_of()" + text.split("times_of()", 1)[1].split("\nresults=", 1)[0]
        with tempfile.TemporaryDirectory(prefix="sail-perf-times-") as tmp:
            log = Path(tmp) / "raw.log"
            log.write_text("total-time: 17 ms\n")
            for partial, mode, expected in [(1, "sail", ["NA"] * 6),
                                            (1, "fork-sail", ["NA"] * 6),
                                            (1, "incremental", ["NA"] * 6),
                                            (1, "plain", ["17", "0", "0", "0", "0", "0"]),
                                            (0, "sail", ["17", "0", "0", "0", "0", "0"])]:
                command = function + '\npartial="$1"; times_of "$2" "$3"'
                result = subprocess.run(["bash", "-c", command, "bash", str(partial), str(log), mode],
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.split("\t"), expected)

    def test_report_reader_retains_unavailable_stage_values(self):
        path = Path(__file__).with_name("perf-real-report.py")
        spec = importlib.util.spec_from_file_location("perf_real_report", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory(prefix="sail-perf-report-") as tmp:
            row = ["1", "sail", "cold", "1.5", "0", "1", "0", "1", "1", "1", "1", "0"] + ["NA"] * 6 + ["0/0"]
            (Path(tmp) / "results.tsv").write_text("\t".join(row) + "\n")
            actual = module.load(tmp)[0]
            self.assertTrue(all(actual[key] is None for key in module.STAGE_COLS))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                module.report(tmp)
            self.assertIn("unavailable", output.getvalue())
            self.assertIn("1.50", output.getvalue())

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
