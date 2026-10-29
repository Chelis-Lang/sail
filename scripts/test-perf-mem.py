#!/usr/bin/env python3
"""Memory sampler controls, including a real allocating subprocess tree."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPT = Path(__file__).with_name('perf-mem.py')
spec = importlib.util.spec_from_file_location('perf_mem', SCRIPT)
mem = importlib.util.module_from_spec(spec)
spec.loader.exec_module(mem)


class Memory(unittest.TestCase):
    def test_readiness_precedes_command_for_both_overhead_arms(self):
        for sampled in [True, False]:
            with self.subTest(sampled=sampled), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                ready, gate, marker = root / 'ready', root / 'go', root / 'ran'
                os.mkfifo(gate)
                output = root / 'memory.json'
                command = [sys.executable, '-I', str(SCRIPT), '--output', str(output),
                           '--ready', str(ready), '--go', str(gate)]
                if not sampled:
                    command.append('--no-sample')
                command += ['--', sys.executable, '-c',
                            'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text("ran"); '
                            'print("child output"); sys.exit(7)', str(marker)]
                process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                try:
                    deadline = time.monotonic() + 5
                    while not ready.exists() and process.poll() is None and time.monotonic() < deadline:
                        time.sleep(.01)
                    self.assertTrue(ready.exists())
                    self.assertFalse(marker.exists(), 'command ran before timing began')
                    with gate.open('wb', buffering=0) as writer:
                        writer.write(b'g')
                    stdout, stderr = process.communicate(timeout=5)
                    self.assertEqual(process.returncode, 7, stderr)
                    self.assertEqual(stdout, b'child output\n')
                    self.assertTrue(marker.exists())
                    usage = json.loads(output.read_text())
                    self.assertEqual(usage['exit'], 7)
                    if not sampled:
                        self.assertTrue(all(usage[k] is None for k in
                                            ['tree_peak_kb', 'largest_kb', 'daemon_peak_kb']))
                finally:
                    if process.poll() is None:
                        process.terminate()
                    process.communicate(timeout=5)

    def test_tree_sum_daemon_reparent_and_pid_reuse(self):
        tracker = mem.Tracker({})
        tracker.sample({10: (1, 5, 'a', ['cargo']),
                        11: (10, 100, 'b', ['rustc']),
                        12: (10, 200, 'c', ['z3']),
                        13: (10, 20, 'd', ['sail', 'serve']),
                        14: (13, 30, 'e', ['worker']),
                        15: (1, 9000, 'f', ['unrelated'])}, 10)
        self.assertEqual(tracker.tree_peak, 305)
        self.assertEqual(tracker.daemon_peak, 50)
        tracker.sample({13: (1, 60, 'd', ['sail', 'serve']),
                        14: (13, 40, 'e', ['worker']),
                        11: (1, 10000, 'new', ['unrelated'])}, 10)
        self.assertEqual(tracker.tree_peak, 305)
        self.assertEqual(tracker.daemon_peak, 100)
        tracker.sample({13: (1, 10000, 'new', ['unrelated'])}, 10)
        self.assertEqual(tracker.daemon_peak, 100)
        self.assertEqual(tracker.daemons, {})

    def test_preexisting_daemon_is_separate(self):
        tracker = mem.Tracker({'5': 'old'})
        tracker.sample({5: (1, 100, 'old', ['sail', '_serve-jobs']),
                        10: (1, 50, 'now', ['cargo'])}, 10)
        self.assertEqual((tracker.tree_peak, tracker.daemon_peak), (50, 100))

    def test_units(self):
        self.assertEqual(mem.largest_kb(4096, 'darwin'), 4)
        self.assertEqual(mem.largest_kb(4096, 'linux'), 4096)

    def test_linux_stat_with_spaces_and_parentheses(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            pid = root / '42'
            pid.mkdir()
            fields = ['S', '17'] + ['0'] * 17 + ['1234']
            (pid / 'stat').write_text('42 (name (with) spaces) ' + ' '.join(fields))
            (pid / 'statm').write_text('9 3 0')
            (pid / 'cmdline').write_bytes(b'sail\0serve\0')
            actual = mem.linux_processes(root)[42]
            self.assertEqual(actual[:3], (17, 3 * os.sysconf('SC_PAGE_SIZE') // 1024, '1234'))

    def test_mac_ps_units_and_identity(self):
        from unittest.mock import patch
        result = subprocess.CompletedProcess([], 0,
            ' 42 17 4096 Thu Oct 29 14:00:00 2026 /usr/local/bin/sail serve --socket a\n')
        with patch.object(mem.subprocess, 'run', return_value=result):
            actual = mem.mac_processes()[42]
        self.assertEqual(actual, (17, 4096, 'Thu Oct 29 14:00:00 2026',
                                  ['/usr/local/bin/sail', 'serve', '--socket', 'a']))

    def wrap(self, root, command, **kwargs):
        output = root / 'memory.json'
        result = subprocess.run([sys.executable, '-I', str(SCRIPT), '--output', str(output),
                                 '--', *command], capture_output=True, timeout=15, **kwargs)
        return result, json.loads(output.read_text())

    def test_real_tree_allocates_two_children(self):
        child = 'import time; data=bytearray(48*1024*1024); time.sleep(.8)'
        parent = ('import subprocess,sys; '
                  f'p=[subprocess.Popen([sys.executable,"-c",{child!r}]) for _ in range(2)]; '
                  '[x.wait() for x in p]')
        with tempfile.TemporaryDirectory() as tmp:
            result, usage = self.wrap(Path(tmp), [sys.executable, '-c', parent])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertGreaterEqual(usage['tree_peak_kb'], 96 * 1024)
        self.assertGreaterEqual(usage['largest_kb'], 48 * 1024)
        self.assertEqual(usage['daemon_peak_kb'], 0)

    def test_exit_streams_arguments_and_environment_are_unchanged(self):
        child = ('import json,os,sys; print(json.dumps(dict(os.environ)),flush=True); '
                 'print(sys.argv[1:],file=sys.stderr); sys.exit(7)')
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            env = dict(os.environ, SAIL_TEST_MEMORY='literal value')
            command = [sys.executable, '-c', child, 'a b', '$(literal)']
            direct = subprocess.run(command, env=env, capture_output=True, timeout=10)
            result, usage = self.wrap(root, command, env=env)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (direct.returncode, direct.stdout, direct.stderr))
        self.assertEqual(usage['exit'], 7)

    def test_signal_exit_matches_shell_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            result, usage = self.wrap(Path(tmp), [sys.executable, '-c',
                'import os,signal; os.kill(os.getpid(),signal.SIGTERM)'])
        self.assertEqual(result.returncode, 128 + signal.SIGTERM)
        self.assertEqual(usage['exit'], -signal.SIGTERM)

    def test_preexisting_real_daemon_status_and_snapshot(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            # argv names a real executable stand-in exactly as Sail's daemon;
            # Python reads the owned file named serve as its program.
            (root / 'serve').write_text('import time\nx=bytearray(16000000)\ntime.sleep(10)\n')
            child = subprocess.Popen(['sail', 'serve'], executable=sys.executable, cwd=root)
            try:
                status, output, state = root / 'status.json', root / 'out.json', root / 'state.json'
                status.write_text(json.dumps({'info': {'pid': child.pid}}))
                command = [sys.executable, '-I', str(SCRIPT), '--output', str(output),
                           '--status', str(status), '--state', str(state)]
                run = subprocess.run(command + ['--', sys.executable, '-c',
                    'import time; time.sleep(.3)'], capture_output=True, timeout=10)
                self.assertEqual(run.returncode, 0, run.stderr)
                self.assertGreater(json.loads(output.read_text())['daemon_peak_kb'], 12000)
                self.assertIn(str(child.pid), json.loads(state.read_text()))
                # A cold server can detach before the first descendant scan.
                # Completion sampling must discover it from fresh status while
                # preserving the measured command's own figures.
                before = json.loads(output.read_text())
                before['daemon_peak_kb'] = 0
                output.write_text(json.dumps(before))
                state.write_text('{}')
                run = subprocess.run(command + ['--snapshot'], capture_output=True, timeout=10)
                self.assertEqual(run.returncode, 0, run.stderr)
                snapshot = json.loads(output.read_text())
                self.assertGreater(snapshot['daemon_peak_kb'], 12000)
                self.assertEqual(snapshot['tree_peak_kb'], before['tree_peak_kb'])
                self.assertEqual(snapshot['exit'], before['exit'])
                if sys.platform.startswith('linux'):
                    self.assertGreater(snapshot['daemon_hwm_kb'][str(child.pid)], 12000)
            finally:
                child.terminate()
                child.wait(timeout=5)


if __name__ == '__main__':
    unittest.main()
