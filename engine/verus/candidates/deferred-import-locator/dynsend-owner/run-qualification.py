from pathlib import Path
import hashlib,json,os,subprocess,time
ROOT=Path('/tmp/sail-m7-deferred-dynsend-build-evidence')
BUILD=Path('/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-73bd8aa61b18ca12')
SCRIPTS=Path('/tmp/sail-m7-combined-c39-evidence/qualification-scripts')
PIPELINE=SCRIPTS/'engine/verus/candidates/qualify-same-input.py'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(PIPELINE)=='f7ddcf603e2ff7c52a2a107ce97139ee2ccc43c75f558b61d14642762d92cc62'
assert sha(SCRIPTS/'engine/verus/candidates/test-private-span-cross-edit.py')=='80b49c69b84599b606f200157c67cdc1f92e73054905f3a22b11ce839789fc2a'
assert sha(ROOT/'candidate.patch')=='886b9b830eda4a4128fa861557427cbeabae92c170988b123d3bcb271749f508'
CONTROL_ROOT=Path('/tmp/sail-m7-deferred-dynsend-controls-evidence')
checks=json.loads((CONTROL_ROOT/'source-checks.json').read_text()); assert checks['all_checks_passed']
assert checks['patch_sha256']==sha(ROOT/'candidate.patch')
identity=json.loads((ROOT/'identity.json').read_text())
for relative,digest in identity['source_files'].items(): assert sha(BUILD/'verus/source'/relative)==digest
assert sha(BUILD/'verus/source/Cargo.lock')=='653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1'
ordinary=json.loads((ROOT/'ordinary-build-result.json').read_text())
assert ordinary['exit_code']==0 and ordinary['patch_sha256']==sha(ROOT/'candidate.patch')
manifest=json.loads((BUILD/'sail-build.json').read_text())
assert manifest['schema_version']==1
assert manifest['commit']=='426d8b01e7ffb910a36c15e1f870ff985bb61eef' and manifest['rust_toolchain']=='1.98.1'
assert manifest['z3_sha256']=='e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765'
assert len(manifest['artifacts'])==13
assert manifest['features']==['sail-interactive:'+sha(ROOT/'candidate.patch'),'host:x86_64-unknown-linux-gnu','source-lock:653e5c0c05bebc4d0d083a25c9aab6d3e78d397c3dcdf02b20286e1a5a1715a1']
for relative,digest in manifest['artifacts'].items(): assert sha(BUILD/relative)==digest
runtime=BUILD/'verus/source/target-verus/release'
assert {str(p.relative_to(BUILD)) for p in runtime.iterdir() if p.is_file()}==set(manifest['artifacts'])
EXPECTED_SCRIPTS={
  "engine/verus/candidates/actor-observation.py": "d01de79ee9de89b823d7ccfb0efa1474a2ed4185f519bffd925e8546895edc34",
  "engine/verus/candidates/audit-graph-ledger.py": "c1385581e251f4f8da24efb3ad176d067e0773b283f92d2253effc5122feb5e6",
  "engine/verus/candidates/audit-private-span-boundary.py": "ecaa208f6ead0dd0d0b459ffbc375e2635c44d870caf19d68b6efe1ab9d398f4",
  "engine/verus/candidates/qualify-same-input.py": "f7ddcf603e2ff7c52a2a107ce97139ee2ccc43c75f558b61d14642762d92cc62",
  "engine/verus/candidates/test-graph-ledger.py": "283f05ed2cfb3f19dcd40ca055b315fe74403f93564f56b77ef87dfd8a59e603",
  "engine/verus/candidates/test-key-delta.py": "c3a46a4c530f9e02234614f49cd5fea2ec4b841b1d133d711c10c749344778e6",
  "engine/verus/candidates/test-private-span-cross-edit.py": "80b49c69b84599b606f200157c67cdc1f92e73054905f3a22b11ce839789fc2a",
  "engine/verus/candidates/test-private-span-migration.py": "dd57b4ef82a9f40720946e89ebb18c7aecd915c84df309627c59ca8896f4deaf",
  "engine/verus/candidates/test-real-he-same-input.py": "2bb7a3817dd6aecfba438f3dc0ab78aa82012527c3977fcd6365ce5dbcac67d2",
  "engine/verus/candidates/test-same-callback.py": "6cd4d5d18efd8d751de89b7c36dec04c3d26ed11f912cac3a0d9ca9050237295",
  "engine/verus/test-cancel.py": "c899b388c6005197f561a73c7aeafe7fe8c49e76cbb9f0f04fc56f296e15744d",
  "engine/verus/test-engine.py": "543be4cecca10c745a4e870511a1e4e7ac841e1c839d190bb2be0f0f03294c5f",
  "engine/verus/test-prepared.py": "22d0436832438d276ac79ef330dc283b67352157f9bd0c75a9ee89cc34ef1f8f",
  "engine/verus/test-real-he.py": "effde8542a0b096aa760b6f24a9d97ef249016cd8aa588129445151a901efb89"
}
script_inventory={str(p.relative_to(SCRIPTS)):sha(p) for p in SCRIPTS.rglob('*.py')}
assert script_inventory==EXPECTED_SCRIPTS
command=['python3',str(PIPELINE),'--build',str(BUILD),'--stock','/opt/verus/verus-x86-linux/verus','--z3','/opt/verus/verus-x86-linux/z3','--he','/root/.cache/sail/corpus/human-eval-verus-bbe93f73b0a5','--out',str(ROOT/'original-qualification'),'--execution','linux-x86_64-container-emulated-sharedload','--patch',str(ROOT/'candidate.patch'),'--private-span-boundary','--old-engine','/root/.cache/sail/verus-src-build/426d8b01e7ffb910a36c15e1f870ff985bb61eef-dd8447d5aca0e5c1/verus/source/target-verus/release/verus']
env={k:v for k,v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
env.update(RUST_TEST_THREADS='1',RUSTC_BOOTSTRAP='1',VERUS_IN_VARGO='1',CARGO_HOME='/opt/cargo',RUSTUP_HOME='/opt/rustup')
record={'version':1,'scope':'Original frozen qualification; no selected runtime/product qualification','command':command,'compiler_limits_seconds':600,'assertions_changed':False,'script_sha256':script_inventory,'manifest_sha256':sha(BUILD/'sail-build.json'),'patch_sha256':sha(ROOT/'candidate.patch'),'status':'running','started_unix_ns':time.time_ns(),'independent_source_cases':'serial; internal concurrency unchanged','selected':False,'source_controls_sha256':sha(CONTROL_ROOT/'source-checks.json')}
started=time.monotonic()
with (ROOT/'original-qualification.log').open('wb') as stream:
 child=subprocess.Popen(command,env=env,stdout=stream,stderr=subprocess.STDOUT,start_new_session=True)
 record.update(pid=child.pid,pgid=os.getpgid(child.pid),nice=os.getpriority(os.PRIO_PROCESS,child.pid))
 assert record['nice']==0
 (ROOT/'original-qualification-actor.json').write_text(json.dumps(record,indent=2)+'\n')
 print(json.dumps({'status':record['status'],'pid':child.pid,'nice':record['nice'],'manifest':record['manifest_sha256']}),flush=True)
 code=child.wait()
record.update(status='passed' if code==0 else 'failed',exit_code=code,elapsed_seconds=time.monotonic()-started,log_sha256=sha(ROOT/'original-qualification.log'))
(ROOT/'original-qualification-actor.json').write_text(json.dumps(record,indent=2)+'\n')
print(json.dumps({k:record[k] for k in ['status','exit_code','elapsed_seconds','log_sha256']}),flush=True)
raise SystemExit(code)
