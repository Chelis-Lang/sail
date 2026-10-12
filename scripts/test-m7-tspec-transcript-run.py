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
        self.assertEqual(g.first_refusal['reason'], 'primary-unavailable-or-dead-or-rebound')

    def test_default_cumulative_budget_stays_partial_after_children_exit(self):
        primary = row(100, 1)
        g = m.Groups(primary)
        self.assertEqual(g.max_groups, 256)
        # At most one child is present per scan; terminated history still counts.
        for pid in range(101, 356):
            g.observe({100: primary, pid: row(pid, 100, start=pid)})
        self.assertEqual(len(g.known), 256)
        self.assertFalse(g.partial)
        g.observe({100: primary, 356: row(356, 100, start=356)})
        self.assertTrue(g.partial)
        self.assertNotIn(356, g.known)
        self.assertEqual(g.first_refusal['reason'], 'group-budget-exhausted')
        first = dict(g.first_refusal)
        g.observe({100: primary})
        g.observe({100: row(100, 1, start=11)})
        self.assertTrue(g.partial)
        self.assertEqual(g.first_refusal, first)
        sent = []
        g.signal_live(read={100: primary, 356: row(356, 100)}.get,
                      send=lambda *value: sent.append(value))
        self.assertEqual(sent, [(100, signal.SIGKILL)])

    def test_selected_budget_is_bounded_and_saturation_still_refuses(self):
        primary = row(100, 1)
        g = m.Groups(primary, max_groups=8192)
        for pid in range(101, 8292):
            g.observe({100: primary, pid: row(pid, 100, start=pid)})
        self.assertEqual(len(g.known), 8192)
        self.assertFalse(g.partial)
        g.observe({100: primary, 8292: row(8292, 100, start=8292)})
        self.assertTrue(g.partial)
        self.assertEqual(g.first_refusal['group_budget'], 8192)
        for budget in (0, -1, True, '8192', m.MAX_SCAN + 1):
            with self.assertRaises(ValueError):
                m.Groups(primary, max_groups=budget)

    def test_selected_budget_does_not_grant_recycled_group_signal_authority(self):
        primary = row(100, 1)
        g = m.Groups(primary, max_groups=8192)
        g.observe({100: primary, 102: row(102, 100, start=20)})
        g.observe({100: primary, 102: row(102, 100, start=21)})
        self.assertTrue(g.partial)
        self.assertEqual(g.first_refusal['reason'], 'known-group-birth-changed')
        sent = []
        g.signal_live(read={102: row(102, 100, start=21)}.get,
                      send=lambda *value: sent.append(value))
        self.assertEqual(sent, [])

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
