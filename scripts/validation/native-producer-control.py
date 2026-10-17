"""Isolated native producer A→B→A controls; no performance/default acceptance."""
from pathlib import Path
import argparse
from collections import Counter
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import time

HOME = Path.home()
MANIFEST = "[package]\nname='boundary_sample'\nversion='0.1.0'\nedition='2021'\n[package.metadata.verus]\nverify=true\n[dependencies]\nvstd='=0.0.0-2026-10-04-0306'\n"
SOURCE = b'use vstd::prelude::*;\nverus! { pub fn identity(x: u64) -> (r: u64) ensures r == x { x } }\n'
CANARY = b'''\n#[doc(hidden)]\n#[proc_macro]\npub fn __sail_native_producer_fixture_canary(input: proc_macro::TokenStream) -> proc_macro::TokenStream { input }\n'''
FILE_LIMIT = 256 * 1024**2
TRACE_LIMIT = 64 * 1024**2


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def read(path, limit):
    before = path.lstat()
    assert stat.S_ISREG(before.st_mode) and before.st_size <= limit, str(path)
    with path.open('rb') as stream:
        raw = stream.read(limit + 1)
        held = os.fstat(stream.fileno())
    assert len(raw) <= limit and (before.st_dev, before.st_ino, before.st_size) == (held.st_dev, held.st_ino, len(raw))
    return raw


def sha(path):
    return hashlib.sha256(read(path, FILE_LIMIT)).hexdigest()


def text(value):
    if isinstance(value, str):
        return value
    assert isinstance(value, dict) and set(value) == {'Unix'}
    return bytes(value['Unix']).decode('utf-8')


def values(args, flag):
    result = []
    for index, arg in enumerate(args):
        if arg == flag:
            result.append(args[index + 1])
        elif arg.startswith(flag + '='):
            result.append(arg[len(flag) + 1:])
        elif flag == '-L' and arg.startswith('-L'):
            result.append(arg[2:])
    return result


def mutation(path, replacement, evidence):
    before = path.lstat()
    original = read(path, FILE_LIMIT)
    assert replacement != original
    evidence.mkdir(parents=True, exist_ok=True)
    (evidence / 'a').write_bytes(original)
    (evidence / 'b').write_bytes(replacement)
    path.chmod(stat.S_IMODE(before.st_mode) | stat.S_IWUSR)
    path.write_bytes(replacement)
    assert read(path, FILE_LIMIT) == replacement
    return path, original, before


def restore(changes):
    errors = []
    for path, original, before in reversed(changes):
        try:
            path.chmod(stat.S_IMODE(before.st_mode) | stat.S_IWUSR)
            path.write_bytes(original)
            path.chmod(stat.S_IMODE(before.st_mode))
            os.utime(path, ns=(before.st_atime_ns, before.st_mtime_ns))
            assert read(path, FILE_LIMIT) == original
        except BaseException as error:
            errors.append({'path': str(path), 'error': repr(error)})
    assert not errors, errors


def graph_pairs(ready):
    original = [text(x) for x in ready['original_args']]
    actual = [text(x) for x in ready['args']]
    left = values(original, '-L')
    right = values(actual, '-L')
    assert len(left) == len(right) and left
    return [(Path(a.removeprefix('dependency=')), Path(b.removeprefix('dependency=')))
            for a, b in zip(left, right) if a.startswith('dependency=') and b.startswith('dependency=')]


def choose_transitive(ready, crate_name):
    original = [text(x) for x in ready['original_args']]
    explicit = {x.split('=', 1)[0] for x in values(original, '--extern')}
    assert crate_name not in explicit, 'selected crate must be transitive, not explicit --extern'
    selected = {}
    for public, private in graph_pairs(ready):
        assert public != private and '.sail-boundary-native-reads' in str(private)
        for source in public.glob('lib' + crate_name + '-*'):
            if source.suffix not in {'.rlib', '.rmeta'}:
                continue
            copied = private / source.name
            assert copied.is_file() and not copied.is_symlink()
            assert not os.path.samefile(source, copied) and sha(source) == sha(copied)
            selected[str(source)] = {'public': source, 'private': copied, 'sha256': sha(copied)}
    assert selected and any(x['public'].suffix == '.rlib' for x in selected.values())
    return list(selected.values())


def trace_proof(case, compiler_pid, selected):
    files = sorted((case / 'strace').glob('native.*'))
    assert files and len(files) <= 4096
    total = sum(path.stat().st_size for path in files)
    assert total <= TRACE_LIMIT, 'complete raw strace bound'
    traces = {int(path.name.rsplit('.', 1)[1]): read(path, TRACE_LIMIT).decode('utf-8') for path in files}
    assert compiler_pid in traces, 'actual owned compiler PID trace missing'
    owned = {compiler_pid}
    pending = [compiler_pid]
    while pending:
        pid = pending.pop()
        for value in re.findall(r'(?:clone|clone3|fork|vfork)\(.*\)\s*=\s*(\d+)\s*$', traces.get(pid, ''), re.M):
            child = int(value)
            if child not in owned:
                owned.add(child)
                pending.append(child)
    opens = [line for pid in owned for line in traces.get(pid, '').splitlines()
             if re.search(r'\bopenat(?:2)?\(', line) and re.search(r'=\s*\d+\s*$', line)]
    private = [str(value['private']) for value in selected]
    public = [str(value['public']) for value in selected]
    actual_private = [line for line in opens if any(json.dumps(path) in line for path in private)]
    actual_public = [line for line in opens if any(json.dumps(path) in line for path in public)]
    assert actual_private and not actual_public, {'private': actual_private, 'public': actual_public}
    return {'actual_compiler_pid': compiler_pid, 'owned_trace_pids': sorted(owned),
            'private_a_successful_opens': actual_private, 'public_b_successful_opens': actual_public,
            'raw_trace_bytes': total}


def run_case(options, label):
    case = options.out / label
    case.mkdir()
    project = case / 'project'
    (project / 'src').mkdir(parents=True)
    (project / 'Cargo.toml').write_text(MANIFEST)
    (project / 'src/lib.rs').write_bytes(SOURCE)
    shutil.copy2(options.lock, project / 'Cargo.lock')
    control = case / 'control'
    control.mkdir(mode=0o700)
    cache = case / 'cache'
    bundle = case / 'bundle'
    environment = dict(os.environ, SAIL_BOUNDARY_TRACE_ROOT=str(case / 'solver-traces'),
                       SAIL_CARGO_VERUS=str(options.runtime / 'cargo-verus'),
                       SAIL_CACHE_DIR=str(cache), SAIL_VSTD_BOUNDARY_RESEARCH='1',
                       SAIL_VSTD_BOUNDARY_LOCAL=str(bundle), SAIL_SHIPPED_TRACE=str(case / 'trace'),
                       SAIL_BOUNDARY_INPUT_FIXTURE=str(control),
                       SAIL_NATIVE_BOUNDARY_PACKAGE=options.package, CARGO_BUILD_JOBS='1')
    command = [str(options.sail), 'verify', '--output', 'json']
    if label == 'transitive-aba':
        (case / 'strace').mkdir()
        command = [str(options.strace), '-ff', '-qq', '-s', '4096', '-o', str(case / 'strace/native'),
                   '-e', 'trace=execve,execveat,openat,openat2,clone,clone3,fork,vfork', *command]
    row = {'label': label, 'qualified': False, 'errors': [], 'command': command}
    process = None
    changes = []
    ready = None
    completed = None
    selected = []
    handled = set()
    started = time.monotonic()
    try:
        with (case / 'stdout').open('xb') as stdout, (case / 'stderr').open('xb') as stderr:
            process = subprocess.Popen(command, cwd=project, env=environment, stdout=stdout,
                                       stderr=stderr, start_new_session=True)
            while process.poll() is None:
                assert time.monotonic() - started < options.case_seconds, 'case deadline'
                assert all((case / x).stat().st_size <= 16 * 1024**2 for x in ['stdout', 'stderr'])
                assert sum(p.stat().st_size for p in (case / 'strace').glob('native.*')) <= TRACE_LIMIT
                for path in sorted(control.glob('ready-*.json')):
                    if path in handled:
                        continue
                    assert ready is None, 'multiple selected native producer invocations'
                    ready = json.loads(read(path, 1024**2))
                    assert ready['kind'] == 'sail-native-producer-spawn-fixture' and ready['schema'] == 1
                    assert ready['package'] == options.package
                    args = [text(x) for x in ready['original_args']]
                    row['ready'] = ready
                    if label == 'source-aba':
                        sources = [Path(x) if Path(x).is_absolute() else Path(ready['cwd']) / x
                                   for x in args if x.endswith('.rs')]
                        assert len(sources) == 1
                        source = sources[0]
                        original = read(source, 8 * 1024**2)
                        assert b'__sail_native_producer_fixture_canary' not in original
                        policy = json.loads(read(options.policy, 8 * 1024**2))
                        admitted = [(package, source.relative_to(parent).as_posix())
                                    for parent in source.parents
                                    if (package := policy['packages'].get(parent.name)) is not None]
                        assert len(admitted) == 1
                        package, relative = admitted[0]
                        expected = package['compiler_files'][relative]
                        assert package['files'][relative] == hashlib.sha256(original).hexdigest()
                        assert expected['size'] == len(original)
                        changes.append(mutation(source, original + CANARY, case / 'source-change'))
                        row['source'] = {'path': str(source), 'a_sha256': hashlib.sha256(original).hexdigest(),
                                         'b_sha256': sha(source), 'admitted_a': expected, 'a_size': len(original), 'b_size': len(original + CANARY)}
                    elif label == 'transitive-aba':
                        selected = choose_transitive(ready, options.transitive)
                        row['selected_graph'] = [{'public': str(x['public']), 'private': str(x['private']), 'a_sha256': x['sha256']} for x in selected]
                        for index, value in enumerate(selected):
                            source = value['public']
                            original = read(source, FILE_LIMIT)
                            prefix = b'INVALID-SAIL-NATIVE-FIXTURE-B\n'
                            assert len(original) > len(prefix)
                            changes.append(mutation(source, prefix + original[len(prefix):], case / 'artifact-change' / str(index)))
                            assert sha(value['private']) == value['sha256'] and sha(source) != value['sha256']
                    save(case / 'progress.json', row)
                    (control / ('continue-' + str(ready['pid']))).write_bytes(b'continue\n')
                    handled.add(path)
                for path in sorted(control.glob('completed-*.json')):
                    if path in handled:
                        continue
                    assert ready is not None and completed is None
                    completed = json.loads(read(path, 1024**2))
                    assert completed['kind'] == 'sail-native-producer-completed-fixture' and completed['pid'] == ready['pid']
                    row['completed'] = completed
                    overrides = {key: text(value) for key, value in ready['environment_overrides'].items() if value is not None}
                    native_document_path = Path(overrides['SAIL_NATIVE_COMPILER_INPUTS']) / 'inputs.json'
                    raw = read(native_document_path, 8 * 1024**2)
                    (case / 'native-inputs.json').write_bytes(raw)
                    document = json.loads(raw)
                    assert document['kind'] == 'sail-native-compiler-read-inputs' and document['complete']
                    assert document['pid'] == completed['compiler_pid'] and document['binding'] == overrides['SAIL_NATIVE_COMPILER_INPUTS_BINDING']
                    if label == 'source-aba':
                        observed = [v for name, v in document['inputs']['sources'].items()
                                    if (Path(name) if Path(name).is_absolute() else Path(ready['cwd']) / name).resolve() == Path(row['source']['path']).resolve()]
                        assert len(observed) == 1 and observed[0]['size'] == row['source']['b_size']
                        assert observed[0]['checksum'] != row['source']['admitted_a']['checksum']
                        row['source']['actual_compiler_retained_b'] = observed[0]
                    if label == 'transitive-aba':
                        assert all(sha(x['private']) == x['sha256'] for x in selected)
                    restore(changes)
                    changes.clear()
                    row['restored_before_producer_completion'] = True
                    save(case / 'progress.json', row)
                    (control / ('publish-' + str(completed['pid']))).write_bytes(b'continue\n')
                    handled.add(path)
                time.sleep(0.005)
            row['exit'] = process.returncode
    except BaseException as error:
        row['errors'].append(repr(error))
    finally:
        # On failure stop the owned process tree before restoring its inputs.
        # Successful cases restore above, after actual wait and before complete.
        if process is not None and process.poll() is None:
            try:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=5)
            except BaseException as error:
                row['errors'].append('owned cleanup ' + repr(error))
        try:
            restore(changes)
        except BaseException as error:
            row['errors'].append('restore ' + repr(error))
        if process is not None:
            row['exit'] = process.returncode
        row['seconds'] = time.monotonic() - started
        save(case / 'result.json', row)
    try:
        assert not row['errors'] and ready is not None and completed is not None, row['errors']
        assert completed['exit'] == 0, 'actual selected native child must succeed'
        records = []
        for path in cache.glob('projects/*/compiler-artifacts/*/target/.sail-boundary-native/*/*.json'):
            record = json.loads(read(path, 1024**2))
            if record.get('compiler_pid') == completed['compiler_pid']:
                records.append(record)
        save(case / 'selected-producer-records.json', records)
        row['selected_producer_records'] = len(records)
        if label == 'source-aba':
            assert not records, 'B-source compiler must not establish an A-source producer'
        else:
            assert records and all(record['crate_name'] == options.package.replace('-', '_') for record in records)
            assert row['exit'] == 0, 'positive project must complete'
            project_report = json.loads(read(case / 'stdout', 16 * 1024**2))
            assert project_report['currentness'] == 'current' and all(x['complete'] for x in project_report['targets'])
            row['per_target_reports'] = outcome_records(project_report)
            row['literal_solver_inputs'] = trace_inventory(case)
        if label == 'transitive-aba':
            row['trace_proof'] = trace_proof(case, completed['compiler_pid'], selected)
        assert (project / 'src/lib.rs').read_bytes() == SOURCE
        row['qualified'] = True
    except BaseException as error:
        row['errors'].append(repr(error))
    save(case / 'result.json', row)
    return row




def outcome_records(document):
    # Retain every function field except its explicitly measured timing. Lists
    # preserve repeated names; no dictionary join may collapse implementations.
    targets = []
    for target in document['targets']:
        report = target['report']
        crates = []
        for crate in report['crates']:
            functions = [{key: value for key, value in function.items() if key != 'timing'}
                         for function in crate['functions']]
            crates.append({'name': crate['name'], 'verus': crate['verus'], 'functions': functions})
        targets.append({'target': target['target'], 'kind': target['kind'],
                        'requested': target['requested'], 'complete': target['complete'],
                        'crates': crates, 'diagnostics': report['diagnostics'],
                        'exit_code': report['exit_code'], 'error': report['error']})
    return sorted(targets, key=lambda value: (value['target'], value['kind']))

def trace_inventory(case):
    root = case / 'solver-traces'
    files = list(root.iterdir())
    assert files and len(files) <= 12000
    assert sum(path.lstat().st_size for path in files) <= 256 * 1024**2
    inputs = sorted(root.glob('*.in.smt2'))
    assert inputs
    expected = set()
    for path in inputs:
        identity = path.name.removesuffix('.in.smt2')
        metadata = root / (identity + '.jsonl')
        output = root / (identity + '.out')
        events = [json.loads(line) for line in read(metadata, 1024**2).splitlines()]
        end = events[-1]
        assert end['type'] == 'end' and end['complete'] and end['paired'] and end['exit_code'] == 0
        read(output, FILE_LIMIT)
        expected.update([path, metadata, output])
    assert set(files) == expected, 'unaccounted raw solver trace member'
    counts = dict(sorted(Counter(sha(path) for path in inputs).items()))
    value = {'streams': len(inputs), 'sha256_multiplicities': counts}
    save(case / 'literal-inputs.json', value)
    return value


def install_trace_adapter(bins):
    original = bins / 'sail-z3'
    real = bins / 'sail-z3-fixture-real'
    assert not real.exists()
    original_body = read(original, FILE_LIMIT)
    original_mode = stat.S_IMODE(original.stat().st_mode)
    adapter = b'#!/bin/sh\nunset SAIL_DAEMON_SOCKET\nexport SAIL_TRACE_DIR="$SAIL_BOUNDARY_TRACE_ROOT"\nexec "$(dirname "$0")/sail-z3-fixture-real" "$@"\n'
    original.rename(real)
    try:
        original.write_bytes(adapter)
        original.chmod(0o755)
    except BaseException:
        original.unlink(missing_ok=True)
        real.rename(original)
        raise
    return {'real_sha256': hashlib.sha256(original_body).hexdigest(),
            'real_mode': original_mode, 'adapter_sha256': sha(original)}


def restore_trace_adapter(bins, identity):
    original = bins / 'sail-z3'
    real = bins / 'sail-z3-fixture-real'
    assert sha(real) == identity['real_sha256'] and sha(original) == identity['adapter_sha256']
    original.unlink()
    real.rename(original)
    assert sha(original) == identity['real_sha256'] and stat.S_IMODE(original.stat().st_mode) == identity['real_mode']

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sail', type=Path, default=HOME / 'producer-evidence/fixture-binaries/sail')
    parser.add_argument('--runtime', type=Path, default=HOME / 'candidate-package/verus/source/target-verus/release')
    parser.add_argument('--lock', type=Path, default=HOME / 'project.lock')
    parser.add_argument('--policy', type=Path, default=HOME / 'sail-fixture/engine/verus/vstd-boundary-inputs.json')
    parser.add_argument('--out', type=Path, default=HOME / 'producer-evidence/application')
    parser.add_argument('--strace', type=Path, default=Path('/usr/bin/strace'))
    parser.add_argument('--package', default='verus_builtin_macros')
    parser.add_argument('--transitive', default='unicode_ident')
    parser.add_argument('--case-seconds', type=int, default=300)
    options = parser.parse_args()
    assert 1 <= options.case_seconds <= 300
    options.out.mkdir(mode=0o700)
    rows = []
    adapter = None
    try:
        adapter = install_trace_adapter(options.sail.parent)
        save(options.out / 'trace-adapter.json', adapter)
        for label in ['clean-a', 'source-aba', 'transitive-aba']:
            row = run_case(options, label)
            rows.append(row)
            assert row['qualified'], row['errors']
        assert rows[0]['per_target_reports'] == rows[2]['per_target_reports'], 'clean/protected target/function/verdict/diagnostic mismatch'
        assert rows[0]['literal_solver_inputs'] == rows[2]['literal_solver_inputs']
        save(options.out / 'literal-query-comparison.json', {'equal': True, 'policy': 'Literal complete SMT input SHA256 multiset, with every stream retained; no filtering or normalization'})
    finally:
        restored = False
        try:
            if adapter is not None:
                restore_trace_adapter(options.sail.parent, adapter)
            restored = True
        finally:
            save(options.out / 'summary.json', {'scope': 'Isolated native producer correctness controls; no default/performance qualification',
                                              'adapter_restored': restored,
                                              'passed': restored and len(rows) == 3 and all(row['qualified'] for row in rows) and (options.out / 'literal-query-comparison.json').is_file(), 'rows': rows})
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
