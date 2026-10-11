#!/usr/bin/env python3
"""Prepare or execute one original reader/retained scope for a sealed product.

Default is preparation only. Product grant may be a host file or a native path
in --container. All staged files are native-owned fresh copies; existing records
and collectors remain unchanged. No compiler is invoked by this launcher.
"""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys
import signal
import unittest

MANIFEST = '54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d'
READER = '84966d77cd679653b96ab9f05019d51f805538f3d4dd509c1591da1679b7634d'
RETAINED = '207b45ef034b6c1e947dbb0efdb87368b62d5ef8a4c42ab5768413670f775a11'
PYTHON = '3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720'
RUST = 'f1632772daacc28e4bb5e7f7747e0e048fce7973aeff31b6ed65c5bbbd0e0b4d'
WRAPPER = '29e2ca3872fd8a25c859379a109f5d170c262b71f9a93cfce1703ec2ff034c99'
SUPPORT = '6f29f27975035ed253ec75e6cfbc8f22eaca24a9ad5256e927cf877253b34779'
SOURCE = 'c81ef6ca164608180e35ca4d6e4d92814dec2fefb8f9b177242e7f2e14416156'
MAX_BUNDLE = 16 * 1024 * 1024


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON field: ' + key)
        result[key] = value
    return result


def grant_check(grant, rev):
    if not re.fullmatch('[0-9a-f]{7,40}', rev):
        raise ValueError('invalid product revision')
    source = grant.get('source_commit', '')
    if not re.fullmatch('[0-9a-f]{40}', source) or not source.startswith(rev):
        raise ValueError('grant source differs from requested revision')
    if grant.get('installation_only') is not True:
        raise ValueError('installation-only grant required')
    phases = grant.get('phases')
    if (not isinstance(phases,list) or len(phases)!=2
            or {p.get('phase') for p in phases}!={'release-build','owned-install'}
            or any(p.get('exit_code') != 0 for p in phases)):
        raise ValueError('complete successful product phases required')
    for name in ('sail', 'sail-z3'):
        row = grant.get('artifacts', {}).get(name, {})
        if (type(row.get('bytes')) is not int or not 0 < row['bytes'] <= 128*1024*1024
                or not re.fullmatch('[0-9a-f]{64}', row.get('sha256', ''))):
            raise ValueError('incomplete product content grant')
    for name in ('product_dir', 'engine_build'):
        if not pathlib.PurePosixPath(grant.get(name, '')).is_absolute():
            raise ValueError('native absolute binding required: ' + name)
    if grant.get('container') not in ('sail-m7-acceptance', 'sail-m7-handoff-validation'):
        raise ValueError('unexpected product container')
    return grant


PRODUCER = r'''
import pathlib,json,hashlib,tarfile,io,sys,stat
root=pathlib.Path('/tmp/sail-m7-descriptor-reader-controller-46-reviewed')
template=pathlib.Path('/tmp/sail-m7-descriptor-source-template-46-reviewed')
probe=pathlib.Path('/opt/sail-m7-probe-staged-02fd1fc')
expected=json.loads((root/'static-freeze.json').read_bytes())['source_files']
assert len(expected)==274 and not (template/'target').exists()
assert hashlib.sha256(json.dumps(expected,sort_keys=True,separators=(',',':')).encode()).hexdigest()==sys.argv[1]
entries=[]
def add(path,name,expected_sha=None):
    assert path.is_file() and not path.is_symlink(),str(path)
    raw=path.read_bytes()
    assert expected_sha is None or hashlib.sha256(raw).hexdigest()==expected_sha,str(path)
    entries.append((name,raw,path.stat().st_mode&511))
for name,row in sorted(expected.items()):
    path=template/name
    assert path.stat().st_size==row['bytes']
    add(path,'source/'+name,row['sha256'])
for path in sorted((root/'scripts').iterdir()):
    if path.is_file(): add(path,'scripts/'+path.name)
add(root/'corpus/repair-edits.json','corpus/repair-edits.json')
for name in ('m7-descriptor-reader-workflow.py','m7-sysroot-proof-diagnostic.py'):
    add(root/name,name)
for name in ('job_read_bench','python-reader-probe.py','freeze.json'):
    add(probe/name,'probes/'+name)
reference=pathlib.Path('/tmp/sail-m7-sysroot-proof-diagnostic-723/summary.json')
add(reference,'source-reference.json')
assert sum(len(raw) for _,raw,_ in entries)<16*1024*1024
with tarfile.open(fileobj=sys.stdout.buffer,mode='w|') as t:
    for name,raw,mode in entries:
        info=tarfile.TarInfo(name); info.size=len(raw); info.mode=mode
        t.addfile(info,io.BytesIO(raw))
'''

PREPARE = r'''
import pathlib,tarfile,io,json,hashlib,sys,os,importlib.util,stat
cfg=json.loads(sys.argv[1]); root=pathlib.Path(cfg['stage'])
assert not root.exists(); root.mkdir(mode=0o700)
raw=sys.stdin.buffer.read(16*1024*1024+1); assert len(raw)<=16*1024*1024
with tarfile.open(fileobj=io.BytesIO(raw)) as t:
    for member in t.getmembers():
        p=pathlib.PurePosixPath(member.name)
        assert member.isfile() and not p.is_absolute() and '..' not in p.parts and len(p.parts)>0
        dst=root/member.name; dst.parent.mkdir(parents=True,exist_ok=True)
        with dst.open('xb') as f: f.write(t.extractfile(member).read())
        dst.chmod(member.mode&0o777)
def sha(p): return hashlib.sha256(p.read_bytes()).hexdigest()
for name,digest in cfg['files'].items(): assert sha(root/name)==digest,name
spec=importlib.util.spec_from_file_location('native_products',root/'scripts/m7-native-product.py')
helper=importlib.util.module_from_spec(spec); spec.loader.exec_module(helper)
private_grant=root/'product-grant.json'; grant=cfg['grant']
actual_bin=pathlib.Path(grant['product_dir']).resolve()
for name in ('sail','sail-z3'):
    assert actual_bin.joinpath(name).stat().st_size==grant['artifacts'][name]['bytes']
    assert sha(actual_bin/name)==grant['artifacts'][name]['sha256']
audit=helper.audit_products(actual_bin); assert audit['ownership_prerequisite_met'],audit
original_bin=actual_bin
if not (actual_bin/'freeze.json').is_file():
    installed=helper.install(actual_bin,root/'product',grant['artifacts'])
    actual_bin=root/'product'
    (actual_bin/'freeze.json').write_text(json.dumps(installed,indent=2,sort_keys=True)+'\n')
frozen,_=helper.load_json(actual_bin/'freeze.json')
for name in ('sail','sail-z3'):
    assert frozen['files'][name]['sha256']==grant['artifacts'][name]['sha256']
    assert frozen['files'][name]['bytes']==grant['artifacts'][name]['bytes']
    assert sha(original_bin/name)==grant['artifacts'][name]['sha256']
runtime=pathlib.Path(grant['engine_build']).resolve(); manifest=runtime/'sail-build.json'
assert sha(manifest)==cfg['manifest']; model,_=helper.load_json(manifest)
assert len(model['artifacts'])==13
who=helper.identity()
for name,digest in model['artifacts'].items():
    path=pathlib.PurePosixPath(name); assert not path.is_absolute() and '..' not in path.parts
    p=runtime/name; assert sha(p)==digest
    assert p.stat().st_uid==who['fsuid'] or who['cap_lease'],name
# Derived success is installation-only and bound to the original complete grant.
normalized=dict(grant,exit_code=0,original_grant_sha256=cfg['grant_sha'],
                product_dir=str(actual_bin),original_product_dir=str(original_bin))
private_grant.write_text(json.dumps(normalized,indent=2,sort_keys=True)+'\n')
source=root/'source'; assert not (source/'target').exists()
wrapper=root/'m7-descriptor-reader-workflow.py'
# Only the human-facing old-product scope label changes. Controller logic stays exact.
text=wrapper.read_text(); old='Original descriptor product46 reader gates then corrected retained functional scope; diagnostic stages only, no source physical, 2x or overall M7 qualification'
assert text.count(old)==1
wrapper.write_text(text.replace(old,'Original combined-product reader gates and corrected retained functional scope; no source qualification or overall M7 acceptance'))
argv=['--product-source',grant['source_commit'],'--bin-dir',str(actual_bin),'--product-grant',str(private_grant),
'--product-grant-sha256',sha(private_grant),'--source-runtime',str(runtime),'--source-workspace',str(source),
'--source-reference',str(root/'source-reference.json'),'--source-reference-sha256',sha(root/'source-reference.json'),
'--scripts-dir',str(root/'scripts'),'--stock-bin-dir','/opt/verus/verus-x86-linux',
'--probe-bin-dir',str(root/'probes'),'--out',str(root/'run'),'--environment-label',cfg['label']]
(root/'argv.json').write_text(json.dumps(argv,indent=2)+'\n')
# The reviewed wrapper checks a literal manifest in its support module; rebind only that identity.
driver="""import importlib.util,json,sys,pathlib,signal\np=pathlib.Path(__file__).parent\ns=importlib.util.spec_from_file_location("bound_reader",p/"m7-descriptor-reader-workflow.py")\nw=importlib.util.module_from_spec(s); s.loader.exec_module(w)\nw.support.MANIFEST="""+repr(cfg['manifest'])+"""\ndef interrupted(signum,frame): raise KeyboardInterrupt("owned reader interrupted")\nfor sig in (signal.SIGTERM,signal.SIGHUP): signal.signal(sig,interrupted)\nsys.argv=[str(p/"m7-descriptor-reader-workflow.py"),*json.loads((p/"argv.json").read_text()),*sys.argv[1:]]\nraise SystemExit(w.main())\n"""
(root/'driver.py').write_text(driver)
# Freeze environment privately. Never export its values in the public seal.
environment=dict(os.environ)
allowed={'SAIL_TEST_Z3','SAIL_VERUS_HOME'}
assert not any(k.startswith('SAIL_') and k not in allowed for k in environment),'inherited Sail selector'
assert not any(k in environment for k in ('VERUS_Z3_PATH','VERUS_DRIVER_ARGS','CI')),'inherited runtime selector'
(root/'private-environment.json').write_text(json.dumps(environment,sort_keys=True)); (root/'private-environment.json').chmod(0o600)
seal={'manifest':cfg['manifest'],'original_grant_sha256':cfg['grant_sha'],'source_commit':grant['source_commit'],
'files':{str(p.relative_to(root)):{'bytes':p.stat().st_size,'sha256':sha(p)} for p in root.rglob('*') if p.is_file() and p.name!='private-environment.json'},
'private_environment_sha256':sha(root/'private-environment.json'),'docker_birth':cfg['docker_birth'],'proof_launched':False}
(root/'preparation.json').write_text(json.dumps(seal,indent=2,sort_keys=True)+'\n')
print(json.dumps({'stage':str(root),'preparation_sha256':sha(root/'preparation.json'),'driver_sha256':sha(root/'driver.py'),'proof_launched':False},sort_keys=True))
'''

EXECUTE = r'''
import pathlib,json,hashlib,sys,subprocess,os,signal
root=pathlib.Path(sys.argv[1]); status=pathlib.Path('/proc/self/stat').read_text(); birth=status[status.rfind(')')+2:].split()[19]
(root/'owned-execution.json').write_text(json.dumps({'pid':os.getpid(),'start_ticks':birth,'uid':os.getuid()})+'\n')
seal=json.loads((root/'preparation.json').read_bytes())
for name,row in seal['files'].items():
 p=root/name; assert p.stat().st_size==row['bytes'] and hashlib.sha256(p.read_bytes()).hexdigest()==row['sha256'],name
p=root/'private-environment.json'; assert hashlib.sha256(p.read_bytes()).hexdigest()==seal['private_environment_sha256']
env=json.loads(p.read_bytes()); child=None
try:
 with (root/'controller.stdout').open('xb') as out,(root/'controller.stderr').open('xb') as err:
  child=subprocess.Popen(['python3',str(root/'driver.py')],env=env,stdout=out,stderr=err,start_new_session=True)
  def interrupted(sig,frame):
   if child.poll() is None: os.killpg(child.pid,signal.SIGINT)
   raise KeyboardInterrupt('owned controller interrupted')
  for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP): signal.signal(sig,interrupted)
  code=child.wait()
 (root/'external-terminal.json').write_text(json.dumps({'exit_code':code,'controller_pid':child.pid,'acceptance':code==0})+'\n')
 raise SystemExit(code)
finally:
 if child is not None and child.poll() is None:
  os.killpg(child.pid,signal.SIGINT)
  try: child.wait(timeout=15)
  except subprocess.TimeoutExpired:
   os.killpg(child.pid,signal.SIGKILL); child.wait(timeout=5)
'''


def docker(container, code, arguments=(), payload=None):
    result = subprocess.run(['docker','exec','-i',container,'python3','-c',code,*arguments], input=payload,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    if result.returncode:
        raise RuntimeError('native phase exit '+str(result.returncode)+': '+result.stderr.decode(errors='replace')[-4000:])
    return result



def execute_owned(container, stage):
    command=['docker','exec','-i',container,'python3','-c',EXECUTE,stage]
    process=subprocess.Popen(command,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    previous={sig:signal.getsignal(sig) for sig in (signal.SIGINT,signal.SIGTERM,signal.SIGHUP)}
    def interrupted(sig,frame): raise KeyboardInterrupt('host controller interrupted')
    for sig in previous: signal.signal(sig,interrupted)
    try:
        stdout,stderr=process.communicate()
    except KeyboardInterrupt:
        # Docker client interruption does not reliably forward into a guest.
        # Signal only the exact retained native supervisor after birth/UID check.
        check="""import pathlib,json,os,signal,sys
root=pathlib.Path(sys.argv[1]); row=json.loads((root/'owned-execution.json').read_bytes()); p=pathlib.Path('/proc')/str(row['pid'])
stat=(p/'stat').read_text(); assert stat[stat.rfind(')')+2:].split()[19]==row['start_ticks']
assert p.stat().st_uid==row['uid']==os.getuid(); os.kill(row['pid'],signal.SIGINT)
"""
        docker(container,check,[stage])
        try: stdout,stderr=process.communicate(timeout=30)
        except subprocess.TimeoutExpired:
            raise RuntimeError('owned interruption cleanup has no terminal confirmation; preserve stage and audit exact actors') from None
        raise KeyboardInterrupt('owned native interruption completed; retained stage remains failed') from None
    finally:
        for sig,handler in previous.items(): signal.signal(sig,handler)
    if process.returncode:
        raise RuntimeError('owned workflow exit '+str(process.returncode)+': '+stderr.decode(errors='replace')[-4000:])
    return stdout


def container_birth(container):
    result=subprocess.run(['docker','inspect','--format','{{json .Id}} {{json .Image}} {{json .State.StartedAt}} {{json .State.Running}}',container],capture_output=True,check=True,text=True)
    decoder=json.JSONDecoder(); values=[]; remaining=result.stdout.strip()
    while remaining:
        value,end=decoder.raw_decode(remaining); values.append(value); remaining=remaining[end:].lstrip()
    if len(values)!=4 or values[3] is not True or not values[0] or not values[1] or not values[2]:
        raise ValueError('running container identity unavailable')
    return {'id':values[0],'image':values[1],'started_at':values[2],'running':True}


def unchanged_birth(container,expected):
    if container_birth(container)!=expected: raise ValueError('Docker container birth changed; old actor grants refused')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rev',required=True)
    parser.add_argument('--product-grant',required=True)
    parser.add_argument('--container',default='sail-m7-handoff-validation')
    parser.add_argument('--environment-label',default='Linux amd64 Docker Desktop emulation on arm64 macOS; actual concurrent development work')
    parser.add_argument('--stage')
    parser.add_argument('--execute',action='store_true')
    args=parser.parse_args()
    path=pathlib.Path(args.product_grant)
    if path.is_file(): raw=path.read_bytes()
    else: raw=docker(args.container,'import pathlib,sys; sys.stdout.buffer.write(pathlib.Path(sys.argv[1]).read_bytes())',[args.product_grant]).stdout
    if len(raw)>1024*1024: raise ValueError('oversized product grant')
    grant=grant_check(json.loads(raw,object_pairs_hook=unique),args.rev)
    stage=args.stage or '/tmp/sail-m7-current-reader-'+args.rev
    if not re.fullmatch('/tmp/sail-m7-current-reader-[A-Za-z0-9_-]+',stage):
        raise ValueError('private Sail stage name required')
    births={name:container_birth(name) for name in {'sail-m7-acceptance',grant['container']}}
    bundle=docker('sail-m7-acceptance',PRODUCER,[SOURCE]).stdout
    for name,birth in births.items(): unchanged_birth(name,birth)
    if len(bundle)>MAX_BUNDLE: raise ValueError('staging bundle too large')
    cfg={'docker_birth':births,'stage':stage,'grant':grant,'grant_sha':hashlib.sha256(raw).hexdigest(),'manifest':MANIFEST,'label':args.environment_label,
         'files':{'m7-descriptor-reader-workflow.py':WRAPPER,'m7-sysroot-proof-diagnostic.py':SUPPORT,
                  'scripts/m7-reader-acceptance.py':READER,'scripts/m7-retained-jobs-acceptance.py':RETAINED,
                  'probes/python-reader-probe.py':PYTHON,'probes/job_read_bench':RUST}}
    prepared=docker(grant['container'],PREPARE,[json.dumps(cfg)],bundle)
    print(prepared.stdout.decode(),end='',flush=True)
    # Independent original wrapper freeze-only checks run before any workflow actor.
    freeze=docker(grant['container'],'import subprocess,sys; raise SystemExit(subprocess.run(["python3",sys.argv[1]+"/driver.py","--freeze-only"]).returncode)',[stage])
    for name,birth in births.items(): unchanged_birth(name,birth)
    host_freeze=pathlib.Path('/tmp')/(pathlib.PurePosixPath(stage).name+'-original-wrapper-freeze.json')
    with host_freeze.open('xb') as stream: stream.write(freeze.stdout)
    print('static original-wrapper freeze passed; proof_launched=false; seal_sha256='+hashlib.sha256(freeze.stdout).hexdigest(),flush=True)
    if args.execute:
        for name,birth in births.items(): unchanged_birth(name,birth)
        stdout=execute_owned(grant['container'],stage)
        print(stdout.decode(),end='')


class Controls(unittest.TestCase):
    def good(self):
        return {'source_commit':'a'*40,'installation_only':True,'container':'sail-m7-handoff-validation',
                'product_dir':'/opt/sail-m7-core-product-aaaaaaa','engine_build':'/opt/source',
                'phases':[{'phase':'release-build','exit_code':0},{'phase':'owned-install','exit_code':0}], 'artifacts':{n:{'bytes':1,'sha256':'b'*64} for n in ('sail','sail-z3')}}
    def test_exact_grant(self): self.assertEqual(grant_check(self.good(),'a'*7)['source_commit'],'a'*40)
    def test_wrong_revision(self):
        with self.assertRaises(ValueError): grant_check(self.good(),'c'*7)
    def test_failed_phase(self):
        g=self.good(); g['phases'][0]['exit_code']=1
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_missing_phase(self):
        g=self.good(); g['phases']=[]
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_incomplete_successful_phases(self):
        g=self.good(); g['phases']=g['phases'][:1]
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_duplicate_successful_phase(self):
        g=self.good(); g['phases'][1]['phase']='release-build'
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_foreign_container(self):
        g=self.good(); g['container']='other-project'
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_duplicate_json(self):
        with self.assertRaises(ValueError): json.loads('{"x":1,"x":2}',object_pairs_hook=unique)
    def test_incomplete_artifact(self):
        g=self.good(); del g['artifacts']['sail-z3']
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_oversized_artifact(self):
        g=self.good(); g['artifacts']['sail']['bytes']=128*1024*1024+1
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_relative_binding(self):
        g=self.good(); g['engine_build']='relative/runtime'
        with self.assertRaises(ValueError): grant_check(g,'a'*7)
    def test_changed_docker_birth(self):
        from unittest.mock import patch
        before={'id':'container','image':'image','started_at':'old','running':True}
        after=dict(before,started_at='new')
        with patch(__name__+'.container_birth',return_value=after):
            with self.assertRaises(ValueError): unchanged_birth('test',before)
    def test_native_syntax(self):
        for source in (PRODUCER,PREPARE,EXECUTE): compile(source,'native','exec')


if __name__=='__main__':
    if sys.argv[1:]==['--self-test']: unittest.main(argv=[sys.argv[0]])
    else: main()
