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

    def observer_fixture(self, root, body):
        # Exercise the real wrapper/observer boundary, with an intentionally
        # expensive inventory in a private copy of the actual executable.
        script = root/'perf-mem.py'
        override = '\n_inventory_calls = 0\ndef processes(*_args):\n    global _inventory_calls\n    _inventory_calls += 1\n'
        override += '    Path('+repr(str(root/'calls'))+').write_text(str(_inventory_calls))\n'
        override += body+'\n    return {}\n\n'
        script.write_text(SCRIPT.read_text().replace("if __name__ == '__main__':", override+"if __name__ == '__main__':"))
        return script

    def test_child_clock_excludes_slow_sampler_teardown(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker, output = root/'exited', root/'memory.json'
            script = self.observer_fixture(root,
                '    if _inventory_calls > 1:\n'
                '        deadline = time.monotonic()+5\n'
                '        while not Path('+repr(str(marker))+').exists() and time.monotonic() < deadline:\n'
                '            time.sleep(.005)\n'
                '        time.sleep(.5)')
            start = time.monotonic()
            result = subprocess.run([sys.executable, '-I', str(script), '--output', str(output),
                '--', sys.executable, '-c',
                'import time,pathlib,sys; time.sleep(.1); pathlib.Path(sys.argv[1]).write_text("done")',
                str(marker)], capture_output=True, timeout=10)
            elapsed = time.monotonic()-start
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertGreaterEqual(int((root/'calls').read_text()), 2)
            usage = json.loads(output.read_text())
            self.assertGreater(usage['wall_seconds'], 0)
            self.assertGreater(elapsed-usage['wall_seconds'], .2)

    def test_child_clock_is_not_held_by_sampler_cpu_work(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root/'memory.json'
            script = self.observer_fixture(root,
                '    if _inventory_calls > 1:\n'
                '        deadline = time.monotonic()+1\n'
                '        while time.monotonic() < deadline:\n'
                '            pass')
            result = subprocess.run([sys.executable, '-I', str(script), '--output', str(output),
                '--', sys.executable, '-c', 'import time; time.sleep(.1)'],
                capture_output=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertGreaterEqual(int((root/'calls').read_text()), 2)
            # A whole second of inventory CPU cannot hold the command reporter.
            self.assertLess(json.loads(output.read_text())['wall_seconds'], .75)

    def test_both_arms_prime_before_spawn_and_keep_command_priority(self):
        for sampled in [False, True]:
            with self.subTest(sampled=sampled), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                output, marker = root/'memory.json', root/'primed'
                script = self.observer_fixture(root,
                    '    Path('+repr(str(marker))+').write_text(str(os.getpid()))')
                command = [sys.executable, '-I', str(script), '--output', str(output)]
                if not sampled:
                    command.append('--no-sample')
                command += ['--', sys.executable, '-c',
                    'import os,pathlib,sys; p=pathlib.Path(sys.argv[1]); '
                    'assert p.exists(); assert int(p.read_text()) != os.getpid(); '
                    'print(os.getpriority(os.PRIO_PROCESS,0))', str(marker)]
                priority = os.getpriority(os.PRIO_PROCESS, 0)
                result = subprocess.run(command, capture_output=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(int(result.stdout), priority)
                self.assertEqual(os.getpriority(os.PRIO_PROCESS, 0), priority)

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
            actual = mem.mac_ps_processes()[42]
        self.assertEqual(actual, (17, 4096, 'Thu Oct 29 14:00:00 2026',
                                  ['/usr/local/bin/sail', 'serve', '--socket', 'a']))

    def test_mac_argument_buffer_preserves_spaces_and_excludes_environment(self):
        import struct
        argv = ['/a path/sail', 'serve', '--socket', 'a socket']
        data = struct.pack('=i', len(argv)) + b'/a path/sail\0\0'
        data += b'\0'.join(os.fsencode(arg) for arg in argv) + b'\0SECRET=value\0'
        self.assertEqual(mem.MacCollector.decode_argv(data), argv)
        for invalid in [b'', struct.pack('=i', -1), struct.pack('=i', 3) + b'x\0']:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                mem.MacCollector.decode_argv(invalid)

    @unittest.skipUnless(sys.platform == 'darwin', 'Darwin process API')
    def test_mac_native_forest_matches_ps_for_stable_child_and_excludes_unrelated(self):
        # A separate sleeping process makes the RSS comparison independent of
        # the observer's own Python allocations and ps-output parsing.
        body = 'import time; data=bytearray(64*1024*1024); print("ready",flush=True); time.sleep(10)'
        child = subprocess.Popen([sys.executable, '-c', body], stdout=subprocess.PIPE)
        unrelated = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(10)'])
        try:
            self.assertEqual(child.stdout.readline(), b'ready\n')
            time.sleep(.1)
            collector = mem.MacCollector()
            native = collector.collect({child.pid})
            oracle = mem.mac_ps_processes()
            self.assertEqual(set(native), {child.pid})
            self.assertNotIn(unrelated.pid, native)
            self.assertEqual(native[child.pid][:3], oracle[child.pid][:3])
            self.assertGreaterEqual(native[child.pid][1], 64*1024)
            self.assertEqual(native.backend, 'darwin-libproc')
        finally:
            child.terminate(); unrelated.terminate()
            child.communicate(timeout=5); unrelated.communicate(timeout=5)

    def test_mac_permission_failure_preserves_the_complete_ps_fallback(self):
        from unittest.mock import Mock, patch
        rows = {42: (17, 4096, 'old', ['cargo']), 99: (42, 8192, 'child', ['worker'])}
        collector = Mock()
        collector.collect.side_effect = PermissionError('protected descendant')
        with patch.object(mem, '_MAC_COLLECTOR', collector), \
                patch.object(mem, 'mac_ps_processes', return_value=rows):
            actual = mem.mac_processes({42})
        self.assertEqual(actual, rows)
        self.assertEqual(actual.backend, 'darwin-ps')

    @unittest.skipUnless(sys.platform == 'darwin', 'Darwin SDK and process API')
    def test_mac_sdk_layout_and_real_sail_argv_with_spaces(self):
        # The SDK is an independent ABI oracle; a real executable named sail
        # also exercises KERN_PROCARGS2, not only the byte-buffer decoder.
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)/'directory with spaces'
            directory.mkdir()
            source, binary = directory/'layout.c', directory/'sail'
            source.write_text("""#include <stddef.h>
#include <stdio.h>
#include <unistd.h>
#include <sys/proc_info.h>
_Static_assert(sizeof(struct proc_bsdinfo) == 136, "bsd ABI");
_Static_assert(sizeof(struct proc_taskinfo) == 96, "task ABI");
_Static_assert(sizeof(struct proc_taskallinfo) == 232, "combined ABI");
_Static_assert(offsetof(struct proc_bsdinfo, pbi_pid) == 12, "pid offset");
_Static_assert(offsetof(struct proc_bsdinfo, pbi_ppid) == 16, "parent offset");
_Static_assert(offsetof(struct proc_bsdinfo, pbi_start_tvsec) == 120, "start offset");
_Static_assert(offsetof(struct proc_taskinfo, pti_resident_size) == 8, "RSS offset");
int main(void) { puts("ready"); fflush(stdout); sleep(10); return 0; }
""")
            subprocess.run(['xcrun', 'clang', '-Wall', '-Werror', str(source), '-o', str(binary)],
                           check=True, capture_output=True)
            argv = [str(binary), 'serve', '--socket', 'a socket']
            child = subprocess.Popen(argv, stdout=subprocess.PIPE)
            try:
                self.assertEqual(child.stdout.readline(), b'ready\n')
                table = mem.MacCollector().collect({child.pid})
                self.assertEqual(table[child.pid][3], argv)
                self.assertTrue(mem.daemon(table[child.pid][3]))
                tracker = mem.Tracker({})
                tracker.sample(table, child.pid)
                self.assertEqual(tracker.tree_peak, 0)
                self.assertGreater(tracker.daemon_peak, 0)
            finally:
                child.terminate(); child.communicate(timeout=5)

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
        if sys.platform == 'darwin':
            self.assertEqual(usage['collection_backends'], ['darwin-libproc'])

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
