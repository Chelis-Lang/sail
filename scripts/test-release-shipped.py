#!/usr/bin/env python3
"""Small owned release-member/report controls; no verifier or native acceptance."""
import copy
import importlib.util
import io
import json
import os
import shutil
from pathlib import Path
import tempfile
import tarfile

spec = importlib.util.spec_from_file_location('release_shipped', Path(__file__).with_name('release-shipped.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


def refuses(call):
    try:call()
    except (AssertionError, KeyError, FileNotFoundError, ValueError):return
    raise AssertionError('expected refusal')


def main():
    os.umask(0o022)
    with tempfile.TemporaryDirectory(prefix='sail-release-members-', dir='/tmp') as temporary:
        root = Path(temporary).resolve()
        release = root/'sail-fixture';bundle = release/'deps/variant'
        (release/'bin').mkdir(parents=True)
        for name in ('sail', 'sail-z3'):
            path = release/'bin'/name;path.write_bytes(b'fixture executable');path.chmod(0o755)
        runtime = release/'engine/variant';runtime.mkdir(parents=True)
        (runtime/'sail-build.json').write_text('{}')
        key = 'a'*64;entry = bundle/'units'/key
        (entry/'files').mkdir(parents=True)
        (bundle/'Cargo.lock').write_text('fixture lock')
        phase = {'sources': {'fixture': {'checksum': 'fixture', 'size': 1}},
                 'extra_tracked_files': [], 'tracked_environment': {}}
        members = []
        for name, data in [('libvstd.rmeta', b'owned metadata'), ('libvstd.vir', b'owned VIR'), ('vstd.d', b'owned depinfo')]:
            path = entry/'files'/name;path.write_bytes(data)
            members.append({'name': name, 'sha256': module.digest(path), 'size': len(data), 'executable': False})
        streams = {}
        for name in ('stdout', 'stderr'):
            path = entry/name;path.write_text('fixture '+name)
            streams[name] = {'name': name, 'sha256': module.digest(path), 'size': path.stat().st_size, 'executable': False}
        unit = dict(schema=12, key=key, crate_name='vstd', package='fixture', verifying=True,
                    source_tree={'source':'hash'}, sources=[['source','hash']], establishing_run='run',
                    compiler_reads={'complete':True,'phases':{'verification':phase,'compilation':phase}},
                    files=members, **streams)
        (entry/'unit.json').write_text(json.dumps(unit))
        (bundle/'manifest.json').write_text(json.dumps({'schema':12,'kind':'sail-vstd-compile-boundary',
                'runtime':'variant','identity':'b'*64,'virtual_base':'/rustc-dev/fixture','units':{key:'vstd fixture'}}))
        proof = module.check_bundle(bundle, 'variant', module.digest(runtime/'sail-build.json'), 'run')
        index = {'schema':1,'proof':proof,'files':module.inventory(release),'directories':module.directories(release)}
        (release/module.INDEX).write_text(json.dumps(index));module.verify(release)
        # Every member including a stored proof body is bound, not only the runtime.
        path = entry/'files/libvstd.vir';old = path.read_bytes();path.write_bytes(b'changed VIR')
        refuses(lambda:module.verify(release));path.write_bytes(old)
        extra = entry/'extra';extra.write_bytes(b'extra');refuses(lambda:module.verify(release));extra.unlink()
        extra.mkdir();refuses(lambda:module.verify(release));extra.rmdir()
        path.chmod(0o755);refuses(lambda:module.verify(release));path.chmod(0o644)
        body = entry/'files/libvstd.rmeta';held = body.read_bytes();body.unlink();body.symlink_to(path)
        refuses(lambda:module.verify(release));body.unlink();body.write_bytes(held)
        module.verify(release)
        packed = root/'fixture.tgz'
        with tarfile.open(packed,'w:gz') as archive:archive.add(release,arcname=release.name)
        unpacked = root/'unpacked';unpacked.mkdir();module.unpack(packed,unpacked)
        for name, kind in [('sail-fixture/../escape','file'),('sail-fixture/alias','link')]:
            attack = root/('attack-'+kind+'.tgz')
            with tarfile.open(attack,'w:gz') as archive:
                member = tarfile.TarInfo(name)
                if kind=='link':member.type=tarfile.SYMTYPE;member.linkname='/tmp/foreign'
                else:member.size=1
                archive.addfile(member,io.BytesIO(b'x') if kind=='file' else None)
            empty = root/('empty-'+kind);empty.mkdir();refuses(lambda:module.unpack(attack,empty))
        # A complete Current source report, not a self-declared manifest flag,
        # is required before the producer makes its deps directory discoverable.
        function = {'verdict':'verified','establishment':'confirmed','unstable':False,'verus_success':True,'provenance':{}}
        report = {'schema_version':2,'mode':'default','exit_code':0,'error':None,'diagnostics':[],
                  'summary':{'functions':1,'verified':1,'failed':0,'unknown':0,'provisional':0,'unstable':0},
                  'crates':[{'functions':[function],'verus':{'success':True,'errors':0,'encountered_vir_error':False}}]}
        target = {'complete':True,'reused':False,'establishing_run':'run','report':report}
        doc = {'kind':'sail-project-result','schema_version':2,'currentness':'current','exit_code':0,'error':None,
               'run_id':'run','execution':{'verifier_invocations':2,'reused_targets':0,'wall_ms':1},
               'targets':[dict(target,target='vstd',requested=False,package_id='vstd',kind=['lib']),dict(target,target='sail_shipped_dependencies',requested=True,package_id='requested',kind=['lib'])]}
        assert module.complete_report(doc)=='run'
        for field,value in [('currentness','historical'),('error',{'code':'fixture'}),('exit_code',1)]:
            wrong=copy.deepcopy(doc);wrong[field]=value;refuses(lambda:module.complete_report(wrong))
        wrong=copy.deepcopy(doc);wrong['targets'][0]['establishing_run']='foreign';refuses(lambda:module.complete_report(wrong))
        wrong=copy.deepcopy(doc);wrong['targets'][0]['complete']=False;refuses(lambda:module.complete_report(wrong))
        wrong=copy.deepcopy(doc);wrong['targets'][0]['report']['diagnostics']=[{'severity':'warning'}];refuses(lambda:module.complete_report(wrong))
        template = root/'proof-template'
        shutil.copytree(bundle, template)
        controlled = root/'controlled-bin';controlled.mkdir()
        cargo = controlled/'cargo';cargo.write_text('#!/bin/sh\nexit 0\n');cargo.chmod(0o755)
        executable = r'''#!/usr/bin/env python3
import json,os,shutil,signal,socket,sys,time
from pathlib import Path
cfg=json.loads(Path(__file__).with_name('fixture-config.json').read_text())
if sys.argv[1:]==['engine','status','--output','json']:
 print(json.dumps({'version':2,'build':cfg['runtime'],'manifest_sha256':cfg['runtime_hash']}))
elif sys.argv[1]=='serve':
 path=Path(sys.argv[sys.argv.index('--socket')+1]);sock=socket.socket(socket.AF_UNIX);sock.bind(str(path));os.chmod(path,0o600)
 Path(cfg['pid_record']).write_text(str(os.getpid()))
 while True:time.sleep(.05)
elif sys.argv[1]=='verify':
 shutil.copytree(cfg['template'],os.environ['SAIL_SHIPPED_RECORD'])
 value=cfg['report'];value['currentness']=cfg['currentness'];print(json.dumps(value))
else:raise SystemExit(3)
'''
        path_before=os.environ['PATH']
        os.environ['PATH']=str(controlled)+os.pathsep+path_before
        try:
            for case in ('current','historical','capture-overflow'):
                stage=root/('producer-'+case)
                (stage/'bin').mkdir(parents=True)
                shutil.copytree(runtime,stage/'engine/variant')
                for name in ('sail','sail-z3'):
                    path=stage/'bin'/name;path.write_text(executable);path.chmod(0o755)
                pid_record=root/(case+'-server-pid')
                (stage/'bin/fixture-config.json').write_text(json.dumps({'runtime':str(stage/'engine/variant'),
                    'runtime_hash':module.digest(stage/'engine/variant/sail-build.json'),
                    'template':str(template),'report':doc,'currentness':'historical' if case=='historical' else 'current',
                    'pid_record':str(pid_record)}))
                evidence=root/(case+'-evidence')
                previous_limit=module.CAPTURE_LIMIT
                try:
                    if case=='capture-overflow':module.CAPTURE_LIMIT=32
                    if case=='current':module.record(stage,evidence)
                    else:refuses(lambda:module.record(stage,evidence))
                finally:module.CAPTURE_LIMIT=previous_limit
                result=json.loads((evidence/'result.json').read_text())
                assert result['server_reaped'] and result['original_stage_unchanged']
                assert result['passed']==(case=='current')
                assert (stage/'deps').exists()==(case=='current')
                if pid_record.exists():
                    try:os.kill(int(pid_record.read_text()),0)
                    except ProcessLookupError:pass
                    else:raise AssertionError('owned producer server survived')
        finally:os.environ['PATH']=path_before
    print('release member/archive/source-report + actual owned producer-entry controls PASS (compiler/runtime doubled)')


if __name__=='__main__':main()
