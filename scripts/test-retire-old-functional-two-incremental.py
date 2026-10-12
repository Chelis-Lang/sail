#!/usr/bin/env python3
"""Host-only policy and provenance controls; no native cleanup actors."""
import ast
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import tempfile
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).parent
SOURCE = HERE / 'retire-old-functional-two-incremental.py'
SPEC = importlib.util.spec_from_file_location('functional_scratch', SOURCE)
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


class Controls(unittest.TestCase):
    def test_exact_scope_and_limits(self):
        self.assertEqual(M.BASE, '/tmp/sail-m7-retained-functional-1e-real')
        self.assertEqual(M.ROOTS, (M.BASE + '/cache/projects',))
        self.assertEqual(len(M.SCRATCH_ROOTS), 2)
        self.assertEqual((M.MAX_FILES, M.MAX_BYTES), (20000, 8 * 1024**3))
        for prefix in M.SCRATCH_RELATIVE:
            self.assertTrue(M.admitted_incremental(Path(prefix + '/session/dep-graph.bin')))
            self.assertFalse(M.admitted_incremental(Path(prefix)))
        for name in ('other/target/debug/incremental/a.bin', '../escape.bin',
                     '/work/target/debug/incremental/a.bin', M.SCRATCH_RELATIVE[0].replace('/incremental', '/deps') + '/a.bin'):
            self.assertFalse(M.admitted_incremental(Path(name)), name)

    def test_suffix_links_mode_magic_and_proof_refusal(self):
        prefix = M.SCRATCH_RELATIVE[0] + '/session/'
        ordinary = types.SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_nlink=1)
        for name in ('dep-graph.bin', 'object.o', 'item.rmeta', 'item.rlib', 'session.lock', 'lock'):
            self.assertTrue(M.admitted_file(Path(prefix + name), ordinary, b'data'), name)
        for name in ('proof.vir', 'receipt.json', 'source.rs', 'stdout', 'ledger', 'unknown'):
            self.assertFalse(M.admitted_file(Path(prefix + name), ordinary, b'data'), name)
        for info, magic in ((ordinary, b'\x7fELF'),
                            (types.SimpleNamespace(st_mode=stat.S_IFREG | 0o700, st_nlink=1), b'data'),
                            (types.SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_nlink=2), b'data')):
            self.assertFalse(M.admitted_file(Path(prefix + 'dep.bin'), info, magic))

    def test_full_projects_inventory_preserves_every_other_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve() / 'projects'
            session = root / M.SCRATCH_RELATIVE[0] / 'session'
            second = root / M.SCRATCH_RELATIVE[1] / 'session'
            session.mkdir(parents=True)
            second.mkdir(parents=True)
            for name, value in (('dep.bin', b'build-state'), ('test.bin', b'\x7fELFimage'),
                                ('receipt.json', b'proof-evidence')):
                (session / name).write_bytes(value)
            (second / 'other.bin').write_bytes(b'build-state')
            (root / 'root-receipt.json').write_bytes(b'preserve')
            (session / 'executable.bin').write_bytes(b'preserve')
            (session / 'executable.bin').chmod(0o700)
            outside = root.parent / 'canonical.bin'
            outside.write_bytes(b'linked')
            os.link(outside, session / 'linked.bin')
            before = M.inventory((str(root),))
            self.assertEqual({Path(p).name for p in before['candidates']}, {'dep.bin', 'other.bin'})
            self.assertEqual(before['protected_hardlinked_files'], 1)
            M.compare_preserved(before, M.inventory((str(root),)), [])
            (root / 'root-receipt.json').write_bytes(b'changed')
            with self.assertRaises(ValueError):
                M.compare_preserved(before, M.inventory((str(root),)), [])

    def test_approval_cannot_expand_inventory_or_candidates(self):
        relative = M.SCRATCH_RELATIVE[0] + '/session/dep.bin'
        name = M.SCOPE + '/' + relative
        value = {'roots': [M.SCOPE], 'root_ids': {M.SCOPE: {}}, 'candidates': [name],
                 'entries': {name: {'root': M.SCOPE, 'relative': relative, 'kind': 'file',
                                   'mode': stat.S_IFREG | 0o600, 'links': 1}}}
        self.assertIs(M.validate_inventory(value), value)
        for change in ({'relative': 'other/session/dep.bin'}, {'links': 2},
                       {'mode': stat.S_IFREG | 0o700}, {'relative': relative.replace('dep.bin', 'receipt.json')}):
            bad = copy.deepcopy(value)
            bad['entries'][name].update(change)
            with self.assertRaises(ValueError):
                M.validate_inventory(bad)
        bad = copy.deepcopy(value)
        bad['roots'] = ['/work/target']
        with self.assertRaises(ValueError):
            M.validate_inventory(bad)

    def test_linked_incremental_namespace_refuses(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            session = root / M.SCRATCH_RELATIVE[0]
            session.mkdir(parents=True)
            (session / 'linked.bin').symlink_to(root / 'absent')
            with self.assertRaises(ValueError):
                M.inventory((str(root),))

    def test_budget_exhaustion_refuses(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / 'ordinary').write_bytes(b'four')
            with patch.object(M, 'MAX_FILES', 0), self.assertRaisesRegex(ValueError, 'entry limit'):
                M.inventory((str(root),))
            with patch.object(M, 'MAX_BYTES', 3), self.assertRaisesRegex(ValueError, 'byte limit'):
                M.inventory((str(root),))
            with patch.object(M.time, 'monotonic', side_effect=[0, 301]), self.assertRaisesRegex(ValueError, 'time limit'):
                M.inventory((str(root),))

    def test_exact_frozen_holder_file_mount_and_preservation_guards(self):
        def canonical(value):
            if isinstance(value, ast.AST):
                return {'node': type(value).__name__, 'fields': [[name, canonical(item)]
                        for name, item in ast.iter_fields(value) if name != 'type_params' or item]}
            if isinstance(value, list):
                return [canonical(item) for item in value]
            if isinstance(value, bytes):
                return {'bytes_hex': value.hex()}
            return value
        plan = json.loads((HERE.parent / 'docs/validation/m7-old-functional-two-incremental-plan.json').read_text())
        provenance = plan['guard_provenance']
        self.assertEqual(provenance['source_sha256'], 'b2c62607087d6057e60e53007880e20ef553b59a90b83ec64556afd66e444ed1')
        nodes = {node.name: node for node in ast.parse(SOURCE.read_text()).body if isinstance(node, ast.FunctionDef)}
        self.assertEqual(set(provenance['function_ast_sha256']),
                         {'row', 'holders', 'canonical_mounts', 'same_inspections', 'inspect_all',
                          'aliases', 'directory_id', 'open_regular', 'compare_preserved'})
        for name, expected in provenance['function_ast_sha256'].items():
            actual = hashlib.sha256(json.dumps(canonical(nodes[name]), ensure_ascii=True,
                                               separators=(',', ':')).encode()).hexdigest()
            self.assertEqual(actual, expected, name)
        self.assertEqual(hashlib.sha256(SOURCE.read_bytes()).hexdigest(), plan['controller_sha256'])


if __name__ == '__main__':
    unittest.main()
