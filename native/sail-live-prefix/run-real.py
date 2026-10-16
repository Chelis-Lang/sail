#!/usr/bin/env python3
"""Sequential strict fresh controls, then eligible one-use research samples."""
import argparse,hashlib,json,os,platform,signal,subprocess,time
from pathlib import Path
import response_policy
SOLVER='e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765'
def digest(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def main():
 ap=argparse.ArgumentParser(description=__doc__);ap.add_argument('--source',type=Path,required=True);ap.add_argument('--out',type=Path,required=True);ap.add_argument('--z3',type=Path,required=True);ap.add_argument('--experiment-numeric-time-only',action='store_true');ap.add_argument('--project',choices=['ironkv','anvil']);a=ap.parse_args()
 assert platform.system()=='Linux' and platform.machine()=='x86_64';src=a.source.resolve(strict=True);z3=a.z3.resolve(strict=True);assert digest(z3)==SOLVER
 hashes=None
 a.out.mkdir(parents=True,exist_ok=False);m={'complete':False,'scope':'real archived full traces; strict research controls only; no default integration','solver_sha256':SOLVER,'steps':[],'projects':{},'errors':[],'qualified':False,'selected':False,'response_policy':response_policy.EXPERIMENT if a.experiment_numeric_time_only else 'exact-bytes'}
 def save():
  tmp=a.out/'manifest.tmp';tmp.write_text(json.dumps(m,indent=2)+'\n');tmp.replace(a.out/'manifest.json')
 def interrupted(signum,frame):raise KeyboardInterrupt('signal '+str(signum))
 signal.signal(signal.SIGTERM,interrupted)
 def run(label,cmd):
  row={'label':label,'command':list(map(str,cmd))};m['steps'].append(row);save();started=time.monotonic()
  with (a.out/(label+'.stdout')).open('wb')as so,(a.out/(label+'.stderr')).open('wb')as se:
   child=subprocess.Popen(cmd,stdout=so,stderr=se,stdin=subprocess.DEVNULL,cwd=src,start_new_session=True)
   try:child.wait(timeout=1100)
   except BaseException as er:
    row['error']=type(er).__name__
    try:os.killpg(child.pid,signal.SIGKILL)
    except ProcessLookupError:pass
    child.wait(timeout=10)
    if not isinstance(er,subprocess.TimeoutExpired):raise
   finally:
    if child.poll() is None:os.killpg(child.pid,signal.SIGKILL);child.wait(timeout=10)
    row.update(exit=child.returncode,wall_seconds=time.monotonic()-started,stdout_sha256=digest(a.out/(label+'.stdout')),stderr_sha256=digest(a.out/(label+'.stderr')));save()
  return row
 try:
  hashes=json.loads((src/'source-hashes.json').read_text())
  for name,value in hashes.items():assert digest(src/name)==value,(name,'source mismatch')
  m['source_sha256']=hashes;m['controller_sha256']=digest(Path(__file__));m['environment_sha256']=hashlib.sha256(json.dumps(sorted(os.environ.items())).encode()).hexdigest();m['platform']=platform.platform()
  selected={r['project']:r for r in json.loads((src/'pairs.json').read_text())['selected']}
  for project in ([a.project] if a.project else ['ironkv','anvil']):
   data=src/project;full=(data/'prefix.smt2').read_bytes()+(data/'suffix.smt2').read_bytes();assert hashlib.sha256(full).hexdigest()==selected[project]['edited.smt2_sha256'];m['projects'][project]={'trace':selected[project]}
   common=['python3',str(src/'run.py'),'--z3',str(z3),'--solver-sha256',SOLVER,'--prefix',str(data/'prefix.smt2'),'--prefix-stdout',str(data/'prefix.stdout'),'--suffix',str(data/'suffix.smt2'),'--native-metrics','--preparation-window','300','--ttl','600','--run-timeout','300','--oracle-timeout','300','--fallback-timeout','300','--max-bytes',str(1024**3),'--max-input-bytes',str(16*1024**2)]
   if a.experiment_numeric_time_only:common+=['--experiment-numeric-time-only']
   freshout=a.out/(project+'-fresh');row=run(project+'-fresh',common+['--out',str(freshout),'--fresh-only','--fresh-runs','3']);fm=json.loads((freshout/'manifest.json').read_text());m['projects'][project]['fresh_exit']=row['exit'];m['projects'][project]['fresh_control_pass']=fm.get('fresh_control_pass',False)
   strict_compatible=(fm.get('fresh_control_pass',False) and (freshout/'fresh-0.stdout').read_bytes().startswith((data/'prefix.stdout').read_bytes()))
   m['projects'][project]['historical_prefix_compatible_strict']=strict_compatible
   if a.experiment_numeric_time_only:
    comparison=response_policy.compare_prefix((data/'prefix.stdout').read_bytes(),(freshout/'fresh-0.stdout').read_bytes())
    m['projects'][project]['historical_prefix_comparison']=comparison
    eligible=fm.get('experiment_fresh_control_pass',False)
    compatible=eligible and comparison['prefix_equal_except_numeric_time']
    m['projects'][project]['experiment_fresh_control_pass']=eligible
   else:
    eligible=fm.get('fresh_control_pass',False)
    compatible=strict_compatible
   m['projects'][project]['historical_prefix_compatible']=compatible
   if compatible:
    liveout=a.out/(project+'-one-use');lr=run(project+'-one-use',common+['--out',str(liveout)]);lm=json.loads((liveout/'manifest.json').read_text());m['projects'][project]['one_use_exit']=lr['exit'];m['projects'][project]['candidate_gate_pass']=lm.get('candidate_gate_pass',False)
    experiment_pass=lm.get('experiment_candidate_gate_pass',False)
    m['projects'][project]['experiment_candidate_gate_pass']=experiment_pass
    gate=experiment_pass if a.experiment_numeric_time_only else lm.get('candidate_gate_pass',False)
    if lr['exit'] or not gate:m['errors'].append(project+' one-use selected-policy gate failed')
   else:
    reason='selected-policy fresh/fresh gate failed (including any retained error)' if not eligible else 'historical expected prefix differs from complete guarded fresh output under selected policy'
    m['projects'][project]['one_use_skipped']=reason;m['errors'].append(project+' '+reason)
   save()
  assert digest(z3)==SOLVER
  for name,value in hashes.items():assert digest(src/name)==value,(name,'source changed')
  m['complete']=True
 except BaseException as er:m['errors'].append(type(er).__name__+': '+str(er));raise
 finally:
  try:
   m['final_solver_identity_stable']=digest(z3)==SOLVER
   m['final_source_identity_stable']=hashes is not None and all(digest(src/name)==value for name,value in hashes.items())
   if not m['final_solver_identity_stable'] or not m['final_source_identity_stable']:m['errors'].append('final solver/source identity check failed')
  except BaseException as er:m['errors'].append('final identity check '+type(er).__name__+': '+str(er))
  save()
 return 1 if m['errors'] else 0
if __name__=='__main__':raise SystemExit(main())
