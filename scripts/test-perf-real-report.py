#!/usr/bin/env python3
"""Keep legacy reports readable and memory/refused samples visible."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('perf_report',
                                             Path(__file__).with_name('perf-real-report.py'))
report = importlib.util.module_from_spec(spec)
spec.loader.exec_module(report)


class Reports(unittest.TestCase):
    def row(self, mode='plain', memory=None, code=0):
        row = ['1', mode, 'cold', '1.5', str(code)] + ['0'] * 7 + ['NA'] * 6 + ['0/0']
        return '\t'.join(row + (memory or [])) + '\n'

    def render(self, text):
        with tempfile.TemporaryDirectory() as tmp:
            (Path(tmp) / 'results.tsv').write_text(text)
            rows = report.load(tmp)
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                report.report(tmp)
        return rows, output.getvalue()

    def test_legacy_memory_is_unavailable(self):
        rows, output = self.render(self.row())
        self.assertTrue(all(rows[0][key] is None for key in report.MEMORY_COLS))
        self.assertIn('NA | NA | NA', output)

    def test_new_memory_columns_and_ratio(self):
        rows, output = self.render(self.row(memory=['2048', '1024', '0']) +
                                   self.row('sail', ['1024', '512', '256']))
        self.assertEqual(rows[1]['tree_peak_kb'], 1024)
        self.assertIn('2.00x', output)
        self.assertIn('0.25 (0.25-0.25)', output)
        self.assertIn('50 ms', output)

    def test_refused_sample_is_retained(self):
        rows, output = self.render(self.row(memory=['1', '1', '0']) +
                                   self.row('sail', ['NA', '1', 'NA'], code=2))
        self.assertEqual(len(rows), 2)
        self.assertIn('EXIT CODES DIFFER', output)

    def test_malformed_width_is_an_error_not_an_empty_table(self):
        with self.assertRaisesRegex(ValueError, 'expected 19 or 22 columns'):
            self.render('1\tplain\n')

    def test_mixed_rows_keep_missing_memory_visible(self):
        _, output = self.render(self.row() + self.row(memory=['1024', '512', '0']))
        self.assertIn('NA | NA | NA', output)


if __name__ == '__main__':
    unittest.main()
