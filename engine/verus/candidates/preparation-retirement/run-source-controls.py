from pathlib import Path
import hashlib,json,os,subprocess,time,tarfile,signal
OUT=Path('/tmp/sail-m7-preparation-retirement-source-controls');OUT.mkdir(exist_ok=True)
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
assert os.getpriority(os.PRIO_PROCESS,0)==0
assert not (OUT/'result.json').exists(), 'Retained scope cannot be overwritten'
stat=os.statvfs(SOURCE)
save('resource-preflight.json',{'free_bytes':stat.f_bavail*stat.f_frsize,'nice':0,'unix_ns':time.time_ns(),'new_target_copy':False})

assert sha(OUT/'delta.patch')==IDENTITY['delta_sha256']
assert sha(OUT/'base-full.patch')==IDENTITY['base_full_patch_sha256']
old=json.loads(Path('/tmp/sail-m7-preparation-namespace-boundary-raw-owner-source-controls/identity.json').read_text())
assert sha(Path('/tmp/sail-m7-preparation-namespace-boundary-raw-owner-source-controls/result.json'))=='6cf37325c65a5c7607fd79625801973b3f7ccce8647364d7641b73dfecfee62a'
assert json.loads(Path('/tmp/sail-m7-preparation-namespace-boundary-raw-owner-source-controls/result.json').read_text())['status']=='PASS'
assert all(sha(SOURCE/name)==digest for name,digest in old['source_sha256'].items())
assert sha(SOURCE/'Cargo.lock')==IDENTITY['lock_sha256']
assert subprocess.check_output(['git','-C',str(SOURCE),'rev-parse','HEAD'],text=True).strip()==BASE
prior=seals();save('prior-runtime-before.json',prior)
with tarfile.open(OUT/'before-source8-lock.tar','w') as archive:
 for name in [*old['source_sha256'],'Cargo.lock']:archive.add(SOURCE/name,arcname=name)
save('before-source8-lock.json',{'source_sha256':old['source_sha256'],'lock_sha256':sha(SOURCE/'Cargo.lock'),'backup_sha256':sha(OUT/'before-source8-lock.tar')})
with (OUT/'apply.log').open('wb') as log:
 for command in [ ['git','-C',str(SOURCE),'apply','--check',str(OUT/'delta.patch')], ['git','-C',str(SOURCE),'apply',str(OUT/'delta.patch')], ['git','-C',str(SOURCE),'apply','--reverse','--check',str(OUT/'delta.patch')] ]:
  subprocess.run(command,stdout=log,stderr=subprocess.STDOUT,check=True)
assert all(sha(SOURCE/name)==digest for name,digest in IDENTITY['source_sha256'].items())
assert sha(SOURCE/'Cargo.lock')==IDENTITY['lock_sha256']
save('applied-source.json',IDENTITY)
env=dict(os.environ,RUSTUP_TOOLCHAIN='1.98.1',RUSTC_BOOTSTRAP='1',VERUS_IN_VARGO='1',VARGO_TARGET_DIR=str(TARGET/'release'),CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='2',RUSTFLAGS='--cfg proc_macro_span --cfg verus_keep_ghost --cfg span_locations --cfg procmacro2_semver_exempt')
removed=[name for name in env if name.startswith('SAIL_ENGINE')]
for name in removed:env.pop(name)
save('execution.json',{'toolchain':'1.98.1','nice':0,'jobs':2,'case_workers':1,'removed_engine_selector_names':sorted(removed),'new_target_copy':False,'scope':'actual library and strict source controls only; no runtime build/vstd/proof/qualification'})
phases=[];child=None;started=time.monotonic()
try:
 for package in ['vir','rust_verify']:
  for mode in ['test','clippy']:
   command=['cargo','+1.98.1',mode,'-p',package,'--release']
   command += ['--lib','sail_','--','--test-threads=1'] if mode=='test' else ['--all-targets','--','-D','warnings']
   log=OUT/(package+'-'+mode+'.log');phase_start=time.monotonic()
   with log.open('wb') as output:
    child=subprocess.Popen(command,cwd=SOURCE,env=env,stdout=output,stderr=subprocess.STDOUT,start_new_session=True)
    fields=(Path('/proc')/str(child.pid)/'stat').read_text().rsplit(') ',1)[1].split()
    save('current-actor.json',{'pid':child.pid,'pgid':os.getpgid(child.pid),'start_ticks':fields[19],'command':command,'nice':os.getpriority(os.PRIO_PROCESS,child.pid),'start_unix_ns':time.time_ns()})
    code=child.wait();child=None
   phases.append({'name':package+'-'+mode,'command':command,'exit_code':code,'elapsed_seconds':time.monotonic()-phase_start,'log_sha256':sha(log)})
   save('progress.json',phases)
   print(json.dumps(phases[-1]),flush=True)
   if code:raise RuntimeError(package+' '+mode+' failed')
 save('result.json',{'status':'PASS','phases':phases,'elapsed_seconds':time.monotonic()-started})
except BaseException as error:
 if child is not None:
  try:os.killpg(child.pid,signal.SIGINT);child.wait(timeout=5)
  except subprocess.TimeoutExpired:os.killpg(child.pid,signal.SIGKILL);child.wait()
  except ProcessLookupError:pass
 save('result.json',{'status':'FAIL','error_type':type(error).__name__,'error':str(error),'phases':phases,'elapsed_seconds':time.monotonic()-started})
 raise
finally:
 final=seals();save('prior-runtime-after.json',final);assert final==prior
 current={name:sha(SOURCE/name) for name in IDENTITY['source_sha256']}
 save('source-closure.json',{'source_sha256':current,'lock_sha256':sha(SOURCE/'Cargo.lock')})
 assert current==IDENTITY['source_sha256'];assert sha(SOURCE/'Cargo.lock')==IDENTITY['lock_sha256']
