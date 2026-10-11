#!/usr/bin/env python3
"""Pure controller controls; no Docker, Cargo, daemon, solver or proof actor."""

import copy
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('benchmark_controller', Path(__file__).with_name('m7-current-benchmark-controller.py'))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)


def grant():
    commit = 'a' * 40
    files = {name: {'bytes': 10, 'sha256': 'b' * 64, 'uid': 0, 'gid': 0, 'mode': '0700'} for name in ('sail', 'sail-z3')}
    return {'version': 1, 'source_commit': commit, 'source_archive_sha256': 'c' * 64, 'container': 'sail-m7-handoff-validation', 'source_path': '/tmp/sail-m7-core-product-source-' + commit[:7], 'product_dir': '/opt/sail-m7-core-product-' + commit[:7], 'phases': [{'phase': name, 'exit_code': 0} for name in ('release-build', 'owned-install')], 'installation_only': True, 'artifacts': {name: {key: row[key] for key in ('bytes', 'sha256')} for name, row in files.items()}, 'installation': {'authority': 'installation-only', 'source_modified': False, 'directory_mode': '0700', 'credentials': {'fsuid': 0}, 'files': files}}


class Controls(unittest.TestCase):
    def test_fresh_namespace_changes_work_and_socket_selection_inputs(self):
        first = M.output_path('abcdef0', False)
        recovered = M.output_path('abcdef0', False, 'after-recovery')
        self.assertNotEqual(first / 'work/results/cache-m6', recovered / 'work/results/cache-m6')
        self.assertEqual(str(first), '/tmp/sail-m7-current-benchmark-abcdef0')
        self.assertEqual(str(M.output_path('abcdef0', True, 'after-recovery')), str(recovered) + '-precheck')

    def test_unsafe_namespace_refused_before_docker(self):
        for value in ('', '../other', '/other', 'a/b', 'a' * 33, 'A', 'a\n'):
            with self.assertRaises(ValueError):
                M.output_path('abcdef0', False, value)

    def test_valid_record_binds_exact_revision(self):
        value = grant()
        self.assertIs(M.validate_grant(value, 'a' * 7), value)
        with self.assertRaises(ValueError):
            M.validate_grant(value, 'a' * 6 + 'f')

    def test_duplicate_json_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'value.json'
            path.write_text('{"source_commit": "a", "source_commit": "b"}')
            with self.assertRaises(ValueError):
                M.read_json(path)

    def test_metadata_read_is_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'value.json'
            path.write_bytes(b' ' * (1024 * 1024 + 1))
            with self.assertRaises(ValueError):
                M.read_json(path)

    def test_failed_missing_or_duplicate_phase_refused(self):
        for mutate in (lambda v: v['phases'][0].update(exit_code=1), lambda v: v['phases'].pop(), lambda v: v['phases'].append(copy.deepcopy(v['phases'][0]))):
            value = grant()
            mutate(value)
            with self.assertRaises((ValueError, KeyError)):
                M.validate_grant(value, 'a' * 7)

    def test_installation_identity_and_path_refused(self):
        for key, replacement in (('sha256', 'd' * 64), ('bytes', 11), ('uid', 501), ('mode', '0755')):
            value = grant()
            value['installation']['files']['sail'][key] = replacement
            with self.assertRaises(ValueError):
                M.validate_grant(value, 'a' * 7)
        value = grant()
        value['product_dir'] += '/../../other'
        with self.assertRaises(ValueError):
            M.validate_grant(value, 'a' * 7)

    def test_exact_unchanged_wrapper_arguments_and_fresh_work(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            identity = {'pid': 123, 'start_ticks': 456, 'pgid': 123}
            with patch.object(M.os, 'getpriority', return_value=0), patch.object(M.os, 'getsid', return_value=M.os.getpid()), patch.object(M, 'process_identity', return_value=identity), patch.object(M.os, 'execvp') as execute:
                M.native_execute(grant(), output)
            program, argv = execute.call_args.args
            self.assertEqual(program, 'python3')
            self.assertEqual(argv[1], grant()['source_path'] + '/' + str(M.ORIGINAL))
            self.assertEqual(argv[argv.index('--product-commit') + 1], 'a' * 40)
            self.assertEqual(argv[argv.index('--work') + 1], str(output / 'work'))
            self.assertFalse((output / 'work').exists())
            self.assertEqual(json.loads((output / 'actor.json').read_text()), identity)
            self.assertNotIn('--timeout', argv)  # Original wrapper owns its unchanged 600.

    def test_wrapper_bytes_are_checked_before_import(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / M.ORIGINAL
            path.parent.mkdir(parents=True)
            path.write_text('raise RuntimeError("must not execute")')
            with self.assertRaisesRegex(ValueError, 'Original wrapper changed'):
                M.original_module(directory)

    def test_sealed_definition_load_does_not_write_source_bytecode(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / M.ORIGINAL
            path.parent.mkdir(parents=True)
            path.write_text('value = 3\nif __name__ == "__main__":\n    raise RuntimeError("must not launch")\n')
            digest = M.sha(path)
            with patch.object(M, 'ORIGINAL_SHA', digest):
                self.assertEqual(M.original_module(directory).value, 3)
            self.assertFalse((path.parent / '__pycache__').exists())
            self.assertEqual(M.sha(path), digest)

    def test_nonordinary_priority_refused_before_exec(self):
        with patch.object(M.os, 'getpriority', return_value=10), patch.object(M.os, 'execvp') as execute:
            with self.assertRaises(ValueError):
                M.native_execute(grant(), Path('unused'))
            execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
