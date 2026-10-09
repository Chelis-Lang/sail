#!/usr/bin/env python3
"""Retain portable summaries and hashes of completed conformance reports.

Pass a JSON manifest listing label, path and platform_note for each report.
Paths are read locally and never included in the generated artifact.
"""

import hashlib
import json
import pathlib
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
report_root = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path('.')
records = []
for entry in manifest['reports']:
    raw = (report_root / entry['path']).read_bytes()
    report = json.loads(raw)
    summary = report['summary']
    count = summary.get('units', summary.get('tests'))
    assert summary['different'] == summary['infra_error'] == 0, entry['label']
    assert count == summary['same'] + summary['unstable'], entry['label']
    assert count == entry['expected_count'], entry['label']
    assert summary.get('projects_skipped', 0) == entry.get('expected_skips', 0), entry['label']
    record = {
        'label': entry['label'],
        'report_sha256': hashlib.sha256(raw).hexdigest(),
        'started_unix': report['started_unix'],
        'verus_release': report['verus_release'],
        'z3_version': report['z3_version'],
        'solver_sha256': entry['solver_sha256'],
        'machine': report['machine'],
        'platform_note': entry['platform_note'],
        'summary': summary,
    }
    if 'verus_commit' in report:
        record['verus_commit'] = report['verus_commit']
        record['features'] = report['features']
        record['verus_threads'] = report['verus_threads']
    if 'tests' in report:
        assert len(report['tests']) == count
        unstable = [test for test in report['tests'] if test['verdict'] == 'unstable']
        assert len(unstable) == summary['unstable']
        assert all(test['plain_varies'] and test['matches_a_plain_run'] for test in unstable)
        record['unstable_cases'] = [test['id'] for test in unstable]
        record['baseline_failed_cases'] = [
            test['id'] for test in report['tests'] if test['plain'] == 'failed'
        ]
    records.append(record)
assert len({record['label'] for record in records}) == len(records)
print(json.dumps({
    'schema_version': 1,
    'scope': 'Initial product acceptance after the solver diagnostic correction',
    'source_revisions': manifest['source_revisions'],
    'limits': manifest['limits'],
    'reports': records,
}, indent=2))
