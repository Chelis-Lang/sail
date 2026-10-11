#!/usr/bin/env python3
"""Bind the unchanged local repair runner to the qualified runtime and granted product."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time

BUILD = Path('/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-c77e1a1879302e06')
M6 = Path('/opt/sail-m6-0fde/sail')
MANIFEST = '54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d'
LOCK = '653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1'
PATCH = 'aaa3b2e4780c6b410b77ac121ef682c9532fb2eb21552a104fa63243e7a95124'
HASHES = {
    'scripts/m7-repair-bench.py': '3931f922af8da8317a857955915754e1d43e7c905b6171f381d0b6096352de33',
    'scripts/m7-job-read-bench.py': '3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720',
    'corpus/repair-edits.json': '6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6',
}
BUNDLE = Path('engine/verus/candidates/preparation-namespace/no-original-range/valid-memo-correction/ordinary')


def sha(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def checkpoint(path, value):
    temporary = path.with_suffix('.tmp')
    temporary.write_text(json.dumps(value, indent=2) + '\n')
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo', type=Path, required=True)
    parser.add_argument('--m7', type=Path, required=True)
    parser.add_argument('--m7-sha256', required=True)
    parser.add_argument('--m7-shim-sha256', required=True)
    parser.add_argument('--product-commit', required=True)
    parser.add_argument('--work', type=Path, required=True)
    args = parser.parse_args()
    repo, product, work = args.repo.resolve(), args.m7.resolve(), args.work.resolve()
    require(re.fullmatch('[0-9a-f]{40}', args.product_commit), 'Invalid granted product commit')
    for digest in (args.m7_sha256, args.m7_shim_sha256):
        require(re.fullmatch('[0-9a-f]{64}', digest), 'Invalid granted binary digest')
    require(all(sha(repo / name) == digest for name, digest in HASHES.items()), 'Original runner/catalogue changed')
    expected_binaries = {
        str(M6): '69288597f5f5c0d0a81a66b00a6b0f174ba17bf6a878447050a2d7f75c0c36d8',
        str(M6.with_name('sail-z3')): 'aaa7e3c43ee6282b575b615ad87a4c964bae14c2f6a56787c98f03d1159582bd',
        str(product): args.m7_sha256,
        str(product.with_name('sail-z3')): args.m7_shim_sha256,
    }
    require(all(sha(Path(name)) == digest for name, digest in expected_binaries.items()), 'Granted product/baseline bytes differ')
    qualified = json.loads((repo / BUNDLE / 'qualification-result.json').read_text())
    delivery = json.loads((repo / BUNDLE / 'packaging/result.json').read_text())
    require(qualified['status'] == delivery['status'] == 'PASS', 'Qualification/delivery is not PASS')
    require(qualified['stage_counts']['passed'] == 13 and qualified['stage_counts']['not_attempted'] == 0, 'Incomplete source qualification')
    require(qualified['manifest_sha256'] == delivery['manifest_sha256'] == MANIFEST, 'Qualification/runtime mismatch')
    require(qualified['patch_sha256'] == delivery['patch_sha256'] == PATCH, 'Qualified patch mismatch')
    modes = {item['name']: item['mode'] for item in delivery['members']}
    manifest = json.loads((BUILD / 'sail-build.json').read_text())
    require(len(manifest['artifacts']) == 13, 'Incomplete runtime inventory')
    require(manifest['features'] == ['sail-interactive:' + PATCH, 'host:x86_64-unknown-linux-gnu', 'source-lock:' + LOCK], 'Runtime feature identity differs')

    def seals():
        require(sha(BUILD / 'sail-build.json') == MANIFEST, 'Runtime manifest differs')
        runtime = BUILD / 'verus/source/target-verus/release'
        require({str(item.relative_to(BUILD)) for item in runtime.iterdir() if item.is_file()} == set(manifest['artifacts']), 'Runtime direct inventory differs')
        require(all(not (BUILD / name).is_symlink() and sha(BUILD / name) == digest and (BUILD / name).stat().st_mode & 0o777 == modes[name]
                    for name, digest in manifest['artifacts'].items()), 'Runtime bytes or modes differ')
        require(all(sha(BUILD / 'verus/source' / name) == digest for name, digest in qualified['source8_sha256'].items()), 'Qualified source differs')
        require(sha(BUILD / 'verus/source/Cargo.lock') == LOCK, 'Source lock differs')
        require(all(sha(repo / name) == digest for name, digest in HASHES.items()), 'Original inputs changed')
        require(all(sha(Path(name)) == digest for name, digest in expected_binaries.items()), 'Product/baseline changed')
        return {'manifest_sha256': MANIFEST, 'runtime13_sha256': manifest['artifacts'], 'source8_sha256': qualified['source8_sha256'], 'source_lock_sha256': LOCK}

    before = seals()
    catalogue = json.loads((repo / 'corpus/repair-edits.json').read_text())
    require(catalogue['minimum_rounds'] == 3 and catalogue['minimum_median_speedup'] == 2, 'Original acceptance thresholds changed')
    for project in catalogue['projects']:
        path = Path('/root/.cache/sail/corpus') / (project['id'] + '-' + project['commit'][:12])
        require(subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip() == project['commit'], 'Corpus commit differs')
    require(os.getpriority(os.PRIO_PROCESS, 0) == 0, 'Benchmark must run at ordinary priority')
    work.mkdir(exist_ok=False)
    excluded = {'CI', 'CARGO_TARGET_DIR', 'CARGO_BUILD_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTDOC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER'}
    environment = {name: value for name, value in os.environ.items() if not name.startswith('SAIL_') and name not in excluded}
    environment.update(CARGO_HOME='/opt/cargo', RUSTUP_HOME='/opt/rustup', RUSTUP_TOOLCHAIN='1.98.1', CARGO_BUILD_JOBS='2')
    runner = repo / 'scripts/m7-repair-bench.py'
    commands = [
        ('prepare', ['python3', str(runner), 'prepare', '--cache', '/root/.cache/sail', '--verus-source', str(BUILD / 'verus'), '--scratch', str(work / 'scratch')]),
        ('run', ['python3', str(runner), 'run', '--m6', str(M6), '--m7', str(product), '--engine-install', str(BUILD), '--scratch', str(work / 'scratch'), '--out', str(work / 'results'), '--rounds', '3', '--timeout', '600', '--environment', 'linux-x86_64-container-emulated-sharedload']),
    ]
    result = {'version': 1, 'status': 'running', 'scope': 'Original whole-loop repair benchmark; reader latency separate',
              'product_commit_as_granted': args.product_commit, 'binary_sha256': expected_binaries,
              'script_catalogue_sha256': HASHES, 'before': before, 'rounds': 3, 'timeout_seconds': 600,
              'minimum_median_speedup_each_project': 2, 'started_unix_ns': time.time_ns(), 'nice': 0,
              'controller_pid': os.getpid(), 'phases': []}
    started = time.monotonic()

    def interrupted(signum, frame):
        raise KeyboardInterrupt('Owned benchmark controller interrupted')

    signal.signal(signal.SIGTERM, interrupted)
    signal.signal(signal.SIGHUP, interrupted)
    try:
        for name, command in commands:
            phase = {'name': name, 'command': command, 'status': 'running'}
            result['phases'].append(phase)
            phase_start = time.monotonic()
            with (work / (name + '.log')).open('wb') as log:
                child = subprocess.Popen(command, env=environment, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
                phase.update(pid=child.pid, pgid=os.getpgid(child.pid))
                checkpoint(work / 'controller.json', result)
                try:
                    code = child.wait()
                except BaseException:
                    # The original runner performs its own exact CLI/job/daemon cleanup.
                    # Give that handler time to finish before a bounded owned fallback.
                    if child.poll() is None:
                        os.kill(child.pid, signal.SIGINT)
                        try:
                            child.wait(timeout=30)
                        except subprocess.TimeoutExpired:
                            os.killpg(child.pid, signal.SIGKILL)
                            child.wait(timeout=5)
                            phase['runner_cleanup_unavailable'] = True
                    phase.update(status='FAIL', exit_code=child.returncode,
                                 elapsed_seconds=time.monotonic() - phase_start)
                    log.flush()
                    phase['log_sha256'] = sha(work / (name + '.log'))
                    raise
            phase.update(status='PASS' if code == 0 else 'FAIL', exit_code=code,
                         elapsed_seconds=time.monotonic() - phase_start, log_sha256=sha(work / (name + '.log')))
            checkpoint(work / 'controller.json', result)
            require(code == 0, 'Original benchmark phase failed: ' + name)
        result['status'] = 'PASS'
    except BaseException as error:
        result.update(status='FAIL', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        result['elapsed_seconds'] = time.monotonic() - started
        checkpoint(work / 'primary.json', result)
        try:
            result['after'] = seals()
            result['seal_closure'] = 'PASS'
        except BaseException as error:
            result.update(status='FAIL', seal_closure='FAIL', seal_error=str(error))
        checkpoint(work / 'controller.json', result)
        print(json.dumps({'status': result['status'], 'elapsed_seconds': result['elapsed_seconds']}), flush=True)
    require(result['status'] == 'PASS', 'Post-benchmark seal closure failed')


if __name__ == '__main__':
    main()
