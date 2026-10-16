#!/usr/bin/env python3
"""Bounded correctness checks only; no Mac performance measurement."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import shutil
import tempfile
from unittest.mock import patch
import live_prefix as implementation
import time
from concurrent.futures import ThreadPoolExecutor
from live_prefix import MAX_INPUT, MAX_INPUT_LIMIT, OneUsePool


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--z3', required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    z3 = str(Path(args.z3).resolve())
    identity = hashlib.sha256(Path(z3).read_bytes()).hexdigest()
    rows = []

    def save(label, result):
        for stream in ['stdout', 'stderr']:
            (args.out / (label + '.' + stream)).write_bytes(result[stream])
        return {k: v for k, v in result.items() if k not in {'stdout', 'stderr', 'prefix_stdout', 'suffix_stdout'}}

    def check(name, condition, **extra):
        rows.append(dict(name=name, passed=bool(condition), **extra))

    if os.uname().sysname != 'Linux':
        try:
            OneUsePool(z3, identity)
        except ValueError as error:
            check('unsupported-image-guard-refuses', 'Linux' in str(error), refusal=str(error))
        else:
            check('unsupported-image-guard-refuses', False)
        manifest = dict(runtime_sha256=identity, platform=list(os.uname()), rows=rows,
                        all_pass=all(row['passed'] for row in rows),
                        scope='unsupported live reuse refuses; native live positives pending')
        (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        print(json.dumps(manifest))
        return 0 if manifest['all_pass'] else 1

    with tempfile.TemporaryDirectory(prefix='sail-live-exec-aba-') as temporary:
        directory = Path(temporary).resolve()
        executable, saved, marker = directory / 'z3', directory / 'original', directory / 'foreign-ran'
        shutil.copyfile(z3, executable)
        executable.chmod(0o700)
        original_popen = subprocess.Popen
        started = []

        def substitute(argv, *values, **options):
            assert Path(argv[0]) == executable
            executable.rename(saved)
            executable.write_text('#!/bin/sh\nprintf ready > foreign-ran\nwhile IFS= read -r line; do :; done\nprintf "sat\\n"\n')
            executable.chmod(0o700)
            try:
                process = original_popen(argv, *values, **options)
                started.append(process)
                deadline = time.monotonic() + 2
                while not marker.exists() and process.poll() is None and time.monotonic() < deadline:
                    time.sleep(.005)
                assert marker.exists(), 'foreign executable never ran'
                return process
            finally:
                executable.unlink()
                saved.rename(executable)

        try:
            with OneUsePool(executable, identity, cwd=directory) as pool:
                with patch.object(implementation.subprocess, 'Popen', substitute):
                    key = pool.seed(b'', b'')
                whole = b'(assert false)\n(check-sat)\n(exit)\n'
                fresh = subprocess.run([str(executable), '-smt2', '-in'], input=whole,
                                       capture_output=True, timeout=5)
                result = None
                if key is not None and pool.wait_ready(key, 3):
                    result = pool.consume(key, whole)
                save('exec-aba-fresh', dict(stdout=fresh.stdout, stderr=fresh.stderr, exit=fresh.returncode))
                if result is not None:
                    save('exec-aba-candidate', result)
                check('executable-path-aba-refuses-actual-foreign-image', marker.exists() and
                      hashlib.sha256(executable.read_bytes()).hexdigest() == identity and
                      key is None and result is None and fresh.stdout == b'unsat\n' and fresh.returncode == 0,
                      refusal=getattr(pool, 'last_refusal', None), substituted_executable_ran=marker.exists(),
                      restored_path_sha256=hashlib.sha256(executable.read_bytes()).hexdigest(),
                      candidate_stdout=result['stdout'].decode() if result is not None else None,
                      fresh_stdout=fresh.stdout.decode(), fresh_exit=fresh.returncode)
        finally:
            for process in started:
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=2)

    cases = [
        ('nul-comment', b'; genuine\0NUL\n', b'', b'(check-sat)\n(exit)\n'),
        ('boolean', b'(declare-const x Bool)\n(assert x)\n', b'',
         b'(push)\n(check-sat)\n(pop)\n(exit)\n'),
        ('idle-timer', b'(set-option :timeout 1000)\n(check-sat)\n', b'sat\n',
         b'(push)\n(assert false)\n(check-sat)\n(pop)\n(exit)\n'),
        ('model', b'(set-option :produce-models true)\n(declare-const x Bool)\n(assert x)\n(check-sat)\n', b'sat\n',
         b'(get-model)\n(exit)\n'),
    ]
    for name, prefix, expected, suffix in cases:
        with OneUsePool(z3, identity) as pool:
            key = pool.seed(prefix, expected)
            ready = pool.wait_ready(key, 3)
            entry = pool.entries.get(key)
            pid = entry.pid if entry else None
            if name == 'idle-timer' and ready:
                if os.uname().sysname == 'Darwin':
                    listed = subprocess.run(['ps', '-M', '-p', str(pid), '-o', 'pid='],
                                            capture_output=True, timeout=2, check=True)
                    count = len(listed.stdout.splitlines()) - 1
                else:
                    count = len(list(Path(f'/proc/{pid}/task').iterdir()))
                check('idle-timer-worker-preserved', count >= 2, threads=count, pid=pid)
            # Wrong bytes do not consume an otherwise exact ready process.
            check(name + '-wrong-prefix-refused', pool.consume(key, b'(exit)\n') is None)
            result = pool.consume(key, prefix + suffix)
            fresh = subprocess.run([z3, '-smt2', '-in'], input=prefix + suffix,
                                   capture_output=True, timeout=5)
            plain = dict(stdout=fresh.stdout, stderr=fresh.stderr, exit=fresh.returncode)
            save(name + '-fresh', plain)
            if result is not None:
                detail = save(name + '-live', result)
            else:
                detail = dict(error=entry.failure if entry else 'seed refused')
            check(name + '-exact', ready and result is not None and
                  (result['stdout'], result['stderr'], result['exit']) ==
                  (fresh.stdout, fresh.stderr, fresh.returncode) and result['pid'] == pid,
                  **detail)
            check(name + '-one-use', pool.consume(key, prefix + suffix) is None)
            check(name + '-child-reaped', entry is not None and entry.process.poll() is not None)

    with OneUsePool(z3, identity, slots=1) as pool:
        prefix = b'(check-sat)\n'
        key = pool.seed(prefix, b'sat\n')
        check('warming-and-ready-counted', key is not None and pool.seed(prefix, b'sat\n') is None)
        ready = pool.wait_ready(key, 3)
        with ThreadPoolExecutor(max_workers=2) as workers:
            results = list(workers.map(lambda _: pool.consume(key, prefix + b'(exit)\n'), range(2)))
        check('concurrent-claim-exclusive', ready and sum(r is not None for r in results) == 1)

    with OneUsePool(z3, identity, max_bytes=1) as pool:
        key = pool.seed(b'', b'')
        ready = pool.wait_ready(key, 2)
        entry = pool.entries.get(key)
        check('rss-budget-refuses', not ready and pool.consume(key, b'(exit)\n') is None and
              entry is not None and entry.failure is not None)

    with OneUsePool(z3, identity, ttl=.05) as pool:
        key = pool.seed(b'(check-sat)\n', b'sat\n')
        entry = pool.entries.get(key)
        time.sleep(.2)
        check('ttl-refuses-and-reaps', pool.consume(key, b'(check-sat)\n(exit)\n') is None and
              entry is not None and entry.process.poll() is not None)

    with OneUsePool(z3, identity) as pool:
        key = pool.seed(b'(check-sat)\n', b'unsat\n')
        ready = pool.wait_ready(key, 2)
        entry = pool.entries.get(key)
        check('unexpected-prefix-refuses', not ready and pool.consume(key, b'(check-sat)\n(exit)\n') is None
              and entry is not None and entry.failure is not None)

    large_prefix = b';' + b'x' * MAX_INPUT + b'\n'
    with OneUsePool(z3, identity) as pool:
        check('default-input-bound-refuses-large-prefix', pool.seed(large_prefix, b'') is None)
        key = pool.seed(b'', b'')
        ready = pool.wait_ready(key, 3)
        check('whole-input-bound-refuses-without-consuming', ready and
              pool.consume(key, b' ' * (MAX_INPUT + 1)) is None and
              pool.consume(key, b'(check-sat)\n(exit)\n') is not None)
    with OneUsePool(z3, identity, max_input_bytes=MAX_INPUT_LIMIT) as pool:
        key = pool.seed(large_prefix, b'')
        ready = pool.wait_ready(key, 3)
        whole = large_prefix + b'(check-sat)\n(exit)\n'
        result = pool.consume(key, whole)
        fresh = subprocess.run([z3, '-smt2', '-in'], input=whole, capture_output=True, timeout=5)
        save('large-input-fresh', dict(stdout=fresh.stdout, stderr=fresh.stderr, exit=fresh.returncode))
        if result is not None:
            save('large-input-live', result)
        check('explicit-bounded-large-input-exact', ready and result is not None and
              (result['stdout'], result['stderr'], result['exit']) ==
              (fresh.stdout, fresh.stderr, fresh.returncode))
    try:
        OneUsePool(z3, identity, max_input_bytes=MAX_INPUT_LIMIT + 1)
    except ValueError:
        check('input-bound-above-ceiling-refuses', True)
    else:
        check('input-bound-above-ceiling-refuses', False)
    refused = 0
    for options in [dict(slots=1.5), dict(max_bytes=float('inf')), dict(max_input_bytes=1.5)]:
        try:
            OneUsePool(z3, identity, **options)
        except ValueError:
            refused += 1
    check('noninteger-resource-bounds-refuse', refused == 3)

    manifest = dict(runtime_sha256=identity, platform=list(os.uname()), rows=rows,
                    all_pass=all(row['passed'] for row in rows), scope='bounded correctness, no performance claim')
    (args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print(json.dumps(manifest))
    return 0 if manifest['all_pass'] else 1


if __name__ == '__main__':
    raise SystemExit(main())
