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

    def test_bad_intervals_and_backwards_counters_refuse(self):
        for seconds in [0, -1, 10, float('nan')]:
            with self.subTest(seconds=seconds), self.assertRaises(ValueError):
                quiet.assess({}, {}, seconds)
        with self.assertRaises(ValueError):
            quiet.assess({1: ('same', 10, 'one')}, {1: ('same', 9, 'one')}, 1)


if __name__ == '__main__':
    unittest.main()
