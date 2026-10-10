#!/usr/bin/env python3
"""Retain hashes and outcomes of local Linux Anvil's three cache rounds.

Run against the complete report directory and the unchanged tested release
binary directory. No local paths or environments are copied into the artifact.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('reports', type=Path)
parser.add_argument('binaries', type=Path)
parser.add_argument('--solver', type=Path, required=True)
parser.add_argument('--cache-entries', type=int, required=True,
                    help='entry count printed by the successful ci/run.sh run')
args = parser.parse_args()
assert args.cache_entries > 0, 'the three-round run must populate its cache'


def fingerprint(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return {'sha256': digest.hexdigest(), 'bytes': path.stat().st_size}


reports = []
for label in ('pass-through', 'cache-fill', 'cache-warm'):
    path = args.reports / label / 'report.json'
    report = json.loads(path.read_bytes())
    summary = report['summary']
    assert summary == {
        'projects_skipped': 0, 'units': 1, 'same': 1, 'unstable': 0,
        'different': 0, 'infra_error': 0, 'functions': 143,
        'units_with_rlimit_drift': 0,
    }, (label, summary)
    assert len(report['units']) == 1
    unit = report['units'][0]
    assert unit['id'] == 'anvil' and unit['verdict'] == 'same'
    for side in ('plain', 'sail'):
        assert unit[side]['exit_code'] == 0
        assert unit[side]['verified'] == 144 and unit[side]['errors'] == 0
    reports.append({
        'label': label, 'report_sha256': fingerprint(path)['sha256'],
        'started_unix': report['started_unix'],
        'verus_release': report['verus_release'],
        'z3_version': report['z3_version'], 'machine': report['machine'],
        'summary': summary,
        'outcomes': {side: {key: unit[side][key] for key in (
            'exit_code', 'verified', 'errors', 'wall_ms', 'smt_ms', 'max_rss_bytes')}
            for side in ('plain', 'sail')},
    })

print(json.dumps({
    'schema_version': 1,
    'recorded_at': datetime.now(timezone.utc).isoformat(),
    'scope': 'Local Linux Anvil differential acceptance with pass-through, cache fill and warm reuse',
    'project_commit': 'a597f9de7dbeb056403bd60b0a2ca67f686a5e1f',
    'limits': [
        'Long local-only validation; never a CI job',
        'Linux x86-64 under emulation on a shared development machine',
        'Times and RSS are acceptance observations, not reference benchmark claims',
        'Binary hashes identify the tested build; later source changes require a new run',
    ],
    'binaries': {name: fingerprint(args.binaries / name)
                 for name in ('sail', 'sail-z3', 'sail-conformance')},
    'solver': fingerprint(args.solver), 'cache_entries': args.cache_entries,
    'reports': reports,
}, indent=2))
