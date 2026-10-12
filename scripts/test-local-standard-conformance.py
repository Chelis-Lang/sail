#!/usr/bin/env python3
"""Pure standard-controller binding/oracle/failure controls; no native actors."""
import copy
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('standard', Path(__file__).with_name('local-standard-conformance.py'))
s = importlib.util.module_from_spec(spec)
spec.loader.exec_module(s)
g, _product = s.helpers(Path(__file__).parent)


def report():
    return {'schema_version': 1, 'tier': 'standard', 'skipped': [],
            'summary': {'units': 159, 'projects_skipped': 0, 'same': 159,
                        'unstable': 0, 'different': 0, 'infra_error': 0},
            'units': [{'id': 'unit-' + str(index)} for index in range(159)]}


class Controls(unittest.TestCase):
    def test_complete_original_command_and_original_inner_deadlines(self):
        self.assertEqual(s.BUILD, ['cargo', 'build', '--release', '--locked', '-p', 'sail-conformance'])
        self.assertEqual(s.BUILD_LIMIT, 600)
        self.assertEqual(s.UNIT_LIMIT, 3600)
        self.assertEqual(s.ROUND_LIMIT, 159 * 4 * 3600 + 300)
        self.assertEqual(s.command('/sealed/harness', '/sealed/shim', '/fresh/out'),
                         ['/sealed/harness', 'run', '--tier', 'standard', '--jobs', '1',
                          '--shim', '/sealed/shim', '--out', '/fresh/out'])

    def test_original_default_environment_removes_every_optional_selector(self):
        with patch.dict(os.environ, {'PATH': '/exact/path', 'RUSTUP_TOOLCHAIN': 'original',
                                    'CI': 'true', 'SAIL_VERUS_SOURCE_BUILD': '/engine',
                                    'SAIL_ENGINE_BUILD': '/engine', 'SAIL_DAEMON_SOCKET': '/old/socket',
                                    'SAIL_PORTFOLIO': 'anything', 'SAIL_UNIT_TIMEOUT': '1'}, clear=True):
            value = s.environment('/opt/verus', '/root/.cache/sail')
        self.assertEqual(value, {'PATH': '/exact/path', 'RUSTUP_TOOLCHAIN': 'original',
                                 'SAIL_VERUS_HOME': '/opt/verus', 'SAIL_CACHE_DIR': '/root/.cache/sail',
                                 'SAIL_UNIT_TIMEOUT': '3600'})

    def test_whole_rounds_original_membership_and_classifier(self):
        self.assertEqual(len(s.complete_report(report())), 159)
        value = report()
        value['summary'].update(same=158, unstable=1)
        self.assertEqual(len(s.complete_report(value)), 159)  # Original exit-zero unstable classification.

    def test_infra_timeout_difference_duplicate_or_skipped_cannot_be_pass(self):
        changes = [lambda v: v['summary'].update(infra_error=1, same=158),
                   lambda v: v['summary'].update(different=1, same=158),
                   lambda v: v['summary'].update(units=158),
                   lambda v: v['summary'].update(projects_skipped=1),
                   lambda v: v['units'].pop(), lambda v: v['units'][-1].update(id='unit-0'),
                   lambda v: v.update(tier='fast'), lambda v: v.update(skipped=['project'])]
        for change in changes:
            value = copy.deepcopy(report())
            change(value)
            with self.assertRaises(ValueError):
                s.complete_report(value)

    def test_normal_rounds_start_cache_once_after_pass_through(self):
        seen = []
        result = s.sequence(lambda name, argv, cap: seen.append((name, argv, cap)) or 0,
                            g.Closure(), '/harness', '/shim', Path('/fresh'), lambda: seen.append('start'))
        self.assertTrue(result)
        self.assertEqual([item if isinstance(item, str) else item[0] for item in seen],
                         ['pass-through', 'start', 'cache-fill', 'cache-warm'])

    def test_each_failed_prefix_stops_without_later_round(self):
        for failure in s.ROUNDS:
            seen = []
            def run(name, _argv, _cap):
                seen.append(name)
                return 2 if name == failure else 0
            self.assertFalse(s.sequence(run, g.Closure(), '/harness', '/shim', Path('/fresh'), lambda: None))
            self.assertEqual(seen, list(s.ROUNDS[:s.ROUNDS.index(failure) + 1]))

    def test_uncertain_zero_exit_cannot_start_or_continue_cache(self):
        seen, closure = [], g.Closure()
        def run(name, _argv, _cap):
            seen.append(name)
            closure.uncertain('original wait timed out; leader reap is insufficient')
            return 0
        with self.assertRaises(RuntimeError):
            s.sequence(run, closure, '/harness', '/shim', Path('/fresh'), lambda: seen.append('start'))
        self.assertEqual(seen, ['pass-through'])

    def test_actual_elf_copy_is_fresh_private_byte_identical_and_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / 'actual', root / 'sealed'
            source.write_bytes(b'\x7fELF' + b'actual-emitted-bytes')
            row = s.seal_harness(g, source, destination)
            self.assertEqual(row['sha256'], g.sha(destination / 'sail-conformance'))
            self.assertEqual(row['uid'], os.geteuid())
            self.assertEqual(destination.stat().st_mode & 0o777, 0o700)
            self.assertEqual((destination / 'sail-conformance').stat().st_mode & 0o777, 0o700)
            with self.assertRaises(FileExistsError):
                s.seal_harness(g, source, destination)

    def test_non_elf_or_source_alias_never_produces_a_harness(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source'
            source.write_bytes(b'not-an-elf')
            with self.assertRaises(ValueError):
                s.seal_harness(g, source, root / 'bad')
            source.write_bytes(b'\x7fELFgood')
            alias = root / 'alias'
            alias.symlink_to(source)
            with self.assertRaises(ValueError):
                s.seal_harness(g, alias, root / 'bad')
            self.assertFalse((root / 'bad').exists())

    def test_original_bounded_phase_snapshot_also_observes_daemon_without_extra_scan(self):
        seen = []
        class Original:
            def observe(self, rows):
                seen.append(('phase', rows))
        background = Original()
        wrapped = s.observing_groups(Original, lambda: background)()
        snapshot = {12: {'start': 30}}
        wrapped.observe(snapshot)
        self.assertEqual(seen, [('phase', snapshot), ('phase', snapshot)])
        self.assertIs(seen[0][1], seen[1][1])


if __name__ == '__main__':
    unittest.main()
