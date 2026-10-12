#!/usr/bin/env python3
"""Reject old preparation keys and correctly checksummed misfiled old entries.

Uses genuine old/new Verus runtimes, identical source/caller context and ordinary
complete tiny proofs. Does not establish project performance or qualification.
"""
import argparse
import json
import os
import pathlib
import shutil
from importlib.machinery import SourceFileLoader

ROOT = pathlib.Path(__file__).resolve().parent
oracle = SourceFileLoader('source_oracle', str(ROOT.parent / 'test-engine.py')).load_module()
controls = SourceFileLoader('same_callback', str(ROOT / 'test-same-callback.py')).load_module()


def run(args):
    out = pathlib.Path(args.out).absolute()
    out.mkdir(parents=True, exist_ok=False)
    source = out / 'lib.rs'
    source.write_text('use verus_builtin::*;\nuse verus_builtin_macros::*;\nverus! { proof fn selected(){assert(true);} proof fn other(){assert(true);} }\nfn main(){}\n')
    cache = out / 'prepared'
    records = []
    summary = {'version':1,'scope':'Old-domain/misfiled entry rejection, genuine compiled actors; not product qualification',
               'old_verifier_sha256':oracle.sha(pathlib.Path(args.old_engine).with_name('rust_verify')),
               'new_verifier_sha256':oracle.sha(pathlib.Path(args.engine).with_name('rust_verify')),
               'records':records,'controls_completed':False}
    def execute(label, binary):
        work = out / label
        work.mkdir()
        env = {k:v for k,v in os.environ.items() if not k.startswith('SAIL_ENGINE')}
        env.update(RUSTUP_TOOLCHAIN='1.98.1',VERUS_Z3_PATH=args.z3,SAIL_ENGINE='interactive',
                   SAIL_ENGINE_FUNCTION='demo::selected',SAIL_ENGINE_CONTEXT='domain-migration-identical-caller-v1',
                   SAIL_ENGINE_PREPARED_DIR=str(cache),SAIL_ENGINE_EVENTS=str(work/'events.jsonl'),
                   SAIL_ENGINE_FOCUS_REPORT=str(work/'focus.json'))
        command = [binary,str(source),'--crate-name','demo','-V','use-crate-name','--no-vstd','--output-json','--time']
        proc = controls.execute(command,env,600,work,'engine')
        (work/'stdout').write_bytes(proc.stdout);(work/'stderr').write_bytes(proc.stderr)
        events = [json.loads(line) for line in (work/'events.jsonl').read_text().splitlines()]
        full=json.loads(proc.stdout)['verification-results']
        focused=[e for e in events if e['phase']=='focus' and e['event']=='prepared-restored']
        item={'case':label,'exit_code':proc.returncode,'full_verification':full,'focus_restores':focused,'assertions_completed':False}
        records.append(item);(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
        assert proc.returncode==0 and full['success'] is True and full['errors']==0 and full['verified']==2
        assert events[-1]['event']=='compiler-sealed' and events[-1]['payload']['normal_termination'] is True
        return item
    old=execute('old-fill',args.old_engine)
    old_files={p.name:p.read_bytes() for p in cache.glob('*.prepared')}
    assert old_files, 'Old complete actor must actually produce preparations'
    new=execute('new-establish',args.engine)
    assert not new['focus_restores'], 'Old dependency entries authorized new focused preparation'
    new_files={p.name:p.read_bytes() for p in cache.glob('*.prepared') if p.name not in old_files}
    assert new_files and set(old_files).isdisjoint(new_files), 'Dependency-domain keys did not change'
    for name,data in old_files.items():
        assert (cache/name).read_bytes()==data, 'Old failed migration evidence was modified'
    warm=execute('new-warm',args.engine)
    assert warm['focus_restores'] and warm['focus_restores'][0]['payload']['source']=='persistent'
    # Deliberately misfile an intact old entry with its original valid body
    # checksum under every new key. Normal decode/key guards must reject it.
    intact_old=next(iter(old_files.values()))
    for name in new_files:
        (cache/name).write_bytes(intact_old)
    corrupt=execute('new-misfiled-old',args.engine)
    assert not corrupt['focus_restores'], 'Checksummed old-key payload escaped migration guard'
    for item in records:item['assertions_completed']=True
    summary.update(old_keys=sorted(old_files),new_keys=sorted(new_files),controls_completed=True)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['old-engine','engine','z3','out']:parser.add_argument('--'+name,required=True)
    run(parser.parse_args())
