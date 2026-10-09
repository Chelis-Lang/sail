#!/usr/bin/env python3
"""Measure managed report reuse against a warm SMT cache, outside CI.

SAIL_CARGO_VERUS selects the installed pin. Run with SAIL executable and output
JSON arguments. Raw reports live beside the output; published summaries omit
local paths and full environments. All three rounds assert verdict equivalence.
"""
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import sys
import tempfile
import time

sail = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2]).resolve()
output.parent.mkdir(parents=True, exist_ok=True)
raw = output.parent / (output.stem + '-raw')
raw.mkdir(exist_ok=True)
workspace = Path(tempfile.mkdtemp(prefix='sail-live-benchmark-'))
env = os.environ.copy()
env['SAIL_CACHE_DIR'] = str(workspace / 'cache')
env['SAIL_DAEMON_SOCKET'] = str(workspace / 'daemon.sock')
env['SAIL_DAEMON_IDLE_TIMEOUT'] = '30'
cargo_verus = env['SAIL_CARGO_VERUS']
subprocess.run([cargo_verus, 'verus', 'new', '--lib', 'demo'], cwd=workspace,
               env=env, stdout=subprocess.DEVNULL, check=True)
project = workspace / 'demo'
source = project / 'src/lib.rs'
text = '''use vstd::prelude::*;
verus! {
pub fn identity(x: u32) -> (r: u32) ensures r == x { x }
pub fn maximum(a: u32, b: u32) -> (r: u32)
    ensures r >= a, r >= b, r == a || r == b,
{ if a > b { a } else { b } }
}
'''
source.write_text(text)


def functions(report):
    return sorted((f['function'].split('::', 1)[-1], f['verdict'])
                  for crate in report['crates'] for f in crate['functions'])


def requested(document):
    targets = [t for t in document['targets'] if t['requested']]
    assert targets and all(t['complete'] for t in targets), document
    return sorted(pair for target in targets for pair in functions(target['report']))


def run(label, managed):
    if not managed:
        # Obtain a fresh complete report, with the SMT cache already warm.
        os.utime(source, None)
    args = [str(sail), 'verify', '--output', 'json']
    if managed:
        args.insert(2, '--incremental')
    timing = raw / (label + '.time')
    timer = ['/usr/bin/time', '-l' if platform.system() == 'Darwin' else '-v',
             '-o', str(timing)]
    start = time.monotonic()
    result = subprocess.run(timer + args, cwd=project, env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=600)
    elapsed = time.monotonic() - start
    (raw / (label + '.json')).write_bytes(result.stdout)
    (raw / (label + '.stderr')).write_bytes(result.stderr)
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    document = json.loads(result.stdout)
    assert document.get('error') is None
    if managed:
        assert document['currentness'] == 'current'
    resident = None
    for line in timing.read_text().splitlines():
        if 'maximum resident set size' in line.lower():
            resident = int(line.split()[0]) if platform.system() == 'Darwin' else int(line.split(':')[-1]) * 1024
    return document, {'label': label, 'wall_ms': elapsed * 1000,
                      'max_single_process_rss_bytes': resident,
                      'report_sha256': hashlib.sha256(result.stdout).hexdigest(),
                      'execution': document.get('execution')}


plain, cold = run('plain-fill', False)
managed, first = run('managed-fill', True)
assert functions(plain) == requested(managed)
rounds = []
for round_number in range(1, 4):
    plain, p = run(f'plain-warm-{round_number}', False)
    managed, m = run(f'managed-reuse-{round_number}', True)
    assert requested(managed) == functions(plain)
    assert managed['execution']['verifier_invocations'] == 0
    assert managed['execution']['reused_targets'] > 0
    rounds.append({'round': round_number, 'plain_warm_smt_cache': p,
                   'managed_complete_report_reuse': m})
# A real proof-body edit must invoke a verifier, retain complete scope and agree.
source.write_text(text.replace('{ x }', '{ assert(x == x); x }'))
plain, edited_plain = run('plain-edited', False)
managed, edited_managed = run('managed-edited', True)
assert functions(plain) == requested(managed)
assert managed['execution']['verifier_invocations'] > 0
values = lambda key: [r[key]['wall_ms'] for r in rounds]
summary = lambda xs: {'median_ms': statistics.median(xs), 'min_ms': min(xs), 'max_ms': max(xs)}
p = values('plain_warm_smt_cache')
m = values('managed_complete_report_reuse')
record = {
    'schema_version': 1,
    'scope': 'Complete managed report reuse versus fresh Verus with a warm SMT cache',
    'machine': {'system': platform.system(), 'machine': platform.machine(),
                'processor': platform.processor(), 'cpu_count': os.cpu_count(),
                'load_average': os.getloadavg()},
    'toolchain': {'rust': subprocess.check_output(['rustc', '--version'], env=env).decode().strip(),
                  'sail_sha256': hashlib.sha256(sail.read_bytes()).hexdigest(),
                  'cargo_verus_sha256': hashlib.sha256(Path(cargo_verus).read_bytes()).hexdigest()},
    'limits': ['Development machine under concurrent compilation and conformance load',
               'RSS is the largest single process measured by time, not summed tree memory',
               'Small successful two-function fixture; broader projects may have different costs'],
    'functions': requested(managed), 'fill': {'plain': cold, 'managed': first},
    'rounds': rounds, 'proof_edit': {'plain': edited_plain, 'managed': edited_managed},
    'summary': {'plain': summary(p), 'managed': summary(m),
                'median_speedup': statistics.median(p) / statistics.median(m)},
}
output.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record['summary'], indent=2))
