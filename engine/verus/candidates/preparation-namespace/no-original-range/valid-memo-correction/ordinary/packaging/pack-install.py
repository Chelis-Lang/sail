from pathlib import Path
import hashlib,json,os,subprocess,tarfile,time
ROOT=Path('/tmp/sail-m7-no-original-range-build-evidence')
REPO=Path('/tmp/sail-m7-original-descriptor-build-candidate')
BUILD=Path('/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-c77e1a1879302e06')
OUT=Path('/tmp/sail-m7-no-original-range-runtime-package')
PATCH='aaa3b2e4780c6b410b77ac121ef682c9532fb2eb21552a104fa63243e7a95124'
LOCK='653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1'
PACKER='590749607ca02a2005cb54c5c30e2f6bd998b362d658439dae8abf1abbf8e92b'
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
 return h.hexdigest()
def runtime(root,manifest):
 files=manifest['artifacts'];base=root/'verus/source/target-verus/release'
 assert len(files)==13 and {str(p.relative_to(root)) for p in base.iterdir() if p.is_file()}==set(files)
 for n,h in files.items():assert not (root/n).is_symlink() and sha(root/n)==h
 return {n:{'sha256':h,'mode':(root/n).stat().st_mode & 0o777,'bytes':(root/n).stat().st_size} for n,h in files.items()}
def prior():
 records=json.loads((ROOT/'preceding-runtime-seals.json').read_text())
 for suffix,r in records.items():
  root=BUILD.parent/('426d8b01e7ffb910a36c15e1f870ff985bb61eef-'+suffix)
  assert sha(root/'sail-build.json')==r['manifest_sha256'];runtime(root,json.loads((root/'sail-build.json').read_text()))
  assert sha(root/'verus/source/Cargo.lock')==r['lock_sha256']
  for n,h in r['source_sha256'].items():assert sha(root/'verus/source'/n)==h
 return records
actor=json.loads((ROOT/'original-qualification-actor.json').read_text());closure=json.loads((ROOT/'qualification-seal-closure.json').read_text())
assert actor['status']=='passed' and actor['exit_code']==0
assert closure['source_gates_passed'] and closure['stage_counts']=={'frozen_script_inventory':14,'total_pipeline_stages':13,'attempted':13,'passed':13,'failed':0,'not_attempted':0}
assert not closure['owned_build_actors'] and closure['prior_complete_runtime_seals_unchanged'] and not closure['selected']
assert closure['patch_sha256']==PATCH and closure['source_lock_sha256']==LOCK
assert sha(REPO/'ci/engine-artifact.py')==PACKER and sha(REPO/'engine/verus/interactive.patch')==PATCH and sha(REPO/'engine/verus/Cargo.lock')==LOCK
assert sha(BUILD/'sail-build.json')==closure['manifest_sha256'];manifest=json.loads((BUILD/'sail-build.json').read_text())
source=BUILD/'verus/source';assert sha(source/'Cargo.lock')==LOCK
for n,h in closure['source_files'].items():assert sha(source/n)==h
before=runtime(BUILD,manifest);old=prior();OUT.mkdir(exist_ok=False)
record={'version':1,'scope':'Local package and fresh-install identities only; no publication/selection/product acceptance','status':'running','patch_sha256':PATCH,'source_lock_sha256':LOCK,'packer_sha256':PACKER,'manifest_sha256':sha(BUILD/'sail-build.json'),'qualification_closure_sha256':sha(ROOT/'qualification-seal-closure.json'),'phases':[],'started_unix_ns':time.time_ns(),'nice':os.getpriority(os.PRIO_PROCESS,0)}
started=time.monotonic()
try:
 env={k:v for k,v in os.environ.items() if k!='CI'}
 for name,cmd in [('pack',['python3',str(REPO/'ci/engine-artifact.py'),'pack','--build',str(BUILD),'--archive',str(OUT/'engine-runtime.tar.gz'),'--pin',str(OUT/'artifact-linux.json')]),('fresh-install',['python3',str(REPO/'ci/engine-artifact.py'),'install','--archive',str(OUT/'engine-runtime.tar.gz'),'--pin',str(OUT/'artifact-linux.json'),'--cache',str(OUT/'fresh-cache')])]:
  began=time.monotonic()
  with (OUT/(name+'.log')).open('wb') as f:r=subprocess.run(cmd,env=env,stdout=f,stderr=subprocess.STDOUT)
  record['phases'].append({'name':name,'exit_code':r.returncode,'elapsed_seconds':time.monotonic()-began,'log_sha256':sha(OUT/(name+'.log'))});assert r.returncode==0
 pin=json.loads((OUT/'artifact-linux.json').read_text());assert pin['patch_sha256']==PATCH and pin['source_lock_sha256']==LOCK and 'asset_id' not in pin
 assert pin['archive_sha256']==sha(OUT/'engine-runtime.tar.gz')
 members=[]
 with tarfile.open(OUT/'engine-runtime.tar.gz','r:gz') as t:
  for m in t:
   assert m.isfile() and m.uid==m.gid==m.mtime==0 and not m.uname and not m.gname
   h=hashlib.sha256()
   f=t.extractfile(m)
   for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
   members.append({'name':m.name,'bytes':m.size,'mode':m.mode,'sha256':h.hexdigest()})
 assert len(members)==14 and sum(m['bytes'] for m in members)<=2*1024**3
 assert {m['name'] for m in members}=={'sail-build.json',*manifest['artifacts']}
 installed=OUT/'fresh-cache/verus-src-build'/pin['variant'];assert sha(installed/'sail-build.json')==sha(BUILD/'sail-build.json')
 assert runtime(installed,manifest)==before and runtime(BUILD,manifest)==before and prior()==old
 assert sha(source/'Cargo.lock')==LOCK and all(sha(source/n)==h for n,h in closure['source_files'].items())
 record.update(status='PASS',archive_sha256=pin['archive_sha256'],archive_bytes=(OUT/'engine-runtime.tar.gz').stat().st_size,pin_sha256=sha(OUT/'artifact-linux.json'),pin=pin,members=members,installed_manifest_sha256=sha(installed/'sail-build.json'),installed_runtime=before,old_complete_runtime_source_lock_seals_unchanged=True,runtime_selected=False,product_qualified=False)
except BaseException as e:
 record.update(status='FAIL',error_type=type(e).__name__,error=str(e));raise
finally:
 record['elapsed_seconds']=time.monotonic()-started;(OUT/'result.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'status':record['status'],'elapsed_seconds':record['elapsed_seconds']}),flush=True)
