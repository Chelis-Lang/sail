#!/usr/bin/env python3
"""One sealed current-product publication diagnostic; no retries or acceptance."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import stat
import subprocess
import sys
import tarfile
import time

REPO = Path(__file__).resolve().parents[1]
common = runpy.run_path(str(REPO / 'docs/validation/m7-current-benchmark-controller.py'))
product = runpy.run_path(str(REPO / 'scripts/m7-native-product.py'))
require, sha, checkpoint = (common[name] for name in ('require', 'sha', 'checkpoint'))
READ_JSON = common['read_json']
FILES = ('scripts/m7-current-publication-controller.py',
         'scripts/m7-publication-timing-diagnostic.py', 'scripts/m7-native-product.py',
         'docs/validation/m7-current-benchmark-controller.py',
         'scripts/local-combined-gates.py', 'ci/test-repair.py')
FIXTURE_SHA = '9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7'
EXTERNAL_SECONDS = 300  # Includes original install/fetch, 120s scenario and cleanup.


def archive_sources(archive):
    """Bind only executed definitions; root's complete source seal stays separate."""
    rows = {}
    with tarfile.open(archive) as source:
        for name in FILES:
            member = source.getmember(name)
            require(member.isfile() and 0 < member.size <= 1024 * 1024,
                    'Executed source member is not a bounded regular file')
            with source.extractfile(member) as stream:
                data = stream.read(member.size + 1)
            require(len(data) == member.size, 'Source member size differs')
            rows[name] = hashlib.sha256(data).hexdigest()
    require(rows['ci/test-repair.py'] == FIXTURE_SHA, 'Original fixture changed')
    return rows


def native_seals(binding):
    grant = binding['grant']
    common['validate_grant'](grant, grant['source_commit'][:7])
    root = Path(grant['source_path'])
    who = product['identity']()
    require(who['fsuid'] == grant['installation']['credentials']['fsuid'],
            'Service identity differs from installed product')
    directory = Path(grant['product_dir'])
    metadata = directory.lstat()
    require(stat.S_ISDIR(metadata.st_mode) and stat.S_IMODE(metadata.st_mode) == 0o700,
            'Product directory is not private')
    files = {}
    for name, expected in grant['artifacts'].items():
        path = directory / name
        descriptor = product['open_regular'](path)
        try:
            actual = product['digest_fd'](descriptor, expected)
            info = os.fstat(descriptor)
            actual.update(uid=info.st_uid, gid=info.st_gid,
                          mode=format(stat.S_IMODE(info.st_mode), '04o'))
            require(actual == grant['installation']['files'][name], 'Product seal differs')
            files[str(path)] = actual
        finally:
            os.close(descriptor)
    for name, expected in binding['sources'].items():
        require(sha(root / name) == expected, 'Executed source differs from product archive')
    engine = Path(grant['engine_build'])
    manifest, digest = product['load_json'](engine / 'sail-build.json', 1024 * 1024)
    require(digest == binding['runtime_manifest_sha256'], 'Selected runtime manifest differs')
    artifacts = manifest['artifacts']
    require(len(artifacts) == 13, 'Runtime inventory is incomplete')
    runtime = {}
    for name, expected in artifacts.items():
        relative = Path(name)
        require(not relative.is_absolute() and '..' not in relative.parts,
                'Runtime artifact escapes engine')
        descriptor = product['open_regular'](engine / relative)
        try:
            info = os.fstat(descriptor)
            require(0 <= info.st_size <= product['MAX_FILE'], 'Runtime artifact exceeds bound')
            # Empty lock artifacts are valid runtime inventory members.
            actual = product['digest_fd'](descriptor, {'bytes': info.st_size, 'sha256': expected})
            runtime[name] = actual
        finally:
            os.close(descriptor)
    head = subprocess.check_output(['git', '-C', binding['verus_source'],
                                    'rev-parse', 'HEAD'], text=True, timeout=5).strip()
    require(head == binding['source_pin'], 'Fixture source pin differs')
    return {'product': files, 'sources': binding['sources'], 'runtime': runtime,
            'runtime_manifest_sha256': digest, 'verus_source_commit': head}


def native_execute(binding, output):
    output = Path(output)
    require(os.getpriority(os.PRIO_PROCESS, 0) == 0, 'Nonordinary priority')
    if os.getsid(0) != os.getpid():
        os.setsid()
    checkpoint(output / 'actor.json', common['process_identity'](os.getpid()))
    grant = binding['grant']
    source = Path(grant['source_path'])
    command = [sys.executable, str(source / 'scripts/m7-publication-timing-diagnostic.py'),
               '--fixture', str(source / 'ci/test-repair.py'),
               '--sail', grant['product_dir'] + '/sail',
               '--sail-sha256', grant['artifacts']['sail']['sha256'],
               '--engine-build', grant['engine_build'], '--verus-source', binding['verus_source'],
               '--out', str(output / 'diagnostic'), '--environment-label', grant['execution']]
    checkpoint(output / 'argv.json', command)
    os.execv(command[0], command)


def matching_terminal(value, grant):
    """Recognize the bound finally record for either a passing or failed fixture."""
    return (isinstance(value, dict)
            and value.get('kind') == 'original-repair-publication-timing-diagnostic'
            and type(value.get('original_fixture_passed')) is bool
            and value.get('qualification_credit') is False
            and value.get('original_limits') == [120, 30, 5]
            and value.get('fixture_sha256') == FIXTURE_SHA
            and value.get('product_sha256') == grant['artifacts']['sail']['sha256'])


def partial_capture_terminal(value, grant):
    """Bind the wrapper's expected exit one for passed fixture/partial timing."""
    captures = value.get('capture') if isinstance(value, dict) else None
    return (matching_terminal(value, grant)
            and value['original_fixture_passed'] is True
            and value.get('original_fixture_unchanged_after') is True
            and value.get('product_unchanged_after') is True
            and isinstance(captures, list) and 0 < len(captures) <= 1
            and all(isinstance(row, dict) and type(row.get('partial')) is bool for row in captures)
            and any(row['partial'] for row in captures))


def host(args):
    output = Path(args.out)
    require(output.is_absolute(), 'Output must be an absolute new owned path')
    output.mkdir(mode=0o700, exist_ok=False)
    record = {'version': 1, 'status': 'running', 'qualification_credit': False,
              'scope': 'One original current-product publication attribution diagnostic',
              'original_limits': [120, 30, 5], 'external_phase_envelope_seconds': EXTERNAL_SECONDS,
              'native_closure_certified': False, 'phases': []}
    started = time.monotonic()
    before = None
    attempted = False
    prepared = False
    checkpoint(output / 'controller.json', record)

    def phase(name, command, timeout=15, stdout=subprocess.PIPE):
        began = time.monotonic()
        row = {'name': name, 'command': command, 'timeout_seconds': timeout}
        try:
            with (output / (name + '.stderr')).open('wb') as errors:
                if stdout == subprocess.PIPE:
                    with (output / (name + '.stdout')).open('wb') as captured:
                        result = subprocess.run(command, stdout=captured, stderr=errors,
                                                timeout=timeout, check=False)
                else:
                    result = subprocess.run(command, stdout=stdout, stderr=errors,
                                            timeout=timeout, check=False)
            row['exit_code'] = result.returncode
            require(result.returncode == 0, name + ' failed')
            if stdout == subprocess.PIPE:
                with (output / (name + '.stdout')).open('rb') as captured:
                    data = captured.read(1024 * 1024 + 1)
                require(len(data) <= 1024 * 1024, 'Metadata response exceeds 1 MiB')
                return data
            return None
        except BaseException as error:
            row['error_type'] = type(error).__name__
            row['transport_uncertain'] = isinstance(
                error, (OSError, KeyboardInterrupt, subprocess.TimeoutExpired))
            raise
        finally:
            row['elapsed_seconds'] = time.monotonic() - began
            record['phases'].append(row)
            checkpoint(output / 'controller.json', record)

    try:
        require(sha(args.product_grant) == args.product_grant_sha256, 'Product record changed')
        grant = READ_JSON(args.product_grant)
        common['validate_grant'](grant, grant['source_commit'][:7])
        archive = Path(args.product_grant).with_name('source.tar')
        require(sha(archive) == grant['source_archive_sha256'], 'Product archive differs')
        binding = {'grant': grant, 'sources': archive_sources(archive),
                   'runtime_manifest_sha256': args.runtime_manifest_sha256,
                   'verus_source': args.verus_source, 'source_pin': args.source_pin}
        require(all(sha(REPO / name) == digest for name, digest in binding['sources'].items()),
                'Host definitions differ from product source')
        require(all(Path(path) != output and Path(path) not in output.parents for path in
                    (grant['source_path'], grant['product_dir'], grant['engine_build'], args.verus_source)),
                'Evidence output overlaps a protected input')
        container = grant['container']
        binding_path = output / 'binding.json'
        base = ['docker', 'exec', container, 'python3', grant['source_path'] + '/' + FILES[0],
                '--native-binding', str(binding_path), '--out', str(output)]
        before = common['container_identity'](container)
        if 'execution_container_birth' in grant:
            require(before == grant['execution_container_birth'], 'Installed container birth differs')
        record.update(source_commit=grant['source_commit'], product_grant_sha256=args.product_grant_sha256,
                      source_archive_sha256=grant['source_archive_sha256'], container_before=before)
        checkpoint(binding_path, binding)
        phase('create-output', ['docker', 'exec', container, 'mkdir', '-m', '700', str(output)])
        phase('copy-binding', ['docker', 'cp', str(binding_path), container + ':' + str(binding_path)], 30)
        prepared = True
        before_seals = json.loads(phase('before-seals', base + ['--native-mode', 'seals'], 60))
        record['before'] = before_seals
        attempted = True
        with (output / 'diagnostic.log').open('wb') as log:
            phase('original-diagnostic', base + ['--native-mode', 'execute'], EXTERNAL_SECONDS, log)
        record['status'] = 'DIAGNOSTIC_PASS'
    except BaseException as error:
        uncertain = isinstance(error, subprocess.TimeoutExpired) or (
            attempted and isinstance(error, (OSError, KeyboardInterrupt)))
        record.update(status='UNCERTAIN' if uncertain else 'FAIL',
                      error_type=type(error).__name__)
    finally:
        record['primary_status'] = record['status']
        # Every following failure is retained independently, never a hidden retry.
        if prepared:
            for name, command, budget in (
                    ('after-seals', base + ['--native-mode', 'seals'], 60),
                    ('native-record', ['docker', 'exec', container, 'cat', str(output / 'diagnostic/diagnostic.json')], 15),
                    ('closure', base + ['--native-mode', 'closure'], 15)) if attempted else (
                    ('after-seals', base + ['--native-mode', 'seals'], 60),):
                try:
                    value = json.loads(phase(name, command, budget))
                    checkpoint(output / (name + '.json'), value)
                    record[name] = value
                    if name == 'after-seals':
                        require(value == record.get('before'), 'Post-run seals differ')
                    elif name == 'native-record':
                        record['native_terminal_record_matches'] = matching_terminal(value, grant)
                        require(record['native_terminal_record_matches'],
                                'Native diagnostic does not match original bindings')
                        if record['status'] == 'DIAGNOSTIC_PASS':
                            require(value['original_fixture_passed'] is True,
                                    'Native fixture did not pass')
                    elif name == 'closure':
                        record['bounded_closure_observed'] = (
                            value.get('complete') is True and value.get('candidates') == [])
                        # A lost transport cannot certify native completion, even if a later scan is empty.
                        if record['status'] == 'DIAGNOSTIC_PASS':
                            require(record['bounded_closure_observed'], 'Bounded closure is incomplete')
                except BaseException as error:
                    record[name + '_error'] = type(error).__name__
                    if record['status'] == 'DIAGNOSTIC_PASS':
                        record['status'] = 'FAIL'
        if attempted:
            archive_complete = False
            try:
                # All files here are external controller metadata or the original
                # public/timing retention; scratch compiler/cache directories are elsewhere.
                with (output / 'native-raw.tar.gz').open('wb') as archive:
                    phase('raw-archive', ['docker', 'exec', container, 'tar', '-C', str(output),
                                         '-czf', '-', '.'], 60, archive)
                archive_complete = True
            except BaseException as error:
                record['archive_error'] = type(error).__name__
                if record['status'] == 'DIAGNOSTIC_PASS':
                    record['status'] = 'FAIL'
            finally:
                archive_path = output / 'native-raw.tar.gz'
                if archive_path.exists():
                    record['raw_archive'] = {'bytes': archive_path.stat().st_size,
                                             'sha256': sha(archive_path), 'complete': archive_complete}
        if before is not None:
            try:
                after = common['container_identity'](container)
                record['container_after'] = after
                require(after == before, 'Container birth changed')
            except BaseException as error:
                record['container_error'] = type(error).__name__
                record['status'] = 'UNCERTAIN'
        if attempted:
            reasons = []
            if any(row.get('transport_uncertain') is True for row in record['phases']):
                reasons.append('transport-or-interruption')
            if record.get('native_terminal_record_matches') is not True:
                reasons.append('native-terminal-unavailable')
            if record.get('bounded_closure_observed') is not True:
                reasons.append('native-closure-unavailable')
            original = next((row for row in record['phases']
                             if row['name'] == 'original-diagnostic'), {})
            code = original.get('exit_code')
            terminal = record.get('native-record')
            failed_fixture = (record.get('native_terminal_record_matches') is True
                              and terminal.get('original_fixture_passed') is False)
            partial = partial_capture_terminal(terminal, grant) if terminal is not None else False
            if code == 1 and partial:
                record['diagnostic_capture_partial'] = True
                record['diagnostic_outcome'] = 'passed-fixture-partial-capture'
                record['status'] = 'FAIL'
            if code not in (0, 1) or (code == 1 and not (failed_fixture or partial)):
                reasons.append('docker-exit-unconfirmed')
            if 'container_error' in record:
                reasons.append('container-identity-unavailable')
            record['uncertainty_reasons'] = reasons
            if reasons:
                record['status'] = 'UNCERTAIN'
        record['external_elapsed_seconds'] = time.monotonic() - started
        record['diagnostic_attempted'] = attempted
        record['native_directory_not_removed'] = prepared
        checkpoint(output / 'controller.json', record)
    return 0 if record['status'] == 'DIAGNOSTIC_PASS' else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True)
    parser.add_argument('--native-binding')
    parser.add_argument('--native-mode', choices=('seals', 'execute', 'closure'))
    for flag in ('product-grant', 'product-grant-sha256', 'runtime-manifest-sha256',
                 'verus-source', 'source-pin'):
        parser.add_argument('--' + flag)
    args = parser.parse_args()
    if args.native_mode:
        binding = READ_JSON(args.native_binding)
        if args.native_mode == 'execute':
            native_execute(binding, args.out)
        else:
            value = native_seals(binding) if args.native_mode == 'seals' else common['closure'](args.out, binding['grant']['source_path'])
            print(json.dumps(value), flush=True)
        return 0
    require(all(getattr(args, flag.replace('-', '_')) for flag in
                ('product-grant', 'product-grant-sha256', 'runtime-manifest-sha256',
                 'verus-source', 'source-pin')), 'Complete host binding required')
    def interrupted(_signum, _frame):
        raise KeyboardInterrupt('Owned host publication controller interrupted')
    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    return host(args)


if __name__ == '__main__':
    raise SystemExit(main())
