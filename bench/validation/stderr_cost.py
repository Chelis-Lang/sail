#!/usr/bin/env python3
"""Summarize twelve paired guide conformance reports without local paths."""
import hashlib
import json
import pathlib
import statistics
import sys

source = pathlib.Path(sys.argv[1])
result = {
    'schema_version': 1,
    'scope': '33 pinned Verus guide units; three fill/warm rounds per variant',
    'before_revision': 'a7d9294b5031683a7e7551327f15030317e1e22a',
    'after_revisions': ['b546e89a11956b81443b5535a520d8a1bd7449f7', '1e4d8df757f7e16e60e06ad3170cfc42f6a9c3b5'],
    'after_changes': 'silent-session publication and complete-input silence records; shared store disabled',
    'load': 'concurrent development, source conformance and compilation; no idle-machine claim',
    'metric': 'sum of per-unit Sail wall times; parallel elapsed time is different',
    'round_order': [['old', 'new'], ['new', 'old'], ['old', 'new']],
    'records': [],
    'totals': {},
}
for variant in ('old', 'new'):
    for phase in ('fill', 'warm'):
        times = []
        for round_no in (1, 2, 3):
            name = f'{variant}-{round_no}-{phase}'
            raw = (source / name / 'report.json').read_bytes()
            report = json.loads(raw)
            summary = report['summary']
            assert summary['units'] == summary['same'] == 33
            assert summary['different'] == summary['unstable'] == summary['infra_error'] == 0
            assert len(report['units']) == 33
            total = sum(u['sail']['wall_ms'] for u in report['units']) / 1000
            times.append(total)
            result['machine'] = report['machine']
            result['verus_release'] = report['verus_release']
            result['z3_version'] = report['z3_version']
            result['records'].append({
                'name': name,
                'report_sha256': hashlib.sha256(raw).hexdigest(),
                'started_unix': report['started_unix'],
                'summary': summary,
                'sail_wall_sum_s': total,
            })
        result['totals'][f'{variant}-{phase}'] = {
            'rounds_s': times, 'median_s': statistics.median(times),
            'min_s': min(times), 'max_s': max(times),
        }
result['comparison'] = {}
for phase in ('fill', 'warm'):
    old = result['totals'][f'old-{phase}']['median_s']
    new = result['totals'][f'new-{phase}']['median_s']
    result['comparison'][phase] = {'median_delta_percent': (new / old - 1) * 100}
result['interpretation'] = 'Ranges overlap widely; no stable speedup or regression is established.'
print(json.dumps(result, indent=2))
