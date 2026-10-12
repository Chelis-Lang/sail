#!/usr/bin/env python3
"""Pure sensitivity caller controls; no Cargo, process or native actor."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('bootstrap_sensitivity', Path(__file__).with_name('sensitivity-run.py'))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class Controls(unittest.TestCase):
    def text(self, negative=False, case=M.CASE, message=M.MESSAGE):
        state, passed, failed = ('FAILED', 0, 1) if negative else ('ok', 1, 0)
        return f'running 1 test\ntest {case} ... {state}\n{message if negative else ""}\ntest result: {state}. {passed} passed; {failed} failed; 0 ignored; 0 measured; 9 filtered out; finished in 1s\n'.encode()

    def test_positive_negative_exact(self):
        M.outcome(0, self.text(), False)
        M.outcome(101, self.text(True), True)

    def test_exit_and_case_cannot_be_substituted(self):
        for code, text, negative in [(1, self.text(True), True), (101, self.text(True, 'other'), True),
                                     (0, self.text(True), False), (101, self.text(), True)]:
            with self.assertRaises(ValueError):
                M.outcome(code, text, negative)

    def test_negative_requires_original_message_and_summary(self):
        for text in [self.text(True, message='other assertion'), self.text(True).replace(b'0 passed; 1 failed', b'1 passed; 1 failed')]:
            with self.assertRaises(ValueError):
                M.outcome(101, text, True)

    def artifact(self):
        return {'reason': 'compiler-artifact', 'profile': {'test': True}, 'target': {'name': 'sail_verus'}, 'executable': '/owned/test'}

    def test_one_explicit_test_executable(self):
        row = self.artifact()
        self.assertEqual(M.executable(b'Cargo progress\n' + json.dumps(row).encode()), '/owned/test')
        for rows in [[], [row, row], [dict(row, executable=None)], [dict(row, executable='relative')]]:
            with self.assertRaises(ValueError):
                M.executable(b'\n'.join(json.dumps(r).encode() for r in rows))

    def test_patch_is_only_exact_emission(self):
        original = b'fn fixture() {\n    super::publish_records(export, compiler, records);\n}\n'
        negative = original.replace(b'super::publish_records(export, compiler, records);', b'let _ = (export, compiler, records);')
        patch = ''.join(M.difflib.unified_diff(original.decode().splitlines(True), negative.decode().splitlines(True), fromfile='a/' + M.FILE, tofile='b/' + M.FILE)).encode()
        self.assertEqual(M.patched(original, patch), negative)
        for bad in [patch + b'extra', patch.replace(b'a/crates/', b'a/source/crates/')]:
            with self.assertRaises(ValueError):
                M.patched(original, bad)

    def test_uncertain_cleanup_forbids_patch_and_restore(self):
        class Runner:
            def require_idle(self):
                raise RuntimeError('sticky uncertainty')
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / M.FILE
            path.parent.mkdir(parents=True)
            path.write_bytes(b'original')
            state = {'inventory': {'files': {}}}
            for content in [b'negative', b'original']:
                with self.assertRaises(RuntimeError):
                    M.replace_source(Runner(), lambda: None, root, state, {}, content)
                self.assertEqual(path.read_bytes(), b'original')

    def test_closed_mutation_changes_only_known_inventory(self):
        class Runner:
            def require_idle(self):
                pass
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            path = root / M.FILE
            path.parent.mkdir(parents=True)
            path.write_bytes(b'original')
            original = {'files': {M.FILE: {'mode': 0o644, 'bytes': 8, 'sha256': 'old'}, 'other': {'sha256': 'same'}}}
            state = {'inventory': copy.deepcopy(original)}
            M.replace_source(Runner(), lambda: None, root, state, original, b'negative')
            self.assertEqual(state['inventory']['files']['other'], original['files']['other'])
            self.assertEqual(state['inventory']['files'][M.FILE]['mode'], 0o644)
            self.assertEqual(path.read_bytes(), b'negative')
            self.assertEqual(original['files'][M.FILE]['sha256'], 'old')

    def test_bounded_capture_refuses_growth(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'log'
            path.write_bytes(b'ab')
            before = M.MAX_LOG
            try:
                M.MAX_LOG = 1
                with self.assertRaises(ValueError):
                    M.bounded(path)
            finally:
                M.MAX_LOG = before

    def test_grant_all_bindings_and_holder_refusal(self):
        config = {'revision': 'a' * 40, 'source': '/new/source', 'target': '/owned/target', 'pid1_birth': 123,
                  'container': {'id': 'id', 'image': 'image', 'birth': 'birth'}, 'archive_sha256': 'archive',
                  'inventory_sha256': 'inventory', 'protected_sha256': 'protected', 'patch_sha256': 'patch',
                  'controller_sha256': 'controller'}
        grant = dict(config, status='GRANTED', scope='ordinary-script-bootstrap-sensitivity', holder_audit={'survivors': [], 'unavailable': []})
        M.prerequisite(config, grant, lambda _: 'controller')
        for key in config:
            bad = dict(grant, **{key: 'different'})
            with self.assertRaises(ValueError):
                M.prerequisite(config, bad, lambda _: 'controller')
        for audit in [None, {'survivors': [1], 'unavailable': []}, {'survivors': [], 'unavailable': [1]}]:
            with self.assertRaises(ValueError):
                M.prerequisite(config, dict(grant, holder_audit=audit), lambda _: 'controller')


if __name__ == '__main__':
    unittest.main()
