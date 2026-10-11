from pathlib import Path
import hashlib,json,os,subprocess,time,tarfile,signal
OUT=Path('/tmp/sail-m7-no-original-range-memo-sensitivity');OUT.mkdir(exist_ok=True)
SOURCE=Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source')
TARGET=SOURCE/'target-verus'
PREFIX=Path('/root/.cache/sail/verus-src-build')
BASE='426d8b01e7ffb910a36c15e1f870ff985bb61eef'
IDENTITY=json.loads((OUT/'identity.json').read_text())
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''):h.update(chunk)
 return h.hexdigest()
def save(name,value):(OUT/name).write_text(json.dumps(value,indent=2)+'\n')
def seals():
 result={}
 for variant in ['0bbc7d378544f972','73bd8aa61b18ca12','ba9253e0e3b8c136','b3b07d47d7542817']:
  root=PREFIX/(BASE+'-'+variant);manifest=root/'sail-build.json';m=json.loads(manifest.read_bytes())
  assert len(m['artifacts'])==13
  runtime=root/'verus/source/target-verus/release'
  assert {str(p.relative_to(root)) for p in runtime.iterdir() if p.is_file()}==set(m['artifacts'])
  assert all(sha(root/name)==digest for name,digest in m['artifacts'].items())
  source=root/'verus/source'
  names=json.loads(Path('/tmp/sail-m7-preparation-namespace-visibility-source-controls/identity.json').read_text())['source_sha256'] if variant=='b3b07d47d7542817' else json.loads(Path('/tmp/sail-m7-late-handoff-source-controls/identity.json').read_text())['source_sha256']
  source_digests={name:sha(source/name) for name in names}
  if variant=='b3b07d47d7542817':assert source_digests==names
  assert sha(source/'Cargo.lock')==IDENTITY['lock_sha256']
  result[variant]={'manifest_sha256':sha(manifest),'artifacts':m['artifacts'],'source_sha256':source_digests,'lock_sha256':sha(source/'Cargo.lock')}
 return result
PLAN=json.loads((OUT/'plan.json').read_text())
POSITIVE=Path('/tmp/sail-m7-no-original-range-valid-memo-source-controls')
assert os.getpriority(os.PRIO_PROCESS,0)==0
assert not (OUT/'result.json').exists()
assert all(sha(SOURCE/n)==h for n,h in IDENTITY['source_sha256'].items())
assert sha(SOURCE/'Cargo.lock')==IDENTITY['lock_sha256']
assert sha(OUT/'full.patch')==IDENTITY['full_patch_sha256']
assert sha(OUT/'remove-only-bypass.patch')==PLAN['negative_patch_sha256']
assert sha(POSITIVE/'result.json')==PLAN['positive_result_sha256']
assert json.loads((POSITIVE/'result.json').read_text())['status']=='PASS'
prior=seals();save('prior-runtime-before.json',prior)
backup=(SOURCE/'rust_verify/src/sail_engine.rs').read_bytes()
(OUT/'positive-engine.rs').write_bytes(backup)
save('launch.json',{'pid':os.getpid(),'pgid':os.getpgid(0),'unix_ns':time.time_ns(),'nice':0,'jobs':2,'case_workers':1,'scope':'targeted memo sensitivity only; no runtime/proof/qualification'})
env=dict(os.environ,RUSTUP_TOOLCHAIN='1.98.1',RUSTC_BOOTSTRAP='1',VERUS_IN_VARGO='1',VARGO_TARGET_DIR=str(TARGET/'release'),CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='2',RUSTFLAGS='--cfg proc_macro_span --cfg verus_keep_ghost --cfg span_locations --cfg procmacro2_semver_exempt')
for name in list(env):
 if name.startswith('SAIL_ENGINE'):env.pop(name)
command=['cargo','+1.98.1','test','-p','rust_verify','--release','--lib','absent_original_range_provenance_binds_current_identity_and_skips_word_only_memo','--','--test-threads=1']
phases=[];started=time.monotonic();child=None
try:
 subprocess.run(['git','-C',str(SOURCE),'apply','--check',str(OUT/'remove-only-bypass.patch')],check=True)
 subprocess.run(['git','-C',str(SOURCE),'apply',str(OUT/'remove-only-bypass.patch')],check=True)
 assert sha(SOURCE/'rust_verify/src/sail_engine.rs')==PLAN['negative_engine_sha256']
 for phase in ['bypass-removed','positive-restored']:
  if phase=='positive-restored':(SOURCE/'rust_verify/src/sail_engine.rs').write_bytes(backup)
  log=OUT/(phase+'.log');phase_start=time.monotonic()
  with log.open('wb') as output:
   child=subprocess.Popen(command,cwd=SOURCE,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
   fields=(Path('/proc')/str(child.pid)/'stat').read_text().rsplit(') ',1)[1].split()
   save('current-actor.json',{'pid':child.pid,'pgid':os.getpgid(child.pid),'start_ticks':fields[19],'command':command,'nice':os.getpriority(os.PRIO_PROCESS,child.pid),'start_unix_ns':time.time_ns()})
   code=child.wait(timeout=180);child=None
  text=log.read_text()
  phases.append({'name':phase,'command':command,'exit_code':code,'elapsed_seconds':time.monotonic()-phase_start,'log_sha256':sha(log)})
  save('progress.json',phases)
  assert 'running 1 test' in text
  if phase=='bypass-removed':
   assert code==101 and 'assertion `left != right` failed' in text
   assert text.count('Some("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef")')==2
   assert 'test result: FAILED. 0 passed; 1 failed;' in text
  else:assert code==0 and 'test result: ok. 1 passed; 0 failed;' in text
 save('result.json',{'status':'PASS','scope':'Expected negative at unchanged inequality and restored positive only','phases':phases,'elapsed_seconds':time.monotonic()-started})
except BaseException as error:
 if child is not None:
  try:os.killpg(child.pid,signal.SIGINT);child.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
  except ProcessLookupError:pass
 save('result.json',{'status':'FAIL','error_type':type(error).__name__,'error':str(error),'phases':phases,'elapsed_seconds':time.monotonic()-started})
 raise
finally:
 (SOURCE/'rust_verify/src/sail_engine.rs').write_bytes(backup)
 final=seals();save('prior-runtime-after.json',final);assert final==prior
 current={n:sha(SOURCE/n) for n in IDENTITY['source_sha256']}
 save('source-closure.json',{'source_sha256':current,'lock_sha256':sha(SOURCE/'Cargo.lock')})
 assert current==IDENTITY['source_sha256'];assert sha(SOURCE/'Cargo.lock')==IDENTITY['lock_sha256']
