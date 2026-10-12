#!/usr/bin/env python3
"""Host-only rebind controls; no Docker, compiler, solver or proof processes."""
import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('rebind', Path(__file__).with_name('local-product-rebind.py'))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class RebindTests(unittest.TestCase):
    def test_rebind_preserves_history_and_full_product_identity(self):
        old = {'producer_grant_sha256': 'a' * 64,
               'phases': [{'phase': 'release-build', 'exit_code': 0, 'elapsed_seconds': 107},
                          {'phase': 'owned-install', 'exit_code': 0}],
               'artifacts': {'sail': {'bytes': 7, 'sha256': 'c' * 64}},
               'installation': {'authority': 'installation-only'},
               'execution_container_birth': {'started_at': 'old'}, 'source_commit': 'd' * 40}
        snapshot = copy.deepcopy(old)
        new = M.rebound(old, 'a' * 64, {'started_at': 'new'}, 'b' * 64, 'e' * 64)
        self.assertEqual(old, snapshot)
        for key in ('phases', 'artifacts', 'installation', 'source_commit', 'producer_grant_sha256'):
            self.assertEqual(new[key], old[key])
        self.assertEqual(new['execution_revalidation']['historical_execution_grant_sha256'], 'e' * 64)
        self.assertFalse(new['execution_revalidation']['installation_performed'])
        self.assertFalse(new['execution_revalidation']['historical_pass_repeated'])

    def test_producer_mismatch_refuses(self):
        with self.assertRaises(ValueError):
            M.rebound({'producer_grant_sha256': 'a' * 64}, 'b' * 64, {}, 'c' * 64, 'd' * 64)

    def test_pointer_commit_archived_original_unchanged(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            pointer, archived = root / 'current.json', root / 'historical.json'
            old = b'{"version":1}\n'
            pointer.write_bytes(old)
            archived.write_bytes(old)
            M.replace_pointer(pointer, M.digest(old), b'{"version":2}\n')
            self.assertEqual(archived.read_bytes(), old)
            self.assertEqual(pointer.read_bytes(), b'{"version":2}\n')
            self.assertFalse(pointer.with_name(pointer.name + '.rebind-new').exists())

    def test_changed_pointer_refuses_and_never_overwrites(self):
        with tempfile.TemporaryDirectory() as raw:
            pointer = Path(raw) / 'current.json'
            pointer.write_bytes(b'changed')
            with self.assertRaises(ValueError):
                M.replace_pointer(pointer, M.digest(b'old'), b'new')
            self.assertEqual(pointer.read_bytes(), b'changed')

    def test_unknown_or_live_holder_refuses(self):
        healthy = {'complete': True, 'holders': [], 'unavailable': [], 'self_excluded': [{'pid': 7, 'start': 8}]}
        M.closed(healthy)
        for field, value in (('complete', False), ('holders', [{'pid': 8}]),
                             ('unavailable', [{'pid': 9}]), ('self_excluded', [])):
            with self.subTest(field=field), self.assertRaises(ValueError):
                M.closed(dict(healthy, **{field: value}))

    def test_birth_mount_and_byte_binding_changes_refuse(self):
        before = {'birth': 'old', 'mounts': [{'RW': True}], 'inode': 4, 'sha256': 'a' * 64}
        M.same(before, copy.deepcopy(before))
        for key, value in (('birth', 'new'), ('mounts', [{'RW': False}]), ('inode', 5), ('sha256', 'b' * 64)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                M.same(before, dict(before, **{key: value}))

    def test_metadata_size_link_and_duplicate_refuse(self):
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            file = root / 'record'
            file.write_bytes(b'12345')
            with self.assertRaises(ValueError):
                M.read(file, 4)
            link = root / 'link'
            link.symlink_to(file)
            with self.assertRaises(ValueError):
                M.read(link)
        with self.assertRaises(ValueError):
            json.loads('{"key":1,"key":2}', object_pairs_hook=M.unique)

    def test_sealed_support_and_original_source_verifier(self):
        repo = Path(__file__).resolve().parents[1]
        code = M.support(repo)
        self.assertEqual(set(code), set(M.SUPPORT))
        spec = importlib.util.spec_from_file_location('install', repo / 'docs/validation/m7-current-benchmark-install.py')
        installer = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(installer)
        with tempfile.TemporaryDirectory() as raw:
            root = Path(raw)
            file = root / 'file'
            file.write_bytes(b'original')
            file.chmod(0o644)
            expected = {'file': {'kind': 'file', 'bytes': 8, 'mode': 0o644, 'sha256': M.digest(b'original')}}
            self.assertEqual(installer.verify_source(root, expected)['entries'], 1)
            file.write_bytes(b'modified')
            with self.assertRaises(ValueError):
                installer.verify_source(root, expected)
            file.write_bytes(b'original')
            file.chmod(0o600)
            with self.assertRaises(ValueError):
                installer.verify_source(root, expected)
            file.chmod(0o644)
            (root / 'extra').write_text('extra')
            with self.assertRaises(ValueError):
                installer.verify_source(root, expected)


if __name__ == '__main__':
    unittest.main()
