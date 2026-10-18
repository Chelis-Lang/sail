#!/usr/bin/env python3
"""Actual owned CLI/group cancellation control; all verifier/runtime calls are doubles."""
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time
import tempfile


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wait_until(predicate, seconds=10):
    end = time.monotonic()+seconds
    while not predicate():
        assert time.monotonic()<end, 'owned control deadline'
        time.sleep(.05)


def alive(pid):
    try:os.kill(pid,0)
    except ProcessLookupError:return False
    return True


def inventory(root):
    return {str(p.relative_to(root)):{'sha256':digest(p),'mode':p.stat().st_mode & 0o777}
            for p in root.rglob('*') if p.is_file()}


FAKE = r'''#!/usr/bin/env python3
import json,os,signal,socket,subprocess,sys,time
from pathlib import Path
cfg=json.loads(Path(__file__).with_name('fixture-config.json').read_text())
def interrupt(_signum,_frame):raise KeyboardInterrupt('owned mock terminated')
if sys.argv[1:]==['engine','status','--output','json']:
 print(json.dumps({'version':2,'build':cfg['runtime'],'manifest_sha256':cfg['runtime_hash']}))
elif sys.argv[1]=='serve':
 path=Path(sys.argv[sys.argv.index('--socket')+1]);sock=socket.socket(socket.AF_UNIX);sock.bind(str(path));os.chmod(path,0o600)
 nested=None
 if cfg['scenario']=='server-descendant':
  nested=subprocess.Popen([sys.executable,'-c','import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print("ready",flush=True); time.sleep(120)'],stdout=subprocess.PIPE)
  assert nested.stdout.readline()==b'ready\n'
 Path(cfg['server']).write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'socket':str(path),
  'nested':nested.pid if nested else None}))
 while True:time.sleep(.05)
elif sys.argv[1]=='verify':
 signal.signal(signal.SIGTERM,interrupt)
 code='import time; time.sleep(120)'
 if cfg['scenario']=='command-crash':
  code='import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); print("ready",flush=True); time.sleep(120)'
 nested=subprocess.Popen([sys.executable,'-c',code],stdout=subprocess.PIPE)
 if cfg['scenario']=='command-crash':assert nested.stdout.readline()==b'ready\n'
 try:
  Path(cfg['ready']).write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'nested':nested.pid,
   'nested_pgid':os.getpgid(nested.pid),'owned':str(Path(os.environ['CARGO_HOME']).parent),
   'socket':os.environ['SAIL_DAEMON_SOCKET']}))
  if cfg['scenario']=='command-crash':os._exit(7)
  while True:time.sleep(.05)
 finally:
  if nested.poll() is None:nested.terminate()
  nested.wait(timeout=5)
else:raise SystemExit(3)
'''


def case(root, helper, scenario):
    root.mkdir(mode=0o700)
    stage=root/'stage';(stage/'bin').mkdir(parents=True)
    runtime=stage/'engine/variant';runtime.mkdir(parents=True)
    manifest=runtime/'sail-build.json';manifest.write_text(json.dumps({'rust_toolchain':'1.98.1'}))
    controlled=root/'controlled-bin';controlled.mkdir()
    cargo=controlled/'cargo';cargo.write_text('#!/bin/sh\nexit 0\n');cargo.chmod(0o755)
    sysroot = root/'fixture-sysroot';rustlib = sysroot/'lib/rustlib'
    source = rustlib/'src/rust/library/lib.rs';source.parent.mkdir(parents=True);source.write_text('owned rust source')
    (rustlib/'components').write_text('rust-src\n')
    (rustlib/'manifest-rust-src').write_text('file:lib/rustlib/src/rust/library/lib.rs\n')
    rustup = controlled/'rustup'
    rustup.write_text('#!/bin/sh\n[ "$*" = "run 1.98.1 rustc --print sysroot" ] || exit 9\nprintf "%s\\n" '+str(sysroot)+'\n')
    rustup.chmod(0o755)
    for name in ('sail','sail-z3'):
        path=stage/'bin'/name;path.write_text(FAKE);path.chmod(0o755)
    ready=root/'verify-ready.json';server_path=root/'server-ready.json'
    (stage/'bin/fixture-config.json').write_text(json.dumps({'runtime':str(runtime),
        'runtime_hash':digest(manifest),'ready':str(ready),'server':str(server_path),'scenario':scenario}))
    before=inventory(stage)
    env=dict(os.environ,PATH=str(controlled)+os.pathsep+os.environ['PATH'])
    child=None;record=None;server=None;owned=None
    result={'helper_sha256':digest(helper),'passed':False}
    try:
        with (root/'parent.stdout').open('xb') as out,(root/'parent.stderr').open('xb') as err:
            child=subprocess.Popen([sys.executable,'-B',str(helper),'record',str(stage),'--evidence',str(root/'evidence')],
                                   env=env,stdout=out,stderr=err,start_new_session=True)
            wait_until(lambda:ready.exists() or child.poll() is not None)
            assert ready.exists(), 'did not enter actual detached verification command'
            record=json.loads(ready.read_text());server=json.loads(server_path.read_text());owned=Path(record['owned'])
            assert owned.resolve()==owned and owned.parent==Path('/tmp').resolve() and owned.name.startswith('sail-release-record-')
            assert owned.stat().st_uid==os.getuid()
            scratch=json.loads((root/'evidence/scratch-root.json').read_text())
            assert scratch=={'pid':child.pid,'path':str(owned),'uid':os.getuid()}
            assert record['pgid']==record['pid'] and record['nested_pgid']==record['pgid']
            assert server['pgid']==server['pid'] and server['pgid'] not in (os.getpgrp(),child.pid,record['pgid'])
            if scenario!='command-crash':
                assert all(alive(pid) for pid in (record['pid'],record['nested'],server['pid']))
            assert Path(record['socket']).is_socket() and server['socket']==record['socket']
            # The only cancellation sent initially is to the actual helper parent.
            if scenario!='command-crash':os.kill(child.pid,signal.SIGTERM)
            child.wait(timeout=35)
            assert child.returncode != 0, 'failed/cancelled helper reported success'
            wait_until(lambda:not any(alive(pid) for pid in (record['pid'],record['nested'],server['pid'])))
            assert not owned.exists() and not Path(record['socket']).exists()
            report=json.loads((root/'evidence/result.json').read_text())
            assert report['passed'] is False and report['server_reaped'] and report['original_stage_unchanged']
            assert report['error'] and all(group['extinct'] for group in report['process_groups']), report
            if scenario in ('command-crash','server-descendant'):
                label='record' if scenario=='command-crash' else 'server'
                group=next(row for row in report['process_groups'] if row['label']==label)
                assert group['signals']==['SIGTERM','SIGKILL'], group
                assert group['returncode']==(7 if scenario=='command-crash' else -signal.SIGTERM)
            if server['nested'] is not None:wait_until(lambda:not alive(server['nested']))
            assert not (stage/'deps').exists() and not (stage/'release-files.json').exists()
            assert inventory(stage)==before
            result.update(passed=True,parent_returncode=child.returncode,ready=record,server=server,
                          original_stage_unchanged=True,observed='cleanup',process_groups=report['process_groups'])
    finally:
        # Failure-only rescue addresses only exact recorded live fixture members,
        # requiring their owned session/group before deleting exact owned scratch.
        groups=[]
        if record is not None:groups.append((record['pgid'],[record['pid'],record['nested']]))
        if server is not None:groups.append((server['pgid'],[server['pid'],server['nested']]))
        for pgid,pids in groups:
            for pid in pids:
                if pid is None or not alive(pid):continue
                assert pgid!=os.getpgrp() and os.getpgid(pid)==pgid and os.getsid(pid)==pgid
                os.killpg(pgid,signal.SIGKILL)
                break
        if child is not None and child.poll() is None:
            child.terminate();child.wait(timeout=25)
        if record is not None:
            wait_until(lambda:not any(alive(pid) for pid in (record['pid'],record['nested'],server['pid'])))
        if owned is not None and owned.exists():
            assert owned.parent==Path('/tmp').resolve() and owned.name.startswith('sail-release-record-')
            shutil.rmtree(owned)
        result['all_owned_processes_gone']=record is None or not any(
            alive(pid) for pid in (record['pid'],record['nested'],server['pid'],server['nested']) if pid is not None)
        result['owned_scratch_gone']=owned is None or not owned.exists()
        (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def extinction_control(root, helper):
    spec=importlib.util.spec_from_file_location('release_shipped_groups',helper)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    # A zombie-only group needs no signal. Once reaped, even a repeated stop
    # must not probe or signal a potentially recycled group ID.
    child=subprocess.Popen([sys.executable,'-c','raise SystemExit(3)'],start_new_session=True)
    groups=[];group=module.OwnedGroup(child,'extinction',groups)
    try:
        wait_until(group.exited)
        group.stop()
        assert child.returncode==3 and group.state['extinct'] and group.state['signals']==[]
        original=module.os.killpg
        def forbidden(*_args):raise AssertionError('signal after extinction')
        module.os.killpg=forbidden
        try:group.stop()
        finally:module.os.killpg=original
    finally:
        group.stop()
    # A process in our own session must be refused without sending a signal.
    foreign=subprocess.Popen([sys.executable,'-c','import time; time.sleep(120)'])
    try:
        try:module.OwnedGroup(foreign,'not-owned',[]).members()
        except AssertionError:pass
        else:raise AssertionError('foreign session admitted')
        assert foreign.poll() is None
    finally:
        foreign.terminate();foreign.wait(timeout=5)


def adoption_control(helper):
    spec=importlib.util.spec_from_file_location('release_shipped_adoption',helper)
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    original=module.subprocess.Popen
    made=[];groups=[]
    def cancel_after_spawn(*args,**kwargs):
        child=original(*args,**kwargs);made.append(child)
        module.subprocess.Popen=original
        os.kill(os.getpid(),signal.SIGTERM)
        return child
    module.subprocess.Popen=cancel_after_spawn
    try:
        try:
            with module.OwnedGroup.running([sys.executable,'-c','import time; time.sleep(120)'],
                                          'cancel-adoption',groups):
                raise AssertionError('cancelled child reached caller')
        except KeyboardInterrupt as error:assert str(error)=='release child creation cancelled'
        assert len(made)==1 and groups[0]['extinct'] and not alive(made[0].pid)
    finally:module.subprocess.Popen=original


def main():
    with tempfile.TemporaryDirectory(prefix='sail-release-cancel-',dir='/tmp') as temporary:
        root=Path(temporary).resolve()
        helper=Path(__file__).with_name('release-shipped.py').resolve()
        for scenario in ('cancel','command-crash','server-descendant'):
            result=case(root/scenario,helper,scenario)
            assert result['passed'] and result['all_owned_processes_gone'] and result['owned_scratch_gone']
        extinction_control(root,helper)
        adoption_control(helper)
    print('release helper cancellation/crashed-leader/TERM-exiting-leader/ignoring-descendant + extinction/ownership controls PASS (compiler/runtime doubled)')


if __name__=='__main__':main()
