#!/usr/bin/env python3
"""Host-only exact-scope policy controls; no native or cleanup actor."""
import ast
import copy
import importlib.util
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest

HERE = Path(__file__).parent
SPEC = importlib.util.spec_from_file_location('deps_inventory', HERE / 'inventory-old-benchmark-single-deps.py')
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)

class Controls(unittest.TestCase):
    def info(self, links=1, mode=0o600):
        return types.SimpleNamespace(st_nlink=links, st_mode=stat.S_IFREG | mode)

    def test_one_exact_fixed_sibling_and_original_bounds(self):
        spec = importlib.util.spec_from_file_location('old_incremental', HERE / 'retire-old-benchmark-single-incremental.py')
        old = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(old)
        self.assertEqual(M.SCOPE, old.SCOPE.replace('/debug/incremental', '/debug/deps'))
        self.assertEqual(M.ROOTS, (M.SCOPE,))
        self.assertEqual((M.MAX_FILES, M.MAX_BYTES), (20000, 8 * 1024**3))
        self.assertEqual(M.SUFFIXES, ('.o', '.rmeta', '.rlib'))

    def test_direct_suffixes_only(self):
        for suffix in M.SUFFIXES:
            self.assertTrue(M.admitted_file(Path('libordinary' + suffix), self.info(), b'plain'))
        for name in ('nested/libordinary.rlib', '../other.rlib', '/work/target/other.rlib',
                     'receipt.json', 'proof.vir', 'libordinary.d', 'lock', 'unknown.bin'):
            self.assertFalse(M.admitted_file(Path(name), self.info(), b'plain'), name)

    def test_single_link_nonexec_nonelf_required(self):
        for info, magic in ((self.info(links=2), b'plain'), (self.info(mode=0o700), b'plain'),
                            (self.info(), bytes.fromhex('7f454c46'))):
            self.assertFalse(M.admitted_file(Path('libordinary.rlib'), info, magic))

    def test_no_mutating_native_mode_or_approval_flag(self):
        source = (HERE / 'inventory-old-benchmark-single-deps.py').read_text()
        tree = ast.parse(source)
        self.assertNotIn('approved_inventory', source)
        self.assertIn("choices=('inventory', 'holders')", source)
        for node in ast.walk(tree):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
                self.assertNotIn(node.func.attr, ('unlink', 'rmdir', 'remove', 'rmtree'))

    def test_inventory_hashes_all_protected_files_but_only_admits_direct_scratch(self):
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary).resolve()
            root = parent / 'deps'
            (root / 'nested').mkdir(parents=True)
            for name, value in (('libordinary.rlib', b'archive'), ('libordinary.rmeta', b'metadata'),
                                ('object.o', b'bitcode'), ('object-elf.o', bytes.fromhex('7f454c46')),
                                ('proof.vir', b'proof'), ('ledger.json', b'ledger')):
                (root / name).write_bytes(value)
            (root / 'nested' / 'other.rlib').write_bytes(b'protected nested')
            (root / 'executable.rlib').write_bytes(b'executable')
            (root / 'executable.rlib').chmod(0o700)
            outside = parent / 'outside.rlib'
            outside.write_bytes(b'linked')
            os.link(outside, root / 'linked.rlib')
            inventory = M.inventory((str(root),))
            self.assertEqual({Path(name).name for name in inventory['candidates']},
                             {'libordinary.rlib', 'libordinary.rmeta', 'object.o'})
            self.assertEqual(inventory['protected_hardlinked_files'], 1)
            self.assertIn(str(root / 'nested' / 'other.rlib'), inventory['entries'])
            self.assertEqual(inventory['entries'][str(root / 'linked.rlib')]['links'], 2)
            M.compare_preserved(inventory, M.inventory((str(root),)), [])

    def test_same_audited_holder_birth_mount_and_file_guards(self):
        def functions(name):
            return {n.name: ast.dump(n, include_attributes=False)
                    for n in ast.parse((HERE / name).read_text()).body
                    if isinstance(n, ast.FunctionDef)}
        old = functions('retire-old-benchmark-single-incremental.py')
        new = functions('inventory-old-benchmark-single-deps.py')
        for name in ('row', 'holders', 'canonical_mounts', 'same_inspections', 'inspect_all',
                     'aliases', 'directory_id', 'open_regular', 'compare_preserved'):
            self.assertEqual(old[name], new[name], name)

    def test_inventory_approval_shape_cannot_widen_fixed_namespace(self):
        name = M.SCOPE + '/libordinary.rlib'
        value = {'roots': [M.SCOPE], 'root_ids': {M.SCOPE: {}},
                 'entries': {name: {'root': M.SCOPE, 'relative': 'libordinary.rlib',
                                   'kind': 'file', 'mode': stat.S_IFREG | 0o600, 'links': 1}},
                 'candidates': [name]}
        self.assertIs(M.validate_inventory(value), value)
        for root in ('/work/target', M.SCOPE.rsplit('/', 1)[0], M.SCOPE.replace('cache-m7', 'cache-m6')):
            bad = copy.deepcopy(value)
            bad['roots'] = [root]
            bad['root_ids'] = {root: {}}
            with self.assertRaises(ValueError):
                M.validate_inventory(bad)

if __name__ == '__main__':
    unittest.main()
