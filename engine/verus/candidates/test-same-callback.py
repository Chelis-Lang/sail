#!/usr/bin/env python3
"""Local-only same-callback raw oracle, including valid-hit corruption controls.

The owner selected this same-input byte oracle; stock verdicts and diagnostics
remain mandatory. Earlier independent-stock byte failures remain preserved.
No source build or installation occurs here; use a separate diagnostic variant.
"""
import argparse
import hashlib
import json
import os
import pathlib
import shutil
import signal
import subprocess


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def execute(command, environment, timeout, output, prefix, cwd=None):
    with subprocess.Popen(command, env=environment, cwd=cwd, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE, start_new_session=True) as process:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
            (output / (prefix + '.stdout')).write_bytes(stdout)
            (output / (prefix + '.stderr')).write_bytes(stderr)
            (output / (prefix + '.deadline.json')).write_text(json.dumps({
                'timed_out': True, 'deadline_seconds': timeout,
                'exit_code': process.returncode, 'stdout_bytes': len(stdout),
                'stderr_bytes': len(stderr)}, indent=2) + '\n')
            raise
        return subprocess.CompletedProcess(command, process.returncode, stdout, stderr)


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=False)
    source = root / 'lib.rs'
    source.write_text('use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! {\n'
                      'spec fn flag() -> bool { true }\n'
                      'spec fn another() -> bool { false }\n'
                      'proof fn selected() { assert(flag() && !another()); }\n'
                      'proof fn other() { assert(true); }\n}\nfn main() {}\n')
    records = []
    for index, fault in enumerate([None, '', 'order', 'body', 'effects']):
        work = root / str(index)
        work.mkdir()
        prepared = work / 'prepared'
        if index:
            shutil.copytree(root / '0/prepared', prepared)
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith('SAIL_ENGINE')}
        environment.update(VERUS_Z3_PATH=args.z3, RUSTUP_TOOLCHAIN=args.toolchain,
                           SAIL_ENGINE='interactive', SAIL_ENGINE_FUNCTION='demo::selected',
                           SAIL_ENGINE_CONTEXT='same-callback-control-v1:' + digest(pathlib.Path(args.engine).with_name('rust_verify')),
                           SAIL_ENGINE_PREPARED_DIR=str(prepared),
                           SAIL_ENGINE_EVENTS=str(work / 'events.jsonl'),
                           SAIL_ENGINE_FOCUS_REPORT=str(work / 'focus.json'))
        if fault is not None:
            environment.update(SAIL_ENGINE_ORACLE_DIR=str(work / 'oracle'),
                               SAIL_ENGINE_ORACLE_Z3=args.z3,
                               SAIL_ENGINE_ORACLE_FAULT=fault)
        command = [args.engine, str(source), '--no-vstd', '--crate-name', 'demo', '-V', 'use-crate-name',
                   '--output-json', '--time', '--no-report-long-running', '--log', 'air',
                   '--log', 'smt-transcript', '--log-dir', str(work / 'logs')]
        process = execute(command, environment, args.timeout, work, "engine")
        (work / 'stdout').write_bytes(process.stdout)
        (work / 'stderr').write_bytes(process.stderr)
        events = [json.loads(line) for line in (work / 'events.jsonl').read_text().splitlines()]
        summaries = [json.loads(path.read_text()) for path in sorted((work / 'oracle').glob('*/summary.json'))]
        actual_hits = sum(event['event'] == 'prepared-hit' for event in events)
        item = {'case': 'fill' if fault is None else fault or 'positive', 'exit_code': process.returncode,
                'actual_hits': actual_hits, 'oracle_summaries': summaries,
                'full': json.loads(process.stdout)['verification-results']}
        records.append(item)
        def save(completed=False):
            (root / 'summary.json').write_text(json.dumps({
                'version': 1,
                'scope': 'Owner-selected same-input raw-byte oracle; stock verdict and diagnostic controls mandatory; candidate unqualified',
                'all_controls_completed': completed,
                'engine_verifier_sha256': digest(pathlib.Path(args.engine).with_name('rust_verify')),
                'z3_sha256': digest(pathlib.Path(args.z3)), 'records': records}, indent=2) + '\n')
        save()
        if fault is None:
            assert process.returncode == 0, 'Cold fixture failed before oracle'
        elif fault == '':
            assert process.returncode == 0 and summaries and actual_hits > 0, 'Actual-hit positive oracle missing'
            assert all(summary['passed'] for summary in summaries), 'Fresh/restored raw oracle failed'
            assert any(summary['fresh_sessions'] > 0 for summary in summaries), 'Positive case has no actual solver session'
        else:
            assert process.returncode != 0 and actual_hits > 0 and summaries, 'Corruption must reach validated HIT then fail oracle'
            assert any(not summary['passed'] and summary['restored_hit'] for summary in summaries), 'Fault silently accepted'
            if fault in ['order', 'body']:
                assert any(not summary['ordered_wire_equal'] and summary['fresh_sessions'] > 0
                           for summary in summaries), 'Wrong lowering did not exercise actual wire comparison'
            else:
                assert any(not summary['lowering_effects_equal'] for summary in summaries), 'Unused effect corruption escaped comparison'
        # A positive shadow run must leave the actual producer's obligation
        # identities/ordering equal to the non-oracle same-source primary run.
        progress = [(event['phase'], event['payload']['function'],
                     event['payload']['obligation'], event['payload']['completed'])
                    for event in events if event['event'] == 'obligation-progress']
        if index == 0:
            fill_progress, fill_stderr, fill_document = progress, process.stderr, json.loads(process.stdout)
        if index == 1:
            assert progress == fill_progress, 'Shadow work changed primary obligation identities/order'
            assert process.stderr == fill_stderr, 'Shadow work changed primary diagnostics'
            assert json.loads(process.stdout)['func-details'] == fill_document['func-details']
            assert item['full'] == records[0]['full'], 'Shadow work changed authoritative full outcome'
        if index == 0:
            stock_env = {key: value for key, value in environment.items() if not key.startswith('SAIL_ENGINE')}
            stock_command = command.copy()
            stock_command[0] = args.stock
            stock_command[-1] = str(work / 'stock-logs')
            stock = execute(stock_command, stock_env, args.timeout, work, "stock")
            (work / 'stock.stdout').write_bytes(stock.stdout)
            (work / 'stock.stderr').write_bytes(stock.stderr)
            assert stock.returncode == process.returncode and stock.stderr == process.stderr
            stock_document = json.loads(stock.stdout)
            assert stock_document['verification-results'] == item['full']
            assert stock_document['func-details'] == fill_document['func-details']
            def verdicts(document):
                modules = document.get('times-ms', {}).get('smt', {}).get('smt-run-module-times', [])
                return sorted((entry['function'], entry['success']) for module in modules
                              for entry in module.get('function-breakdown', []))
            assert verdicts(stock_document) == verdicts(fill_document), 'Stock function verdicts differ'
            item['stock_verdicts_and_diagnostics_equal'] = True
        item['assertions_completed'] = True
        save(completed=index == 4)
        print(item['case'], 'passed control', 'hits', actual_hits, flush=True)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ['engine', 'stock', 'z3', 'out']:
        parser.add_argument('--' + flag, required=True)
    parser.add_argument('--toolchain', default='1.98.1')
    parser.add_argument('--timeout', type=int, default=600)
    run(parser.parse_args())
