#!/usr/bin/env python3
"""Correct only standalone loader setup, reuse the identical failed-control ELF."""
import hashlib,json,os,signal,subprocess,time
from pathlib import Path
OUT=Path('/tmp/sail-m7-preparation-retirement-source-controls')
old=json.loads((OUT/'sensitivity.json').read_text())
assert old['status']=='FAIL' and old['phases'][-1]['exit_code']==127
assert 'librustc_driver' in (OUT/'sensitivity-test.stderr').read_text()
for name in ['sensitivity.json','sensitivity-test.stdout','sensitivity-test.stderr']:
 path=OUT/name;path.rename(OUT/('initial-'+name))
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
binary=OUT/'tested-binaries/finish-removed-probe'
assert sha(binary)==old['counterfactual_binary_sha256']
source=Path('/tmp/sail-m7-original-descriptor-source-controls/verus/source')
identity=json.loads((OUT/'identity.json').read_text())
assert all(sha(source/name)==digest for name,digest in identity['source_sha256'].items())
sysroot=Path(subprocess.check_output(['rustc','+1.98.1','--print','sysroot'],text=True).strip())
targetlib=Path(subprocess.check_output(['rustc','+1.98.1','--print','target-libdir'],text=True).strip())
env={k:v for k,v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
env['LD_LIBRARY_PATH']=str(sysroot/'lib')+':'+str(targetlib)
name='verifier::sail_engine::namespace_retirement_adapter_tests::actual_worker_adapter_retires_both_success_and_inner_refusal_before_return'
start=time.monotonic();timed_out=False
with (OUT/'sensitivity-test.stdout').open('wb') as stdout,(OUT/'sensitivity-test.stderr').open('wb') as stderr:
 child=subprocess.Popen([str(binary),name,'--exact','--test-threads=1'],env=env,stdout=stdout,stderr=stderr,start_new_session=True)
 try:code=child.wait(timeout=30)
 except subprocess.TimeoutExpired:timed_out=True;os.killpg(child.pid,signal.SIGKILL);code=child.wait()
text=(OUT/'sensitivity-test.stdout').read_text()+(OUT/'sensitivity-test.stderr').read_text()
passed=code==101 and not timed_out and 'worker returned before compiler guard retirement' in text
record={'status':'PASS_EXPECTED_NEGATIVE' if passed else 'FAIL','scope':'Same retained actual call-removed ELF; pinned loader-path-only fixture correction','exit_code':code,'elapsed_seconds':time.monotonic()-start,'deadline_seconds':30,'timed_out':timed_out,'binary_sha256':sha(binary),'initial_failure_sha256':sha(OUT/'initial-sensitivity.json'),'unchanged_source_sha256':identity['source_sha256']}
(OUT/'sensitivity.json').write_text(json.dumps(record,indent=2)+'\n')
assert all(sha(source/name)==digest for name,digest in identity['source_sha256'].items())
print(json.dumps(record),flush=True)
raise SystemExit(0 if passed else 1)
