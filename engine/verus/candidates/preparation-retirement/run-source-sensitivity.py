#!/usr/bin/env python3
"""Single actual adapter counterfactual; restore source in finally; no proof."""
from pathlib import Path
import hashlib,json,os,re,shutil,signal,subprocess,time
OUT=Path('/tmp/sail-m7-preparation-retirement-source-controls')
SOURCE=Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source')
ENGINE=SOURCE/'rust_verify/src/sail_engine.rs'
TARGET=SOURCE/'target-verus'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
identity=json.loads((OUT/'identity.json').read_text())
assert json.loads((OUT/'result.json').read_text())['status']=='PASS'
assert all(sha(SOURCE/name)==digest for name,digest in identity['source_sha256'].items())
assert sha(OUT/'actual-finish-removed.patch')==identity['actual_call_removed_counterfactual_sha256']
retained=OUT/'tested-binaries';retained.mkdir(exist_ok=False)
binaries={}
for package in ['vir','rust_verify']:
 text=(OUT/(package+'-test.log')).read_text()
 matches=re.findall(r'Running unittests .* \(([^)]+)\)',text)
 assert len(matches)==1
 path=SOURCE/matches[0];dest=retained/path.name;shutil.copy2(path,dest)
 assert sha(path)==sha(dest) and path.stat().st_ino!=dest.stat().st_ino
 binaries[package]={'name':dest.name,'bytes':dest.stat().st_size,'sha256':sha(dest)}
(OUT/'tested-binaries.json').write_text(json.dumps(binaries,indent=2)+'\n')
original=ENGINE.read_bytes();started=time.monotonic();record={'scope':'Actual with_namespace finish-call removal, unchanged paused-guard adapter assertion; expected negative only','status':'running','source_before_sha256':sha(ENGINE),'phases':[]}
env=dict(os.environ,RUSTUP_TOOLCHAIN='1.98.1',RUSTC_BOOTSTRAP='1',VERUS_IN_VARGO='1',VARGO_TARGET_DIR=str(TARGET/'release'),CARGO_TARGET_DIR=str(TARGET),CARGO_BUILD_JOBS='2',RUSTFLAGS='--cfg proc_macro_span --cfg verus_keep_ghost --cfg span_locations --cfg procmacro2_semver_exempt')
for name in list(env):
 if name.startswith('SAIL_ENGINE'):env.pop(name)
sysroot=Path(subprocess.check_output(['rustc','+1.98.1','--print','sysroot'],text=True).strip())
target_lib=Path(subprocess.check_output(['rustc','+1.98.1','--print','target-libdir'],text=True).strip())
env['LD_LIBRARY_PATH']=str(sysroot/'lib')+':'+str(target_lib)
def save():(OUT/'sensitivity.json').write_text(json.dumps(record,indent=2)+'\n')
child=None
try:
 subprocess.run(['git','-C',str(SOURCE),'apply','--check',str(OUT/'actual-finish-removed.patch')],check=True)
 subprocess.run(['git','-C',str(SOURCE),'apply',str(OUT/'actual-finish-removed.patch')],check=True)
 record['source_counterfactual_sha256']=sha(ENGINE);save()
 command=['cargo','+1.98.1','test','-p','rust_verify','--release','--lib','--no-run','--message-format=json']
 phase_start=time.monotonic()
 with (OUT/'sensitivity-compile.stdout').open('wb') as stdout,(OUT/'sensitivity-compile.stderr').open('wb') as stderr:
  child=subprocess.Popen(command,cwd=SOURCE,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  code=child.wait();child=None
 record['phases'].append({'phase':'compile','exit_code':code,'elapsed_seconds':time.monotonic()-phase_start});save();assert code==0
 outputs=[json.loads(line) for line in (OUT/'sensitivity-compile.stdout').read_text().splitlines() if line.startswith('{')]
 artifacts=[x for x in outputs if x.get('reason')=='compiler-artifact' and x.get('target',{}).get('name')=='rust_verify' and x.get('profile',{}).get('test') and x.get('executable')]
 assert len(artifacts)==1
 binary=Path(artifacts[0]['executable']);dest=retained/'finish-removed-probe';shutil.copy2(binary,dest);assert sha(dest)==sha(binary)
 record['counterfactual_binary_sha256']=sha(dest)
 name='verifier::sail_engine::namespace_retirement_adapter_tests::actual_worker_adapter_retires_both_success_and_inner_refusal_before_return'
 phase_start=time.monotonic();timed_out=False
 with (OUT/'sensitivity-test.stdout').open('wb') as stdout,(OUT/'sensitivity-test.stderr').open('wb') as stderr:
  child=subprocess.Popen([str(dest),name,'--exact','--test-threads=1'],env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  try:code=child.wait(timeout=30)
  except subprocess.TimeoutExpired:timed_out=True;os.killpg(child.pid,signal.SIGKILL);code=child.wait()
  child=None
 record['phases'].append({'phase':'test','exit_code':code,'elapsed_seconds':time.monotonic()-phase_start,'deadline_seconds':30,'timed_out':timed_out});save()
 text=(OUT/'sensitivity-test.stdout').read_text()+(OUT/'sensitivity-test.stderr').read_text()
 assert code==101 and not timed_out and 'worker returned before compiler guard retirement' in text
 record['status']='PASS_EXPECTED_NEGATIVE'
except BaseException as error:
 record.update(status='FAIL',error_type=type(error).__name__,error=str(error));raise
finally:
 if child is not None:
  try:os.killpg(child.pid,signal.SIGKILL)
  except ProcessLookupError:pass
  child.wait()
 ENGINE.write_bytes(original)
 record['source_restored_sha256']=sha(ENGINE)
 record['elapsed_seconds']=time.monotonic()-started
 save()
 assert all(sha(SOURCE/name)==digest for name,digest in identity['source_sha256'].items())
 assert sha(SOURCE/'Cargo.lock')==identity['lock_sha256']
 print(json.dumps(record),flush=True)
