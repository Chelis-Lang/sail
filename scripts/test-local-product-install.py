#!/usr/bin/env python3
"""Pure product prerequisites, shared lifecycle and grant controls; no actors."""
import copy
import importlib.util
import json
import io
from pathlib import Path
import shutil
import tarfile
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location('product', Path(__file__).with_name('local-product-install.py'))
p = importlib.util.module_from_spec(spec)
spec.loader.exec_module(p)
g = p.gate_module()


def inputs():
    commit, archive = 'a' * 40, 'b' * 64
    identity = {'id': 'actual-id', 'image': 'actual-image', 'birth': 'actual-birth'}
    controller = {'status': 'PASS', 'uncertainty': [], 'transport_exit_code': 0,
                  'container_before': identity, 'container_after': identity,
                  'revision': commit, 'archive_sha256': archive}
    config = {'revision': commit, 'archive_sha256': archive,
              'source': '/tmp/sail-m7-core-product-source-' + commit[:7]}
    record = {'status': 'PASS', 'uncertainty': [], 'closure': 'closed', 'jobs': 2,
              'nextest_independent_tests': 1, 'revision': commit, 'archive_sha256': archive,
              'phases': [{'name': name, 'command': g.phase_command(name),
                          'limit_seconds': limit, 'exit_code': 0,
                          'group_closure': {'survivors': [], 'unavailable': []}}
                         for name, limit in (('gate', 3600), ('solver', 1800))]}
    return controller, record, config


def grant_inputs():
    config = {'gate_config': inputs()[2], 'container': 'sail-m7-handoff-validation',
              'execution': 'explicit test environment',
              'execution_container_birth': {'id': 'id', 'image': 'image', 'created': 'created', 'started_at': 'started'},
              'product_dir': '/opt/sail-m7-core-product-' + 'a' * 7,
              'engine': {'engine_build': '/selected-engine'}, 'gate_inputs': {'result.json': 'c' * 64}}
    record = {'status': 'PASS', 'uncertainty': [], 'closure': 'closed-recorded-scope',
              'phases': [{'name': 'release-build', 'command': p.BUILD_COMMAND,
                          'limit_seconds': 600, 'exit_code': 0},
                         {'name': 'owned-install', 'command': ['installer'],
                          'limit_seconds': 60, 'exit_code': 0}]}
    artifacts = {name: {'bytes': 10, 'sha256': 'd' * 64} for name in ('sail', 'sail-z3')}
    installation = {'authority': 'installation-only', 'source_modified': False,
                    'directory_mode': '0700', 'credentials': {'fsuid': 0},
                    'files': {name: dict(row, mode='0700', uid=0, gid=0) for name, row in artifacts.items()}}
    return config, record, artifacts, installation


def retired_fixture(base):
    """Real compact archive/inventory, qualified source tar and immutable originals."""
    r = p.module(Path(p.__file__).with_name('retire-completed-run.py'), 'retirement_fixture')
    original, backing, scratch = base / 'original', base / 'backing', base / 'scratch'
    for path in (original, backing, scratch, backing / 'host-evidence'):
        path.mkdir()
    controller, result, config = inputs()
    controller['native_root'] = '/tmp/sail-local-gates-fixture'
    with tarfile.open(original / 'source.tar', 'w') as archive:
        item = tarfile.TarInfo('Cargo.lock')
        raw = b'exact frozen lock\n'
        item.size, item.mode = len(raw), 0o644
        archive.addfile(item, io.BytesIO(raw))
    source_hash = g.sha(original / 'source.tar')
    config['archive_sha256'] = controller['archive_sha256'] = result['archive_sha256'] = source_hash
    for phase in result['phases']:
        log = scratch / (phase['name'] + '.log')
        log.write_bytes((phase['name'] + ' original raw\n\n').encode())
        phase['log_sha256'] = g.sha(log)
    records = {'config.json': config, 'result.json': result,
               'source-inventory.json': g.inventory(original / 'source.tar'), 'protected.json': {'files': []}}
    for name, value in records.items():
        g.save(scratch / name, value)
        shutil.copyfile(scratch / name, original / name)
    plan = r.inventory(scratch)
    r.archive(scratch, plan, backing / 'evidence.tar.gz')
    plan['root'] = controller['native_root']
    r.save(backing / 'inventory.json', plan)
    identity = {'handoff': {'birth': 'original'}, 'acceptance': {'birth': 'original'}}
    retirement = {'status': 'PASS', 'original_status': 'PASS', 'removed': True,
                  'root': controller['native_root'], 'containers_before': identity, 'containers_after': identity,
                  'verified_evidence': {'archive_sha256': g.sha(backing / 'evidence.tar.gz'),
                                       'inventory_sha256': g.sha(backing / 'inventory.json'),
                                       'full_evidence_bytes': plan['evidence_bytes']}, 'host_evidence': []}
    for name in ('source.tar', 'source-inventory.json', 'protected.json', 'result.json'):
        path = backing / 'host-evidence' / name
        shutil.copyfile(original / name, path)
        retirement['host_evidence'].append({'name': name, 'bytes': path.stat().st_size, 'sha256': g.sha(path)})
    r.save(backing / 'original-terminal.json', dict(result, native_root=controller['native_root']))
    r.save(backing / 'controller.json', retirement)
    controller['retirement'] = retirement
    g.save(original / 'controller.json', controller)
    return original, backing, base / 'restored'


class Controls(unittest.TestCase):
    def test_restaging_restores_exact_records_without_source_or_history_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            original, backing, restored = retired_fixture(Path(directory))
            before = {str(path): path.read_bytes() for root in (original, backing) for path in root.rglob('*') if path.is_file()}
            source = Path(directory) / 'externally-present-source'
            source.mkdir()
            (source / 'unchanged').write_bytes(b'original source')
            provenance = p.restage_gate(g, original, backing, restored)
            self.assertFalse(provenance['re_execution'])
            self.assertEqual(provenance['authority'], 'restored-qualified-evidence')
            config = {'gate_restaging': provenance, 'gate_config': g.load(original / 'config.json'),
                      'gate_controller': g.load(original / 'controller.json'),
                      'gate_inputs': {name: g.sha(restored / name) for name in [*provenance['files'], 'restaging.json']}}
            p.verify_restaging(g, restored, config)
            self.assertEqual(before, {str(path): path.read_bytes() for root in (original, backing) for path in root.rglob('*') if path.is_file()})
            self.assertEqual((source / 'unchanged').read_bytes(), b'original source')
            self.assertEqual((restored / 'gate.log').read_bytes(), b'gate original raw\n\n')
            self.assertEqual(config['gate_config']['source'], inputs()[2]['source'])

    def test_restaging_requires_original_pass_retirement_and_stable_births(self):
        for mutation in (lambda row: row.update(status='FAIL'), lambda row: row.update(removed=False),
                         lambda row: row.update(original_status='FAIL'), lambda row: row.update(root='/tmp/other'),
                         lambda row: row.update(containers_after={'birth': 'changed'})):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                original, backing, restored = retired_fixture(Path(directory))
                retirement = g.load(backing / 'controller.json')
                mutation(retirement)
                g.save(backing / 'controller.json', retirement)
                controller = g.load(original / 'controller.json')
                controller['retirement'] = retirement
                g.save(original / 'controller.json', controller)
                with self.assertRaises(ValueError):
                    p.restage_gate(g, original, backing, restored)
                self.assertFalse(restored.exists())

    def test_restaging_missing_corrupted_and_partial_archives_refuse(self):
        for variant in ('corrupt', 'missing', 'partial'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as directory:
                original, backing, restored = retired_fixture(Path(directory))
                if variant == 'missing':
                    (backing / 'evidence.tar.gz').unlink()
                elif variant == 'corrupt':
                    (backing / 'evidence.tar.gz').write_bytes(b'wrong raw archive')
                else:
                    with tarfile.open(backing / 'evidence.tar.gz', 'w:gz'):
                        pass
                    retirement = g.load(backing / 'controller.json')
                    retirement['verified_evidence']['archive_sha256'] = g.sha(backing / 'evidence.tar.gz')
                    g.save(backing / 'controller.json', retirement)
                    controller = g.load(original / 'controller.json')
                    controller['retirement'] = retirement
                    g.save(original / 'controller.json', controller)
                with self.assertRaises((ValueError, FileNotFoundError)):
                    p.restage_gate(g, original, backing, restored)
                self.assertFalse(restored.exists())

    def test_restaging_does_not_rebind_wrong_source_or_relax_qualification(self):
        with tempfile.TemporaryDirectory() as directory:
            original, backing, restored = retired_fixture(Path(directory))
            config = g.load(original / 'config.json')
            config['source'] = '/tmp/sail-m7-current-source-' + config['revision'][:7]
            g.save(original / 'config.json', config)
            with self.assertRaisesRegex(ValueError, 'common product grant path'):
                p.restage_gate(g, original, backing, restored)
            self.assertFalse(restored.exists())

    def test_restaging_source_archive_and_retained_original_must_match(self):
        for name in ('source.tar', 'original-terminal.json', 'source-inventory.json'):
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                original, backing, restored = retired_fixture(Path(directory))
                path = backing / ('host-evidence' if name != 'original-terminal.json' else '') / name
                path.write_bytes(b'{}')
                with self.assertRaises(ValueError):
                    p.restage_gate(g, original, backing, restored)

    def test_restaged_raw_replacement_and_mode_changes_refuse(self):
        for variant in ('bytes', 'mode', 'symlink', 'record'):
            with self.subTest(variant=variant), tempfile.TemporaryDirectory() as directory:
                original, backing, restored = retired_fixture(Path(directory))
                provenance = p.restage_gate(g, original, backing, restored)
                config = {'gate_restaging': provenance, 'gate_config': g.load(original / 'config.json'),
                          'gate_controller': g.load(original / 'controller.json'),
                          'gate_inputs': {name: g.sha(restored / name) for name in [*provenance['files'], 'restaging.json']}}
                path = restored / 'gate.log'
                if variant == 'bytes':
                    path.write_bytes(b'changed')
                elif variant == 'mode':
                    path.chmod(0o700)
                elif variant == 'symlink':
                    path.unlink()
                    path.symlink_to(original / 'result.json')
                else:
                    config['gate_config']['source'] = '/different/source'
                with self.assertRaises(ValueError):
                    p.verify_restaging(g, restored, config)

    def test_exact_complete_gate_required(self):
        controller, record, config = inputs()
        p.prerequisite(g, controller, record, config)
        for mutate in (lambda c, r, f: r['phases'].pop(),
                       lambda c, r, f: r['phases'][0].update(exit_code=100),
                       lambda c, r, f: r.update(status='FAIL'),
                       lambda c, r, f: c.update(status='FAIL'),
                       lambda c, r, f: c.update(transport_exit_code=1),
                       lambda c, r, f: c.update(uncertainty=['transport']),
                       lambda c, r, f: r['phases'][0]['group_closure'].update(survivors=[{'pid': 10}]),
                       lambda c, r, f: r.update(jobs=3),
                       lambda c, r, f: f.update(source='/another/source'),
                       lambda c, r, f: c.update(container_after={'birth': 'changed'})):
            args = copy.deepcopy(inputs())
            mutate(*args)
            with self.assertRaises(ValueError):
                p.prerequisite(g, *args)

    def test_product_requires_no_live_groups_and_retains_dead_row_record(self):
        controller, record, config = inputs()
        dead = {'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z', 'terminated_evidence': {'status': 'terminated', 'complete': True, 'task_rows': [{'pid': 100, 'pgid': 100, 'start': 10, 'state': 'Z'}]}}
        record['phases'][0]['group_closure']['terminated_unreaped'] = [dead]
        p.prerequisite(g, controller, record, config)
        self.assertEqual(record['phases'][0]['group_closure']['terminated_unreaped'], [dead])
        dead['terminated_evidence']['task_rows'][0]['state'] = 'R'
        with self.assertRaises(ValueError):
            p.prerequisite(g, controller, record, config)

    def test_original_product_sequence_and_caps(self):
        seen = []
        self.assertTrue(p.product_sequence(lambda *args: seen.append(args) or 0, g.Closure(), ['installer']))
        self.assertEqual(seen, [('release-build', p.BUILD_COMMAND, 600), ('owned-install', ['installer'], 60)])

    def test_failed_build_does_not_install_cached_binary(self):
        seen = []
        self.assertFalse(p.product_sequence(lambda *args: seen.append(args) or 101, g.Closure(), ['installer']))
        self.assertEqual(len(seen), 1)

    def test_uncertain_zero_exit_does_not_install(self):
        seen, closure = [], g.Closure()
        def run(*args):
            seen.append(args)
            closure.uncertain('unclosed descendant')
            return 0
        with self.assertRaises(RuntimeError):
            p.product_sequence(run, closure, ['installer'])
        self.assertEqual(len(seen), 1)

    def test_actual_common_grant_validator_accepts_exact_output(self):
        validator = p.module(Path(__file__).parents[1] / 'docs/validation/m7-current-benchmark-controller.py', 'common_grant')
        config, record, artifacts, installation = grant_inputs()
        result = p.make_grant(g, config, record, artifacts, installation)
        self.assertIs(validator.validate_grant(result, 'a' * 7), result)
        self.assertEqual(result['execution'], config['execution'])
        self.assertEqual(result['execution_container_birth'], config['execution_container_birth'])
        self.assertTrue(result['installation_only'])
        self.assertFalse(result['source_modified'])
        self.assertEqual(set(result['artifacts']), {'sail', 'sail-z3'})

    def test_failed_partial_or_uncertain_product_cannot_make_grant(self):
        for mutate in (lambda c, r, a, i: r.update(status='FAIL'),
                       lambda c, r, a, i: r.update(uncertainty=['timeout']),
                       lambda c, r, a, i: r['phases'].pop(),
                       lambda c, r, a, i: r['phases'][0].update(exit_code=1),
                       lambda c, r, a, i: r['phases'][0].update(command=['copied-old-binary']),
                       lambda c, r, a, i: r['phases'][0].update(limit_seconds=900)):
            args = copy.deepcopy(grant_inputs())
            mutate(*args)
            with self.assertRaises(ValueError):
                p.make_grant(g, *args)

    def test_installation_identity_modes_and_bytes_are_mandatory(self):
        for mutate in (lambda i: i.update(source_modified=True),
                       lambda i: i.update(authority='proof-authority'),
                       lambda i: i['files']['sail'].update(sha256='e' * 64),
                       lambda i: i['files']['sail'].update(mode='0755'),
                       lambda i: i['files']['sail'].update(uid=1),
                       lambda i: i['files'].pop('sail-z3')):
            args = copy.deepcopy(grant_inputs())
            mutate(args[3])
            with self.assertRaises(ValueError):
                p.make_grant(g, *args)

    def test_built_image_must_be_regular_bounded_elf(self):
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            (target / 'release').mkdir()
            for name in ('sail', 'sail-z3'):
                (target / 'release' / name).write_bytes(b'\x7fELFactual')
            rows = p.built_artifacts(g, target)
            self.assertEqual(rows['sail']['sha256'], g.sha(target / 'release/sail'))
            (target / 'release/sail').write_bytes(b'not-an-ELF')
            with self.assertRaises(ValueError):
                p.built_artifacts(g, target)
            (target / 'release/sail').unlink()
            (target / 'release/sail').symlink_to(target / 'release/sail-z3')
            with self.assertRaises(ValueError):
                p.built_artifacts(g, target)

    def test_exact_selected_engine_thirteen_file_manifest_and_pin(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory)
            root, source = base / 'selected-variant', base / 'source'
            runtime = root / 'verus/source/target-verus/release'
            runtime.mkdir(parents=True)
            (source / 'engine/verus').mkdir(parents=True)
            files = {}
            for index in range(13):
                path = runtime / ('artifact-' + str(index))
                path.write_bytes(('actual-' + str(index)).encode())
                path.chmod(0o644)
                files[str(path.relative_to(root))] = {'bytes': path.stat().st_size, 'sha256': g.sha(path), 'mode': 0o644}
            pin = {'variant': root.name, 'patch_sha256': 'patch', 'source_lock_sha256': 'lock'}
            manifest = {'artifacts': {name: row['sha256'] for name, row in files.items()},
                        'features': ['sail-interactive:patch', 'host:x86_64-unknown-linux-gnu', 'source-lock:lock']}
            g.save(source / 'engine/verus/artifact-linux.json', pin)
            g.save(root / 'sail-build.json', manifest)
            expected = {'engine_build': str(root), 'manifest_sha256': g.sha(root / 'sail-build.json'), 'files': files}
            self.assertIs(p.engine_seal(g, source, expected), expected)
            extra = runtime / 'extra'
            extra.write_bytes(b'extra')
            with self.assertRaises(ValueError):
                p.engine_seal(g, source, expected)
            extra.unlink()
            short = copy.deepcopy(expected)
            short['files'].pop(next(iter(short['files'])))
            with self.assertRaises(ValueError):
                p.engine_seal(g, source, short)
            path = runtime / 'artifact-0'
            path.write_bytes(b'changed')
            with self.assertRaises(ValueError):
                p.engine_seal(g, source, expected)
            path.write_bytes(b'actual-0')
            pin['variant'] = 'other'
            g.save(source / 'engine/verus/artifact-linux.json', pin)
            with self.assertRaises(ValueError):
                p.engine_seal(g, source, expected)

    def test_shared_phase_runner_records_original_owner_log_and_checks(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = {'phases': []}
            owner = {'pid': 123, 'pgid': 123, 'start': 10}
            who = Mock()
            who.row.return_value = owner
            groups = Mock(leader=owner, known={123: owner}, partial=False, max_groups=256, first_refusal=None)
            who.Groups.return_value = groups
            check = Mock()
            runner = g.OwnedPhaseRunner(root, root, '/external/target', who, g.Closure(), record, check)
            child = Mock(pid=123)
            with patch.object(g.subprocess, 'Popen', return_value=child) as spawn, patch.object(g, 'run_owned', return_value=0), patch.object(g, 'group_closure', return_value={'survivors': [], 'unavailable': []}):
                self.assertEqual(runner.run('release-build', p.BUILD_COMMAND, 600), 0)
            self.assertEqual(check.call_count, 2)
            who.Groups.assert_called_once_with(owner)
            self.assertEqual(record["phases"][0]["ownership_group_budget"], 256)
            self.assertTrue(spawn.call_args.kwargs['start_new_session'])
            self.assertEqual(spawn.call_args.kwargs['env']['CARGO_BUILD_JOBS'], '2')
            self.assertEqual(record['phases'][0]['owner'], owner)
            self.assertEqual(record['phases'][0]['exit_code'], 0)
            self.assertEqual(record['phases'][0]['log_sha256'], g.sha(root / 'release-build.log'))
            runner.require_idle()

    def test_shared_phase_uncertainty_prevents_followup(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            who = Mock()
            who.row.return_value = {'pid': 123, 'pgid': 123, 'start': 10}
            who.Groups.return_value = Mock(known={}, partial=False, max_groups=256, first_refusal=None)
            closure = g.Closure()
            runner = g.OwnedPhaseRunner(root, root, '/target', who, closure, {'phases': []}, Mock())
            with patch.object(g.subprocess, 'Popen', return_value=Mock(pid=123)), patch.object(g, 'run_owned', return_value=0), patch.object(g, 'group_closure', return_value={'survivors': [{'pid': 123}], 'unavailable': []}):
                with self.assertRaises(RuntimeError):
                    runner.run('release-build', p.BUILD_COMMAND, 600)
            with self.assertRaises(RuntimeError):
                runner.require_idle()
    def test_partial_known_groups_still_refuse_even_if_only_dead_rows_remain(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            who = Mock()
            who.row.return_value = {'pid': 123, 'pgid': 123, 'start': 10}
            who.Groups.return_value = Mock(known={}, partial=True, max_groups=256, first_refusal={"reason": "group-budget-exhausted"})
            closure = g.Closure()
            runner = g.OwnedPhaseRunner(root, root, '/target', who, closure, {'phases': []}, Mock())
            audit = {'survivors': [], 'unavailable': [], 'terminated_unreaped': [{'pid': 123, 'state': 'Z'}]}
            with patch.object(g.subprocess, 'Popen', return_value=Mock(pid=123)), patch.object(g, 'run_owned', return_value=0), patch.object(g, 'group_closure', return_value=audit):
                with self.assertRaises(RuntimeError):
                    runner.run('release-build', p.BUILD_COMMAND, 600)
            self.assertTrue(closure.reasons)


if __name__ == '__main__':
    unittest.main()
