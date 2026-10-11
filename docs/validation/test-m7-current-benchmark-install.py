#!/usr/bin/env python3
"""Pure byte-installation controls; never install binaries or launch actors."""

import copy
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

DIRECTORY = Path(__file__).parent


def module(name, filename):
    spec = importlib.util.spec_from_file_location(name, DIRECTORY / filename)
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


I = module('benchmark_install', 'm7-current-benchmark-install.py')
T = module('benchmark_controls', 'test-m7-current-benchmark-controller.py')
M = I.M


def archive(path, extra=()):
    with tarfile.open(path, 'w') as stream:
        row = tarfile.TarInfo(I.INSTALLER)
        data = b'print("installation-only")\n'
        row.size = len(data)
        stream.addfile(row, io.BytesIO(data))
        for name, kind, target in extra:
            row = tarfile.TarInfo(name)
            row.type = kind
            row.linkname = target
            stream.addfile(row)


class Controls(unittest.TestCase):
    def test_exact_archive_and_installer_digest(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'source.tar'
            archive(path, [('CLAUDE.md', tarfile.SYMTYPE, 'AGENTS.md')])
            value = I.source_archive(path, M.sha(path))
            self.assertEqual(value['members'], 2)
            self.assertEqual(value['installer_sha256'], __import__('hashlib').sha256(b'print("installation-only")\n').hexdigest())
            with self.assertRaises(ValueError):
                I.source_archive(path, '0' * 64)

    def test_unsafe_archive_shapes_refused(self):
        for extra in ([('../escape', tarfile.REGTYPE, '')], [('/absolute', tarfile.REGTYPE, '')], [('link', tarfile.SYMTYPE, '../escape')], [('link', tarfile.SYMTYPE, '/absolute')], [('link', tarfile.SYMTYPE, 'safe'), ('link/child', tarfile.REGTYPE, '')], [('hard', tarfile.LNKTYPE, 'elsewhere')], [(I.INSTALLER, tarfile.REGTYPE, '')]):
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'source.tar'
                archive(path, extra)
                with self.assertRaises(ValueError):
                    I.source_archive(path, M.sha(path))

    def test_complete_extracted_source_and_mutation_refusal(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            path = directory / 'source.tar'
            with tarfile.open(path, 'w') as stream:
                folder = tarfile.TarInfo('scripts')
                folder.type = tarfile.DIRTYPE
                folder.mode = 0o755
                stream.addfile(folder)
                row = tarfile.TarInfo(I.INSTALLER)
                row.size = 3
                stream.addfile(row, io.BytesIO(b'abc'))
            inventory = I.source_archive(path, M.sha(path))['inventory']
            root = directory / 'source'
            (root / 'scripts').mkdir(parents=True)
            (root / I.INSTALLER).write_bytes(b'abc')
            self.assertEqual(I.verify_source(root, inventory)['status'], 'PASS')
            (root / I.INSTALLER).chmod(0o600)
            with self.assertRaisesRegex(ValueError, 'Source mode differs'):
                I.verify_source(root, inventory)
            (root / I.INSTALLER).chmod(0o644)
            (root / I.INSTALLER).write_bytes(b'abd')
            with self.assertRaisesRegex(ValueError, 'Source bytes differ'):
                I.verify_source(root, inventory)
            (root / I.INSTALLER).write_bytes(b'abc')
            (root / 'unexpected').write_bytes(b'')
            with self.assertRaisesRegex(ValueError, 'Unexpected extracted source entry'):
                I.verify_source(root, inventory)

    def test_new_execution_record_keeps_producer_immutable(self):
        producer = T.grant()
        original = copy.deepcopy(producer)
        installation = producer['installation']
        result = I.execution_record(producer, 'd' * 64, installation, {'id': 'owned'}, ['actual', 'installation'], 1.25)
        self.assertEqual(producer, original)
        self.assertEqual(result['artifacts'], producer['artifacts'])
        self.assertEqual(result['source_archive_sha256'], producer['source_archive_sha256'])
        self.assertEqual(result['container'], 'sail-m7-acceptance')
        self.assertEqual(result['producer_grant_sha256'], 'd' * 64)
        self.assertEqual(result['phases'][0], producer['phases'][0])
        self.assertEqual(result['phases'][1]['command'], ['actual', 'installation'])
        self.assertEqual(result['phases'][1]['container'], 'sail-m7-acceptance')

    def test_changed_installed_bytes_cannot_publish_record(self):
        producer = T.grant()
        installation = copy.deepcopy(producer['installation'])
        installation['files']['sail']['sha256'] = 'e' * 64
        with self.assertRaises(ValueError):
            I.execution_record(producer, 'd' * 64, installation, {}, [], 1)

    def test_launcher_binds_derived_record_to_exact_producer(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            producer = T.grant()
            path = directory / 'product-grant.json'
            path.write_text(json.dumps(producer))
            result = I.execution_record(producer, M.sha(path), producer['installation'], {}, [], 1)
            execution = directory / 'product-grant-acceptance.json'
            execution.write_text(json.dumps(result))
            self.assertEqual(M.execution_grant(directory, 'a' * 7)[0], execution)
            path.write_text(json.dumps(producer) + '\n')
            with self.assertRaisesRegex(ValueError, 'Producer record changed'):
                M.execution_grant(directory, 'a' * 7)

    def test_self_consistent_other_product_does_not_match_producer(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            producer = T.grant()
            path = directory / 'product-grant.json'
            path.write_text(json.dumps(producer))
            result = I.execution_record(producer, M.sha(path), producer['installation'], {}, [], 1)
            result['artifacts']['sail']['sha256'] = 'f' * 64
            result['installation']['files']['sail']['sha256'] = 'f' * 64
            (directory / 'product-grant-acceptance.json').write_text(json.dumps(result))
            with self.assertRaisesRegex(ValueError, 'Execution/producer identity differs'):
                M.execution_grant(directory, 'a' * 7)

    def test_only_original_record_present_preserves_original_selection(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'product-grant.json'
            path.write_text(json.dumps(T.grant()))
            self.assertEqual(M.execution_grant(directory, 'a' * 7)[0], path)


if __name__ == '__main__':
    unittest.main()
