#!/usr/bin/env python3
"""Retain bounded original e2e failure observations, excluding build/cache payloads."""
import argparse
import fcntl
import hashlib
import itertools
import json
import os
from pathlib import Path
import platform
import signal
import stat
import sys
import time

MAX_FILES = 1024
MAX_FILE_BYTES = 8 * 1024**2
MAX_TOTAL_BYTES = 64 * 1024**2
PATTERNS = (
    '*.out', '*.err', '*.rc', '*.json',
    'commands/*',
    'demo/Cargo.toml', 'demo/Cargo.lock', 'demo/src/**/*.rs',
    'ws/Cargo.toml', 'ws/Cargo.lock', 'ws/*/Cargo.toml', 'ws/*/src/**/*.rs',
    'cache/projects/*/*/runtime.json',
    'cache/projects/*/*/runs/*/result.json',
    'cache/projects/*/*/runs/*/cargo.stdout',
    'cache/projects/*/*/runs/*/cargo.stderr',
    'cache/projects/*/*/runs/*/artifacts.jsonl',
    'cache/projects/*/*/runs/*/invocations/*.json',
    'cache/projects/*/*/runs/*/sidecars/*/verus.stdout',
    'cache/projects/*/*/runs/*/sidecars/*/verus.stderr',
    'cache/projects/*/*/runs/*/sidecars/*/verus.shown-stdout',
)


def capture(work, destination, part, original_exit, sail):
    """Capture selected ordinary files; every omission/read failure is indexed."""
    work = work.resolve(strict=True)
    destination = destination.absolute()
    assert original_exit != 0, 'failure capture is not a passing-test operation'
    assert not destination.resolve().is_relative_to(work), 'evidence must outlive fixture cleanup'
    destination.mkdir(parents=True, exist_ok=False)
    index = {
        'schema_version': 1, 'part': part, 'original_exit': original_exit,
        'captured_unix': time.time(), 'platform': platform.platform(),
        'scope': 'Selected original streams, command/source snapshots and producer records; no targets, cache payloads or executable copies.',
        'capture_complete': False, 'files': [], 'errors': [],
        'limits': {'files': MAX_FILES, 'per_file_bytes': MAX_FILE_BYTES,
                   'total_bytes': MAX_TOTAL_BYTES},
        'producer_mapping': 'Paths/run IDs are observations; timing does not reconstruct cross-stream write order or assign a run to a command.',
    }
    if (work / 'metadata-errors.err').exists():
        index['errors'].append('command metadata recording reported a failure; see metadata-errors.err')
    copied = 0
    seen = set()
    try:
        try:
            descriptor = os.open(sail, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
            with os.fdopen(descriptor, 'rb') as source:
                metadata = os.fstat(source.fileno())
                if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 256 * 1024**2:
                    raise ValueError('Sail identity is not a bounded ordinary executable')
                digest = hashlib.sha256()
                identity_bytes = 0
                while identity_bytes <= 256 * 1024**2:
                    block = source.read(min(1024**2, 256 * 1024**2 + 1 - identity_bytes))
                    if not block:
                        break
                    identity_bytes += len(block)
                    digest.update(block)
                if identity_bytes > 256 * 1024**2:
                    raise ValueError('Sail executable grew beyond identity read bound')
                after = os.fstat(source.fileno())
            index['sail'] = {'path': str(sail), 'sha256': digest.hexdigest(),
                             'bytes': metadata.st_size, 'mtime_ns': metadata.st_mtime_ns}
            if (metadata.st_size, metadata.st_mtime_ns, metadata.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise ValueError('Sail executable changed while identified')
        except (OSError, ValueError) as error:
            index['errors'].append('Sail identity: ' + repr(error))
        candidates = itertools.chain.from_iterable(work.glob(pattern) for pattern in PATTERNS)
        for path in candidates:
            name = path.relative_to(work).as_posix()
            if name in seen:
                continue
            if len(seen) >= MAX_FILES:
                index['errors'].append('selected-file count limit reached; remaining selected paths omitted')
                break
            seen.add(name)
            row = {'path': name, 'copied': False}
            index['files'].append(row)
            descriptor = None
            try:
                before = path.lstat()
                resolved = path.resolve()
                relative = resolved.relative_to(work) if resolved.is_relative_to(work) else None
                forbidden = {'target', 'target-plain', 'target-sail', 'compiler-artifacts', 'dependency-store'}
                if (not stat.S_ISREG(before.st_mode) or relative is None
                        or forbidden.intersection(relative.parts)):
                    raise ValueError('nonregular/external selected file refused')
                descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
                held = os.fstat(descriptor)
                if (not stat.S_ISREG(held.st_mode)
                        or (before.st_dev, before.st_ino) != (held.st_dev, held.st_ino)):
                    raise ValueError('selected file identity changed before read')
                allowance = min(MAX_FILE_BYTES, MAX_TOTAL_BYTES - copied)
                chunks = []
                read_bytes = 0
                while read_bytes <= allowance:
                    chunk = os.read(descriptor, min(65536, allowance + 1 - read_bytes))
                    if not chunk:
                        break
                    chunks.append(chunk)
                    read_bytes += len(chunk)
                data = b''.join(chunks)
                after = os.fstat(descriptor)
                retained = data[:allowance]
                row.update(source_bytes=held.st_size, retained_bytes=len(retained),
                           retained_sha256=hashlib.sha256(retained).hexdigest(),
                           truncated=held.st_size > len(retained),
                           changed_during_read=(held.st_size, held.st_mtime_ns, held.st_ctime_ns)
                           != (after.st_size, after.st_mtime_ns, after.st_ctime_ns))
                target = destination / 'files' / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(retained)
                copied += len(retained)
                row['copied'] = True
                if row['truncated'] or row['changed_during_read']:
                    index['errors'].append(name + ': truncated or changed while captured')
            except (OSError, ValueError) as error:
                row['error'] = repr(error)
                index['errors'].append(name + ': ' + repr(error))
            finally:
                if descriptor is not None:
                    os.close(descriptor)
        index['capture_complete'] = not index['errors']
    except Exception as error:
        index['errors'].append(repr(error))
    finally:
        index['retained_bytes'] = copied
        (destination / 'index.json').write_text(json.dumps(index, indent=2) + '\n')
    return 0 if index['capture_complete'] else 1


def _snapshot_locked(work, cwd, label, stage):
    """Bound diagnostic duplicate snapshots before the final failure collector."""
    work, cwd = work.resolve(strict=True), cwd.resolve(strict=True)
    assert cwd.is_relative_to(work) and label.replace('-', '').isalnum()
    commands = work / 'commands'
    commands.mkdir(exist_ok=True)
    # The caller serializes warmup observers with a fixture-owned file lock.
    # Existing duplicate snapshots count against this shared ceiling.
    allowance = max(0, 32 * 1024**2 - sum(q.stat().st_size for q in commands.iterdir()
                                        if q.is_file() and not q.is_symlink()))
    sources = [(cwd / name, label + '.' + name.replace('/', '.'))
               for name in ('src/lib.rs', 'Cargo.toml', 'Cargo.lock',
                            'a/src/lib.rs', 'b/src/lib.rs', 'a/Cargo.toml', 'b/Cargo.toml')]
    if stage == 'finish':
        sources = [(work / (label + '.' + stream), label + '.raw.' + stream)
                   for stream in ('out', 'err')]
    rows, errors = [], []
    for path, name in sources:
        if not path.exists() and not path.is_symlink():
            continue
        row = {'source': str(path.relative_to(work)), 'snapshot': name}
        rows.append(row)
        try:
            before = path.lstat()
            row['source_bytes'] = before.st_size
            if not stat.S_ISREG(before.st_mode) or not path.resolve().is_relative_to(work):
                raise ValueError('nonregular/external snapshot refused')
            descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW | os.O_CLOEXEC)
            with os.fdopen(descriptor, 'rb') as source:
                held = os.fstat(source.fileno())
                assert (before.st_dev, before.st_ino) == (held.st_dev, held.st_ino)
                assert stat.S_ISREG(held.st_mode)
                data = source.read(min(MAX_FILE_BYTES, allowance) + 1)
                after = os.fstat(source.fileno())
            data = data[:min(MAX_FILE_BYTES, allowance)]
            (commands / name).write_bytes(data)
            allowance -= len(data)
            row.update(source_bytes=held.st_size, retained_bytes=len(data),
                       sha256=hashlib.sha256(data).hexdigest(),
                       truncated=held.st_size > len(data),
                       changed=(held.st_size, held.st_mtime_ns, held.st_ctime_ns)
                       != (after.st_size, after.st_mtime_ns, after.st_ctime_ns))
            if row['truncated'] or row['changed']:
                raise ValueError('snapshot truncated or changed while read')
        except (OSError, ValueError, AssertionError) as error:
            row['error'] = repr(error)
            errors.append(repr(error))
            # Passing fixtures are removed too. Keep bounded metadata about the
            # refusal in the outer log without printing any retained payload.
            print(json.dumps({'event': 'e2e-output-snapshot-incomplete',
                              'label': label[:128], 'stage': stage,
                              'source': row['source'][:512],
                              'source_bytes': row.get('source_bytes'),
                              'retained_bytes': row.get('retained_bytes', 0),
                              'truncated': row.get('truncated'),
                              'changed': row.get('changed'),
                              'reason': repr(error)[:512]}, sort_keys=True), file=sys.stderr)
    (commands / (label + '.' + stage + '.json')).write_text(
        json.dumps({'files': rows, 'errors': errors, 'complete': not errors}, indent=2) + '\n')
    return 1 if errors else 0


def snapshot(work, cwd, label, stage):
    """Serialize only diagnostic copies, with one shared bounded byte budget."""
    with (work / 'snapshot.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        return _snapshot_locked(work, cwd, label, stage)


def snapshot_main():
    parser = argparse.ArgumentParser(description='Bounded command diagnostic snapshots')
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--cwd', type=Path, required=True)
    parser.add_argument('--label', required=True)
    parser.add_argument('--stage', choices=['start', 'finish'], required=True)
    args = parser.parse_args(sys.argv[2:])
    return snapshot(args.work, args.cwd, args.label, args.stage)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--part', choices=['loop', 'loader', 'flags', 'noop', 'extra'], required=True)
    parser.add_argument('--exit', type=int, required=True)
    parser.add_argument('--sail', type=Path, required=True)
    args = parser.parse_args()
    destination = args.out.resolve() / (args.part + '-' + str(time.time_ns()))
    return capture(args.work, destination, args.part, args.exit, args.sail.resolve())


if __name__ == '__main__':
    def interrupted(number, _frame):
        raise TimeoutError('failure collector interrupted by signal ' + str(number))
    signal.signal(signal.SIGTERM, interrupted)
    raise SystemExit(snapshot_main() if sys.argv[1:2] == ['snapshot'] else main())
