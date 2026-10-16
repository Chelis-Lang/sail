#!/usr/bin/env python3
"""Bounded direct A/B compiler research; preserves raw streams and every failure."""
import argparse,collections,hashlib,json,os,platform,resource,shutil,signal,subprocess,time
from pathlib import Path
PROJECTS={
 'vest':('https://github.com/secure-foundations/vest','11af01ddaef76cceded03517cd48764adb9d35b1','vest_lib','vest_lib/src/asn1/der_ord.rs'),
 'ironkv':('https://github.com/verus-lang/verified-ironkv','7e35ed097c4089b9fbda483ea2629a0033f4d874','ironsht','ironsht/src/host_impl_v.rs')}
ARMS=['a-base','b-readonly-pruning']
def sha(p):
 h=hashlib.sha256()
 with p.open('rb') as f:
  while b:=f.read(65536):h.update(b)
 return h.hexdigest()
def tree(p):return {str(f.relative_to(p)):sha(f) for f in sorted(p.rglob('*')) if f.is_file()}
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--work',type=Path,required=True);ap.add_argument('--rlibs',type=Path,required=True);ap.add_argument('--timeout',type=int,default=600);a=ap.parse_args()
 if platform.system()!='Linux' or platform.machine()!='x86_64' or not 1<=a.timeout<=900:ap.error('bounded native Linux only')
 work=a.work.resolve(strict=True);out=work/'profile-v2-results';out.mkdir(exist_ok=False)
 build=work/'build-results';bm=json.loads((build/'manifest.json').read_text());assert bm['complete']
 m={'scope':'direct A/B compiler research; no Sail/default/corpus acceptance','complete':False,'candidate_gate':False,'research_gate':False,'runtime_hashes':{},'steps':[],'rows':[],'errors':[],'platform':platform.platform(),'build_manifest_sha256':sha(build/'manifest.json')}
 env=dict(os.environ,RUSTUP_TOOLCHAIN='1.98.1',CARGO_BUILD_JOBS='8',CARGO_INCREMENTAL='0')
 for k in list(env):
  if k.startswith(('SAIL_','__VERUS_')) or k in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','VERUS_Z3_PATH','LD_PRELOAD','DYLD_INSERT_LIBRARIES','CARGO_BUILD_BUILD_DIR']:env.pop(k,None)
 restorations=[]
 def restore(root,saved):
  for path in root.rglob('Cargo.lock'):
   if '.git' not in path.relative_to(root).parts and path not in saved:path.unlink()
  for path,data in saved.items():path.write_bytes(data)
 def save():
  temp=out/'manifest.tmp';temp.write_text(json.dumps(m,indent=2)+'\n');temp.replace(out/'manifest.json')
 def interrupted(signum,frame):raise KeyboardInterrupt('signal '+str(signum))
 signal.signal(signal.SIGTERM,interrupted)
 def run(label,cmd,cwd,e,limit=None):
  row={'label':label,'command':list(map(str,cmd)),'cwd':str(cwd),'environment_sha256':hashlib.sha256(json.dumps(sorted(e.items())).encode()).hexdigest()};m['steps'].append(row);save();t=time.monotonic();before=resource.getrusage(resource.RUSAGE_CHILDREN)
  with (out/(label+'.stdout')).open('wb') as so,(out/(label+'.stderr')).open('wb') as se:
   p=subprocess.Popen(cmd,cwd=cwd,env=e,stdout=so,stderr=se,stdin=subprocess.DEVNULL,start_new_session=True)
   try:p.wait(timeout=limit or a.timeout)
   except BaseException as er:
    row['error']=type(er).__name__
    try:os.killpg(p.pid,signal.SIGKILL)
    except ProcessLookupError:pass
    p.wait(timeout=10)
    if not isinstance(er,subprocess.TimeoutExpired):raise
   finally:
    if p.poll() is None:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=10)
    after=resource.getrusage(resource.RUSAGE_CHILDREN);row.update(exit=p.returncode,wall_seconds=time.monotonic()-t,child_user_seconds=after.ru_utime-before.ru_utime,child_system_seconds=after.ru_stime-before.ru_stime)
    row['stdout_sha256']=sha(out/(label+'.stdout'));row['stderr_sha256']=sha(out/(label+'.stderr'))
    raw=(out/(label+'.stdout')).read_text(errors='replace');decoder=json.JSONDecoder();objects=[];pos=0
    while pos<len(raw):
     start=raw.find('{',pos)
     if start<0:break
     try:obj,end=decoder.raw_decode(raw,start)
     except json.JSONDecodeError:pos=start+1;continue
     pos=end
     if isinstance(obj,dict) and 'verification-results' in obj:objects.append(obj)
    row['func_details']=[obj.get('func-details') for obj in objects];row['verification_results']=[obj['verification-results'] for obj in objects];row['times_ms']=[obj.get('times-ms') for obj in objects]
    if label.endswith(('-cold','-warm','-capture')) and (not objects or not sum(obj['verification-results'].get('verified',0) for obj in objects) or any(obj['verification-results'].get('errors',1)!=0 or obj['verification-results'].get('encountered-error',True) or obj['verification-results'].get('encountered-vir-error',True) for obj in objects)):
     row['actual_verification_error']='missing positive verification or reported errors';m['errors'].append(label+' '+row['actual_verification_error'])
    save()
  return row
 try:
  assert bm['base']=='8a373f81a507b7ef4f144767ccd237d3d3de6c48'
  for arm in ARMS:
   expected=next(r['runtime_sha256'] for r in bm['steps'] if r.get('arm')==arm)
   actual=tree(build/arm);assert actual==expected,(arm,'runtime changed before profiles');m['runtime_hashes'][arm]=actual
   r=run(arm+'-version',[str(build/arm/'verus'),'--version'],work,env,60);assert r['exit']==0
  # Compile only a tiny patcher against existing read-only gate rlibs, never Cargo.
  patchsrc=out/'patcher.rs';patchsrc.write_text('fn main()->Result<(),Box<dyn std::error::Error>>{let a:Vec<std::path::PathBuf>=std::env::args_os().skip(1).map(Into::into).collect();if a.len()!=3{return Err("need package/root/verus".into())}let p=sail_verus::patch::patch_verus_deps(&a[0],&a[1],&a[2])?;println!("{:?} {:?}",p.manifest,p.crates);Ok(())}\n')
  rlibs=list(a.rlibs.glob('libsail_verus-*.rlib'));assert len(rlibs)==1
  pe=dict(env,RUSTUP_TOOLCHAIN='1.99.0');r=run('patcher-build',['rustc','--edition=2024',str(patchsrc),'-L','dependency='+str(a.rlibs),'--extern','sail_verus='+str(rlibs[0]),'-o',str(out/'patcher')],work,pe,60);assert r['exit']==0
  m['patcher_rlib_sha256']=sha(rlibs[0])
  tee=out/'tee-z3.sh';tee.write_text('#!/bin/sh\nf=$(mktemp "$SAIL_M8_CAPTURE/z3-XXXXXXXXXX") || exit 125\ntee "$f" | "$SAIL_M8_REAL_Z3" "$@"\n');tee.chmod(0o755)
  projects=work/'profiles-v2';projects.mkdir(exist_ok=False)
  for project,(url,pin,package,edit) in PROJECTS.items():
   root=projects/project
   for label,cmd in [('clone',['git','clone','--no-checkout','--filter=blob:none',url,str(root)]),('checkout',['git','checkout','--detach',pin])]:
    r=run(project+'-'+label,cmd,work if label=='clone' else root,env,180);assert r['exit']==0
   head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root).decode().strip();assert head==pin
   original=(root/edit).read_bytes();target=projects/(project+'-target')
   saved={path:path.read_bytes() for path in root.rglob('*') if path.is_file() and '.git' not in path.relative_to(root).parts and path.name in ['Cargo.toml','Cargo.lock']};saved[root/edit]=original
   restorations.append((root,saved));m.setdefault('pristine_inputs',{})[project]={str(path.relative_to(root)):hashlib.sha256(data).hexdigest() for path,data in saved.items()}
   # Two rounds reverse arm order; cold work retained, same paths and source bytes per round.
   for roundno,order in enumerate([ARMS,list(reversed(ARMS))]):
    for arm in order:
     if target.exists():shutil.rmtree(target)
     restore(root,saved)
     pristine_diff=subprocess.check_output(['git','diff'],cwd=root);assert not pristine_diff,('pristine restore failed',project,arm)
     m.setdefault('arm_pristine_inputs',[]).append({'project':project,'round':roundno,'arm':arm,'files':{str(path.relative_to(root)):sha(path) for path in saved}})
     r=run(f'{project}-r{roundno}-{arm}-patch',[str(out/'patcher'),str(root/package),str(root),str(build/arm)],root,env,60);assert r['exit']==0
     e=dict(env,CARGO_TARGET_DIR=str(target),VERUS_Z3_PATH=str(build/arm/'z3'))
     cmd=[str(build/arm/'cargo-verus'),'verify','--','--time','--time-expanded','--output-json']
     cold=run(f'{project}-r{roundno}-{arm}-cold',cmd,root/package,e);m['rows'].append(dict(project=project,round=roundno,arm=arm,phase='cold',**cold))
     # Same-byte noop edit forces the primary compiler, without changing SMT queries.
     (root/edit).write_bytes(original+b'\n// sail frontend research noop\n')
     warm=run(f'{project}-r{roundno}-{arm}-warm',cmd,root/package,e);m['rows'].append(dict(project=project,round=roundno,arm=arm,phase='warm',**warm))
     if cold['exit']!=0 or warm['exit']!=0:m['errors'].append(project+' '+arm+' verification failure')
     if roundno==0:
      cap=out/(project+'-'+arm+'-smt');cap.mkdir();ce=dict(e,VERUS_Z3_PATH=str(tee),SAIL_M8_CAPTURE=str(cap),SAIL_M8_REAL_Z3=str(build/arm/'z3'))
      (root/edit).write_bytes(original+b'\n// sail frontend research capture\n')
      captured=run(f'{project}-{arm}-capture',cmd,root/package,ce);m['rows'].append(dict(project=project,round=roundno,arm=arm,phase='capture-excluded-from-timing',**captured));m.setdefault('smt',{})[project+'-'+arm]=dict(collections.Counter(sha(f) for f in cap.iterdir() if f.is_file()))
      if not m['smt'][project+'-'+arm]:m['errors'].append(project+' '+arm+' empty SMT capture')
      if captured['exit']!=0:m['errors'].append(project+' '+arm+' capture failure')
     m.setdefault('arm_resolved_inputs',[]).append({'project':project,'round':roundno,'arm':arm,'manifest_files':{str(path.relative_to(root)):sha(path) for path in saved if path.name=='Cargo.toml'},'lock_sha256':sha(root/'Cargo.lock') if (root/'Cargo.lock').exists() else None})
     save()
   restore(root,saved)
   m.setdefault('projects',{})[project]={'pin':pin,'dirty_diff_sha256':hashlib.sha256(subprocess.check_output(['git','diff'],cwd=root)).hexdigest(),'lock_sha256':sha(root/'Cargo.lock') if (root/'Cargo.lock').exists() else None}
  m['verification_equal']={}
  for project in PROJECTS:
   for roundno in [0,1]:
    for phase in ['cold','warm']:
     pair=[next(row for row in m['rows'] if row['project']==project and row['round']==roundno and row['phase']==phase and row['arm']==arm) for arm in ARMS]
     equal=all(pair[0][field]==pair[1][field] for field in ['verification_results','func_details'])
     m['verification_equal'][f'{project}-r{roundno}-{phase}']=equal
     if not equal:m['errors'].append(f'{project} r{roundno} {phase} verification JSON differs')
  m['raw_smt_multiset_equal']={p:m['smt'][p+'-'+ARMS[0]]==m['smt'][p+'-'+ARMS[1]] for p in PROJECTS}
  for arm in ARMS:assert tree(build/arm)==m['runtime_hashes'][arm],('runtime mutated',arm)
  if not all(m['raw_smt_multiset_equal'].values()):m['errors'].append('raw SMT stream multiset differs across A/B')
  m['complete']=True;m['research_gate']=not m['errors']
 except BaseException as er:
  m['errors'].append(type(er).__name__+': '+str(er));raise
 finally:
  for root,saved in restorations:
   try:
    restore(root,saved);diff=subprocess.check_output(['git','diff'],cwd=root)
    if diff:raise ValueError('unexpected tracked project changes after restoration')
   except BaseException as er:m['errors'].append('restoration '+str(root)+': '+str(er));m['research_gate']=False
  save()
 return 0 if m['research_gate'] else 1
if __name__=='__main__':raise SystemExit(main())
