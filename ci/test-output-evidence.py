#!/usr/bin/env python3
"""Real filesystem/process controls for diagnostics; never a verifier acceptance run."""
import contextlib
import hashlib
import io
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('output_evidence', HERE / 'capture-output-evidence.py')
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


def filesystem_controls():
    with tempfile.TemporaryDirectory(prefix='sail-output-evidence-') as temporary:
        root = Path(temporary)
        work = root / 'work'
        work.mkdir()
        sail = root / 'sail'
        sail.write_bytes(b'fixture executable identity only')
        (work / 'combined-sail.out').write_bytes(b'original\x00bytes\xff\n')
        (work / 'combined-sail.rc').write_bytes(b'101\n')
        producer = work / 'cache/projects/p/c/runs/r'
        producer.mkdir(parents=True)
        (producer / 'result.json').write_bytes(b'{"run_id":"r","fixture":true}\n')
        source = work / 'demo/src'
        source.mkdir(parents=True)
        (source / 'lib.rs').write_bytes(b'original source\n')
        target = work / 'target-sail'
        target.mkdir()
        (target / 'binary').write_bytes(b'excluded executable payload')
        assert collector.capture(work, root / 'exact', 'loop', 17, sail) == 0
        index = json.loads((root / 'exact/index.json').read_text())
        assert index['original_exit'] == 17 and index['capture_complete']
        assert (root / 'exact/files/combined-sail.out').read_bytes() == b'original\x00bytes\xff\n'
        assert (root / 'exact/files/cache/projects/p/c/runs/r/result.json').read_bytes() == (producer / 'result.json').read_bytes()
        assert not any('binary' in row['path'] for row in index['files'])
        # Missing producer executable must not prevent preserving failed streams.
        assert collector.capture(work, root / 'missing-binary', 'loop', 17, root / 'missing') == 1
        assert (root / 'missing-binary/files/combined-sail.out').read_bytes() == b'original\x00bytes\xff\n'
        # These are ordinary fixture files, never private config/toolchain paths.
        external = root / 'outside'
        external.write_bytes(b'external bytes must not be copied')
        (work / 'external.err').symlink_to(external)
        os.mkfifo(work / 'pipe.out')
        assert collector.capture(work, root / 'refusal', 'loop', 17, sail) == 1
        refused = json.loads((root / 'refusal/index.json').read_text())
        assert any(row['path'] == 'pipe.out' and not row['copied'] for row in refused['files'])
        assert not (root / 'refusal/files/external.err').exists()
        (work / 'pipe.out').unlink()
        (work / 'external.err').unlink()
        # A bounded prefix remains indexed as incomplete, never silently dropped.
        limits = collector.MAX_FILE_BYTES, collector.MAX_TOTAL_BYTES
        collector.MAX_FILE_BYTES, collector.MAX_TOTAL_BYTES = 4, 32
        try:
            assert collector.capture(work, root / 'bounded', 'loop', 17, sail) == 1
        finally:
            collector.MAX_FILE_BYTES, collector.MAX_TOTAL_BYTES = limits
        bounded = json.loads((root / 'bounded/index.json').read_text())
        assert not bounded['capture_complete'] and any(row.get('truncated') for row in bounded['files'])
        workspace = work / 'ws'
        for member in ('a', 'b'):
            (workspace / member / 'src').mkdir(parents=True)
            (workspace / member / 'src/lib.rs').write_bytes((member + ' original source').encode())
            (workspace / member / 'Cargo.toml').write_text('[package]')
        assert collector.snapshot(work, workspace, 'workspace-plain', 'start') == 0
        assert (work / 'commands/workspace-plain.b.src.lib.rs').read_bytes() == b'b original source'
        assert (work / 'commands/workspace-plain.a.Cargo.toml').read_bytes() == b'[package]'
        # An ordinary vstd JSON result exceeds the former 1 MiB duplicate cap.
        ordinary = b'{"fixture":"json"}\n' * 100000
        assert len(ordinary) > 1024**2
        (work / 'ordinary.out').write_bytes(ordinary)
        assert collector.snapshot(work, work, 'ordinary', 'finish') == 0
        positive = json.loads((work / 'commands/ordinary.finish.json').read_text())
        assert positive['complete'] and not positive['errors']
        assert (work / 'commands/ordinary.raw.out').read_bytes() == ordinary
        assert (work / 'ordinary.out').read_bytes() == ordinary
        # Preserve both the per-file bound and the fixture-wide 32 MiB bound.
        with (work / 'large.out').open('wb') as stream:
            stream.truncate(collector.MAX_FILE_BYTES + 1)
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            assert collector.snapshot(work, work, 'large', 'finish') == 1
        bounded_snapshot = json.loads((work / 'commands/large.finish.json').read_text())
        assert not bounded_snapshot['complete'] and bounded_snapshot['files'][0]['truncated']
        assert (work / 'commands/large.raw.out').stat().st_size == collector.MAX_FILE_BYTES
        reason = json.loads(warning.getvalue())
        assert reason['source'] == 'large.out' and reason['stage'] == 'finish'
        assert reason['source_bytes'] == collector.MAX_FILE_BYTES + 1
        assert reason['retained_bytes'] == collector.MAX_FILE_BYTES and reason['truncated']
        assert 'snapshot truncated' in reason['reason'] and len(warning.getvalue()) < 2048
        used = sum(q.stat().st_size for q in (work / 'commands').iterdir()
                   if q.is_file() and not q.is_symlink())
        with (work / 'commands/already-recorded').open('wb') as stream:
            stream.truncate(32 * 1024**2 - used - 512)
        global_bytes = b'fixture global cap' * 100
        (work / 'global.out').write_bytes(global_bytes)
        warning = io.StringIO()
        with contextlib.redirect_stderr(warning):
            assert collector.snapshot(work, work, 'global', 'finish') == 1
        global_snapshot = json.loads((work / 'commands/global.finish.json').read_text())
        assert not global_snapshot['complete'] and global_snapshot['files'][0]['truncated']
        assert (work / 'commands/global.raw.out').read_bytes() == global_bytes[:512]
        reason = json.loads(warning.getvalue())
        assert reason['source'] == 'global.out' and reason['retained_bytes'] == 512
        assert reason['source_bytes'] == len(global_bytes) and reason['truncated']
        assert len(warning.getvalue()) < 2048
        (work / 'metadata-errors.err').write_text('large snapshot incomplete')
        assert collector.capture(work, root / 'snapshot-incomplete', 'loop', 17, sail) == 1
        print('output evidence: exact raw/producers, missing executable, FIFO/external refusal and indexed size limits pass')


def selection_controls():
    with tempfile.TemporaryDirectory(prefix='sail-output-selection-') as temporary:
        root = Path(temporary)
        work = root / 'work'
        commands = work / 'commands'
        commands.mkdir(parents=True)
        sail = root / 'sail'
        sail.write_bytes(b'fixture executable identity only')
        # Use the real count cap: old duplicate snapshots must not displace
        # either the latest command or original producer observations.
        for index in range(collector.MAX_FILES + 8):
            (commands / f'old-{index:04}.argv').write_bytes(b'old command\x00')
        (commands / 'last-sail.argv').write_bytes(b'original last command\x00')
        (work / 'last-sail.err').write_bytes(b'original last stderr\n')
        (work / 'noop.trace').write_bytes(b'not recorded: original producer unavailable\n')
        for run in ('older', 'newer'):
            producer = work / 'cache/projects/p/c/runs' / run
            producer.mkdir(parents=True)
            (producer / 'result.json').write_text(json.dumps({'run_id': run}))
            (producer / 'cargo.stderr').write_bytes(b'original producer stderr\n')
        assert collector.capture(work, root / 'evidence', 'loop', 17, sail, 'last-sail') == 1
        output = root / 'evidence/files'
        assert (output / 'commands/last-sail.argv').read_bytes() == b'original last command\x00'
        assert (output / 'last-sail.err').read_bytes() == b'original last stderr\n'
        assert (output / 'noop.trace').read_bytes() == b'not recorded: original producer unavailable\n'
        for run in ('older', 'newer'):
            assert (output / 'cache/projects/p/c/runs' / run / 'result.json').is_file()
        index = json.loads((root / 'evidence/index.json').read_text())
        assert index['original_exit'] == 17 and not index['capture_complete']
        assert len(index['files']) == collector.MAX_FILES
        assert 'selected-file count limit reached' in '\n'.join(index['errors'])
        assert index['last_command'] == 'last-sail'
        assert index['limits']['files'] == 1024
        print('output evidence: last command and producer records survive old snapshot overflow under the original cap')


VERIFIER = r'''#!PYTHON
import json,os,sys,time
from pathlib import Path
state=Path(os.environ['SAIL_E2E_TEST_STATE'])
a=sys.argv[1:]
if a[:2]==['engine','status']:print(json.dumps({'rust_toolchain':'sail-test-pin'}));sys.exit(0)
if a[:2]==['verus','new']:
 state.write_text(os.getcwd());p=Path('demo/src');p.mkdir(parents=True);(p/'lib.rs').write_text('fixture');Path('demo/Cargo.toml').write_text('[package]\nname="demo"\nversion="0.0.0"\n');sys.exit(0)
if a[:1]==['clean']:sys.exit(0)
if a[:1]==['generate-lockfile']:
 if os.environ['SAIL_E2E_TEST_CASE'] in ['child-failed','both-failed','relative-child-failed']:print('original fixture child failure',file=sys.stderr);sys.exit(23)
 Path('Cargo.lock').write_text('fixture lock');sys.exit(0)
assert 'SAIL_E2E_OUTPUT_OUT' not in os.environ, 'diagnostic path reached verifier'
is_sail=Path(sys.argv[0]).name=='sail'
# Stock runs with the toolchain Sail selects (ADR 0058); Sail selects it itself.
assert os.environ.get('RUSTUP_TOOLCHAIN')==(None if is_sail else 'sail-test-pin'), os.environ.get('RUSTUP_TOOLCHAIN')
if is_sail:
 run=str(time.time_ns());p=Path(os.environ['SAIL_CACHE_DIR'])/'projects/p/c/runs'/run;p.mkdir(parents=True)
 (p/'result.json').write_text(json.dumps({'run_id':run,'fixture':True}));(p/'cargo.stderr').write_bytes(b'original producer bytes\n')
source=Path('src/lib.rs').read_text()
if '--verify-function' in a:os.write(2,b'Partial verification must use focus\n');sys.exit(101)
if 'pub fn broken' in source:os.write(2,b'cannot find value y\n');sys.exit(101)
# The compiler-visible input witnesses (#385, #393) all fail.
if 'witness::witnesses' in source:
 os.write(1,b'verification results:: 0 verified, 3 errors\n');os.write(2,b'error: assertion failed\nerror: could not compile `demo`\n');sys.exit(101)
if is_sail and os.environ['SAIL_E2E_TEST_CASE'] in ['mismatch','collector-failed']:os.write(1,b'intentional fixture mismatch\n')
if 'DisplaySpecImpl' in source:
 os.write(2,b'[rust_verify/src/verifier.rs:113:17] &sp.as_string = "/rustc/fixture/library/core/src/fmt/mod.rs:1:1: 1:2 (#1)"\nerror: could not compile `demo`\n');sys.exit(101)
# The seeded-mutation smoke's failing mutants.
if 'ensures r > x,' in source or 'r == a && r == b' in source or ('fn sum_to' in source and 's = s + i as u64;' not in source):
 os.write(1,b'verification results:: 1 verified, 1 errors\n');os.write(2,b'error: postcondition not satisfied\nerror: could not compile `demo`\n');sys.exit(101)
os.write(1,b'verification results:: 2 verified, 0 errors\n')
'''


def shell_controls():
    for case in ('success', 'mismatch', 'child-failed', 'collector-failed', 'both-failed', 'relative-child-failed'):
        with tempfile.TemporaryDirectory(prefix='sail-output-shell-') as temporary:
            root = Path(temporary)
            tools = root / 'tools'
            tools.mkdir()
            for name in ('cargo', 'sail'):
                binary = tools / name
                binary.write_text(VERIFIER.replace('PYTHON', sys.executable))
                binary.chmod(0o755)
            # The pinned toolchain is installed with rust-src.
            rustup = tools / 'rustup'
            rustup.write_text('#!' + sys.executable + '''
import os,sys
assert sys.argv[1:]==['component','list','--installed','--toolchain','sail-test-pin'], sys.argv
assert os.environ.get('RUSTUP_AUTO_INSTALL')=='0'
print('cargo-x86_64-unknown-linux-gnu')
print('rust-src')
''')
            rustup.chmod(0o755)
            # The harness edits fixtures with portable sed (no GNU `-i`), so
            # the system sed runs unchanged on Linux and macOS.
            timeout = tools / 'timeout'
            timeout.write_text('#!' + sys.executable + '''
import os,sys
a=sys.argv[1:]
while a[0].startswith('--'):a=a[1:]
a=a[1:];os.execvp(a[0],a)
''')
            timeout.chmod(0o755)
            # Select the same Python as this test without changing HOME/toolchains.
            python = tools / 'python3'
            python.symlink_to(sys.executable)
            # The smoke's per-function verdict checker, which needs real JSON.
            check_output = tools / 'check_output'
            check_output.write_text('#!/bin/sh\nexit 0\n')
            check_output.chmod(0o755)
            evidence = root / ('relative-evidence' if case == 'relative-child-failed' else 'evidence')
            if case in ('collector-failed', 'both-failed', 'relative-child-failed'):
                evidence.write_text('intentional destination failure')
            env = {key: value for key, value in os.environ.items() if key != 'RUSTUP_TOOLCHAIN'} | {
                'PATH': str(tools) + os.pathsep + os.environ['PATH'],
                'SAIL_E2E_OUTPUT_OUT': evidence.name if case == 'relative-child-failed' else str(evidence),
                'SAIL_E2E_TEST_CASE': case,
                'SAIL_E2E_TEST_STATE': str(root / 'fixture-work'),
                'SAIL_E2E_CHECK_OUTPUT': str(check_output),
            }
            result = subprocess.run(['bash', str(HERE / 'test-output.sh'), str(tools / 'sail'), 'loop'],
                                    env=env, cwd=root, capture_output=True, timeout=30)
            expected = 0 if case == 'success' else 23 if case in ('child-failed', 'both-failed', 'relative-child-failed') else 1
            assert result.returncode == expected, (case, result.returncode, result.stderr)
            assert not Path((root / 'fixture-work').read_text()).exists(), 'normal fixture cleanup must always run'
            if case == 'success':
                assert not evidence.exists(), 'passing run should leave no evidence'
            elif case in ('collector-failed', 'both-failed', 'relative-child-failed'):
                assert b'failure evidence collector exited' in result.stderr
            else:
                captures = list(evidence.glob('loop-*'))
                assert len(captures) == 1, (case, result.stderr, list(evidence.glob("*")))
                record = captures[0]
                index = json.loads((record / 'index.json').read_text())
                assert index['original_exit'] == expected and index['capture_complete']
                if case == 'mismatch':
                    assert index['last_command'] == 'combined-sail'
                    raw = record / 'files/combined-sail.out'
                    assert b'intentional fixture mismatch' in raw.read_bytes()
                    assert (record / 'files/combined-sail.rc').read_bytes() == b'101\n'
                    command = (record / 'files/commands/combined-sail.argv').read_bytes().split(b'\0')
                    assert command == [str(tools / 'sail').encode(), b'verify', b'']
                    assert (record / 'files/commands/combined-sail.raw.out').read_bytes() == raw.read_bytes()
                    assert any(row['path'].endswith('/result.json') for row in index['files'])
                else:
                    assert index['last_command'] is None, 'setup failed before any recorded verifier command'
            print('output evidence: actual shell', case, 'preserves original exit and cleanup')


if __name__ == '__main__':
    filesystem_controls()
    selection_controls()
    shell_controls()
