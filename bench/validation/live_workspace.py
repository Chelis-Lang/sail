#!/usr/bin/env python3
"""Measure managed report reuse against a warm SMT cache, outside CI.

SAIL_CARGO_VERUS selects the installed pin. Run with SAIL executable and output
JSON arguments. Raw reports live beside the output; published summaries omit
local paths and full environments. All three rounds assert verdict equivalence.
"""
import atexit
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import statistics
import subprocess
import sys
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('sail', type=Path)
parser.add_argument('output', type=Path)
parser.add_argument('--functions', type=int, default=2,
                    help='total functions in the explicit synthetic fixture (minimum 2)')
options = parser.parse_args()
assert 2 <= options.functions <= 2000, 'fixture size must be between 2 and 2000'
sail = options.sail.resolve()
output = options.output.resolve()
output.parent.mkdir(parents=True, exist_ok=True)
raw = output.parent / (output.stem + '-raw')
raw.mkdir(exist_ok=True)
workspace = Path(tempfile.mkdtemp(prefix='sail-live-benchmark-', dir='/tmp'))
env = os.environ.copy()
env['SAIL_CACHE_DIR'] = str(workspace / 'cache')
env['SAIL_DAEMON_SOCKET'] = str(workspace / 'daemon.sock')
env['SAIL_DAEMON_IDLE_TIMEOUT'] = '0'
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
if options.functions > 2:
    additional = ''.join(
        f'pub fn identity_{number}(x: u32) -> (r: u32) ensures r == x {{ x }}\n'
        for number in range(options.functions - 2))
    text = text.rsplit('}\n', 1)[0] + additional + '}\n'
    source.write_text(text)
# Own the daemon throughout the experiment. A cold Cargo build can outlast
# ordinary idle expiry, so an on-demand daemon alone cannot ensure warm SMT
# cache comparisons. Only this process is terminated during cleanup.
daemon = subprocess.Popen([str(sail), 'serve', '--cache', env['SAIL_CACHE_DIR'],
                           '--socket', env['SAIL_DAEMON_SOCKET'], '--idle-timeout', '0'],
                          env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
def stop_daemon():
    if daemon.poll() is None:
        daemon.terminate()
        try:
            daemon.wait(timeout=10)
        except subprocess.TimeoutExpired:
            daemon.kill()
            daemon.wait(timeout=10)
atexit.register(stop_daemon)
deadline = time.monotonic() + 10
while not Path(env['SAIL_DAEMON_SOCKET']).exists() and time.monotonic() < deadline:
    assert daemon.poll() is None, 'benchmark cache daemon failed to start'
    time.sleep(.02)
assert Path(env['SAIL_DAEMON_SOCKET']).exists(), 'benchmark cache daemon unavailable'


def functions(report):
    return sorted((f['function'].split('::', 1)[-1], f['verdict'])
                  for crate in report['crates'] for f in crate['functions']
                  if f['function'].startswith('demo::'))


def requested(document):
    targets = [t for t in document['targets'] if t['requested']]
    assert targets and all(t['complete'] for t in targets), document
    return sorted(pair for target in targets for pair in functions(target['report']))


def bounded_verification(arguments):
    process = subprocess.Popen(arguments, cwd=project, env=env,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=600)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.communicate(timeout=10)
        raise
    return subprocess.CompletedProcess(arguments, process.returncode, stdout, stderr)


def run(label, managed):
    if not managed:
        # Obtain a fresh complete report, with the SMT cache already warm.
        subprocess.run([env.get('CARGO', 'cargo'), 'clean', '-p', 'demo'], cwd=project,
                       env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
    args = [str(sail), 'verify', '--output', 'json']
    if managed:
        args.insert(2, '--incremental')
    timing = raw / (label + '.time')
    timer = ['/usr/bin/time', '-l' if platform.system() == 'Darwin' else '-v',
             '-o', str(timing)]
    start = time.monotonic()
    load_start = os.getloadavg()
    result = bounded_verification(timer + args)
    elapsed = time.monotonic() - start
    (raw / (label + '.json')).write_bytes(result.stdout)
    (raw / (label + '.stderr')).write_bytes(result.stderr)
    assert result.returncode == 0, result.stderr.decode(errors='replace')
    document = json.loads(result.stdout)
    assert document.get('error') is None
    if managed:
        assert document['currentness'] == 'current' or label == 'managed-fill' and document['currentness'] == 'historical'
    resident = None
    for line in timing.read_text().splitlines():
        if 'maximum resident set size' in line.lower():
            resident = int(line.split()[0]) if platform.system() == 'Darwin' else int(line.split(':')[-1]) * 1024
    return document, {'label': label, 'wall_ms': elapsed * 1000,
                      'load_average_start': load_start,
                      'load_average_end': os.getloadavg(),
                      'max_single_process_rss_bytes': resident,
                      'report_sha256': hashlib.sha256(result.stdout).hexdigest(),
                      'execution': document.get('execution')}


plain, cold = run('plain-fill', False)
managed, first = run('managed-fill', True)
assert functions(plain) == requested(managed)
establishment = None
if managed['currentness'] == 'historical':
    managed, establishment = run('managed-establish', True)
    assert functions(plain) == requested(managed)
    assert managed['execution']['verifier_invocations'] > 0
establishing_reports = {(target['package_id'], target['target'], tuple(target['kind'])): target['report']
                       for target in managed['targets'] if target['requested']}
rounds = []
for round_number in range(1, 4):
    plain, p = run(f'plain-warm-{round_number}', False)
    managed, m = run(f'managed-reuse-{round_number}', True)
    assert requested(managed) == functions(plain)
    assert managed['execution']['verifier_invocations'] == 0
    assert managed['execution']['reused_targets'] > 0
    reused_reports = {(target['package_id'], target['target'], tuple(target['kind'])): target['report']
                      for target in managed['targets'] if target['requested']}
    assert reused_reports == establishing_reports, 'reuse must preserve establishing reports'
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
                  'sail_bytes': sail.stat().st_size,
                  'cargo_verus_sha256': hashlib.sha256(Path(cargo_verus).read_bytes()).hexdigest()},
    'limits': ['Development machine under concurrent compilation and conformance load',
               'RSS is the largest single process measured by time, not summed tree memory',
               f'Synthetic successful {options.functions}-function fixture; real projects may have different costs',
               'Baseline cleans its selected caller artifact before timed verification to obtain a complete fresh report; dependency artifacts and SMT entries stay warm'],
    'functions': requested(managed), 'fill': {'plain': cold, 'managed': first},
    'fully_observed_establishment': establishment,
    'rounds': rounds, 'proof_edit': {'plain': edited_plain, 'managed': edited_managed},
    'summary': {'plain': summary(p), 'managed': summary(m),
                'median_speedup': statistics.median(p) / statistics.median(m)},
}
output.write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps(record['summary'], indent=2))
