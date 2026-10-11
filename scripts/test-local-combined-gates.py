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
