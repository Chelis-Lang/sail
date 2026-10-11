#!/usr/bin/env python3
"""One separately reviewed diagnostic replay; never original qualification credit."""
import argparse
from collections import Counter
import hashlib
from importlib.machinery import SourceFileLoader
import json
import os
from pathlib import Path
import time

PATCH_SHA = '3892328a418109596f6a4a5f3649f5e7d370b19321419c29780d7a3961b44d61'
MANIFEST_SHA = 'acae733747b439a5a7777d446e88f4d25c1e855d015420806c7d0f7456c89e9f'
SOURCE_INVENTORY_SHA = 'dd10bf6fe593a617e71174de432429d6ba9a4eb49974b0c67377bb84988decf5'
BUILD = Path('/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-b3b07d47d7542817')
EVIDENCE = Path('/tmp/sail-m7-preparation-namespace-visibility-build-evidence')
DIAGNOSTIC = Path('/tmp/sail-m7-preparation-namespace-boundary-diagnostic-build')
CONTROLS = Path('/tmp/sail-m7-preparation-namespace-boundary-raw-owner-source-controls')
SOURCE = Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source')
SCRIPTS = Path('/tmp/sail-m7-combined-c39-evidence/qualification-scripts/engine/verus/candidates')


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(chunk)
    return digest.hexdigest()


def source_inventory(source):
    files = {str(p.relative_to(source)): sha(p) for p in sorted(source.rglob('*')) if p.is_file()}
    assert len(files) == 265
    assert hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode()).hexdigest() == SOURCE_INVENTORY_SHA
    return files


def seal():
    build = json.loads((DIAGNOSTIC / 'build.json').read_text())
    assert build['status'] == 'PASS' and build['exit_code'] == 0
    image = json.loads((DIAGNOSTIC / 'diagnostic-image.json').read_text())
    assert image['unsealed'] is True and image['qualified'] is False and image['copied_receipts'] == 0
    manifest = json.loads((BUILD / 'sail-build.json').read_text())
    assert sha(BUILD / 'sail-build.json') == MANIFEST_SHA == image['donor_manifest_sha256']
    assert manifest['artifacts'] == image['donor_artifacts']
    runtime = BUILD / 'verus/source/target-verus/release'
    assert {str(p.relative_to(BUILD)) for p in runtime.iterdir() if p.is_file()} == set(manifest['artifacts'])
    assert len(manifest['artifacts']) == 13
    assert all(not (BUILD / name).is_symlink() and sha(BUILD / name) == digest for name, digest in manifest['artifacts'].items())
    diagnostic_runtime = DIAGNOSTIC / 'runtime'
    assert {p.name for p in diagnostic_runtime.iterdir()} == set(image['diagnostic_artifacts'])
    assert len(image['diagnostic_artifacts']) == 13
    assert all(not (diagnostic_runtime / name).is_symlink() and sha(diagnostic_runtime / name) == digest for name, digest in image['diagnostic_artifacts'].items())
    assert all(image['diagnostic_artifacts'][Path(name).name] == digest for name, digest in manifest['artifacts'].items() if Path(name).name != 'rust_verify')
    identity = json.loads((CONTROLS / 'identity.json').read_text())
    assert sha(CONTROLS / 'full.patch') == PATCH_SHA == identity['full_patch_sha256']
    assert sha(CONTROLS / 'result.json') == '6cf37325c65a5c7607fd79625801973b3f7ccce8647364d7641b73dfecfee62a'
    assert all(sha(SOURCE / name) == digest for name, digest in identity['source_sha256'].items())
    assert sha(SOURCE / 'Cargo.lock') == identity['lock_sha256']
    prior = json.loads((DIAGNOSTIC / 'prior-runtime-after.json').read_text())
    for variant, record in prior.items():
        root = BUILD.parent / ('426d8b01e7ffb910a36c15e1f870ff985bb61eef-' + variant)
        assert sha(root / 'sail-build.json') == record['manifest_sha256']
        sealed = root / 'verus/source/target-verus/release'
        assert {str(p.relative_to(root)) for p in sealed.iterdir() if p.is_file()} == set(record['artifacts'])
        assert all(sha(root / name) == digest for name, digest in record['artifacts'].items())
        assert all(sha(root / 'verus/source' / name) == digest for name, digest in record['source_sha256'].items())
        assert sha(root / 'verus/source/Cargo.lock') == record['lock_sha256']
    return {'donor_manifest_sha256': MANIFEST_SHA, 'donor_artifacts': manifest['artifacts'],
            'diagnostic_artifacts': image['diagnostic_artifacts'], 'diagnostic_driver_sha256': image['driver_sha256'],
            'source_sha256': identity['source_sha256'], 'source_lock_sha256': identity['lock_sha256'],
            'prior_runtime_seals': prior}


def active_owned(output):
    # Compiler children inherit these exact selectors even in separate sessions.
    # Only match ownership; do not retain process environment values or signal.
    bound = []
    for path in Path('/proc').iterdir():
        if not path.name.isdigit() or int(path.name) == os.getpid():
            continue
        try:
            raw = (path / 'stat').read_text()
            fields = raw[raw.rfind(')') + 2:].split()
            if fields[0] == 'Z':
                continue
            command = (path / 'cmdline').read_bytes()
            environment = (path / 'environ').read_bytes()
            owned = str(output).encode()
            matches = owned in command or any(
                item.startswith((b'SAIL_ENGINE_EVENTS=', b'SAIL_ENGINE_PREPARED_DIR=',
                                 b'SAIL_ENGINE_ORACLE_DIR=', b'SAIL_ENGINE_FOCUS_REPORT='))
                and owned in item for item in environment.split(b'\0'))
            if matches:
                bound.append({'pid': int(path.name), 'starttime_ticks': int(fields[19]),
                              'pgid': int(fields[2]), 'state': fields[0]})
        except FileNotFoundError:
            pass  # An actor exited between this read-only bracket.
    return bound


def run(output):
    assert os.getpriority(os.PRIO_PROCESS, 0) == 0
    terminal = json.loads((EVIDENCE / 'original-qualification-actor.json').read_text())
    assert terminal['status'] == 'failed' and terminal['exit_code'] == 1
    assert sha(EVIDENCE / 'original-qualification.log') == terminal['log_sha256'] == '534589717a91971966c3f5633a8915908389e3ad14e2dc02f9172be9963d5119'
    assert sha(SCRIPTS / 'test-private-span-cross-edit.py') == '80b49c69b84599b606f200157c67cdc1f92e73054905f3a22b11ce839789fc2a'
    helper = SCRIPTS / 'test-same-callback.py'
    assert sha(helper) == '6cd4d5d18efd8d751de89b7c36dec04c3d26ed11f912cac3a0d9ca9050237295'
    assert sha(SCRIPTS / 'actor-observation.py') == 'd01de79ee9de89b823d7ccfb0efa1474a2ed4185f519bffd925e8546895edc34'
    controls = SourceFileLoader('frozen_same_callback', str(helper)).load_module()
    original = EVIDENCE / 'original-qualification/previous-revision-focus'
    source = original / 'project/tasks'
    inventory = source_inventory(source)
    before = seal()
    output.mkdir(parents=True, exist_ok=False)
    # Fresh owned preparations and oracle output; no receipt or result copying.
    prepared = output / 'prepared'
    prepared.mkdir()
    environment = {k: v for k, v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
    environment.update(RUST_TEST_THREADS='1', RUSTC_BOOTSTRAP='1', VERUS_IN_VARGO='1',
                       CARGO_HOME='/opt/cargo', RUSTUP_HOME='/opt/rustup',
                       VERUS_Z3_PATH='/opt/verus/verus-x86-linux/z3', RUSTUP_TOOLCHAIN='1.98.1',
                       SAIL_ENGINE='interactive',
                       SAIL_ENGINE_FUNCTION='human_eval_verus::human_eval_013::compute_gcd',
                       SAIL_ENGINE_CONTEXT='private-span-cross-edit-v1:d34ba8cdda91b365aedda1317e52c92a8a50720338cd6c68c9f368cdd73de81e',
                       SAIL_ENGINE_PREPARED_DIR=str(prepared), SAIL_ENGINE_EVENTS=str(output / 'events.jsonl'),
                       SAIL_ENGINE_FOCUS_REPORT=str(output / 'focus.json'), SAIL_ENGINE_ORACLE_DIR=str(output / 'oracle'),
                       SAIL_ENGINE_ORACLE_Z3='/opt/verus/verus-x86-linux/z3', SAIL_ENGINE_IMPORT_TRACE='1')
    assert 'SAIL_ENGINE_IMPORT_TRACE_TARGET' not in environment
    command = [str(DIAGNOSTIC / 'runtime/verus'), str(source / 'lib.rs'),
               '--crate-type', 'lib', '--crate-name', 'human_eval_verus', '-V', 'use-crate-name',
               '--verify-only-module', 'human_eval_013', '--output-json', '--time',
               '--no-report-long-running', '--log', 'air', '--log', 'smt-transcript',
               '--log-dir', str(output / 'engine')]
    record = {'version': 1, 'scope': 'Independent trace-only driver, exact failed revision0 input/context; no runtime/source qualification or acceptance credit',
              'status': 'running', 'selected': False, 'product_qualified': False, 'compiler_limit_seconds': 600,
              'command': command, 'trace': {'opt_in': '1', 'target_filter': None, 'max_records': 4096,
              'max_published_bytes': 4194304, 'max_complete_record_bytes': 32768},
              'source_inventory_sha256': SOURCE_INVENTORY_SHA, 'source_files': inventory,
              'pre_run_seal': before, 'context_is_retained_original_literal_not_diagnostic_driver_identity': True, 'controller_sha256': sha(Path(__file__)),
              'started_unix_ns': time.time_ns(), 'nice': 0, 'original_failure_log_sha256': terminal['log_sha256']}
    def save():
        (output / 'diagnostic.json').write_text(json.dumps(record, indent=2) + '\n')
    save()
    started = time.monotonic()
    try:
        process = controls.execute(command, environment, 600, output, 'engine', diagnostics=True)
        (output / 'engine.stdout').write_bytes(process.stdout)
        (output / 'engine.stderr').write_bytes(process.stderr)
        record.update(status='completed', exit_code=process.returncode,
                      stderr_equal_to_original=process.stderr == (original / '0/engine.stderr').read_bytes())
        events = [json.loads(line) for line in (output / 'events.jsonl').read_text().splitlines()]
        states = [e for e in events if e['event'] == 'import-authority-state']
        failures = [e for e in events if e['event'] == 'preparation-namespace-refused']
        stock=json.loads((original/'0/stock.stdout').read_text()); engine=json.loads(process.stdout)
        checks=[json.loads(p.read_text()) for p in sorted((output/'oracle').glob('*/summary.json'))]
        hits=sum(e['event']=='prepared-hit' for e in events)
        predicates={
            'stock_engine_exit0':process.returncode==0,
            'exact_stock_diagnostics':process.stderr==(original/'0/stock.stderr').read_bytes(),
            'module_verdict_equal':stock['verification-results']==engine['verification-results'],
            'function_verdict_equal':stock['func-details']==engine['func-details'],
            'checked_positive_hit':hits==len(checks) and hits>0,
            'same_input_checks':all(c['passed'] and c['restored_hit'] and c['same_callback'] for c in checks),
            'normal_terminal':events[-1]['event']=='compiler-sealed' and events[-1]['payload']['normal_termination'] is True,
        }
        record['original_revision0_predicates']=predicates
        record.update(event_counts=dict(sorted(Counter(e['event'] for e in events).items())),
                      import_state_records=len(states), namespace_boundary_records=failures,
                      first_ineligible_import_state=next((e for e in states if not e['payload']['eligible']), None),
                      observer_unavailable=[e for e in events if e['event'] == 'observer-unavailable'],
                      observation_limit='Main exact helper failure and correlated worker span identity are failure-only, before provenance. Service admission failures are separate. Frozen classification is not fresh authority; no source loading/clone or validation change.')
        save()
        assert all(predicates.values()), 'unchanged original revision0 predicates did not all pass; diagnostic only'
    except BaseException as error:
        record.update(status='failed', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        # Preserve the primary diagnostic outcome before independent seal/owner checks.
        record['elapsed_seconds'] = time.monotonic() - started
        save()
        after = seal()
        assert before == after and source_inventory(source) == inventory
        actors = active_owned(output)
        closure = {'version': 1, 'scope': 'Independent post-diagnostic runtime/source and present owned actor closure',
                   'same_pre_post_seals': True, 'source_inventory_unchanged': True,
                   'post_run_seal': after, 'active_bound_processes': actors,
                   'primary_diagnostic_sha256': sha(output / 'diagnostic.json')}
        (output / 'closure.json').write_text(json.dumps(closure, indent=2) + '\n')
        assert not actors, actors


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', required=True)
    run(Path(parser.parse_args().out).absolute())
