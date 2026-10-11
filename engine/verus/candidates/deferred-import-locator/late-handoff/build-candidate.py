from pathlib import Path
import hashlib
import json
import os
import shutil
import signal
import subprocess
import tarfile
import time

OUT = Path('/tmp/sail-m7-late-handoff-build-evidence')
PATCH_SHA = 'bb9cf6ab85b3316b5a90551dd615864bd564b5e052c0d767fa2b97fb5ba84372'
LOCK_SHA = '653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1'
COMMIT = '426d8b01e7ffb910a36c15e1f870ff985bb61eef'
SUFFIX = 'ba9253e0e3b8c136'
OLD = Path('/root/.cache/sail/verus-src-build') / (COMMIT + '-0bbc7d378544f972')
BUILD = OLD.parent / (COMMIT + '-' + SUFFIX)
SNAPSHOT = Path('/tmp/sail-m7-original-descriptor-build-candidate')

def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def save(name, value):
    (OUT / name).write_text(json.dumps(value, indent=2) + '\n')

freeze=json.loads((OUT/'build-input-freeze.json').read_text())
assert sha(OUT/'builder-source.tar')==freeze['builder_archive_sha256']
assert freeze['patch_sha256']==PATCH_SHA
identity = json.loads((OUT / 'identity.json').read_text())
patch = OUT / 'candidate.patch'
assert sha(patch) == identity['full_patch_sha256'] == PATCH_SHA
assert sha(OUT / 'candidate-delta.patch') == identity['delta_sha256']
assert sha(SNAPSHOT / 'engine/verus/Cargo.lock') == LOCK_SHA
# Verify the complete retained private builder source, not merely its patch.
archive_sources={}
with tarfile.open(OUT/'builder-source.tar','r') as archived:
    for member in archived.getmembers():
        relative=Path(member.name)
        assert not relative.is_absolute() and '..' not in relative.parts
        if member.isfile():
            stream=archived.extractfile(member);assert stream is not None
            digest=hashlib.sha256()
            for chunk in iter(lambda:stream.read(1024*1024),b''):digest.update(chunk)
            archive_sources[member.name]=('file',digest.hexdigest())
        elif member.issym():archive_sources[member.name]=('link',member.linkname)
        else:assert member.isdir()
actual_sources={}
for entry in SNAPSHOT.rglob('*'):
    relative=entry.relative_to(SNAPSHOT)
    if any(part in {'.git','target','target-verus','__pycache__'} for part in relative.parts):continue
    if entry.is_symlink():actual_sources[str(relative)]=('link',os.readlink(entry))
    elif entry.is_file():actual_sources[str(relative)]=('file',sha(entry))
assert actual_sources==archive_sources
save('builder-archive-validation.json',{'archive_sha256':sha(OUT/'builder-source.tar'),'entry_count':len(actual_sources),'complete_source_match':True})
for relative, digest in identity['source_sha256'].items():
    assert sha(OUT / 'source' / relative) == digest
assert not BUILD.exists()
assert os.getpriority(os.PRIO_PROCESS, 0) == 0
CONTROL=Path('/tmp/sail-m7-late-handoff-source-controls')
assert freeze['source_control_inputs']
for name,digest in freeze['source_control_inputs'].items():
    assert sha(CONTROL/name)==digest
checks=json.loads((CONTROL/'result.json').read_text())
assert checks['status']=='PASS' and len(checks['phases'])==4
expected_phases={'vir-test','vir-clippy','rust_verify-test','rust_verify-clippy'}
assert {p['name'] for p in checks['phases']}==expected_phases
for phase in checks['phases']:
    assert phase['exit_code']==0 and sha(CONTROL/(phase['name']+'.log'))==phase['log_sha256']
    package,mode=phase['name'].split('-')
    expected=['cargo','+1.98.1',mode,'-p',package,'--release']
    expected+=['--lib','sail_','--','--test-threads=1'] if mode=='test' else ['--all-targets','--','-D','warnings']
    assert phase['command']==expected
seed_source=Path('/tmp/sail-m7-original-descriptor-source-controls')/'verus/source'
assert sha(seed_source/'Cargo.lock')==LOCK_SHA
for relative,digest in identity['source_sha256'].items():assert sha(seed_source/relative)==digest
protected_targets=[Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source/target-verus'),Path('/tmp/sail-m7-engine-xtask-target')]
holders=[]
unreadable_live=[]
for proc in Path('/proc').iterdir():
    if not proc.name.isdigit() or int(proc.name)==os.getpid(): continue
    try:
        stat=(proc/'stat').read_text();state=stat[stat.rfind(')')+2:].split()[0]
        if state=='Z':continue
        command=(proc/'cmdline').read_bytes()[:65536]
        cwd=(proc/'cwd').resolve(strict=True)
        settings=(proc/'environ').read_bytes().split(b'\0')
        target=next((item.removeprefix(b'CARGO_TARGET_DIR=') for item in settings if item.startswith(b'CARGO_TARGET_DIR=')),b'')
        for path in protected_targets:
            if str(path).encode() in command or target==str(path).encode() or cwd.is_relative_to(path): holders.append({'pid':int(proc.name),'scope':str(path)})
        for fd in (proc/'fd').iterdir():
            try:
                opened=fd.resolve(strict=True)
                if any(opened.is_relative_to(path) for path in protected_targets): holders.append({'pid':int(proc.name),'scope':'owned compiler-target open FD'})
            except FileNotFoundError:pass
            except (OSError,RuntimeError):
                if fd.exists():unreadable_live.append({'pid':int(proc.name),'kind':'fd'})
    except FileNotFoundError:pass
    except (OSError,RuntimeError):
        if proc.exists():unreadable_live.append({'pid':int(proc.name),'kind':'process'})
assert not holders,holders
assert not unreadable_live,unreadable_live
save('resource-preflight.json',{'version':1,'owned_target_holders':holders,'unreadable_live':unreadable_live,'private_target':'/tmp/sail-m7-engine-xtask-target','native_disk_free_bytes':shutil.disk_usage(OUT).free,'scope':'Read-only holder audit before compiler seed and launch; unrelated actors untouched'})

sealed = {}
for suffix in ['73bd8aa61b18ca12','0bbc7d378544f972']:
    old = OLD.parent / (COMMIT + '-' + suffix)
    raw = (old / 'sail-build.json').read_bytes()
    manifest = json.loads(raw)
    assert len(manifest['artifacts'])==13
    assert all(sha(old / name) == digest for name, digest in manifest['artifacts'].items())
    runtime=old/'verus/source/target-verus/release'
    assert {str(p.relative_to(old)) for p in runtime.iterdir() if p.is_file()}==set(manifest['artifacts'])
    sealed[suffix] = {'manifest_sha256': hashlib.sha256(raw).hexdigest(),
                      'artifacts': manifest['artifacts'], 'lock_sha256':sha(old/'verus/source/Cargo.lock'),
                      'source_sha256':{name:sha(old/'verus/source'/name) for name in identity['source_sha256']}}
save('preceding-runtime-seals.json', sealed)

started = time.monotonic()
archive = OUT / 'builder-before.tar'
with tarfile.open(archive, 'w') as tar:
    for entry in sorted(SNAPSHOT.rglob('*')):
        relative = entry.relative_to(SNAPSHOT)
        if any(part in {'.git', 'target', 'target-verus'} for part in relative.parts):
            continue
        if entry.is_file() or entry.is_symlink():
            tar.add(entry, arcname=str(relative), recursive=False)
assert sha(SNAPSHOT / 'engine/verus/interactive.patch') == '1ad773b66e1b58dac62936499e37d81179e1138565cc1774d4a79d168fe935a8'
save('builder-before.json', {'archive_sha256': sha(archive), 'bytes': archive.stat().st_size,
                            'elapsed_seconds': time.monotonic() - started,
                            'preceding_patch_sha256': sha(SNAPSHOT / 'engine/verus/interactive.patch'),
                            'source_lock_sha256': LOCK_SHA})
(SNAPSHOT / 'engine/verus/interactive.patch').write_bytes(patch.read_bytes())

BUILD.mkdir()
with (OUT / 'setup.log').open('wb') as log:
    subprocess.run(['git', 'clone', '--no-hardlinks', '--no-checkout', str(OLD / 'verus'),
                    str(BUILD / 'verus')], stdout=log, stderr=subprocess.STDOUT, check=True)
    subprocess.run(['git', '-C', str(BUILD / 'verus'), 'checkout', '--detach', COMMIT],
                   stdout=log, stderr=subprocess.STDOUT, check=True)
    subprocess.run(['git', '-C', str(BUILD / 'verus'), 'remote', 'set-url', 'origin',
                    'https://github.com/verus-lang/verus.git'], stdout=log, stderr=subprocess.STDOUT, check=True)
    subprocess.run(['git', '-C', str(BUILD / 'verus'), 'apply', '--check', str(patch)],
                   stdout=log, stderr=subprocess.STDOUT, check=True)
    subprocess.run(['git', '-C', str(BUILD / 'verus'), 'apply', str(patch)],
                   stdout=log, stderr=subprocess.STDOUT, check=True)
(BUILD / 'verus/.sail-fetched').write_text(COMMIT)
for relative, digest in identity['source_sha256'].items():
    assert sha(BUILD / 'verus/source' / relative) == digest
save('applied-source.json', {'patch_sha256': PATCH_SHA, 'source_files': identity['source_sha256'],
                            'source_commit': COMMIT, 'checked_against_frozen_inputs': True})

# Ordinary compiler-cache setup only. No runtime, receipt, proof or seal copy.
# Changed pinned source crates must compile; copies have distinct inodes.
started = time.monotonic()
source = Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source/target-verus/release')
destination = BUILD / 'verus/source/target-verus/release'
copied, excluded = [], []
for dirname in ['deps', 'build', '.fingerprint']:
    for path in sorted((source / dirname).rglob('*')):
        if not path.is_file():
            continue
        relative = path.relative_to(source)
        if relative.parts[1].startswith(('vir-', 'libvir-', 'vir_macros-', 'libvir_macros-',
                                         'vir-macros-', 'rust_verify-', 'librust_verify-', 'vstd-', 'libvstd-')):
            excluded.append(str(relative))
            continue
        assert not path.is_symlink()
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
        digest = sha(path)
        assert sha(target) == digest
        assert path.stat().st_ino != target.stat().st_ino
        copied.append({'path': str(relative), 'bytes': target.stat().st_size, 'sha256': digest})
save('compiler-seed.json', {'version': 1, 'scope': 'Inactive compiler intermediates only',
                            'files': copied, 'excluded_changed_crate_paths': excluded,
                            'elapsed_seconds': time.monotonic() - started,
                            'runtime_or_proof_or_seal_copied': False, 'hardlinks': False})
for suffix, record in sealed.items():
    old = OLD.parent / (COMMIT + '-' + suffix)
    assert sha(old / 'sail-build.json') == record['manifest_sha256']
    assert all(sha(old / name) == digest for name, digest in record['artifacts'].items())

env = {key: value for key, value in os.environ.items()
       if not key.startswith(('SAIL_ENGINE', 'SAIL_JOB', 'SAIL_PROOF', 'SAIL_MANAGED'))
       and key not in {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS'}}
env.update(CARGO_BUILD_JOBS='2', SAIL_BUILD_JOBS='2',
           CARGO_TARGET_DIR='/tmp/sail-m7-engine-xtask-target', CARGO_HOME='/opt/cargo',
           RUSTUP_HOME='/opt/rustup', RUSTUP_TOOLCHAIN='1.99.0',
           SAIL_CACHE_DIR='/root/.cache/sail', SAIL_VERUS_HOME='/opt/verus')
started = time.monotonic()
with (OUT / 'ordinary-build.log').open('wb') as log:
    child = subprocess.Popen(['cargo', 'xtask', 'build-verus-engine', '--keep-intermediates'],
                             cwd=SNAPSHOT, env=env, stdout=log, stderr=subprocess.STDOUT,
                             start_new_session=True)
    actor = {'pid': child.pid, 'pgid': os.getpgid(child.pid),
             'nice': os.getpriority(os.PRIO_PROCESS, child.pid), 'jobs': 2, 'source_jobs': 2,
             'started_unix_ns': time.time_ns(), 'patch_sha256': PATCH_SHA,
             'variant_suffix': SUFFIX, 'execution': 'linux-x86_64-container-emulated-sharedload',
             'concurrent_work': 'Root/A common gate and independent team work may overlap; launch timestamps and actual actors retained without causal load claim'}
    assert actor['nice'] == 0
    save('ordinary-build-actor.json', actor)
    print(json.dumps(actor), flush=True)
    try:
        code = child.wait()
    except BaseException:
        try:
            os.killpg(child.pid, signal.SIGINT)
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
        except ProcessLookupError:
            pass
        save('ordinary-build-interruption.json', {'pid':child.pid,'pgid':child.pid,'elapsed_seconds':time.monotonic()-started,'cleanup':'only exact owned process group interrupted then bounded kill'})
        raise
result = {'version': 1, 'exit_code': code, 'elapsed_seconds': time.monotonic() - started,
          'patch_sha256': PATCH_SHA, 'scope': 'Ordinary unselected late-handoff source build; actual source controls passed, original qualification pending'}
save('ordinary-build-result.json', result)
print(json.dumps(result), flush=True)
raise SystemExit(code)
