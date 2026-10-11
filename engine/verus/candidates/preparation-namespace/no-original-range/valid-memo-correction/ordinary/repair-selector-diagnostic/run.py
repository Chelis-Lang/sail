#!/usr/bin/env python3
"""One retained success-prefix/missing-selector diagnostic, never suite acceptance."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import time

FIXTURE_SHA = '9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7'
OBSERVER_SHA = 'c7bf44ba1c204f8730e4aa628f9be9bf2bab07fd3e95e48357fad0c16e5ee8cd'
PRODUCT_SHA = '3cc0fef6511f1e52d729c59514c10eec0e80e3d561221d306cee51f0c0894832'
SHIM_SHA = '65a9f47c331de4f6baf1d741b27ab87fb303e88f3123111b3977e857b5837c15'
MANIFEST_SHA = '54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d'
SOURCES = '426d8b01e7ffb910a36c15e1f870ff985bb61eef'


def sha(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024*1024), b''):
            h.update(block)
    return h.hexdigest()


def load(name, path, expected):
    assert sha(path) == expected, 'Frozen helper changed'
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def write(path, document):
    path.write_text(json.dumps(document, indent=2) + '\n')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('repo', 'grant', 'source', 'out'):
        p.add_argument('--'+name, type=Path, required=True)
    a = p.parse_args()
    fixture = load('unchanged_repair_fixture', a.repo/'ci/test-repair.py', FIXTURE_SHA)
    observer = load('unchanged_managed_observer', a.repo/'ci/managed-evidence.py', OBSERVER_SHA)
    grant = json.loads(a.grant.read_text())
    assert grant['source_commit'] == '071b778d9be2a708ff10ff632effb5c53266ae80'
    sail = Path(grant['product_dir'])/'sail'
    engine = Path(grant['engine_build'])
    def seals():
        assert sha(sail) == PRODUCT_SHA and sha(sail.with_name('sail-z3')) == SHIM_SHA
        assert sha(engine/'sail-build.json') == MANIFEST_SHA
        m = json.loads((engine/'sail-build.json').read_text())
        assert len(m['artifacts']) == 13
        assert all(sha(engine/name) == digest for name,digest in m['artifacts'].items())
        return {'manifest_sha256':MANIFEST_SHA,'runtime13_sha256':m['artifacts'],'product_sha256':PRODUCT_SHA,'shim_sha256':SHIM_SHA}
    before = seals()
    head = subprocess.check_output(['git','-C',str(a.source),'rev-parse','HEAD'],text=True).strip()
    assert head == SOURCES
    assert os.getpriority(os.PRIO_PROCESS,0) == 0
    a.out.mkdir(mode=0o700,exist_ok=False)
    project=a.out/'project';(project/'src').mkdir(parents=True)
    builtin=a.source/'source/builtin';macros=a.source/'source/builtin_macros'
    (project/'Cargo.toml').write_text('[package]\nname="repair_demo"\nversion="0.1.0"\nedition="2021"\n'
        '[package.metadata.verus]\nverify=true\nno-vstd=true\n[dependencies]\n'
        f'verus_builtin={{path={json.dumps(str(builtin))}}}\n'
        f'verus_builtin_macros={{path={json.dumps(str(macros))}}}\n')
    (project/'src/lib.rs').write_text('use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! {\n'+fixture.GOOD+'\n}\n')
    environment=dict(os.environ,SAIL_CACHE_DIR=str(a.out/'cache'),SAIL_DAEMON_SOCKET=str(a.out/'d.sock'))
    environment.pop('VSTD_KIND',None)
    assert not any(name.startswith('SAIL_ENGINE_') or name == 'SAIL_MANAGED_FORCE' for name in environment)
    def setup(label,command,timeout):
        value=fixture.execute(command,project,environment,timeout)
        (a.out/(label+'.stdout')).write_bytes(value.stdout);(a.out/(label+'.stderr')).write_bytes(value.stderr)
        value.check_returncode();return value
    install=setup('engine-install',[str(sail),'engine','install','--from',str(engine),'--output','json'],30)
    status=json.loads(install.stdout);assert status['commit']==head
    environment['RUSTUP_TOOLCHAIN']=status['rust_toolchain']
    setup('cargo-fetch',['cargo','+'+status['rust_toolchain'],'fetch'],60)
    errors=(a.out/'daemon.stderr').open('wb')
    daemon=subprocess.Popen([str(sail),'serve','--cache',environment['SAIL_CACHE_DIR'],'--socket',environment['SAIL_DAEMON_SOCKET'],'--idle-timeout','0'],env=environment,stdout=subprocess.DEVNULL,stderr=errors,start_new_session=True)
    began=time.monotonic();deadline=began+120
    capture=observer.Capture();birth=observer.process_stat(capture.read(Path('/proc')/str(daemon.pid)/'stat'))
    if birth is None:
        fixture.stop_owned(daemon,5);errors.close()
        raise RuntimeError('Owned daemon birth unavailable')
    result={'version':1,'scope':'Reduced success-prefix then missing selector only; no whole-suite acceptance/retry credit','status':'running','fixture_sha256':FIXTURE_SHA,'observer_sha256':OBSERVER_SHA,'grant_sha256':sha(a.grant),'before':before,'project_source_sha256':sha(project/'src/lib.rs'),'proof_tool_budget_s':120,'cli_limit_s':30,'ready_limit_s':5,'started_unix_ns':time.time_ns(),'daemon_pid':daemon.pid,'daemon_start_ticks':birth['start_ticks'],'controller_pid':os.getpid(),'nice':0,'operations':[],'observations':[]}
    seen=set()
    def observe(label):
        if label in seen or len(seen)>=6:return
        seen.add(label);capture=observer.Capture()
        processes=observer.owned_processes(daemon.pid,capture,expected_start_ticks=birth['start_ticks'])
        result['observations'].append({'label':label,'elapsed_s':time.monotonic()-began,'processes':processes,'limited':capture.limited,'errors':capture.errors,'bytes_read':observer.MAX_BYTES-capture.remaining})
        write(a.out/'progress.json',result)
    def remaining(maximum=30):
        value=min(maximum,deadline-time.monotonic());assert value>0,'Reduced diagnostic exceeded original120 seconds';return value
    def cli(label,args,expected=0):
        started=time.monotonic()
        try:value=fixture.execute([str(sail),*args],project,environment,remaining())
        except subprocess.TimeoutExpired as error:
            (a.out/(label+'.stdout.json')).write_bytes(error.stdout or b'');(a.out/(label+'.stderr')).write_bytes(error.stderr or b'')
            result['operations'].append({'operation':label,'elapsed_s':time.monotonic()-started,'timed_out':True});raise
        (a.out/(label+'.stdout.json')).write_bytes(value.stdout);(a.out/(label+'.stderr')).write_bytes(value.stderr)
        result['operations'].append({'operation':label,'elapsed_s':time.monotonic()-started,'exit_code':value.returncode})
        write(a.out/'progress.json',result);assert value.returncode==expected,(label,value.returncode,value.stderr)
        return json.loads(value.stdout)
    def submit(label,function='repair_demo::selected'):
        job=cli(label+'-submit',['repair-check','--engine','interactive','--package','repair_demo','--lib','--function',function,'--background','--output','json'])['id']
        if label=='missing':result['missing_job']=job;result['missing_started_s']=time.monotonic()-began;observe('missing-submitted')
        attempt=0
        while True:
            value=cli(f'{label}-result-{attempt}',['jobs','result',job,'--output','json'])['value']
            result['latest_job']=value['job']
            if label=='missing':
                elapsed=time.monotonic()-began-result['missing_started_s']
                if elapsed>=15:observe('missing-15-seconds')
                if elapsed>=30:observe('missing-30-seconds')
                if deadline-time.monotonic()<=1:observe('before-deadline')
            if value['job']['state']=='completed':
                assert value.get('repair') is not None and value.get('result') is not None,value
                return job,value
            assert value['job']['state'] in ('queued','running'),value
            remaining();attempt+=1;time.sleep(.25)
    def full(value):
        doc=value['repair'];full=doc['batch']['full'];targets=[t for t in full['targets'] if t['requested']]
        assert len(targets)==1 and targets[0]['target']=='repair_demo' and targets[0]['kind']==['lib']
        assert value['result']==full
        return doc,full,targets[0]
    try:
        while not Path(environment['SAIL_DAEMON_SOCKET']).exists():
            assert daemon.poll() is None;remaining(5);time.sleep(.02)
        observe('daemon-ready')
        good_job,good=submit('success');doc,complete,target=full(good)
        if 'Build-script input declarations were established during this run' in complete['notes']:
            assert complete['exit_code']==0 and target['complete']
            if complete['currentness']!='current':assert not doc['accepted']
            good_job,good=submit('success-established');doc,complete,target=full(good)
        assert complete['currentness']=='current' and doc['accepted'] and complete['exit_code']==0 and target['complete']
        if doc['batch']['compiler_invocations']!=1:
            environment['SAIL_MANAGED_FORCE']='1'
            try:good_job,good=submit('success-single-driver')
            finally:environment.pop('SAIL_MANAGED_FORCE',None)
            doc,complete,target=full(good)
            assert complete['currentness']=='current' and doc['accepted'] and complete['exit_code']==0 and target['complete']
        assert doc['batch']['batched'] and doc['batch']['compiler_invocations']==1
        page=cli('success-events',['jobs','events',good_job,'--output','json']);cursor=page['next']
        cli('success-resume',['jobs','events',good_job,'--stream',cursor['stream_id'],'--after',str(cursor['after']),'--output','json'])
        cli('baseline-save',['baseline','save','before-repair','--job',good_job,'--output','json'])
        missing_job,missing=submit('missing','repair_demo::missing');doc,complete,target=full(missing)
        assert not doc['accepted'] and doc['batch']['selection_error']['code']=='SAIL3122'
        assert complete['exit_code']==0 and target['complete']
        result['status']='completed-correctly';result['missing_full_currentness']=complete['currentness']
    except BaseException as error:
        result.update(status='FAIL',error_type=type(error).__name__,error=str(error));observe('failure-before-shutdown');raise
    finally:
        result['primary_elapsed_s']=time.monotonic()-began;write(a.out/'primary.json',result)
        try:fixture.stop_owned(daemon,5)
        except BaseException as error:result['cleanup_error']=str(error)
        errors.close();result['daemon_exit_code']=daemon.returncode
        jobs=a.out/'cache/projects/jobs'
        if jobs.is_dir():
            for job in jobs.iterdir():
                if job.is_dir():
                    for name in ('stdout','stderr','events.jsonl','proof-source.jsonl'):
                        source=job/name
                        if source.is_file():(a.out/(job.name+'.worker.'+name)).write_bytes(source.read_bytes())
                    for source in job.glob('events.setup-*.jsonl'):
                        if source.is_file():(a.out/(job.name+'.'+source.name)).write_bytes(source.read_bytes())
        result['after']=seals();write(a.out/'result.json',result)
        print(json.dumps({'status':result['status'],'elapsed_s':result['primary_elapsed_s'],'missing_job':result.get('missing_job')}),flush=True)

if __name__=='__main__':main()
