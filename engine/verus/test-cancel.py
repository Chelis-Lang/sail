#!/usr/bin/env python3
"""Local-only cancellation seal oracle; uses an already built engine."""
import argparse
import hashlib
import json
import os
import pathlib
import signal
import subprocess
import sys
import time
from importlib.machinery import SourceFileLoader
oracle = SourceFileLoader('oracle', str(pathlib.Path(__file__).with_name('test-engine.py'))).load_module()


def capture(root, name, command, env):
    record = {'command': command, 'timeout_seconds': 120}
    (root / (name + '.command.json')).write_text(json.dumps(record, indent=2) + '\n')
    try:
        result = subprocess.run(command, capture_output=True, env=env, timeout=120)
    except subprocess.TimeoutExpired as error:
        (root / (name + '.stdout')).write_bytes(error.stdout or b'')
        (root / (name + '.stderr')).write_bytes(error.stderr or b'')
        record.update(timed_out=True, returncode=None)
        (root / (name + '.command.json')).write_text(json.dumps(record, indent=2) + '\n')
        raise
    (root / (name + '.stdout')).write_bytes(result.stdout)
    (root / (name + '.stderr')).write_bytes(result.stderr)
    record.update(timed_out=False, returncode=result.returncode)
    (root / (name + '.command.json')).write_text(json.dumps(record, indent=2) + '\n')
    return result


def run(args):
    root = pathlib.Path(args.out).absolute()
    root.mkdir(parents=True, exist_ok=True)
    source = root / 'lib.rs'
    source.write_text('use verus_builtin::*; use verus_builtin_macros::*; verus! { proof fn selected(x:int) ensures x == x {} } fn main() {}\n')
    marker, events = root / 'blocked', root / 'events.jsonl'
    marker.unlink(missing_ok=True)
    events.unlink(missing_ok=True)
    wrapper = root / 'blocking-z3'
    wrapper.write_text('#!' + sys.executable + '\n' + '''import os,sys,subprocess,threading,time
child=subprocess.Popen([os.environ['SAIL_TEST_Z3'],*sys.argv[1:]],stdin=subprocess.PIPE,stdout=subprocess.PIPE)
def output():
    while True:
        data=os.read(child.stdout.fileno(),65536)
        if not data:return
        os.write(sys.stdout.fileno(),data)
threading.Thread(target=output,daemon=True).start()
for line in sys.stdin.buffer:
    if b'(check-sat' in line:
        open(os.environ['SAIL_TEST_BLOCKED'],'wb').close()
        while True:time.sleep(1)
    child.stdin.write(line);child.stdin.flush()
child.stdin.close();sys.exit(child.wait())
''')
    wrapper.chmod(0o755)
    env = {key:value for key,value in os.environ.items() if not key.startswith('SAIL_ENGINE')}
    env.update(VERUS_Z3_PATH=str(wrapper), SAIL_TEST_Z3=args.z3, SAIL_TEST_BLOCKED=str(marker),
               SAIL_ENGINE='interactive', SAIL_ENGINE_FUNCTION='demo::selected',
               SAIL_ENGINE_CONTEXT='local-cancel-v1:' + oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),
               SAIL_ENGINE_EVENTS=str(events),
               SAIL_ENGINE_FOCUS_REPORT=str(root/'focus.json'))
    command = [args.engine,str(source),'--no-vstd','--crate-name','demo','-V','use-crate-name','--output-json','--time','--no-report-long-running']
    cancel_command = {'command': command, 'blocked_deadline_seconds': 30, 'kill_wait_seconds': 10}
    (root/'cancel.command.json').write_text(json.dumps(cancel_command,indent=2)+'\n')
    with (root/'cancel.stdout').open('wb') as output, (root/'cancel.stderr').open('wb') as error:
        child = subprocess.Popen(command,env=env,stdout=output,stderr=error,start_new_session=True)
        try:
            deadline = time.monotonic()+30
            while not marker.exists() and child.poll() is None and time.monotonic()<deadline:
                time.sleep(0.02)
            assert marker.exists(), 'verifier never reached blocked solver check'
            os.killpg(child.pid,signal.SIGKILL)
            child.wait(timeout=10)
        finally:
            if child.poll() is None:
                os.killpg(child.pid,signal.SIGKILL)
                child.wait(timeout=10)
            cancel_command['returncode'] = child.returncode
            (root/'cancel.command.json').write_text(json.dumps(cancel_command,indent=2)+'\n')
    frames = [json.loads(line) for line in events.read_text().splitlines()]
    assert not any(event['event']=='compiler-sealed' for event in frames), 'cancelled compiler produced authoritative seal'
    assert not (root/'focus.json').exists(), 'unfinished focus reported complete'
    # Retained lowerings may be reused after cancellation, but always re-prove.
    env['VERUS_Z3_PATH']=args.z3
    result=capture(root,'restart',command,env)
    stock=capture(root,'stock',[args.stock,*command[1:]],{key:value for key,value in env.items() if not key.startswith('SAIL_ENGINE')})
    assert result.returncode==stock.returncode and result.stderr==stock.stderr, 'restart verdict or diagnostics differ'
    assert json.loads(result.stdout)['verification-results']==json.loads(stock.stdout)['verification-results'], 'restart skipped proof'
    frames = [json.loads(line) for line in events.read_text().splitlines()]
    assert frames[-1]['event']=='compiler-sealed', 'restart missing final seal'
    assert frames[-1]['payload']['normal_termination'] is True, 'restart missing normal completion'
    (root/'summary.json').write_text(json.dumps({'version':1,'engine_verifier_sha256':oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),'cancel_sealed':False,'restart_exit_code':result.returncode},indent=2)+'\n')
    print('cancel no seal; restart matches stock',flush=True)

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['engine','stock','z3','out']:
        parser.add_argument('--'+name,required=True)
    run(parser.parse_args())
