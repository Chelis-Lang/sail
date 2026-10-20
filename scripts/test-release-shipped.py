#!/usr/bin/env python3
"""Small owned release-member/report controls; no verifier or native acceptance."""
import copy
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
import tempfile
import tarfile

spec = importlib.util.spec_from_file_location('release_shipped', Path(__file__).with_name('release-shipped.py'))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
PIN = module.tomllib.loads((module.ROOT/'corpus/toolchain.toml').read_text())['verus']['rust_toolchain']
MINIMAL = 'cargo-x86_64-unknown-linux-gnu\nrust-src\nrust-std-x86_64-unknown-linux-gnu\nrustc-x86_64-unknown-linux-gnu\n'


def refuses(call):
    try:call()
    except (AssertionError, KeyError, FileNotFoundError, ValueError):return
    raise AssertionError('expected refusal')


def main():
    os.umask(0o022)
    assert module.process_census()[os.getpid()][0] == os.getpgrp()
    original_run = module.subprocess.run
    def missing_census(*_args, **_kwargs):
        raise FileNotFoundError('owned missing-ps control')
    module.subprocess.run = missing_census
    try:
        try:module.process_census()
        except RuntimeError as error:assert 'cleanup requires ps' in str(error)
        else:raise AssertionError('missing process census was accepted')
    finally:module.subprocess.run = original_run
    for optimization in ('-O', '-OO'):
        refused = subprocess.run(
            [sys.executable, optimization, '-B', str(Path(__file__).with_name('release-shipped.py')), 'verify', '/unused-stage'],
            capture_output=True, check=False, timeout=10,
        )
        assert refused.returncode != 0
        assert b'required validation needs Python without -O or PYTHONOPTIMIZE' in refused.stderr
    with tempfile.TemporaryDirectory(prefix='sail-release-members-', dir='/tmp') as temporary:
        root = Path(temporary).resolve()
        release = root/'sail-fixture';bundle = release/'deps/variant'
        (release/'bin').mkdir(parents=True)
        for name in ('sail', 'sail-z3'):
            path = release/'bin'/name;path.write_bytes(b'fixture executable');path.chmod(0o755)
        runtime = release/'engine/variant';runtime.mkdir(parents=True)
        (runtime/'sail-build.json').write_text(json.dumps({'rust_toolchain':PIN}))
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
        cargo = controlled/'cargo';cargo.write_text('#!/bin/sh\n[ "$RUSTUP_TOOLCHAIN" = "'+PIN+'" ] || exit 87\nexit 0\n');cargo.chmod(0o755)
        sysroot = root/'fixture-sysroot';rustlib = sysroot/'lib/rustlib'
        source = rustlib/'src/rust/library/lib.rs';source.parent.mkdir(parents=True);source.write_text('owned rust source')
        (rustlib/'components').write_text(MINIMAL)
        (rustlib/'manifest-rust-src').write_text('file:lib/rustlib/src/rust/library/lib.rs\n')
        rustup = controlled/'rustup'
        rustup.write_text('#!/bin/sh\n[ "$*" = "run '+PIN+' rustc --print sysroot" ] || exit 9\nprintf "%s\\n" '+str(sysroot)+'\n')
        rustup.chmod(0o755)
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
 assert os.environ['RUSTUP_TOOLCHAIN']==cfg['toolchain']
 trace=Path(os.environ['SAIL_SHIPPED_TRACE']);assert trace==Path(cfg['evidence'])/'boundary-trace'
 trace.mkdir();(trace/'fixture.json').write_text(json.dumps({'event':'actual-owned-mock-verify-entry'}))
 for directory in ('.sail-boundary-inputs','.sail-boundary-native-reads'):
  if cfg.get('missing_certificate') and directory=='.sail-boundary-inputs':continue
  observed=Path(os.environ['CARGO_HOME']).parent/directory/'fixture';observed.mkdir(parents=True);(observed/'inputs.json').write_text('{}')
 shutil.copytree(cfg['template'],os.environ['SAIL_SHIPPED_RECORD'])
 value=cfg['report'];value['currentness']=cfg['currentness'];print(json.dumps(value))
else:raise SystemExit(3)
'''
        # Certificate-only retention has a complete per-path content-once index.
        certs=root/'certs';certs.mkdir()
        for kind in ('.sail-boundary-inputs','.sail-boundary-native-reads'):
            path=certs/kind/'fixture';path.mkdir(parents=True);(path/'inputs.json').write_text('{}')
        skipped=certs/'skipped';skipped.mkdir();(skipped/'reason').write_text('fixture refusal')
        captured=root/'cert-captured';captured.mkdir();module.retain_compiler_inputs(certs,captured,True)
        captured_index=json.loads((captured/'compiler-inputs/index.json').read_text())
        assert captured_index['complete'] and len(captured_index['paths'])==3 and captured_index['unique_bytes']==17
        assert len(list((captured/'compiler-inputs/bodies').iterdir()))==2
        def retained(failed):
            folder=failed/'compiler-inputs'
            value=json.loads((folder/'index.json').read_text())
            assert not value['complete'] and value['paths']
            for row in value['paths']:
                body=folder/'bodies'/row['sha256']
                assert body.stat().st_size==row['bytes'] and module.digest(body)==row['sha256']
            return value
        missing=certs/'.sail-boundary-inputs/fixture/inputs.json';missing.unlink()
        failed=root/'cert-missing';failed.mkdir();refuses(lambda:module.retain_compiler_inputs(certs,failed,True));retained(failed);missing.write_text('{}')
        # Real admitted bytes must survive a later scan refusal as well.
        original_walk=module.os.walk
        def late_alias(*args,**kwargs):
            yield str(certs/'.sail-boundary-native-reads/fixture'), [], ['inputs.json']
            late=certs/'late-alias';late.symlink_to(certs/'.sail-boundary-native-reads')
            try:yield str(certs), ['late-alias'], []
            finally:late.unlink()
        module.os.walk=late_alias
        try:
            failed=root/'cert-late-refusal';failed.mkdir()
            refuses(lambda:module.retain_compiler_inputs(certs,failed,True));retained(failed)
        finally:module.os.walk=original_walk
        limit=module.CAPTURE_LIMIT
        try:
            module.CAPTURE_LIMIT=1
            failed=root/'cert-oversize';failed.mkdir();refuses(lambda:module.retain_compiler_inputs(certs,failed,True))
        finally:module.CAPTURE_LIMIT=limit
        missing.unlink();missing.symlink_to(certs/'.sail-boundary-native-reads/fixture/inputs.json')
        failed=root/'cert-alias';failed.mkdir();refuses(lambda:module.retain_compiler_inputs(certs,failed,True));missing.unlink();missing.write_text('{}')
        unreadable=certs/'unreadable';unreadable.mkdir();unreadable.chmod(0)
        try:
            failed=root/'cert-unreadable';failed.mkdir()
            try:module.retain_compiler_inputs(certs,failed,False)
            except PermissionError:pass
            else:raise AssertionError('unreadable certificate subtree silently omitted')
            assert json.loads((failed/'compiler-inputs/index.json').read_text())['complete'] is False
        finally:unreadable.chmod(0o700)
        path_before=os.environ['PATH']
        os.environ['PATH']=str(controlled)+os.pathsep+path_before
        try:
            # Complete owned source component, not a system-preinstalled source tree.
            prerequisite = module.rust_source(runtime)
            assert prerequisite['toolchain']==PIN and prerequisite['source_files']==1
            assert prerequisite['source_bytes']==len(b'owned rust source')
            previous_source_limit=module.SOURCE_LIMIT
            try:
                module.SOURCE_LIMIT=1
                refuses(lambda:module.rust_source(runtime))
            finally:module.SOURCE_LIMIT=previous_source_limit
            held = source.read_bytes()
            source.unlink();refuses(lambda:module.rust_source(runtime));source.write_bytes(held)
            extra = source.parent/'extra.rs';extra.write_text('extra')
            refuses(lambda:module.rust_source(runtime));extra.unlink()
            alias = source.parent/'alias.rs';source.rename(alias);source.symlink_to(alias)
            refuses(lambda:module.rust_source(runtime));source.unlink();alias.rename(source)
            source.write_bytes(b'changed same component')
            assert module.rust_source(runtime)['prerequisite_inventory_sha256'] != prerequisite['prerequisite_inventory_sha256']
            source.write_bytes(held)
            components = rustlib/'components';components.write_text('rustc\n')
            refuses(lambda:module.rust_source(runtime))
            # Components beyond the minimal profile change the bound compiler files.
            for extra in ('rustc-dev-x86_64-unknown-linux-gnu', 'llvm-tools-x86_64-unknown-linux-gnu', 'rust-std-x86_64-unknown-none'):
                components.write_text(MINIMAL+extra+'\n');refuses(lambda:module.rust_source(runtime))
            components.write_text('rust-src\n');refuses(lambda:module.rust_source(runtime))
            components.write_text(MINIMAL)
            manifest = rustlib/'manifest-rust-src';text=manifest.read_text()
            manifest.write_text(text+text);refuses(lambda:module.rust_source(runtime));manifest.write_text(text)
            manifest.write_text('file:../outside\n');refuses(lambda:module.rust_source(runtime));manifest.write_text(text)
            # Genuine release-origin shape is mandatory even after a successful
            # full fallback. Warm local receipt reuse is not fresh first Serve.
            served = copy.deepcopy(doc);served['run_id']='consumer'
            served['execution']['verifier_invocations']=1
            identity='b'*64+'/'+key
            origin={'kind':'release','identity':identity,'establishing_run':'shipped:'+identity}
            served['targets'][0]['dependency_build']=origin
            served['targets'][0]['establishing_run']=origin['establishing_run']
            served['targets'][1]['establishing_run']='consumer'
            assert module.first_use(release,served)==identity
            refuses(lambda:module.first_use(release,doc))  # Current full SAFE_MISS.
            for field,value in [('kind','machine-local'),('identity','foreign'),('establishing_run','foreign')]:
                wrong=copy.deepcopy(served);wrong['targets'][0]['dependency_build'][field]=value
                refuses(lambda:module.first_use(release,wrong))
            for change in ('warm','requested-origin','store','partial','wrong-count','diagnostic'):
                wrong=copy.deepcopy(served)
                if change=='warm':wrong['targets'][0]['reused']=True
                elif change=='requested-origin':wrong['targets'][1]['dependency_build']=origin
                elif change=='store':wrong['targets'][0]['dependency_store']={'key':'foreign'}
                elif change=='partial':wrong['targets'][1]['complete']=False
                elif change=='wrong-count':wrong['execution']['verifier_invocations']=2
                else:wrong['targets'][0]['report']['diagnostics']=[{'severity':'warning'}]
                refuses(lambda:module.first_use(release,wrong))
            # The factory must override the Sail builder's newer Cargo and bind
            # portable lock permissions before indexing a group-writable input.
            (template/'Cargo.lock').chmod(0o664)
            builder_toolchain = os.environ.get('RUSTUP_TOOLCHAIN')
            os.environ['RUSTUP_TOOLCHAIN'] = '1.99.0'
            for case in ('current','historical','capture-overflow','certificate-missing','scratch-cleanup-failure'):
                stage=root/('producer-'+case)
                (stage/'bin').mkdir(parents=True)
                shutil.copytree(runtime,stage/'engine/variant')
                for name in ('sail','sail-z3'):
                    path=stage/'bin'/name;path.write_text(executable);path.chmod(0o755)
                pid_record=root/(case+'-server-pid')
                (stage/'bin/fixture-config.json').write_text(json.dumps({'runtime':str(stage/'engine/variant'),'toolchain':PIN,
                    'runtime_hash':module.digest(stage/'engine/variant/sail-build.json'),
                    'template':str(template),'report':doc,'currentness':'historical' if case=='historical' else 'current',
                    'pid_record':str(pid_record),'evidence':str(root/(case+'-evidence')),'missing_certificate':case=='certificate-missing'}))
                evidence=root/(case+'-evidence')
                previous_limit=module.CAPTURE_LIMIT
                original_temporary=module.tempfile.TemporaryDirectory
                class CleanupFailure(original_temporary):
                    def __exit__(self,*args):
                        super().__exit__(*args)
                        raise KeyboardInterrupt('owned scratch exit injection')
                try:
                    if case=='scratch-cleanup-failure':module.tempfile.TemporaryDirectory=CleanupFailure
                    if case=='capture-overflow':module.CAPTURE_LIMIT=32
                    if case=='current':module.record(stage,evidence)
                    elif case=='scratch-cleanup-failure':
                        try:module.record(stage,evidence)
                        except KeyboardInterrupt as error:assert str(error)=='owned scratch exit injection'
                        else:raise AssertionError('scratch failure swallowed')
                    else:refuses(lambda:module.record(stage,evidence))
                finally:
                    module.CAPTURE_LIMIT=previous_limit
                    module.tempfile.TemporaryDirectory=original_temporary
                result=json.loads((evidence/'result.json').read_text())
                assert json.loads((evidence/'boundary-trace/fixture.json').read_text())=={'event':'actual-owned-mock-verify-entry'}
                assert result['server_reaped'] and result['original_stage_unchanged']
                assert result['passed']==(case=='current')
                if case=='current':
                    assert module.verify(stage)['proof']['members']['Cargo.lock']['mode']==0o644
                if case=='certificate-missing':retained(evidence)
                if case=='scratch-cleanup-failure':
                    scratch=json.loads((evidence/'scratch-root.json').read_text())
                    assert not Path(scratch['path']).exists()
                    assert not (stage/module.INDEX).exists()
                assert (stage/'deps').exists()==(case=='current')
                if pid_record.exists():
                    try:os.kill(int(pid_record.read_text()),0)
                    except ProcessLookupError:pass
                    else:raise AssertionError('owned producer server survived')
        finally:
            os.environ['PATH']=path_before
            if 'builder_toolchain' in locals():
                if builder_toolchain is None:os.environ.pop('RUSTUP_TOOLCHAIN',None)
                else:os.environ['RUSTUP_TOOLCHAIN']=builder_toolchain
    print('release source-prerequisite/origin/member/archive/source-report + actual owned producer-entry controls PASS (compiler/runtime doubled)')


if __name__=='__main__':main()
