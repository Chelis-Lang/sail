#!/usr/bin/env python3
"""Strict and ADR 0097 controls, including actual capture/CLI failure paths."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from verus_resource_statistics import MAX_BYTES, classify_stdout

ROOT = Path(__file__).resolve().parent
BASE = '{"verdict":"verified","diagnostic":"same","times-ms":{"smt":{"rlimit-run":12,"smt-run-module-times":[{"rlimit":12,"function-breakdown":[{"rlimit":12,"name":"f"}]}]}}}'
CHANGED = BASE.replace('12', '13')
FAKE = '''import json,os,subprocess,sys
changed=not os.environ['SAIL_DETERMINISM_DIR'].endswith('run-0')
kind=sys.argv[1]
limit=41 if changed and kind=='query' else 40
query=f'(set-option :rlimit {limit})\\n(check-sat)\\n'.encode()
for _ in range(2 if changed and kind=='multiplicity' else 1):
 subprocess.run([os.environ['VERUS_Z3_PATH']],input=query,stdout=subprocess.PIPE,check=True)
value=13 if changed and kind!='same' else 12
print(json.dumps({'verdict':'error' if changed and kind=='verdict' else 'verified',
 'times-ms':{'smt':{'rlimit-run':value,'smt-run-module-times':[{'rlimit':value,'function-breakdown':[{'rlimit':value,'name':'f'}]}]}}}))
print('changed' if changed and kind=='stderr' else 'stable',file=sys.stderr)
raise SystemExit(1 if changed and kind=='exit' else 0)
'''


class ResourceStatistics(unittest.TestCase):
    def test_only_three_known_consumption_fields_are_classified(self):
        self.assertEqual(classify_stdout(BASE, BASE)['classification'], 'strict-same')
        result = classify_stdout(BASE, CHANGED)
        self.assertEqual(result['classification'], 'consumed-resource-only')
        self.assertEqual(len(result['differences']), 3)
        self.assertTrue(all(x['reference'] == 12 and x['candidate'] == 13
                            for x in result['differences']))

    def test_other_bytes_shapes_types_and_bounds_still_fail(self):
        controls = [
            CHANGED.replace('verified', 'error'),
            CHANGED.replace('same', 'different'),
            CHANGED.replace('"name":"f"', '"name":"g"'),
            CHANGED.replace('"verdict":"verified","diagnostic":"same"',
                            '"diagnostic":"same","verdict":"verified"'),
            CHANGED.replace(':13', ': 13', 1),
            CHANGED.replace('rlimit-run', 'rlimit-\\u0072un'),
            CHANGED.replace('"name":"f"', '"name":"f","rlimit":3'),
            CHANGED.replace('"rlimit-run":13', '"rlimit-run":13,"rlimit-run":13'),
            CHANGED.replace('"rlimit-run":13', '"rlimit-run":'),
            CHANGED[:-1], CHANGED + '{}', CHANGED.replace('"smt":{', '"smt":['),
            ' ' * (MAX_BYTES + 1), '[' + ','.join(['0'] * 100001) + ']',
            '[' * 66 + '0' + ']' * 66, '"' + ('\\"' * 100000),
        ]
        controls += [CHANGED.replace(':13', ':' + value, 1) for value in
                     ['-1', '1.0', '1e2', '18446744073709551616', 'true', 'null', '"13"', '{}', '[]']]
        for text in controls:
            with self.subTest(prefix=text[:80]):
                self.assertEqual(classify_stdout(BASE, text)['classification'], 'strict-different')

    def invoke(self, root, kind, classify):
        script = root / 'compiler.py'
        script.write_text(FAKE)
        output = root / ('capture-' + kind + ('-classified' if classify else '-strict'))
        command = [sys.executable, str(ROOT / 'verus-determinism.py'), '--runs', '2', '--out', str(output)]
        if classify:
            command.append('--classify-consumed-resources')
        command += ['--', sys.executable, str(script), kind]
        env = dict(os.environ, SAIL_REAL_Z3='/bin/cat')
        result = subprocess.run(command, env=env, capture_output=True, timeout=20)
        return result, output

    def test_cli_default_is_strict_and_explicit_mode_keeps_original_counters(self):
        with tempfile.TemporaryDirectory(prefix='sail-statistics-') as directory:
            root = Path(directory)
            strict, _ = self.invoke(root, 'counter', False)
            self.assertEqual(strict.returncode, 1, strict.stderr)
            accepted, output = self.invoke(root, 'counter', True)
            self.assertEqual(accepted.returncode, 0, accepted.stderr)
            report = json.loads((output / 'consumed-resource-classifications.json').read_text())
            self.assertEqual(report['strict_same'], 0)
            self.assertEqual(report['failed'], 0)
            self.assertEqual(len(report['consumed_resource_only']), 1)
            self.assertIn(b'"rlimit-run": 12', (output / 'input-0/run-0.stdout').read_bytes())
            self.assertIn(b'"rlimit-run": 13', (output / 'input-0/run-1.stdout').read_bytes())
            same, output = self.invoke(root, 'same', True)
            self.assertEqual(same.returncode, 0, same.stderr)
            report = json.loads((output / 'consumed-resource-classifications.json').read_text())
            self.assertEqual(report['strict_same'], 1)
            self.assertEqual(report['consumed_resource_only'], [])

    def test_explicit_cli_mode_rejects_query_multiplicity_verdict_stderr_and_exit_changes(self):
        with tempfile.TemporaryDirectory(prefix='sail-statistics-') as directory:
            for kind in ('query', 'multiplicity', 'verdict', 'stderr', 'exit'):
                with self.subTest(kind=kind):
                    result, output = self.invoke(Path(directory), kind, True)
                    self.assertEqual(result.returncode, 1, result.stderr)
                    report = json.loads((output / 'consumed-resource-classifications.json').read_text())
                    self.assertEqual(report['failed'], 1)
                    self.assertEqual(report['consumed_resource_only'], [])


if __name__ == '__main__':
    unittest.main()
