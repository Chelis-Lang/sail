#!/usr/bin/env python3
"""Metadata-only observer controls; never launch a proof or conformance actor."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('observer', Path(__file__).with_name('m7-conformance-observation.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class Controls(unittest.TestCase):
    def test_stat_comm_delimiters_and_cpu_fields_are_not_misparsed(self):
        fields = ['S', '10', '20'] + ['0'] * 19
        fields[11], fields[12], fields[19], fields[21] = '31', '32', '33', '34'
        row = module.stat(b'99 (name with ) nested () text) ' + ' '.join(fields).encode())
        self.assertEqual(row, {'state': 'S', 'ppid': 10, 'pgid': 20, 'utime_ticks': 31,
                              'stime_ticks': 32, 'starttime_ticks': 33, 'rss_pages': 34})
        for malformed in (b'99 name', b'99 (name) S 1 2', b'99 (name) ' + b'bad ' * 22):
            with self.assertRaises((ValueError, IndexError)):
                module.stat(malformed)

    def test_nested_unit_groups_are_descendants_but_unrelated_group_is_not(self):
        rows = {10: {'ppid': 1, 'starttime_ticks': 100},
                11: {'ppid': 10, 'starttime_ticks': 101},
                12: {'ppid': 11, 'starttime_ticks': 102},
                40: {'ppid': 1, 'starttime_ticks': 103},
                41: {'ppid': 40, 'starttime_ticks': 104},
                50: {'ppid': 51, 'starttime_ticks': 105},
                51: {'ppid': 50, 'starttime_ticks': 106}}
        self.assertEqual(module.descendants(rows, 10, 100), [10, 11, 12])
        self.assertEqual(module.descendants(rows, 10, 999), [])
        self.assertEqual(module.descendants(rows, 999, 100), [])
        rows[12]['ppid'] = 1
        self.assertEqual(module.descendants(rows, 10, 100), [10, 11])

    def test_output_refuses_oversize_once_without_publishing_partial_row(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'records'
            output = module.Output(path)
            self.assertFalse(output.write({'event': 'sample', 'text': 'x' * module.MAX_RECORD_BYTES}))
            self.assertFalse(output.write({'event': 'sample', 'text': 'later'}))
            output.close()
            raw = path.read_bytes()
            self.assertEqual(raw.count(b'\n'), 1)
            self.assertIn(b'observation-truncated', raw)
            self.assertNotIn(b'later', raw)
            with self.assertRaises(FileExistsError):
                module.Output(path)

    def test_aggregate_cap_reserves_complete_unavailable_record(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'records'
            output = module.Output(path)
            output.bytes = module.MAX_OUTPUT_BYTES - 512
            self.assertFalse(output.write({'event': 'sample', 'value': 1}))
            self.assertLessEqual(output.bytes, module.MAX_OUTPUT_BYTES)
            output.close()
            self.assertTrue(path.read_bytes().endswith(b'\n'))


if __name__ == '__main__':
    unittest.main()
