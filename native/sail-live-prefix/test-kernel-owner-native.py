#!/usr/bin/env python3
"""Mandatory bounded native guards for the named kernel-owner experiment.

Real pinned Z3 and real held Linux pidfd/executable controls. Not conformance,
trace eligibility, performance qualification, shared-library or process-tree
validation. Every failed/missing mandatory case blocks the later real traces.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import select
import subprocess
import sys
import time
from unittest.mock import patch

from owned_wait4 import OwnedWait4, positive_accounting, drain_unstarted
import kernel_accounting
import live_prefix
import owned_wait4

SOLVER = 'e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765'
PREFIX = b'(set-logic QF_UF)\n(assert false)\n(check-sat)\n'
SUFFIX = b'(exit)\n'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--z3',type=Path,required=True)
    parser.add_argument('--out',type=Path,required=True)
    a=parser.parse_args()
    assert (os.uname().sysname,os.uname().machine)==('Linux','x86_64')
    z3=str(a.z3.resolve(strict=True));assert live_prefix.path_digest(z3)==SOLVER
    a.out.mkdir(parents=True,exist_ok=False)
    rows=[];actual_capture=live_prefix.capture_running_image
    captured={};starts=[];start=OwnedWait4.start_reaper
    def capture(pid,expected):
        result=actual_capture(pid,expected)
        captured[pid]=result
        return result
    def release(owner):
        assert not owner._reaping_started and owner.pid in captured
        pidfd,image,_=captured[owner.pid]
        os.fstat(pidfd);os.fstat(image)
        assert not owner._done.is_set()
        starts.append(owner.pid)
        return start(owner)
    def save():
        data=dict(passed=len(rows)==10 and all(r['passed'] for r in rows),cases=rows,
                  source_sha256={n:hashlib.sha256(Path(__file__).with_name(n).read_bytes()).hexdigest() for n in ['test-kernel-owner-native.py','owned_wait4.py','kernel_accounting.py','live_prefix.py']},
                  platform=list(os.uname()),solver_sha256=SOLVER,owner_releases=starts,
                  scope=__doc__)
        (a.out/'controls.json').write_text(json.dumps(data,indent=2)+'\n')
    def one(name,fn):
        row=dict(case=name,passed=False);rows.append(row)
        try:fn(row);row['passed']=True
        except BaseException as error:row['error']=type(error).__name__+': '+str(error)
        finally:save()
    def retain(name,result,row):
        for stream in ('stdout','stderr'):
            raw=result[stream];(a.out/(name+'.'+stream)).write_bytes(raw)
        row['result']={k:(dict(bytes=len(v),sha256=hashlib.sha256(v).hexdigest()) if isinstance(v,bytes) else v) for k,v in result.items() if k not in ('stdout','stderr')}
        row['raw']={k:dict(bytes=len(result[k]),sha256=hashlib.sha256(result[k]).hexdigest()) for k in ('stdout','stderr')}
    with patch.object(live_prefix,'capture_running_image',capture),patch.object(kernel_accounting,'capture_running_image',capture),patch.object(OwnedWait4,'start_reaper',release):
        def fresh(row):
            r=kernel_accounting.fresh(z3,SOLVER,PREFIX+SUFFIX,10,1024**3);retain('fresh',r,row)
            assert r['kernel_resource_gate_pass'] and r['stdout']==b'unsat\n' and r['stderr']==b'' and r['exit']==0
            assert r['input_bytes_written']==len(PREFIX+SUFFIX)
        one('actual-pinned-fresh-complete-io',fresh)
        def live(row):
            pool=live_prefix.OneUsePool(z3,SOLVER,max_bytes=1024**3,ttl=10,run_timeout=10,experiment_numeric_time_only=True,experiment_kernel_accounting_owner=True)
            try:
                key=pool.seed(PREFIX,b'unsat\n');assert pool.wait_ready(key,5)
                pid=pool.entries[key].pid
                r=pool.consume(key,PREFIX+SUFFIX)
                assert r is not None
                retain('one-use',r,row)
                assert r['pid']==pid and r['stdout']==b'unsat\n' and r['stderr']==b'' and r['exit']==0
                assert r['kernel_accounting']['pid']==pid and r['kernel_accounting']['accounting']['peak_rss_bytes']>0
                assert pool.consume(key,PREFIX+SUFFIX) is None
            finally:
                pool.close()
                entry=pool.last_attempt
                if entry is not None:
                    for stream in ('stdout','stderr'):
                        (a.out/('one-use-attempt.'+stream)).write_bytes(getattr(entry,stream))
                    row['attempt_accounting']=entry.kernel_accounting
                    row['attempt_failure']=entry.failure
                    row['attempt_cleanup_errors']=entry.cleanup_errors
        one('actual-same-owned-pid-continuation',live)
        def over(row):
            r=kernel_accounting.fresh(z3,SOLVER,PREFIX+SUFFIX,10,1);retain('over-bound',r,row)
            assert not r['kernel_resource_gate_pass'] and r['exit']==0 and r['stdout']==b'unsat\n'
            assert r['kernel_accounting']['accounting']['peak_rss_bytes']>1 and r['error']
        one('actual-positive-peak-over-bound-refuses',over)
        def image(row):
            r=kernel_accounting.fresh(z3,'0'*64,PREFIX+SUFFIX,10,1024**3);retain('wrong-image',r,row)
            assert not r['kernel_resource_gate_pass'] and not r['image_verified']
            assert r['input_bytes_written']==0 and r['stdout']==b'' and r['error']
        one('actual-held-image-mismatch-before-input',image)
        def malformed(row):
            r=kernel_accounting.fresh(z3,SOLVER,b'(definitely-not-an-smt-command)\n',10,1024**3);retain('nonzero',r,row)
            assert r['exit']!=0 and r['error'] and not r['kernel_resource_gate_pass']
            assert b'error' in r['stdout']+r['stderr']
        one('actual-solver-nonzero-retains-diagnostic',malformed)
        def helper(row,kill=False):
            code='import os,sys\na=bytearray(64*1024*1024);a[::4096]=b"x"*16384;del a;print("released",flush=True);sys.stdin.buffer.read();os._exit(0)'
            owner=OwnedWait4([sys.executable,'-B','-c',code],deferred=True,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
            pidfd=imagefd=None;raw={'stdout':bytearray(),'stderr':bytearray()}
            try:
                expected=live_prefix.path_digest(str(Path(sys.executable).resolve(strict=True)))
                pidfd,imagefd,_=capture(owner.pid,expected)
                owner.start_reaper()
                # Bound readiness internally as well as by the native stage.
                os.set_blocking(owner.stdout.fileno(),False)
                deadline=time.monotonic()+10
                while b'\n' not in raw['stdout']:
                    remaining=deadline-time.monotonic()
                    if remaining<=0 or not select.select([owner.stdout],[],[],max(0,remaining))[0]:
                        raise subprocess.TimeoutExpired(owner.args,10)
                    block=os.read(owner.stdout.fileno(),512)
                    if not block:raise OSError('owned helper readiness EOF')
                    raw['stdout'].extend(block)
                    assert len(raw['stdout'])<=512,'owned readiness output cap'
                assert raw['stdout']==b'released\n'
                assert owner.poll() is None
                if kill:signal.pidfd_send_signal(pidfd,signal.SIGKILL)
                else:owner.stdin.close()
                owner.wait(10);row['accounting']=owner.accounting()
                if kill:assert row['accounting']['exit']==-signal.SIGKILL
                else:
                    positive_accounting(owner,1024**3)
                    assert row['accounting']['accounting']['peak_rss_bytes']>=64*1024**2
            finally:
                owner.cancel_unstarted()
                if owner.poll() is None:
                    if pidfd is not None:signal.pidfd_send_signal(pidfd,signal.SIGKILL)
                    else:os.kill(owner.pid,signal.SIGKILL)
                owner.wait(10)
                errors=[]
                for stream in ('stdout','stderr'):
                    errors+=drain_unstarted(getattr(owner,stream),raw[stream],16*1024**2)
                    (a.out/('helper-'+str(kill)+'.'+stream)).write_bytes(raw[stream])
                row['cleanup_errors']=errors;row['actual_exit']=owner.returncode
                for stream in (owner.stdin,owner.stdout,owner.stderr):stream.close()
                for fd in (imagefd,pidfd):
                    if fd is not None:os.close(fd)
                assert not errors
                try:os.wait4(owner.pid,os.WNOHANG)
                except ChildProcessError:row['sole_reap_confirmed']=True
                else:raise AssertionError('competing/uncompleted child reap')
        one('actual-released-allocation-kernel-lifetime-peak',helper)
        one('actual-held-pidfd-cancel-and-complete-accounting',lambda row:helper(row,True))
        def failed_wait4(row, already_reaped=False):
            import tempfile
            with tempfile.TemporaryDirectory() as td:
                ready=Path(td)/'ready';worker=Path(td)/'worker'
                worker.write_text('#!'+sys.executable+'\nimport os,pathlib,time\nos.write(1,b"startup-out");os.write(2,b"startup-err");pathlib.Path('+repr(str(ready))+').touch();'+('os.read(0,1);os._exit(0)' if already_reaped else 'time.sleep(30)')+'\n')
                worker.chmod(0o700)
                expected=live_prefix.path_digest(str(Path(sys.executable).resolve(strict=True)))
                actual_wait4=os.wait4;held=[];wait4_observed=[]
                def captured(pid, identity):
                    value=capture(pid,identity)
                    held.append(os.dup(value[0]))
                    deadline=time.monotonic()+3
                    while not ready.exists():
                        if time.monotonic()>deadline:raise OSError('native helper startup deadline')
                        time.sleep(.005)
                    return value
                def broken(pid,flags):
                    if already_reaped:
                        actual_pid,status,usage=actual_wait4(pid,flags)
                        wait4_observed.append(dict(pid=actual_pid,status=status,actual_exit=os.waitstatus_to_exitcode(status)))
                    raise ChildProcessError('injected native sole-wait4 accounting failure')
                try:
                    with patch.object(kernel_accounting,'capture_running_image',captured),patch.object(owned_wait4.os,'wait4',broken):
                        result=kernel_accounting.fresh(str(worker),expected,b'',.2,1024**3)
                    retain('failed-wait4-'+str(already_reaped),result,row)
                    row['helper_sha256']=hashlib.sha256(worker.read_bytes()).hexdigest()
                    row['helper_image_sha256']=expected;row['original_wait4_observed']=wait4_observed
                    assert result['error'] and not result['kernel_resource_gate_pass'] and result['kernel_accounting'] is None
                    assert result['stdout']==b'startup-out' and result['stderr']==b'startup-err'
                    cleanup=result['kernel_error_cleanup'];assert cleanup['accounting_available'] is False
                    if already_reaped:
                        assert cleanup['terminal_pidfd'] is True and cleanup['no_longer_waitable'] is True
                        assert result['exit'] is None and len(wait4_observed)==1
                    else:assert cleanup['actual_exit']==-signal.SIGKILL and result['exit']==-signal.SIGKILL
                    assert len(held)==1
                    observer=select.poll();observer.register(held[0],select.POLLIN);assert observer.poll(0)
                    try:os.waitid(os.P_PIDFD,held[0],os.WEXITED|os.WNOHANG)
                    except ChildProcessError:row['native_original_pidfd_no_longer_waitable']=True
                    else:raise AssertionError('original child still waitable after caller cleanup')
                finally:
                    for fd in held:os.close(fd)
        one('actual-P_PIDFD-cleanup-after-wait4-error',failed_wait4)
        one('actual-already-reaped-error-retains-unknown-status',lambda row:failed_wait4(row,True))
        def stable(row):
            assert len(starts)==8
            assert live_prefix.path_digest(z3)==SOLVER
            row['every_reaper_released_after_actual_capture']=True
            row['final_solver_sha256']=SOLVER
        one('all-starts-follow-unreaped-image-capture-final-identity',stable)
    save()
    return 0 if len(rows)==10 and all(r['passed'] for r in rows) else 1


if __name__=='__main__':raise SystemExit(main())
