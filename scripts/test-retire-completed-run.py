#!/usr/bin/env python3
"""Pure temporary-fixture retirement controls; no Docker or child processes."""
import copy
import importlib.util
import io
import os
from pathlib import Path
import tarfile
import tempfile
import types
import unittest
from unittest.mock import patch
from unittest.mock import Mock

spec = importlib.util.spec_from_file_location('retirement', Path(__file__).with_name('retire-completed-run.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class Controls(unittest.TestCase):
    def fixture(self, base):
        root = base / 'scratch'
        root.mkdir()
        (root / 'result.json').write_bytes(b'{"status":"FAIL"}\n')
        (root / 'stdout').write_bytes(b'full unnormalized stdout\n\n')
        (root / 'target').mkdir()
        (root / 'target' / 'compiler.rlib').write_bytes(b'compiler bytes')
        (root / 'target' / 'compiler.log').write_bytes(b'compiler log excluded')
        return root

    def test_explicit_scope_and_protected_overlap(self):
        self.assertEqual(m.validate_root('/tmp/sail-example', ['/opt/runtime', '/work/target']), Path('/tmp/sail-example'))
        for root in ('/', '/tmp', '/tmp/sail-', '/tmp/sail-example/child', '/tmp/sail-run-retirement-x', '/tmp/sail-example/../sail-other'):
            with self.assertRaises(ValueError):
                m.validate_root(root, ['/opt/runtime'])
        for protected in ('/tmp', '/tmp/sail-example', '/tmp/sail-example/product', 'relative'):
            with self.assertRaises(ValueError):
                m.validate_root('/tmp/sail-example', [protected])

    def test_terminal_failure_preserved_and_uncertainty_refuses(self):
        record = {'status': 'FAIL', 'closure': {'complete': True, 'candidates': [], 'unreadable': []}}
        m.terminal_closed(record)
        self.assertEqual(record['status'], 'FAIL')
        for changes in ({'status': 'RUNNING'}, {'uncertainty': ['transport lost']}, {'uncertain': True},
                        {'closure_complete': False}, {'closure': {'complete': False, 'candidates': []}},
                        {'closure': {'complete': True, 'candidates': [{'pid': 7}]}},
                        {'closure': {'complete': True, 'candidates': [], 'unreadable': [7]}}, {'closure': None}):
            with self.assertRaises(ValueError):
                m.terminal_closed(dict(record, **changes))

    def test_known_group_closure_requires_every_phase(self):
        record = {'status': 'PASS', 'closure': 'original source/protected bytes and PID1 equal; known phase groups closed, not an arbitrary orphan-absence proof',
                  'phases': [{'group_closure': {'survivors': [], 'unavailable': []}}]}
        m.terminal_closed(record)
        record['phases'].append({'group_closure': {'survivors': [{'pid': 8}], 'unavailable': []}})
        with self.assertRaises(ValueError):
            m.terminal_closed(record)

    def test_full_text_verified_without_build_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            plan = m.inventory(root)
            m.validate_plan(root, plan)
            archive = base / 'evidence.tar.gz'
            m.archive(root, plan, archive)
            m.verify_archive(archive, plan)
            with tarfile.open(archive) as stream:
                self.assertEqual(set(stream.getnames()), {'result.json', 'stdout'})
                self.assertEqual(stream.extractfile('stdout').read(), b'full unnormalized stdout\n\n')
            self.assertEqual(archive.stat().st_mode & 0o777, 0o600)

    def test_archive_missing_changed_bytes_and_modes_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            plan = m.inventory(root)
            for label, names, altered in [('missing', ['result.json'], None),
                                          ('bytes', ['result.json', 'stdout'], 'bytes'),
                                          ('mode', ['result.json', 'stdout'], 'mode')]:
                archive = base / (label + '.tar.gz')
                with tarfile.open(archive, 'w:gz') as output:
                    for name in names:
                        row = plan['entries'][name]
                        raw = (root / name).read_bytes()
                        if name == 'stdout' and altered == 'bytes':
                            raw = b'X' + raw[1:]
                        item = tarfile.TarInfo(name)
                        item.size, item.uid, item.gid = len(raw), row['uid'], row['gid']
                        item.mode = (row['mode'] & 0o777) ^ (1 if altered == 'mode' else 0)
                        output.addfile(item, io.BytesIO(raw))
                with self.assertRaises(ValueError):
                    m.verify_archive(archive, plan)

    def test_inventory_expansion_and_accounting_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.fixture(Path(temporary).resolve())
            original = m.inventory(root)
            for kind in ('root', 'escape', 'accounting', 'classification'):
                plan = copy.deepcopy(original)
                if kind == 'root':
                    plan['root'] += '-other'
                elif kind == 'escape':
                    plan['entries']['../outside'] = plan['entries'].pop('stdout')
                elif kind == 'accounting':
                    plan['evidence_bytes'] += 1
                else:
                    plan['entries']['target/compiler.log']['retained'] = True
                with self.assertRaises(ValueError):
                    m.validate_plan(root, plan)

    def test_real_special_file_refuses_before_deletion(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.fixture(Path(temporary).resolve())
            os.mkfifo(root / 'pipe')
            with self.assertRaises(ValueError):
                m.inventory(root)
            self.assertTrue((root / 'stdout').exists())

    def test_limits_leave_scratch_intact(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.fixture(Path(temporary).resolve())
            for key in ('MAX_ENTRIES', 'MAX_EVIDENCE_BYTES', 'MAX_SCRATCH_BYTES'):
                with patch.object(m, key, 1), self.assertRaises(ValueError):
                    m.inventory(root)
            self.assertTrue((root / 'result.json').exists())

    def test_sibling_creation_and_removal_do_not_change_inventory(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            before = m.inventory(root)
            parent_before = m.identity(base.stat())
            sibling = base / 'unrelated-sibling'
            sibling.mkdir()
            (sibling / 'temporary.txt').write_bytes(b'unrelated work')
            self.assertNotEqual(m.identity(base.stat()), parent_before)
            self.assertEqual(m.inventory(root), before)
            (sibling / 'temporary.txt').unlink()
            sibling.rmdir()
            self.assertEqual(m.inventory(root), before)
            self.assertEqual(set(before['parent_identity']), {'device', 'inode', 'mode', 'uid', 'gid'})
            m.validate_plan(root, before)

    def test_replaced_parent_refuses_before_removal(self):
        import shutil
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            parent = base / 'parent'
            parent.mkdir()
            root = self.fixture(parent)
            before = m.inventory(root)
            parent.rename(base / 'original-parent')
            shutil.copytree(base / 'original-parent', parent)
            with self.assertRaisesRegex(ValueError, 'scratch parent binding changed'):
                m.delete_scratch(root, before, lambda *_: None)
            self.assertTrue((root / 'stdout').exists())
            self.assertTrue((base / 'original-parent' / 'scratch' / 'stdout').exists())

    def test_replaced_root_refuses_before_removal(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            before = m.inventory(root)
            root.rename(base / 'original-root')
            self.fixture(base)
            with self.assertRaisesRegex(ValueError, 'ancestor binding changed'):
                m.delete_scratch(root, before, lambda *_: None)
            self.assertTrue((root / 'stdout').exists())
            self.assertTrue((base / 'original-root' / 'stdout').exists())

    def test_symlink_and_hardlinks_do_not_delete_outside_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            outside = base / 'outside'
            outside.write_bytes(b'outside retained bytes')
            os.link(outside, root / 'linked.txt')
            os.link(outside, root / 'linked-again.txt')
            (root / 'external').symlink_to(outside)
            before = outside.stat()
            plan = m.inventory(root)
            archive = base / 'evidence.tar.gz'
            m.archive(root, plan, archive)
            m.verify_archive(archive, plan)
            marks = []
            removed = m.delete_scratch(root, plan, lambda done, pending: marks.append((list(done), pending)))
            self.assertEqual(set(removed), set(plan['entries']))
            self.assertFalse(root.exists())
            self.assertEqual(outside.read_bytes(), b'outside retained bytes')
            self.assertEqual(outside.stat().st_ino, before.st_ino)
            self.assertEqual(outside.stat().st_nlink, 1)
            self.assertIsNone(marks[-1][1])

    def test_replaced_member_and_directory_refuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            for mode in ('file', 'directory'):
                root = self.fixture(base)
                plan = m.inventory(root)
                path = root / 'stdout' if mode == 'file' else root / 'target'
                path.rename(base / ('original-' + mode))
                if mode == 'file':
                    path.write_bytes(b'full unnormalized stdout\n\n')
                else:
                    path.mkdir()
                with self.assertRaises(ValueError):
                    m.delete_scratch(root, plan, lambda *_: None)
                # Each fixture has an independent explicit root.
                import shutil
                shutil.rmtree(root)

    def test_new_member_prevents_parent_removal_without_touching_new_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = self.fixture(Path(temporary).resolve())
            plan = m.inventory(root)
            (root / 'new.txt').write_bytes(b'not approved')
            with self.assertRaises(OSError):
                m.delete_scratch(root, plan, lambda *_: None)
            self.assertEqual((root / 'new.txt').read_bytes(), b'not approved')

    def test_redirected_ancestor_refuses_without_deleting_outside(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary).resolve()
            root = self.fixture(base)
            (root / 'target' / 'nested').mkdir()
            (root / 'target' / 'nested' / 'artifact').write_bytes(b'exact original bytes')
            plan = m.inventory(root)
            (root / 'target').rename(base / 'moved-outside')
            (root / 'target').symlink_to(base / 'moved-outside', target_is_directory=True)
            with self.assertRaises(OSError):
                m.delete_scratch(root, plan, lambda *_: None)
            self.assertEqual((base / 'moved-outside' / 'nested' / 'artifact').read_bytes(), b'exact original bytes')

    def test_host_output_must_outlive_tmp_and_mount_aliases_refuse(self):
        with self.assertRaises(ValueError):
            m.durable_output('/tmp/sail-evidence')
        root = Path('/tmp/sail-example')
        m.validate_mounts(b'1 0 1:1 / / rw - overlay overlay rw\n', root)
        for point in (b'/tmp', b'/tmp/sail-example', b'/tmp/sail-example/volume'):
            with self.assertRaises(ValueError):
                m.validate_mounts(b'2 1 1:2 / ' + point + b' rw - ext4 disk rw\n', root)

    def test_unknown_terminal_never_reaches_docker(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary).resolve() / 'evidence'
            fake_holders = types.SimpleNamespace(inspect_all=unittest.mock.Mock())
            with patch.object(m, 'helpers', return_value=(None, fake_holders)), patch.object(m, 'durable_output', return_value=output):
                result = m.retire_run('sail-m7-acceptance', '/tmp/sail-example', output,
                                      {'status': 'FAIL', 'uncertainty': ['lost connection']}, ['/opt/product'], ['result.json'])
            self.assertEqual(result['status'], 'UNCERTAIN')
            self.assertEqual(result['original_status'], 'FAIL')
            self.assertFalse(result['removed'])
            fake_holders.inspect_all.assert_not_called()

    def test_gate_adapter_refuses_unfrozen_helper_before_native_action(self):
        g, _ = m.helpers(Path(__file__).parent)
        with tempfile.TemporaryDirectory() as temporary:
            args = types.SimpleNamespace(output=Path(temporary), native_root='/tmp/sail-gate', target='/work/target')
            record = g.retire_closed_gate(args, {'status': 'FAIL'}, {'files': {}})
            self.assertEqual(record['status'], 'UNCERTAIN')
            self.assertFalse(record['removed'])
            self.assertEqual(record['original_status'], 'FAIL')

    def test_gate_adapter_passes_exact_logs_and_leaves_external_source_outside_scope(self):
        g, _ = m.helpers(Path(__file__).parent)
        with tempfile.TemporaryDirectory() as temporary:
            args = types.SimpleNamespace(output=Path(temporary), native_root='/tmp/sail-gate',
                                         target='/work/target', protected=Path(temporary)/'protected.json',
                                         source='/tmp/sail-product-source', retirement_output=None,
                                         container='sail-m7-acceptance')
            fake = types.SimpleNamespace(terminal_closed=Mock(), retire_run=Mock(return_value={'status': 'PASS'}))
            frozen = {'files': {'scripts/'+name: {'sha256': 'exact'} for name in
                               ('retire-completed-run.py', 'local-combined-gates.py', 'retire-old-functional-two-incremental.py')}}
            terminal = {'status': 'FAIL', 'phases': [{'name': 'gate'}]}
            with patch.object(g, 'sha', return_value='exact'), patch.object(g, 'load', return_value={'files': [{'path': '/opt/product'}]}), \
                 patch.object(g.importlib.util, 'spec_from_file_location', return_value=types.SimpleNamespace(loader=Mock())), \
                 patch.object(g.importlib.util, 'module_from_spec', return_value=fake):
                self.assertEqual(g.retire_closed_gate(args, terminal, frozen), {'status': 'PASS'})
            passed = fake.retire_run.call_args.args
            self.assertEqual(passed[1], '/tmp/sail-gate')
            self.assertNotEqual(passed[1], args.source)
            self.assertEqual(passed[3]['status'], 'FAIL')
            self.assertEqual(passed[4], ['/work/target', '/opt/product'])
            self.assertIn('gate.log', passed[5])
            self.assertEqual(passed[6][-1], args.output/'result.json')

    def test_late_birth_change_cannot_leave_native_pass(self):
        records = []
        config = {'root': '/tmp/sail-example', 'container': 'sail-m7-acceptance',
                  'protected': ['/opt/product'], 'required': ['result.json'], 'script_sha256': 'exact',
                  'support_sha256': {}, 'pid1_starts': {'sail-m7-acceptance': 1}}
        h = types.SimpleNamespace(row=Mock(side_effect=[{'start': 1}, {'start': 2}]))
        private = types.SimpleNamespace(st_mode=0o40700, st_uid=os.getuid())
        args = types.SimpleNamespace(native='/tmp/sail-run-retirement-test', mode='capture', container='sail-m7-acceptance')
        plan = {'entries': {'result.json': {'retained': True}}}
        with patch.object(Path, 'lstat', return_value=private), patch.object(m, 'helpers', return_value=(None, h)), \
             patch.object(m, 'load', return_value=config), patch.object(m, 'sha', return_value='exact'), \
             patch.object(m, 'inventory', return_value=plan), patch.object(m, 'validate_plan'), \
             patch.object(m, 'audit', return_value={'complete': True}), patch.object(m, 'archive'), \
             patch.object(m, 'verify_archive'), patch.object(m, 'save', side_effect=lambda path, value: records.append((str(path), dict(value)))):
            self.assertEqual(m.native(args), 1)
        self.assertEqual(records[-1][1]['status'], 'UNCERTAIN')
        self.assertIn('PID1 birth changed at closure', records[-1][1]['error'])


if __name__ == '__main__':
    unittest.main()
