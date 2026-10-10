#!/usr/bin/env python3
"""Retain native CI managed acceptance without local paths or environments.

Pass downloaded managed evidence, run metadata from `gh run view --json
headSha,url,jobs`, and the corresponding complete workflow log.
"""
import argparse
from datetime import datetime
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('reports', type=Path)
parser.add_argument('run', type=Path)
parser.add_argument('log', type=Path)
args = parser.parse_args()
run = json.loads(args.run.read_text())
required = {'No AI authorship markers', 'Workspace gate', 'Solver tests',
            'End to end', 'Conformance', 'CI completion'}
jobs = {job['name']: job for job in run['jobs']}
assert required <= jobs.keys()
assert {'End to end (legacy)', 'End to end (managed)'} <= jobs.keys()
assert all(job['conclusion'] == 'success' for job in jobs.values())
assert 'managed acceptance: cold, unchanged, CLI/MCP jobs, missing receipt, and restored-mtime failing edit passed' in args.log.read_text()

records = []
for path in sorted(args.reports.glob('managed-*.stdout.json')):
    raw = path.read_bytes()
    document = json.loads(raw)
    selected = [target for target in document['targets'] if target['requested']]
    assert len(selected) == 1 and document['error'] is None
    records.append({
        'label': path.stem.removesuffix('.stdout'),
        'report_sha256': hashlib.sha256(raw).hexdigest(),
        'stderr_sha256': hashlib.sha256(path.with_name(path.name.replace('.stdout.json', '.stderr')).read_bytes()).hexdigest(),
        'currentness': document['currentness'], 'exit_code': document['exit_code'],
        'execution': document['execution'],
        'targets': [{key: target[key] for key in ('target', 'kind', 'requested', 'complete', 'reused')}
                    | {'summary': target['report']['summary']} for target in document['targets']],
    })
assert len(records) in (4, 5)
cold, warm, refill, failed = records[0], *records[-3:]
assert cold['exit_code'] == 0 and cold['execution']['verifier_invocations'] == 2
assert warm['currentness'] == 'current' and warm['exit_code'] == 0
assert warm['execution']['verifier_invocations'] == 0 and warm['execution']['reused_targets'] == 2
assert all(target['complete'] and target['reused'] for target in warm['targets'])
assert refill['exit_code'] == 0 and refill['execution']['verifier_invocations'] == 1
assert refill['execution']['reused_targets'] == 1
assert failed['exit_code'] != 0 and failed['execution']['verifier_invocations'] == 1
assert next(target for target in failed['targets'] if target['requested'])['summary']['failed'] == 1

durations = {}
for name, job in jobs.items():
    elapsed = (datetime.fromisoformat(job['completedAt'].replace('Z', '+00:00'))
               - datetime.fromisoformat(job['startedAt'].replace('Z', '+00:00'))).total_seconds()
    assert 0 <= elapsed < 600, name
    durations[name] = elapsed
print(json.dumps({
    'schema_version': 1, 'scope': 'Native Linux managed CLI/MCP acceptance in the pinned CI image',
    'source_commit': run['headSha'], 'run_url': run['url'],
    'checks': {'all_jobs_successful': True, 'installed_cli_and_async_mcp': True,
               'missing_receipt_refreshes_only_selected_target': True,
               'restored_mtime_failure_matches_plain_verifier': True},
    'limits': ['Bounded acceptance, not a three-round performance benchmark',
               'Cold and unchanged times compare different cache states and are not a speedup claim',
               'Raw reports contain only the controlled synthetic fixture; no environments are retained'],
    'job_duration_seconds': durations, 'reports': records,
}, indent=2))
