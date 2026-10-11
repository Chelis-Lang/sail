#!/usr/bin/env python3
"""Pure ownership and supplementary-storage controls; no verifier invocation."""
import importlib.util
from pathlib import Path
import signal
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('controller', Path(__file__).with_name('m7-tspec-transcript-run.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def row(pid, ppid, start=10, group=None):
    return {'pid': pid, 'ppid': ppid, 'start': start, 'pgid': pid if group is None else group, 'state': 'S'}


class Controls(unittest.TestCase):
    def test_catchable_termination_enters_unwind(self):
        with self.assertRaises(KeyboardInterrupt):
            m.interrupted(signal.SIGTERM, None)

    def test_owned_group_parent_chain(self):
        g = m.Groups(row(100, 1))
        g.observe({100: row(100, 1), 101: row(101, 100, group=100), 102: row(102, 101), 200: row(200, 1)})
        self.assertEqual(set(g.known), {100, 102})

    def test_missing_parent_not_owned(self):
        g = m.Groups(row(100, 1))
        g.observe({100: row(100, 1), 102: row(102, 101)})
        self.assertEqual(set(g.known), {100})

    def test_child_predating_recycled_intermediate_parent_not_owned(self):
        g = m.Groups(row(100, 1))
        g.observe({100: row(100, 1), 101: row(101, 100, start=30, group=100),
                   102: row(102, 101, start=20)})
        self.assertEqual(set(g.known), {100})

    def test_dead_parent_cannot_grant_child_authority(self):
        g = m.Groups(row(100, 1))
        parent = row(101, 100, group=100)
        parent['state'] = 'Z'
        g.observe({100: row(100, 1), 101: parent, 102: row(102, 101)})
        self.assertEqual(set(g.known), {100})

    def test_reused_leader_refuses_subtree(self):
        g = m.Groups(row(100, 1))
        g.observe({100: row(100, 1, start=11), 102: row(102, 100)})
        self.assertTrue(g.partial)
        self.assertEqual(set(g.known), {100})

    def test_cleanup_rechecks_birth_and_group(self):
        g = m.Groups(row(100, 1))
        g.observe({100: row(100, 1), 102: row(102, 100)})
        sent = []
        g.signal_live(read=lambda p: row(p, 1, start=11), send=lambda *x: sent.append(x))
        self.assertEqual(sent, [])
        g.signal_live(read=lambda p: row(p, 1, group=999), send=lambda *x: sent.append(x))
        self.assertEqual(sent, [])

    def test_cleanup_children_before_primary(self):
        g = m.Groups(row(100, 1))
        rows = {100: row(100, 1), 102: row(102, 100)}
        g.observe(rows)
        sent = []
        g.signal_live(read=rows.get, send=lambda *x: sent.append(x))
        self.assertEqual(sent, [(102, signal.SIGKILL), (100, signal.SIGKILL)])

    def test_dead_peer_not_signalled(self):
        g = m.Groups(row(100, 1))
        dead = row(100, 1)
        dead['state'] = 'Z'
        self.assertEqual(g.signal_live(read=lambda _: dead, send=lambda *_: self.fail('signalled zombie')), [])

    def test_storage_never_exports_environment_or_deletes_raw(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'private-environment.json').write_text('secret')
            (root / 'stdout').write_bytes(b'raw\n')
            result = m.archive(root)
            self.assertTrue(result['complete'])
            with tarfile.open(root / 'supplementary.tar') as archive:
                self.assertEqual(archive.getnames(), ['stdout'])
                self.assertEqual(archive.extractfile('stdout').read(), b'raw\n')
            self.assertEqual((root / 'private-environment.json').read_text(), 'secret')
            self.assertEqual((root / 'stdout').read_bytes(), b'raw\n')

    def test_storage_cap_marks_partial_and_preserves_input(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'stdout').write_bytes(b'raw\n')
            previous = m.MAX_BYTES
            try:
                m.MAX_BYTES = 3
                result = m.archive(root)
            finally:
                m.MAX_BYTES = previous
            self.assertFalse(result['complete'])
            self.assertEqual(result['files'], 0)
            self.assertEqual((root / 'stdout').read_bytes(), b'raw\n')


if __name__ == '__main__':
    unittest.main()
