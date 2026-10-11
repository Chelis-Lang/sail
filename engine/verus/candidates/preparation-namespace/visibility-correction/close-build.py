from pathlib import Path
import hashlib,json,os,subprocess,time
ROOT=Path('/tmp/sail-m7-preparation-namespace-visibility-build-evidence')
COMMIT='426d8b01e7ffb910a36c15e1f870ff985bb61eef'
BUILD=Path('/root/.cache/sail/verus-src-build')/(COMMIT+'-b3b07d47d7542817')
PATCH='955cd584c19f99a5dece3d1f69da49b55530457a6ace334d56baf5af724bcf5e'
LOCK='653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1'
def sha(path):
 h=hashlib.sha256()
 with path.open('rb') as f:
  for chunk in iter(lambda:f.read(1024*1024),b''): h.update(chunk)
 return h.hexdigest()
result=json.loads((ROOT/'ordinary-build-result.json').read_text())
assert result['exit_code']==0 and result['patch_sha256']==PATCH
assert sha(ROOT/'candidate.patch')==PATCH
identity=json.loads((ROOT/'identity.json').read_text())
source=BUILD/'verus/source'
assert subprocess.check_output(['git','-C',str(BUILD/'verus'),'rev-parse','HEAD'],text=True).strip()==COMMIT
assert sha(source/'Cargo.lock')==LOCK
for relative,digest in identity['source_sha256'].items(): assert sha(source/relative)==digest
manifest_path=BUILD/'sail-build.json'
manifest=json.loads(manifest_path.read_text())
assert manifest['schema_version']==1 and manifest['commit']==COMMIT and manifest['rust_toolchain']=='1.98.1'
assert manifest['features']==['sail-interactive:'+PATCH,'host:x86_64-unknown-linux-gnu','source-lock:'+LOCK]
assert len(manifest['artifacts'])==13
runtime=source/'target-verus/release'
actual={str(p.relative_to(BUILD)) for p in runtime.iterdir() if p.is_file()}
assert actual==set(manifest['artifacts'])
for relative,digest in manifest['artifacts'].items():
 p=BUILD/relative
 assert not p.is_symlink() and p.is_file() and sha(p)==digest
assert manifest['z3_sha256']==manifest['artifacts']['verus/source/target-verus/release/z3']=='e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765'
log=(ROOT/'ordinary-build.log').read_text(errors='replace')
assert 'vstd outdated, rebuilding' in log and 'verification results:: 2060 verified, 0 errors' in log
actor=json.loads((ROOT/'ordinary-build-actor.json').read_text())
rows=[]
for p in Path('/proc').iterdir():
 if not p.name.isdigit(): continue
 try:
  s=(p/'stat').read_text(); v=s[s.rfind(')')+2:].split()
  if int(v[2])==actor['pgid'] and v[0]!='Z': rows.append({'pid':int(p.name),'comm':s[s.find('(')+1:s.rfind(')')],'state':v[0]})
 except (OSError,ValueError,IndexError): continue
assert not rows,rows
previous=json.loads((ROOT/'preceding-runtime-seals.json').read_text())
for suffix,record in previous.items():
 old=BUILD.parent/(COMMIT+'-'+suffix)
 assert sha(old/'sail-build.json')==record['manifest_sha256']
 assert all(sha(old/relative)==digest for relative,digest in record['artifacts'].items())
 oldruntime=old/'verus/source/target-verus/release'
 assert {str(p.relative_to(old)) for p in oldruntime.iterdir() if p.is_file()}==set(record['artifacts'])
for suffix,record in previous.items():
 oldsource=BUILD.parent/(COMMIT+'-'+suffix)/'verus/source'
 assert sha(oldsource/'Cargo.lock')==record['lock_sha256']
 assert all(sha(oldsource/n)==h for n,h in record['source_sha256'].items())
checks=Path('/tmp/sail-m7-preparation-namespace-visibility-source-controls')/'result.json'
freeze=json.loads((ROOT/'build-input-freeze.json').read_text())
for name,digest in freeze['source_control_inputs'].items():assert sha(checks.parent/name)==digest
control=json.loads(checks.read_text());assert control['status']=='PASS' and len(control['phases'])==4 and all(p['exit_code']==0 for p in control['phases'])
record={'version':1,'scope':'Ordinary private source build and fresh vstd proof only; original14/product acceptance pending','closed_unix_ns':time.time_ns(),'selected':False,'qualified':False,'patch_sha256':PATCH,'source_commit':COMMIT,'source_lock_sha256':LOCK,'source_files':identity['source_sha256'],'manifest_sha256':sha(manifest_path),'manifest':manifest,'complete_direct_artifact_inventory':True,'owned_build_actors':rows,'fresh_vstd_verified':2060,'fresh_vstd_errors':0,'ordinary_result':result,'source_controls_sha256':sha(checks),'raw_artifacts':{name:{'sha256':sha(ROOT/name),'bytes':(ROOT/name).stat().st_size} for name in ['ordinary-build.log','ordinary-build-result.json','ordinary-build-actor.json','compiler-seed.json','applied-source.json','builder-before.json','preceding-runtime-seals.json','resource-preflight.json']},'native_available_bytes':os.statvfs(ROOT).f_bavail*os.statvfs(ROOT).f_frsize,'prior_complete_runtime_seals_unchanged':True,'original14_launched':False}
(ROOT/'closed-ordinary-build.json').write_text(json.dumps(record,indent=2)+'\n')
(ROOT/'ordinary-runtime-manifest.json').write_bytes(manifest_path.read_bytes())
print(json.dumps({k:record[k] for k in ['manifest_sha256','fresh_vstd_verified','fresh_vstd_errors','complete_direct_artifact_inventory','native_available_bytes','selected','qualified']}))
