#!/usr/bin/env python3
"""Actual full caller cleanup after injected wait4 failure.

Owned sleeping children, streams and reap facts are real. Mac pidfd/image and
P_PIDFD waitid are explicit descriptor-checked doubles; Linux qualification is
separate. Optional old-source run preserves the original refusal/leak witness
and always cleans its exact still-unreaped fixture child afterward.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
from unittest.mock import patch


def main():
 p=argparse.ArgumentParser();p.add_argument('--source',type=Path,default=Path(__file__).parent);p.add_argument('--out',type=Path,required=True);p.add_argument('--old',action='store_true');a=p.parse_args();a.out.mkdir(exist_ok=False)
 sys.path.insert(0,str(a.source.resolve(strict=True)))
 import owned_wait4,kernel_accounting,live_prefix
 actual_wait4=os.wait4;actual_thread=threading.Thread;owner_type=owned_wait4.OwnedWait4
 rows=[]
 for caller in ('fresh','pool-startup'):
  with tempfile.TemporaryDirectory() as td:
   root=Path(td);ready=root/'ready';worker=root/'worker'
   worker.write_text('#!'+sys.executable+'\nimport pathlib,os,time\nos.write(1,b"startup-out");os.write(2,b"startup-err");pathlib.Path('+repr(str(ready))+').touch();time.sleep(30)\n');worker.chmod(0o700)
   owners=[];fd_pids={};wait_errors=[];signals=[];reaps=[];thread_calls=[]
   def create(*args,**kw):
    owner=owner_type(*args,**kw);owners.append(owner);return owner
   def capture(pid,expected):
    deadline=time.monotonic()+3
    while not ready.exists():
     if time.monotonic()>deadline:raise OSError('owned fixture startup deadline')
     time.sleep(.005)
    first,second=os.open(os.devnull,os.O_RDONLY),os.open(os.devnull,os.O_RDONLY);fd_pids[first]=pid;return first,second,None
   def send(fd,sig):
    os.fstat(fd);signals.append(dict(fd=fd,pid=fd_pids[fd],signal=sig));os.kill(fd_pids[fd],sig)
   def broken_wait4(pid,flags):
    wait_errors.append(pid);raise ChildProcessError('injected wait4 accounting error without reap')
   def waitid(kind,fd,flags):
    assert kind==4242 and flags==os.WEXITED|os.WNOHANG
    os.fstat(fd);pid,status,_=actual_wait4(fd_pids[fd],os.WNOHANG)
    if pid==0:return None
    reaps.append(dict(pid=pid,status=status))
    return SimpleNamespace(si_pid=pid,si_signo=signal.SIGCHLD,si_code=os.CLD_EXITED if os.WIFEXITED(status) else os.CLD_KILLED,si_status=os.WEXITSTATUS(status) if os.WIFEXITED(status) else os.WTERMSIG(status))
   def thread(*args,**kw):
    thread_calls.append(True)
    if caller=='pool-startup' and len(thread_calls)==2:
     assert owners[0]._done.wait(2)
     raise RuntimeError('injected worker-constructor failure after accounting error')
    return actual_thread(*args,**kw)
   result=None;entry=None;call_error=None
   try:
    with contextlib.ExitStack() as stack:
     for mod in (owned_wait4,kernel_accounting):stack.enter_context(patch.object(mod,'OwnedWait4',create))
     stack.enter_context(patch.object(live_prefix,'capture_running_image',capture));stack.enter_context(patch.object(kernel_accounting,'capture_running_image',capture))
     stack.enter_context(patch.object(live_prefix.Entry,'_image_ok',lambda _:True));stack.enter_context(patch.object(os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))))
     stack.enter_context(patch.object(os,'pidfd_open',lambda *_:None,create=True));stack.enter_context(patch.object(signal,'pidfd_send_signal',send,create=True))
     stack.enter_context(patch.object(os,'P_PIDFD',4242,create=True));stack.enter_context(patch.object(os,'waitid',waitid,create=True))
     stack.enter_context(patch.object(owned_wait4.os,'wait4',broken_wait4));stack.enter_context(patch.object(threading,'Thread',thread))
     if caller=='fresh':result=kernel_accounting.fresh(str(worker),'doubled-image',b'input',.05,1024**3)
     else:
      pool=live_prefix.OneUsePool(str(worker),hashlib.sha256(worker.read_bytes()).hexdigest(),experiment_numeric_time_only=True,experiment_kernel_accounting_owner=True)
      assert pool.seed(b'prefix',b'') is None
      entry=pool.last_attempt;pool.close()
   except BaseException as er:call_error=type(er).__name__+': '+str(er)
   assert len(owners)==1
   owner=owners[0];alive=False;unreaped=False;live=False;fixture_reap=None
   try:
    os.kill(owner.pid,0);alive=True
   except ProcessLookupError:pass
   # WNOHANG here is fixture cleanup only, after the entire product caller.
   # The owner is held, its injected waiter never reaped, and no replacement
   # child is created. Production error cleanup never uses this PID API.
   try:
    pid,status,_=actual_wait4(owner.pid,os.WNOHANG)
    if pid==0:
     live=True;unreaped=True;os.kill(owner.pid,signal.SIGKILL);pid,status,_=actual_wait4(owner.pid,0)
    else:unreaped=True
    owner._child.returncode=os.waitstatus_to_exitcode(status)
    fixture_reap=dict(pid=pid,status=status,exit=owner._child.returncode)
   except ChildProcessError:pass
   raw=result if result is not None else dict(stdout=bytes(entry.stdout),stderr=bytes(entry.stderr))
   for stream in ('stdout','stderr'):(a.out/(caller+'.'+stream)).write_bytes(raw[stream])
   row=dict(caller=caller,old=a.old,passed=False,caller_error=call_error,wait4_errors=wait_errors,signals=signals,safe_descriptor_reaps=reaps,child_present_after_caller=alive,live_child_after_caller=live,fixture_reap_needed=unreaped,fixture_reap=fixture_reap,raw={k:dict(bytes=len(raw[k]),sha256=hashlib.sha256(raw[k]).hexdigest()) for k in ('stdout','stderr')})
   if result is not None:row['result']={k:v for k,v in result.items() if k not in ('stdout','stderr')}
   if entry is not None:row.update(entry_failure=entry.failure,entry_accounting=entry.kernel_accounting,entry_cleanup_errors=entry.cleanup_errors,error_cleanup=entry.process.cleanup_observation() if hasattr(entry.process,'cleanup_observation') else None)
   if a.old:
    assert unreaped,'original caller no longer reproduces missing safe reap'
    if caller=='pool-startup':assert alive and not signals
   else:
    assert not alive and not unreaped and len(reaps)==1,row
    assert signals and not call_error
    assert raw['stdout']==b'startup-out' and raw['stderr']==b'startup-err'
    if result is not None:
     assert result['error'] and result['kernel_accounting'] is None and not result['kernel_resource_gate_pass']
     assert result['exit']==-signal.SIGKILL and result['kernel_error_cleanup']['accounting_available'] is False
    else:assert entry.kernel_accounting is None and entry.kernel_accounting_error and entry.failure
   assert wait_errors
   row['passed']=True;rows.append(row);print(caller,'PASS',flush=True)
 record=dict(passed=True,old=a.old,cases=len(rows),rows=rows,source_sha256={n:hashlib.sha256((a.source/n).read_bytes()).hexdigest() for n in ('owned_wait4.py','kernel_accounting.py','live_prefix.py')},scope=__doc__)
 (a.out/'results.json').write_text(json.dumps(record,indent=2)+'\n')
 return 0


if __name__=='__main__':raise SystemExit(main())
