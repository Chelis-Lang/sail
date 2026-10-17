"""Bounded local accounting-owner controls, not Linux/pool qualification."""
import concurrent.futures
import os
from pathlib import Path
import signal
import subprocess
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))
from owned_wait4 import OwnedWait4, usage_record


class Controls(unittest.TestCase):
    def child(self, code):
        return OwnedWait4([sys.executable, '-B', '-c', code], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def close(self, child):
        # Controls always retain the actual owned PID until the sole reap.
        if child.poll() is None:
            os.kill(child.pid, signal.SIGKILL)
        child.wait(timeout=5)
        for stream in (child.stdin, child.stdout, child.stderr):
            stream.close()

    def test_complete_bytes_exit_and_repeat_wait(self):
        child = self.child("import sys; d=sys.stdin.buffer.read(); sys.stdout.buffer.write(d); sys.stderr.buffer.write(b'err'); sys.exit(7)")
        try:
            child.stdin.write(b'original\x00bytes\xff\n')
            child.stdin.close()
            self.assertEqual(child.stdout.read(), b'original\x00bytes\xff\n')
            self.assertEqual(child.stderr.read(), b'err')
            self.assertEqual(child.wait(5), 7)
            with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
                self.assertEqual(list(pool.map(lambda _: child.wait(5), range(64))), [7] * 64)
            record = child.accounting()
            self.assertEqual(record['pid'], child.pid)
            self.assertEqual(record['exit'], 7)
            self.assertGreater(record['accounting']['peak_rss_bytes'], 0)
            with self.assertRaises(ChildProcessError):
                os.wait4(child.pid, os.WNOHANG)
        finally:
            self.close(child)

    def test_timeout_retains_live_child_then_signal_status(self):
        child = self.child('import time; time.sleep(20)')
        try:
            with self.assertRaises(subprocess.TimeoutExpired):
                child.wait(.01)
            self.assertIsNone(child.poll())
            with self.assertRaises(ChildProcessError):
                child.accounting()
            os.kill(child.pid, signal.SIGKILL)
            self.assertEqual(child.wait(5), -signal.SIGKILL)
            self.assertEqual(child.accounting()['status'], signal.SIGKILL)
        finally:
            self.close(child)

    def test_independent_children_accounting_not_process_wide(self):
        children = [self.child('import sys; sys.exit(' + str(x) + ')') for x in range(8)]
        try:
            self.assertEqual([c.wait(5) for c in children], list(range(8)))
            self.assertEqual({c.accounting()['pid'] for c in children}, {c.pid for c in children})
        finally:
            for child in children:
                self.close(child)

    def test_peak_survives_released_allocation_and_child_exit(self):
        child = self.child("import sys; a=bytearray(16*1024*1024); a[::4096]=b'x'*4096; del a; print('released', flush=True); sys.stdin.buffer.read()")
        try:
            self.assertEqual(child.stdout.readline(), b'released\n')
            self.assertIsNone(child.poll())
            child.stdin.close()
            self.assertEqual(child.wait(5), 0)
            self.assertGreaterEqual(child.accounting()['accounting']['peak_rss_bytes'], 16 * 1024 * 1024)
        finally:
            self.close(child)

    def test_units_and_invalid_values(self):
        usage = SimpleNamespace(ru_maxrss=42, ru_utime=.1, ru_stime=.2)
        self.assertEqual(usage_record(usage, 'linux')['peak_rss_bytes'], 43008)
        self.assertEqual(usage_record(usage, 'darwin')['peak_rss_bytes'], 42)
        for platform in ('unsupported',):
            with self.assertRaises(ValueError):
                usage_record(usage, platform)
        for peak in (-1, 1.5, True):
            with self.assertRaises(ValueError):
                usage_record(SimpleNamespace(ru_maxrss=peak, ru_utime=0, ru_stime=0), 'linux')
        for value in (-1, float('nan'), float('inf'), True):
            with self.assertRaises(ValueError):
                usage_record(SimpleNamespace(ru_maxrss=1, ru_utime=value, ru_stime=0), 'linux')

    def test_missing_accounting_never_becomes_zero_success(self):
        # The waiter is deliberately broken, not the kernel. Reap the actual
        # child explicitly after proving refusal; no leaked process or zombie.
        with patch('owned_wait4.os.wait4', side_effect=ChildProcessError('competing waiter')):
            child = self.child('pass')
            child._reaper.join(5)
            with self.assertRaises(ChildProcessError):
                child.wait(1)
            with self.assertRaises(ChildProcessError):
                child.accounting()
        pid, status, _ = os.wait4(child.pid, 0)
        child._child.returncode = os.waitstatus_to_exitcode(status)
        self.assertEqual(pid, child.pid)
        for stream in (child.stdin, child.stdout, child.stderr):
            stream.close()

    def test_thread_start_failure_reaps_and_closes_owned_child(self):
        original = subprocess.Popen
        created = []

        def spawn(*args, **kwargs):
            child = original(*args, **kwargs)
            created.append(child)
            return child

        with patch('owned_wait4.subprocess.Popen', side_effect=spawn), patch(
                'owned_wait4.threading.Thread.start', side_effect=RuntimeError('no thread')):
            with self.assertRaises(RuntimeError):
                self.child('import time; time.sleep(20)')
        self.assertEqual(len(created), 1)
        self.assertEqual(created[0].returncode, -signal.SIGKILL)
        self.assertTrue(all(s.closed for s in (created[0].stdin, created[0].stdout, created[0].stderr)))
        with self.assertRaises(ChildProcessError):
            os.wait4(created[0].pid, os.WNOHANG)

    def test_thread_constructor_failure_never_spawns_child(self):
        with patch('owned_wait4.threading.Thread', side_effect=RuntimeError('no thread')), patch(
                'owned_wait4.subprocess.Popen') as spawn:
            with self.assertRaises(RuntimeError):
                self.child('import time; time.sleep(20)')
            spawn.assert_not_called()


if __name__ == '__main__':
    unittest.main()
