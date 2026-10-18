#!/usr/bin/env python3
"""Actual owned CLI/group cancellation control; all verifier/runtime calls are doubles."""
import hashlib
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
 Path(cfg['server']).write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'socket':str(path)}))
 while True:time.sleep(.05)
elif sys.argv[1]=='verify':
 signal.signal(signal.SIGTERM,interrupt)
 nested=subprocess.Popen([sys.executable,'-c','import time; time.sleep(120)'])
 try:
  Path(cfg['ready']).write_text(json.dumps({'pid':os.getpid(),'pgid':os.getpgrp(),'nested':nested.pid,
   'nested_pgid':os.getpgid(nested.pid),'owned':str(Path(os.environ['CARGO_HOME']).parent),
   'socket':os.environ['SAIL_DAEMON_SOCKET']}))
  while True:time.sleep(.05)
 finally:
  if nested.poll() is None:nested.terminate()
  nested.wait(timeout=5)
else:raise SystemExit(3)
'''


def case(root, helper):
    root.mkdir(mode=0o700)
    stage=root/'stage';(stage/'bin').mkdir(parents=True)
    runtime=stage/'engine/variant';runtime.mkdir(parents=True)
    manifest=runtime/'sail-build.json';manifest.write_text('{}')
    controlled=root/'controlled-bin';controlled.mkdir()
    cargo=controlled/'cargo';cargo.write_text('#!/bin/sh\nexit 0\n');cargo.chmod(0o755)
    for name in ('sail','sail-z3'):
        path=stage/'bin'/name;path.write_text(FAKE);path.chmod(0o755)
    ready=root/'verify-ready.json';server_path=root/'server-ready.json'
    (stage/'bin/fixture-config.json').write_text(json.dumps({'runtime':str(runtime),
        'runtime_hash':digest(manifest),'ready':str(ready),'server':str(server_path)}))
    before=inventory(stage)
    env=dict(os.environ,PATH=str(controlled)+os.pathsep+os.environ['PATH'])
    child=None;record=None;server=None;owned=None
    result={'helper_sha256':digest(helper),'passed':False}
    try:
        with (root/'parent.stdout').open('xb') as out,(root/'parent.stderr').open('xb') as err:
            child=subprocess.Popen([sys.executable,'-B',str(helper),'record',str(stage),'--evidence',str(root/'evidence')],
                                   env=env,stdout=out,stderr=err,start_new_session=True)
            wait_until(lambda:ready.exists() or child.poll() is not None)
            assert ready.exists() and child.poll() is None, 'did not enter actual detached verification command'
            record=json.loads(ready.read_text());server=json.loads(server_path.read_text());owned=Path(record['owned'])
            assert owned.resolve()==owned and owned.parent==Path('/tmp').resolve() and owned.name.startswith('sail-release-record-')
            assert owned.stat().st_uid==os.getuid()
            scratch=json.loads((root/'evidence/scratch-root.json').read_text())
            assert scratch=={'pid':child.pid,'path':str(owned),'uid':os.getuid()}
            assert record['pgid']==record['pid'] and record['nested_pgid']==record['pgid']
            assert server['pgid']==server['pid'] and server['pgid'] not in (os.getpgrp(),child.pid,record['pgid'])
            assert all(alive(pid) for pid in (record['pid'],record['nested'],server['pid']))
            assert Path(record['socket']).is_socket() and server['socket']==record['socket']
            # The only cancellation sent initially is to the actual helper parent.
            os.kill(child.pid,signal.SIGTERM)
            child.wait(timeout=25)
            assert child.returncode != 0, 'cancelled helper reported success'
            wait_until(lambda:not any(alive(pid) for pid in (record['pid'],record['nested'],server['pid'])))
            assert not owned.exists() and not Path(record['socket']).exists()
            report=json.loads((root/'evidence/result.json').read_text())
            assert report['passed'] is False and report['server_reaped'] and report['original_stage_unchanged']
            assert not (stage/'deps').exists() and not (stage/'release-files.json').exists()
            assert inventory(stage)==before
            result.update(passed=True,parent_returncode=child.returncode,ready=record,server=server,
                          original_stage_unchanged=True,observed='cleanup')
    finally:
        # Validate only the identities recorded by this case, and reap the nested
        # child through its mock verifier before deleting its exact owned scratch.
        if record is not None and alive(record['pid']):
            assert os.getpgid(record['pid'])==record['pgid']
            os.killpg(record['pgid'],signal.SIGTERM)
        if server is not None and alive(server['pid']):
            assert os.getpgid(server['pid'])==server['pgid']
            os.killpg(server['pgid'],signal.SIGTERM)
        if child is not None and child.poll() is None:
            child.terminate();child.wait(timeout=25)
        if record is not None:
            wait_until(lambda:not any(alive(pid) for pid in (record['pid'],record['nested'],server['pid'])))
        if owned is not None and owned.exists():
            assert owned.parent==Path('/tmp').resolve() and owned.name.startswith('sail-release-record-')
            shutil.rmtree(owned)
        result['all_owned_processes_gone']=record is None or not any(alive(pid) for pid in (record['pid'],record['nested'],server['pid']))
        result['owned_scratch_gone']=owned is None or not owned.exists()
        (root/'result.json').write_text(json.dumps(result,indent=2)+'\n')
    return result


def main():
    with tempfile.TemporaryDirectory(prefix='sail-release-cancel-',dir='/tmp') as temporary:
        root=Path(temporary).resolve()
        result=case(root/'control',Path(__file__).with_name('release-shipped.py').resolve())
        assert result['passed'] and result['all_owned_processes_gone'] and result['owned_scratch_gone']
    print('release helper parent-SIGTERM/nested-child/server/socket cleanup PASS (compiler/runtime doubled)')


if __name__=='__main__':main()
