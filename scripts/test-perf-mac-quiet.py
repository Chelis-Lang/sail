#!/usr/bin/env python3
"""Check the Mac measurement environment refusal without starting workloads."""
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location('quiet', Path(__file__).with_name('perf-mac-quiet.py'))
quiet = importlib.util.module_from_spec(spec)
spec.loader.exec_module(quiet)


class Quiet(unittest.TestCase):
    def test_cpu_time_and_paths_with_spaces(self):
        self.assertEqual(quiet.cpu_seconds('230:43.02'), 13843.02)
        self.assertEqual(quiet.cpu_seconds('2-01:02:03.5'), 176523.5)
        text = '41 Fri Oct 30 01:00:00 2026 230:43.02 /System/Service With Spaces\n'
        self.assertEqual(quiet.parse(text)[41],
                         ('Fri Oct 30 01:00:00 2026', 13843.02, '/System/Service With Spaces'))

    def test_idle_inventory_passes_but_audio_and_config_load_refuses(self):
        before = {41: ('same', 100., 'configd'), 336: ('same', 200., 'audiomxd')}
        idle = {41: ('same', 100.01, 'configd'), 336: ('same', 200., 'audiomxd')}
        busy = {41: ('same', 100.43, 'configd'), 336: ('same', 200.73, 'audiomxd')}
        self.assertTrue(quiet.assess(before, idle, 1)['quiet'])
        report = quiet.assess(before, busy, 1)
        self.assertFalse(report['quiet'])
        self.assertAlmostEqual(report['total_cores'], 1.16)

    def test_new_and_reused_pids_do_not_inherit_old_cpu_counters(self):
        result = quiet.assess({5: ('old', 1000., 'old')},
                              {5: ('new', .2, 'replacement'), 6: ('new', .3, 'new')}, 1)
        self.assertAlmostEqual(result['total_cores'], .5)
        self.assertFalse(result['quiet'])

    def test_partial_duplicate_or_empty_inventories_refuse(self):
        line = '41 Fri Oct 30 01:00:00 2026 0:00.01 configd\n'
        for text in ['', '41 broken\n', line+line, line+'broken']:
            with self.subTest(text=text), self.assertRaises(ValueError):
                quiet.parse(text)

    def test_setup_wait_retains_busy_windows_and_requires_a_fresh_pass(self):
        now, records = [0.], []
        windows = iter([.2, .02])
        def probe():
            now[0] += 1
            return quiet.assess({1: ('same', 0., 'one')},
                                {1: ('same', next(windows), 'one')}, 1)
        def sleep(seconds):
            now[0] += seconds
        result = quiet.wait_for_quiet(10, probe=probe, record=records.append,
                                      clock=lambda: now[0], sleeper=sleep)
        self.assertTrue(result['quiet'])
        self.assertEqual(result['maximum_cores'], .10)
        self.assertEqual([r['quiet'] for r in result['observations']], [False, True])
        self.assertEqual(len(records), 2)

    def test_single_check_and_exhausted_setup_wait_still_refuse(self):
        for budget in [0, 2]:
            now, records = [0.], []
            def probe():
                now[0] += 1
                return quiet.assess({1: ('same', 0., 'one')},
                                    {1: ('same', .2, 'one')}, 1)
            def sleep(seconds):
                now[0] += seconds
            result = quiet.wait_for_quiet(budget, probe=probe, record=records.append,
                                          clock=lambda: now[0], sleeper=sleep)
            self.assertFalse(result['quiet'])
            self.assertEqual(result['maximum_cores'], .10)
            self.assertTrue(all(not r['quiet'] for r in result['observations']))
            self.assertLessEqual(now[0], max(1, budget))
        for budget in [-1, 121, float('nan'), float('inf')]:
            with self.subTest(budget=budget), self.assertRaises(ValueError):
                quiet.wait_for_quiet(budget)

    def test_unreadable_inventory_cannot_wait_into_a_passing_result(self):
        def probe():
            raise ValueError('incomplete inventory')
        with self.assertRaises(ValueError):
            quiet.wait_for_quiet(120, probe=probe)

    def test_bad_intervals_and_backwards_counters_refuse(self):
        for seconds in [0, -1, 10, float('nan')]:
            with self.subTest(seconds=seconds), self.assertRaises(ValueError):
                quiet.assess({}, {}, seconds)
        with self.assertRaises(ValueError):
            quiet.assess({1: ('same', 10, 'one')}, {1: ('same', 9, 'one')}, 1)


if __name__ == '__main__':
    unittest.main()
