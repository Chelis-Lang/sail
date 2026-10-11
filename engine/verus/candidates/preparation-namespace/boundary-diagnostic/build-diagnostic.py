#!/usr/bin/env python3
"""Build one unsealed diagnostic driver image; no proof or runtime qualification."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

SOURCE = Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source')
CONTROLS = Path('/tmp/sail-m7-preparation-namespace-boundary-raw-owner-source-controls')
DONOR = Path('/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-b3b07d47d7542817')
OUT = Path('/tmp/sail-m7-preparation-namespace-boundary-diagnostic-build')


def sha(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')


def seals():
    prior = json.loads((CONTROLS / 'prior-runtime-after.json').read_text())
    for variant, record in prior.items():
        root = DONOR.parent / ('426d8b01e7ffb910a36c15e1f870ff985bb61eef-' + variant)
        assert sha(root / 'sail-build.json') == record['manifest_sha256']
        runtime = root / 'verus/source/target-verus/release'
        assert {str(p.relative_to(root)) for p in runtime.iterdir() if p.is_file()} == set(record['artifacts'])
        assert all(not (root / name).is_symlink() and sha(root / name) == digest for name, digest in record['artifacts'].items())
        assert all(sha(root / 'verus/source' / name) == digest for name, digest in record['source_sha256'].items())
        assert sha(root / 'verus/source/Cargo.lock') == record['lock_sha256']
    return prior


def main():
    assert os.getpriority(os.PRIO_PROCESS, 0) == 0
    identity = json.loads((CONTROLS / 'identity.json').read_text())
    assert identity['full_patch_sha256'] == '3892328a418109596f6a4a5f3649f5e7d370b19321419c29780d7a3961b44d61'
    assert sha(CONTROLS / 'result.json') == '6cf37325c65a5c7607fd79625801973b3f7ccce8647364d7641b73dfecfee62a'
    assert json.loads((CONTROLS / 'result.json').read_text())['status'] == 'PASS'
    assert all(sha(SOURCE / name) == digest for name, digest in identity['source_sha256'].items())
    assert sha(SOURCE / 'Cargo.lock') == identity['lock_sha256']
    assert sha(CONTROLS / 'full.patch') == identity['full_patch_sha256']
    subprocess.run(['git', '-C', str(SOURCE), 'apply', '--reverse', '--check', str(CONTROLS / 'full.patch')], check=True)
    before = seals()
    OUT.mkdir(exist_ok=False)
    save('prior-runtime-before.json', before)
    save('source-identity.json', identity)
    env = {k: v for k, v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
    env.update(RUSTUP_TOOLCHAIN='1.98.1', RUSTC_BOOTSTRAP='1', VERUS_IN_VARGO='1',
               VARGO_TARGET_DIR=str(SOURCE / 'target-verus/release'), CARGO_TARGET_DIR=str(SOURCE / 'target-verus'),
               CARGO_BUILD_JOBS='2', RUSTFLAGS='--cfg proc_macro_span --cfg verus_keep_ghost --cfg span_locations --cfg procmacro2_semver_exempt')
    command = ['cargo', '+1.98.1', 'build', '-p', 'rust_verify', '--release', '--bin', 'rust_verify']
    record = {'scope': 'Unsealed diagnostic driver/image only; no fresh vstd, manifest, proof or qualification',
              'status': 'running', 'command': command, 'nice': 0, 'jobs': 2,
              'controller_sha256': sha(Path(__file__)), 'start_unix_ns': time.time_ns(),
              'native_free_bytes': shutil.disk_usage(SOURCE).free, 'source_controls_sha256': sha(CONTROLS / 'result.json')}
    save('build.json', record)
    started = time.monotonic()
    child = None
    try:
        with (OUT / 'build.log').open('wb') as log:
            child = subprocess.Popen(command, cwd=SOURCE, env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            fields = (Path('/proc') / str(child.pid) / 'stat').read_text().rsplit(') ', 1)[1].split()
            record.update(pid=child.pid, pgid=os.getpgid(child.pid), start_ticks=fields[19])
            save('build.json', record)
            code = child.wait()
            child = None
        record.update(exit_code=code, log_sha256=sha(OUT / 'build.log'))
        assert code == 0, 'Direct driver build failed'
        image = OUT / 'runtime'
        image.mkdir()
        manifest = json.loads((DONOR / 'sail-build.json').read_text())
        assert len(manifest['artifacts']) == 13
        for name, digest in manifest['artifacts'].items():
            original = DONOR / name
            destination = image / original.name
            assert not destination.exists()
            shutil.copy2(original, destination)
            assert sha(destination) == digest and destination.stat().st_ino != original.stat().st_ino
        shutil.copy2(SOURCE / 'target-verus/release/rust_verify', image / 'rust_verify')
        files = {p.name: sha(p) for p in image.iterdir()}
        assert len(files) == 13 and not (image / 'sail-build.json').exists()
        save('diagnostic-image.json', {'unsealed': True, 'qualified': False, 'copied_receipts': 0,
             'donor_manifest_sha256': sha(DONOR / 'sail-build.json'), 'donor_artifacts': manifest['artifacts'],
             'diagnostic_artifacts': files, 'changed_file': 'rust_verify', 'driver_sha256': files['rust_verify']})
        record['status'] = 'PASS'
    except BaseException as error:
        if child is not None:
            try:
                os.killpg(child.pid, signal.SIGINT)
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait()
            except ProcessLookupError:
                pass
        record.update(status='FAIL', error_type=type(error).__name__, error=str(error))
        raise
    finally:
        record['elapsed_seconds'] = time.monotonic() - started
        save('build.json', record)
        after = seals()
        save('prior-runtime-after.json', after)
        assert after == before
        assert all(sha(SOURCE / name) == digest for name, digest in identity['source_sha256'].items())
        assert sha(SOURCE / 'Cargo.lock') == identity['lock_sha256']


if __name__ == '__main__':
    main()
