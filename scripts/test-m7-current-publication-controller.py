#!/usr/bin/env python3
"""Pure controller failure controls; no subprocess, Docker or proof actors."""

import argparse
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location(
    'controller', Path(__file__).with_name('m7-current-publication-controller.py'))
controller = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controller)
DIGEST = '0' * 64


class Controls(unittest.TestCase):
    def exercise(self, fault=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            grant = {'source_commit': '1' * 40, 'source_archive_sha256': DIGEST,
                     'container': 'sail-m7-handoff-validation', 'source_path': '/sealed/source',
                     'product_dir': '/sealed/product', 'engine_build': '/sealed/engine',
                     'execution': 'test-only', 'artifacts': {'sail': {'sha256': DIGEST}}}
            grant_path = root / 'product-grant.json'
            grant_path.write_text(json.dumps(grant))
            args = argparse.Namespace(out=str(root / 'output'), product_grant=str(grant_path),
                                      product_grant_sha256=DIGEST, runtime_manifest_sha256=DIGEST,
                                      verus_source='/sealed/verus', source_pin='2' * 40)
            commands = []
            identity = {'id': 'owned', 'started_at': 'birth', 'image': 'image'}

            def run(command, **kwargs):
                commands.append(command)
                destination = Path(kwargs['stdout'].name).name
                if fault == destination:
                    raise OSError('transport unavailable')
                if fault == 'timeout' and '--native-mode' in command and command[-1] == 'execute':
                    self.assertEqual(kwargs['timeout'], controller.EXTERNAL_SECONDS)
                    kwargs['stderr'].write(b'partial original failure\n')
                    raise subprocess.TimeoutExpired(command, kwargs['timeout'])
                if '--native-mode' in command:
                    mode = command[-1]
                    value = {'seals': {'exact': 'same'},
                             'closure': {'complete': True, 'candidates': []}}.get(mode)
                    if mode == 'execute':
                        if fault == 'execute-oserror':
                            raise OSError('Docker transport lost')
                        if fault == 'execute-interrupt':
                            raise KeyboardInterrupt('Controller interrupted')
                        if fault == 'docker-failure':
                            return subprocess.CompletedProcess(command, 125)
                        if fault == 'fixture-failure' or str(fault).startswith('partial-capture'):
                            return subprocess.CompletedProcess(command, 1)
                        value = None
                    if mode == 'seals' and fault == 'post-seal' and destination == 'after-seals.stdout':
                        value = {'exact': 'changed'}
                    if mode == 'closure' and fault == 'partial-capture-closure-loss':
                        raise OSError('closure transport lost')
                    if mode == 'closure' and fault == 'closure-incomplete':
                        value = {'complete': False, 'candidates': []}
                    if mode == 'closure' and fault == 'closure-holder':
                        value = {'complete': True, 'candidates': [{'pid': 42}]}
                    if value is not None:
                        kwargs['stdout'].write(json.dumps(value).encode())
                elif command[2:4] == ['sail-m7-handoff-validation', 'cat']:
                    value = {'kind': 'original-repair-publication-timing-diagnostic',
                             'original_fixture_passed': fault not in ('false-native-record', 'fixture-failure'),
                             'qualification_credit': False, 'original_limits': [120, 30, 5],
                             'fixture_sha256': controller.FIXTURE_SHA, 'product_sha256': DIGEST}
                    if str(fault).startswith('partial-capture'):
                        value.update(capture=[{'partial': True}], original_fixture_unchanged_after=True, product_unchanged_after=fault != 'partial-capture-unsealed')
                    if fault == 'native-record-mismatch':
                        value['product_sha256'] = 'f' * 64
                    if fault == 'native-record-malformed':
                        value = []
                    kwargs['stdout'].write(json.dumps(value).encode())
                elif 'tar' in command:
                    kwargs['stdout'].write(b'exact retained public archive')
                return subprocess.CompletedProcess(command, 0)

            sources = dict.fromkeys(controller.FILES, DIGEST)
            sources['ci/test-repair.py'] = controller.FIXTURE_SHA
            def digest(path):
                return controller.FIXTURE_SHA if str(path).endswith('ci/test-repair.py') else DIGEST

            with mock.patch.object(controller, 'sha', side_effect=digest), \
                    mock.patch.object(controller, 'archive_sources', return_value=sources), \
                    mock.patch.dict(controller.common, {'validate_grant': lambda *a: None,
                                                       'container_identity': lambda *a: identity}), \
                    mock.patch.object(controller.subprocess, 'run', side_effect=run):
                code = controller.host(args)
            record = json.loads((Path(args.out) / 'controller.json').read_text())
            errors = (Path(args.out) / 'original-diagnostic.stderr')
            partial = errors.read_bytes() if errors.exists() else None
            return code, record, commands, partial

    def test_success_is_diagnostic_only_and_exact_existing_repository_launcher(self):
        code, record, commands, _ = self.exercise()
        self.assertEqual(code, 0)
        self.assertEqual(record['status'], 'DIAGNOSTIC_PASS')
        self.assertFalse(record['qualification_credit'])
        self.assertFalse(record['native_closure_certified'])
        self.assertEqual(record['original_limits'], [120, 30, 5])
        self.assertEqual(record['before'], record['after-seals'])
        self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)
        self.assertTrue(record['raw_archive']['bytes'])

    def test_original_failure_retained_and_not_retried(self):
        code, record, commands, _ = self.exercise('fixture-failure')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'FAIL')
        self.assertEqual(record['primary_status'], 'FAIL')
        self.assertTrue(record['native_terminal_record_matches'])
        self.assertEqual(record['uncertainty_reasons'], [])
        self.assertIn('raw_archive', record)
        self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)
        self.assertEqual(next(p for p in record['phases'] if p['name'] == 'original-diagnostic')['exit_code'], 1)

    def test_external_timeout_never_claims_native_closure_even_after_empty_scan(self):
        code, record, commands, partial = self.exercise('timeout')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'UNCERTAIN')
        self.assertFalse(record['native_closure_certified'])
        self.assertTrue(record['bounded_closure_observed'])
        self.assertEqual(partial, b'partial original failure\n')
        self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)

    def test_post_seal_and_false_native_record_refuse_success(self):
        for fault in ('post-seal', 'false-native-record'):
            code, record, _, _ = self.exercise(fault)
            self.assertEqual(code, 1)
            self.assertEqual(record['status'], 'FAIL')
            self.assertNotEqual(record['status'], 'running')
            self.assertIn('raw_archive', record)

    def test_attempted_transport_loss_and_interruption_remain_uncertain(self):
        for fault, error in (('execute-oserror', 'OSError'), ('execute-interrupt', 'KeyboardInterrupt')):
            code, record, commands, _ = self.exercise(fault)
            self.assertEqual(code, 1)
            self.assertEqual(record['status'], 'UNCERTAIN')
            self.assertEqual(record['error_type'], error)
            self.assertTrue(record['bounded_closure_observed'])
            self.assertFalse(record['native_closure_certified'])
            self.assertIn('transport-or-interruption', record['uncertainty_reasons'])
            self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)

    def test_lost_malformed_or_unbound_terminal_record_is_uncertain(self):
        for fault in ('native-record.stdout', 'native-record-mismatch', 'native-record-malformed'):
            code, record, _, _ = self.exercise(fault)
            self.assertEqual(code, 1)
            self.assertEqual(record['status'], 'UNCERTAIN')
            self.assertEqual(record['primary_status'], 'DIAGNOSTIC_PASS')
            self.assertIn('native-terminal-unavailable', record['uncertainty_reasons'])
            self.assertFalse(record['native_closure_certified'])

    def test_unavailable_incomplete_or_nonempty_closure_is_uncertain(self):
        for fault in ('closure.stdout', 'closure-incomplete', 'closure-holder'):
            code, record, _, _ = self.exercise(fault)
            self.assertEqual(code, 1)
            self.assertEqual(record['status'], 'UNCERTAIN')
            self.assertIn('native-closure-unavailable', record['uncertainty_reasons'])
            self.assertFalse(record['native_closure_certified'])

    def test_docker_nonzero_without_confirmed_fixture_failure_is_uncertain(self):
        code, record, commands, _ = self.exercise('docker-failure')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'UNCERTAIN')
        self.assertEqual(record['primary_status'], 'FAIL')
        self.assertEqual(record['error_type'], 'ValueError')
        self.assertIn('docker-exit-unconfirmed', record['uncertainty_reasons'])
        self.assertEqual(next(p for p in record['phases'] if p['name'] == 'original-diagnostic')['exit_code'], 125)
        self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)

    def test_passed_fixture_partial_capture_is_expected_diagnostic_failure(self):
        code, record, commands, _ = self.exercise('partial-capture')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'FAIL')
        self.assertEqual(record['diagnostic_outcome'], 'passed-fixture-partial-capture')
        self.assertTrue(record['diagnostic_capture_partial'])
        self.assertEqual(record['uncertainty_reasons'], [])
        self.assertFalse(record['qualification_credit'])
        self.assertEqual(sum('--native-mode' in c and c[-1] == 'execute' for c in commands), 1)

    def test_partial_capture_without_unchanged_seals_does_not_confirm_exit(self):
        code, record, _, _ = self.exercise('partial-capture-unsealed')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'UNCERTAIN')
        self.assertIn('docker-exit-unconfirmed', record['uncertainty_reasons'])

    def test_bound_partial_capture_never_clears_transport_uncertainty(self):
        code, record, _, _ = self.exercise('partial-capture-closure-loss')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'UNCERTAIN')
        self.assertTrue(record['diagnostic_capture_partial'])
        self.assertIn('transport-or-interruption', record['uncertainty_reasons'])
        self.assertIn('native-closure-unavailable', record['uncertainty_reasons'])
        self.assertFalse(record['native_closure_certified'])

    def test_partial_terminal_is_exact_and_bounded(self):
        grant = {'artifacts': {'sail': {'sha256': DIGEST}}}
        value = {'kind': 'original-repair-publication-timing-diagnostic',
                 'original_fixture_passed': True, 'qualification_credit': False,
                 'original_limits': [120, 30, 5], 'fixture_sha256': controller.FIXTURE_SHA,
                 'product_sha256': DIGEST, 'original_fixture_unchanged_after': True,
                 'product_unchanged_after': True, 'capture': [{'partial': True}]}
        self.assertTrue(controller.partial_capture_terminal(value, grant))
        for captures in ([], [{'partial': False}], [{'partial': 1}], [None], [{'partial': True}] * 2):
            self.assertFalse(controller.partial_capture_terminal(dict(value, capture=captures), grant))
        self.assertFalse(controller.partial_capture_terminal(dict(value, product_sha256='f' * 64), grant))

    def test_copy_failure_is_terminal_without_native_invocation(self):
        code, record, commands, _ = self.exercise('copy-binding.stdout')
        self.assertEqual(code, 1)
        self.assertEqual(record['status'], 'FAIL')
        self.assertFalse(record['diagnostic_attempted'])
        self.assertFalse(any('--native-mode' in c for c in commands))

    def test_repository_schema_is_23_fields_including_output_guarded(self):
        diagnostic = controller.runpy.run_path(str(controller.REPO / 'scripts/m7-publication-timing-diagnostic.py'))
        self.assertEqual(len(diagnostic['ROW_KEYS']), 23)
        self.assertIn('output_guarded', diagnostic['ROW_KEYS'])
        self.assertEqual(diagnostic['FIXTURE_SHA'], controller.FIXTURE_SHA)

    def test_native_execution_preserves_original_fixture_and_supplies_no_budget_override(self):
        with tempfile.TemporaryDirectory() as temporary:
            binding = {'grant': {'source_path': '/sealed/source', 'product_dir': '/sealed/product',
                                 'artifacts': {'sail': {'sha256': DIGEST}},
                                 'engine_build': '/sealed/engine', 'execution': 'actual load'},
                       'verus_source': '/sealed/verus'}
            with mock.patch.object(controller.os, 'getpriority', return_value=0), \
                    mock.patch.object(controller.os, 'getsid', return_value=99), \
                    mock.patch.object(controller.os, 'getpid', return_value=99), \
                    mock.patch.dict(controller.common, {'process_identity': lambda p: {'pid': p}}), \
                    mock.patch.object(controller.os, 'execv') as execute:
                controller.native_execute(binding, temporary)
            command = execute.call_args.args[1]
            self.assertEqual(command[1], '/sealed/source/scripts/m7-publication-timing-diagnostic.py')
            self.assertEqual(command[command.index('--fixture') + 1], '/sealed/source/ci/test-repair.py')
            self.assertNotIn('--timeout', command)
            self.assertEqual(json.loads((Path(temporary) / 'argv.json').read_text()), command)


if __name__ == '__main__':
    unittest.main()
