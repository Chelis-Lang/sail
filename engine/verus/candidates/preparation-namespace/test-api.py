#!/usr/bin/env python3
"""Run the fixed-slot helper and actual pinned SourceMap controls; not qualification."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

p=argparse.ArgumentParser();p.add_argument('--work-dir',required=True,type=Path);args=p.parse_args()
base=Path(__file__).resolve().parent; work=args.work_dir.resolve();work.mkdir(parents=True,exist_ok=False)
for name in ['namespace.rs','tests.rs']:shutil.copyfile(base/name,work/name)
env=dict(os.environ,RUSTC_BOOTSTRAP='1');env['DYLD_LIBRARY_PATH']=subprocess.check_output(['rustc','+1.98.1','--print','sysroot'],text=True).strip()+'/lib'
commands=[['rustc','+1.98.1','--edition=2024','--test','-C','prefer-dynamic',str(work/'tests.rs'),'-o',str(work/'probe')],[str(work/'probe'),'--test-threads=1']]
records=[]
for name,command,limit in zip(['compile','tests'],commands,[60,30]):
 start=time.monotonic()
 with (work/(name+'.stdout')).open('wb') as out,(work/(name+'.stderr')).open('wb') as err:
  child=subprocess.Popen(command,env=env,stdout=out,stderr=err,start_new_session=True)
  try:code=child.wait(timeout=limit)
  except subprocess.TimeoutExpired:
   import signal
   os.killpg(child.pid,signal.SIGKILL);child.wait();raise
 records.append({'name':name,'argv':command,'exit_code':code,'elapsed_seconds':time.monotonic()-start,'timeout_seconds':limit})
 if code:break
sha=lambda f:hashlib.sha256(f.read_bytes()).hexdigest()
summary={'version':1,'scope':'fixed-slot helper and actual pinned default-mode SourceMap API; source integration/runtime/qualification pending','status':'PASS' if len(records)==2 and all(r['exit_code']==0 for r in records) else 'FAIL','records':records,'raw_sha256':{f.name:sha(f) for f in work.iterdir() if f.is_file()}}
(work/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({'status':summary['status'],'records':records}));raise SystemExit(0 if summary['status']=='PASS' else 1)
