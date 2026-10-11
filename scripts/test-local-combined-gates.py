#!/usr/bin/env python3
"""Pure combined-gate lifecycle controls; no Docker, Cargo or child invocation."""
import importlib.util
import io
from pathlib import Path
import signal
import subprocess
import tarfile
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('local_gates', Path(__file__).with_name('local-combined-gates.py'))
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


def terminal(codes, status='PASS'):
    return {'revision': 'rev', 'archive_sha256': 'sha', 'status': status,
            'uncertainty': [], 'closure': 'known groups closed',
            'phases': [{'name': name, 'command': m.phase_command(name),
                        'limit_seconds': limit, 'exit_code': code}
                       for name, limit, code in zip(('gate', 'solver'), (3600, 1800), codes)]}


class Controls(unittest.TestCase):
    def test_explicit_product_source_and_external_target(self):
        m.validate_paths('/tmp/sail-m7-core-product-source-abcdef0', '/work/target', '/tmp/sail-local-gates-abcdef0')
        for source in ('relative', '/', '/work/target', '/work/target/inside', '/work', '/tmp'):
            with self.assertRaises(ValueError):
                m.validate_paths(source, '/work/target', '/tmp/sail-local-gates-abcdef0')

    def test_original_sequence_and_caps(self):
        seen = []
        self.assertTrue(m.sequence(lambda *args: seen.append(args) or 0, m.Closure()))
        self.assertEqual(seen, [('gate', ['bash', 'ci/run.sh', 'gate'], 3600),
                                ('solver', ['bash', 'ci/run.sh', 'solver'], 1800)])

    def test_failed_gate_never_launches_solver(self):
        seen = []
        self.assertFalse(m.sequence(lambda *args: seen.append(args) or 100, m.Closure()))
        self.assertEqual(len(seen), 1)

    def test_uncertain_zero_exit_never_launches_solver(self):
        closure, seen = m.Closure(), []
        def run(*args):
            seen.append(args)
            closure.uncertain('descendant identity unavailable')
            return 0
        with self.assertRaises(RuntimeError):
            m.sequence(run, closure)
        self.assertEqual(len(seen), 1)

    def test_reap_cannot_clear_sticky_uncertainty(self):
        closure = m.Closure()
        closure.uncertain('timeout')
        closure.uncertain('timeout')
        self.assertEqual(closure.reasons, ['timeout'])
        with self.assertRaises(RuntimeError):
            closure.require()

    def test_completed_child_needs_no_signal(self):
        child = Mock(returncode=0)
        child.poll.return_value = 0
        observe = Mock()
        self.assertEqual(m.run_owned(child, 60, Mock(), Mock(), m.Closure(), observe), 0)
        observe.assert_not_called()

    def test_timeout_reaps_before_raising_and_keeps_uncertainty(self):
        child = Mock(pid=123, args=['original'])
        child.poll.side_effect = [None, None, 0]
        child.wait.return_value = 0
        groups = Mock(leader={'pid': 123})
        module = Mock()
        module.matching.return_value = True
        closure = m.Closure()
        with patch.object(m.time, 'monotonic', side_effect=[0, 61]), patch.object(m.os, 'killpg') as send:
            with self.assertRaises(subprocess.TimeoutExpired):
                m.run_owned(child, 60, groups, module, closure, Mock())
        send.assert_called_once_with(123, signal.SIGINT)
        child.wait.assert_called_once_with(timeout=10)
        groups.signal_live.assert_called_once_with()
        self.assertTrue(closure.reasons)

    def test_recycled_phase_birth_is_never_signalled(self):
        child = Mock(pid=123, args=['original'])
        child.poll.side_effect = [None, None, 0]
        groups = Mock()
        module = Mock()
        module.matching.return_value = False
        with patch.object(m.time, 'monotonic', side_effect=[0, 61]), patch.object(m.os, 'killpg') as send:
            with self.assertRaises(subprocess.TimeoutExpired):
                m.run_owned(child, 60, groups, module, m.Closure(), Mock())
        send.assert_not_called()

    def test_transport_timeout_has_bounded_cleanup(self):
        child = Mock(pid=456)
        child.communicate.side_effect = subprocess.TimeoutExpired(['docker'], 30)
        child.poll.side_effect = [None, None, None]
        child.wait.side_effect = [subprocess.TimeoutExpired(['docker'], 2), 0]
        closure = m.Closure()
        with patch.object(m.subprocess, 'Popen', return_value=child), patch.object(m.os, 'killpg') as send:
            with self.assertRaises(subprocess.TimeoutExpired):
                m.bounded_command(['docker'], timeout=30, closure=closure)
        self.assertEqual(send.call_args_list[0].args, (456, signal.SIGTERM))
        self.assertEqual(send.call_args_list[1].args, (456, signal.SIGKILL))
        self.assertEqual(child.wait.call_args_list[0].kwargs, {'timeout': 2})
        self.assertTrue(closure.reasons)

    def test_transport_signal_unwind_also_cleans_up(self):
        child = Mock(pid=456)
        child.communicate.side_effect = InterruptedError('signal')
        child.poll.side_effect = [None, None, 0]
        closure = m.Closure()
        with patch.object(m.subprocess, 'Popen', return_value=child), patch.object(m.os, 'killpg') as send:
            with self.assertRaises(InterruptedError):
                m.bounded_command(['docker'], timeout=30, closure=closure)
        send.assert_called_once_with(456, signal.SIGTERM)
        self.assertTrue(closure.reasons)

    def test_transport_capture_bound_before_interpretation(self):
        child = Mock(returncode=0)
        def factory(*_args, **kwargs):
            kwargs['stdout'].write(b'12345')
            return child
        with patch.object(m.subprocess, 'Popen', side_effect=factory):
            with self.assertRaises(ValueError):
                m.bounded_command(['fake'], timeout=1, capture_limit=4)

    def test_complete_terminal_requires_both_original_phases(self):
        m.validate_terminal(terminal([0, 0]), 'rev', 'sha')
        for codes in ([], [0], [0, 100]):
            with self.assertRaises(ValueError):
                m.validate_terminal(terminal(codes), 'rev', 'sha')

    def test_failed_gate_cannot_be_followed_by_solver_credit(self):
        m.validate_terminal(terminal([100], 'FAIL'), 'rev', 'sha')
        with self.assertRaises(ValueError):
            m.validate_terminal(terminal([100, 0], 'FAIL'), 'rev', 'sha')

    def test_terminal_rejects_source_or_command_substitution(self):
        record = terminal([0, 0])
        with self.assertRaises(ValueError):
            m.validate_terminal(record, 'other', 'sha')
        record['phases'][1]['command'].append('--changed')
        with self.assertRaises(ValueError):
            m.validate_terminal(record, 'rev', 'sha')

    def test_uncertain_terminal_never_passes(self):
        record = terminal([0, 0])
        record['uncertainty'] = ['forced termination']
        with self.assertRaises(ValueError):
            m.validate_terminal(record, 'rev', 'sha')

    def test_archive_rejects_escape_and_absolute_links(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'source.tar'
            for name, link in (('../escape', None), ('safe', '/outside'), ('safe', '../outside')):
                with tarfile.open(archive, 'w') as handle:
                    member = tarfile.TarInfo(name)
                    if link:
                        member.type, member.linkname = tarfile.SYMTYPE, link
                    handle.addfile(member)
                with self.assertRaises(ValueError):
                    m.inventory(archive)

    def test_archive_preserves_exact_bytes_modes_and_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'source.tar'
            with tarfile.open(archive, 'w') as handle:
                member = tarfile.TarInfo('file')
                member.size, member.mode = 3, 0o755
                handle.addfile(member, io.BytesIO(b'raw'))
                member = tarfile.TarInfo('alias')
                member.type, member.linkname = tarfile.SYMTYPE, 'file'
                handle.addfile(member)
            rows = m.inventory(archive)
            self.assertEqual(rows['files']['file']['mode'], 0o755)
            self.assertEqual(rows['files']['file']['bytes'], 3)
            self.assertEqual(rows['symlinks'], {'alias': 'file'})

    @unittest.skipUnless(hasattr(tarfile, 'data_filter'), 'safe extraction requires Python data filter')
    def test_actual_git_mode_extraction_and_claude_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive, source = root / 'source.tar', root / 'source'
            source.mkdir()
            with tarfile.open(archive, 'w') as handle:
                member = tarfile.TarInfo('scripts/')
                member.type, member.mode = tarfile.DIRTYPE, 0o775
                handle.addfile(member)
                for name, mode in (('AGENTS.md', 0o664), ('scripts/run', 0o775)):
                    member = tarfile.TarInfo(name)
                    member.size, member.mode = 3, mode
                    handle.addfile(member, io.BytesIO(b'raw'))
                member = tarfile.TarInfo('CLAUDE.md')
                member.type, member.linkname, member.mode = tarfile.SYMTYPE, 'AGENTS.md', 0o777
                handle.addfile(member)
            expected = m.inventory(archive)
            # Counterfactual: the original data-filter-only staging loses 0664.
            with tarfile.open(archive) as handle:
                handle.extractall(source, filter='data')
            self.assertEqual((source / 'AGENTS.md').stat().st_mode & 0o777, 0o644)
            with self.assertRaises(ValueError):
                m.verify_source(source, expected)
            m.extract_source(archive, source, expected)
            m.verify_source(source, expected)
            self.assertEqual((source / 'AGENTS.md').stat().st_mode & 0o777, 0o664)
            self.assertEqual((source / 'scripts/run').stat().st_mode & 0o777, 0o775)
            self.assertEqual((source / 'scripts').stat().st_mode & 0o777, 0o775)
            self.assertEqual((source / 'CLAUDE.md').readlink(), Path('AGENTS.md'))

    def test_special_permissions_and_unsafe_types_refuse_before_extraction(self):
        with tempfile.TemporaryDirectory() as directory:
            archive = Path(directory) / 'source.tar'
            for mode, kind in ((0o4664, tarfile.REGTYPE), (0o1775, tarfile.DIRTYPE), (0o644, tarfile.FIFOTYPE), (0o644, tarfile.LNKTYPE)):
                with tarfile.open(archive, 'w') as handle:
                    member = tarfile.TarInfo('unsafe')
                    member.mode, member.type = mode, kind
                    handle.addfile(member)
                with self.assertRaises(ValueError):
                    m.inventory(archive)

    def test_source_extras_changes_and_modes_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory)
            file = source / 'original'
            file.write_bytes(b'raw')
            file.chmod(0o644)
            (source / 'target').symlink_to('/external-target')
            expected = {'files': {'original': {'sha256': m.sha(file), 'bytes': 3, 'mode': 0o644}}, 'symlinks': {}}
            m.verify_source(source, expected)
            (source / '__pycache__').mkdir()
            (source / '__pycache__' / 'extra.pyc').write_bytes(b'x')
            with self.assertRaises(ValueError):
                m.verify_source(source, expected)
            (source / '__pycache__' / 'extra.pyc').unlink()
            (source / '__pycache__').rmdir()
            file.chmod(0o600)
            with self.assertRaises(ValueError):
                m.verify_source(source, expected)
            file.chmod(0o644)
            file.write_bytes(b'bad')
            with self.assertRaises(ValueError):
                m.verify_source(source, expected)

    def test_protected_file_change_refuses(self):
        with tempfile.TemporaryDirectory() as directory:
            file = Path(directory) / 'protected'
            file.write_bytes(b'raw')
            file.chmod(0o644)
            expected = {'files': [{'path': str(file), 'sha256': m.sha(file), 'bytes': 3, 'mode': 0o644}]}
            m.verify_protected(expected)
            file.write_bytes(b'bad')
            with self.assertRaises(ValueError):
                m.verify_protected(expected)

    def test_retained_image_is_exact_but_not_execution_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            image = root / 'actual-operator-image'
            image.write_bytes(b'\x7fELFpayload')
            image.chmod(0o755)
            record = m.retained_images(root, [str(image)])
            self.assertEqual(record['errors'], [])
            self.assertEqual(record['files'][0]['sha256'], m.sha(image))
            self.assertFalse(record['all_cargo_artifacts_archived'])
            self.assertIn('not inferred', record['files'][0]['execution_identity'])

    def test_live_s_and_r_refuse_but_z_and_x_are_retained_separately(self):
        rows = {pid: {'pid': pid, 'pgid': 100, 'start': pid, 'state': state}
                for pid, state in enumerate(('S', 'R', 'Z', 'X'), 100)}
        entries = [types.SimpleNamespace(name=str(pid)) for pid in rows]
        module = types.SimpleNamespace(MAX_SCAN=10, row=rows.get)
        groups = types.SimpleNamespace(known={100: {}})
        def evidence(_module, leader):
            return {'status': 'terminated', 'complete': True, 'task_rows': [leader]}
        with patch.object(m.Path, 'iterdir', return_value=iter(entries)), patch.object(m, 'dead_threads', side_effect=evidence):
            audit = m.group_closure(module, groups)
        self.assertEqual([r['state'] for r in audit['survivors']], ['S', 'R'])
        self.assertEqual([r['state'] for r in audit['terminated_unreaped']], ['Z', 'X'])
        self.assertFalse(m.closed_groups(audit))
        audit['survivors'] = []
        self.assertTrue(m.closed_groups(audit))
        audit['terminated_unreaped'][0]['state'] = 'S'
        self.assertFalse(m.closed_groups(audit))

    def test_true_single_thread_zombie_requires_stable_complete_census(self):
        leader = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z'}
        module = types.SimpleNamespace(row=lambda _pid: dict(leader))
        evidence = m.dead_threads(module, leader, members=lambda _pid: [100])
        self.assertEqual(evidence['status'], 'terminated')
        self.assertTrue(evidence['complete'])
        self.assertTrue(m.terminated_row(dict(leader, terminated_evidence=evidence)))
        self.assertFalse(m.terminated_row(leader))

    def test_zombie_leader_live_or_unknown_sibling_refuses_termination(self):
        leader = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z'}
        for state, expected in (('S', 'live'), ('R', 'live'), ('?', 'unavailable')):
            rows = {100: leader, 101: dict(leader, pid=101, start=11, state=state)}
            evidence = m.dead_threads(types.SimpleNamespace(row=rows.get), leader, members=lambda _pid: [100, 101])
            self.assertEqual(evidence['status'], expected)
            self.assertFalse(evidence['complete'])
            self.assertFalse(m.terminated_row(dict(leader, terminated_evidence=evidence)))

    def test_unreadable_task_missing_leader_and_census_failure_refuse(self):
        leader = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z'}
        module = types.SimpleNamespace(row=lambda pid: leader if pid == 100 else None)
        cases = (lambda _pid: [100, 101], lambda _pid: [101], Mock(side_effect=PermissionError('unreadable')))
        for members in cases:
            evidence = m.dead_threads(module, leader, members=members)
            self.assertEqual(evidence['status'], 'unavailable')
            self.assertFalse(evidence['complete'])

    def test_changing_task_membership_birth_and_state_refuse(self):
        leader = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z'}
        module = types.SimpleNamespace(row=lambda _pid: dict(leader))
        evidence = m.dead_threads(module, leader, members=Mock(side_effect=[[100], [100, 101]]))
        self.assertEqual(evidence['status'], 'unavailable')
        for changed in (dict(leader, start=11), dict(leader, state='S')):
            module = types.SimpleNamespace(row=Mock(side_effect=[leader, leader, changed]))
            evidence = m.dead_threads(module, leader, members=lambda _pid: [100])
            self.assertEqual(evidence['status'], 'unavailable')
            self.assertFalse(evidence['complete'])

    def test_dead_task_bound_refuses_before_reading_unbounded_members(self):
        leader = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'X'}
        read = Mock(return_value=leader)
        evidence = m.dead_threads(types.SimpleNamespace(row=read), leader, members=lambda _pid: list(range(100, 100 + m.MAX_DEAD_TASKS + 1)))
        self.assertEqual(evidence['status'], 'unavailable')
        self.assertEqual(read.call_count, 1)

    def test_missing_raced_row_is_distinct_from_unreadable_existing_row(self):
        module = types.SimpleNamespace(MAX_SCAN=10, row=lambda _pid: None)
        groups = types.SimpleNamespace(known={100: {}})
        for exists in (False, True):
            entry = types.SimpleNamespace(name='100', exists=lambda: exists)
            with patch.object(m.Path, 'iterdir', return_value=iter([entry])):
                audit = m.group_closure(module, groups)
            self.assertEqual(audit['unavailable'], [100] if exists else [])
            self.assertEqual(audit['terminated_unreaped'], [])
            self.assertEqual(m.closed_groups(audit), not exists)

    def test_group_scan_overflow_remains_unavailable(self):
        with patch.object(m.Path, 'iterdir', return_value=iter([Mock(), Mock()])):
            audit = m.group_closure(types.SimpleNamespace(MAX_SCAN=1), Mock())
        self.assertFalse(m.closed_groups(audit))
        self.assertEqual(audit['unavailable'], ['process scan bound'])

    def test_normal_parent_exit_does_not_invent_scan_failure(self):
        primary = {'pid': 123, 'state': 'Z'}
        entry = types.SimpleNamespace(name='123')
        module, groups = Mock(), Mock(leader={'pid': 123})
        module.MAX_SCAN = 10
        module.row.return_value = primary
        context = Mock()
        context.__enter__ = Mock(return_value=iter([entry]))
        context.__exit__ = Mock(return_value=False)
        with patch.object(m.os, 'scandir', return_value=context):
            m.observe_owned(module, groups)
        groups.observe.assert_not_called()


if __name__ == '__main__':
    unittest.main()
