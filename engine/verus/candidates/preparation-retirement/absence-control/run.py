#!/usr/bin/env python3
"""Pinned stock resolver + exact sealed metadata; no candidate authority or proof."""
import argparse,hashlib,importlib.util,json,os,signal,subprocess,time
from pathlib import Path
parser=argparse.ArgumentParser();parser.add_argument('--source',type=Path,required=True);parser.add_argument('--work-dir',type=Path,required=True);parser.add_argument('--vstd',type=Path,required=True);args=parser.parse_args()
base=Path(__file__).resolve().parent;work=args.work_dir;work.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);obj=importlib.util.module_from_spec(spec);spec.loader.exec_module(obj);return obj
preflight=module('metadata_input',base/'metadata-input.py');suffix,facts=preflight.bounded_input(args.vstd,expected_sha='cf8e7b5a273073dc7e07e863c03e2283183f99ca67cda139f269a73ba63b34f4');(work/'metadata-suffix').write_bytes(suffix)
libs,dependency=module('pinned_libraries',base/'pinned-libraries.py').pinned_libraries(args.source/'target-verus/release/deps')
def stock(name):return subprocess.check_output(['git','-C',str(args.source),'show','426d8b01e7ffb910a36c15e1f870ff985bb61eef:source/'+name],text=True)
s=stock('rust_verify/src/spans.rs');export=stock('rust_verify/src/import_export.rs');messages=stock('vir/src/messages.rs')
types=s[s.index('#[derive(Debug, Clone)]\nenum ExternSourceInfo'):s.index('pub(crate) type SpanContext')]
metadata=export[export.index('#[derive(Debug, Deserialize, Serialize)]'):export.index('#[derive(Debug, Deserialize, Serialize)]',export.index('pub(crate) struct CrateMetadata'))]
remaining=s[s.index('        for (imported_crate, files) in remaining_crate_files.iter()'):s.index('        let next_span_id')]
ranges=s[s.index('    fn pos_to_extern_source_file('):s.index('    fn pack_span(')]
unpack=s[s.index('    fn unpack_span('):s.index('    pub(crate) fn get_next_span_id(')]
from_air=s[s.index('    pub(crate) fn from_air_span('):s.index('    pub(crate) fn spanned_new<')]
from_raw=s[s.index('pub(crate) fn from_raw_span('):s.index('// Note: this produces a span')]
# The stock Span declaration ends at the next non-Span declaration/comment.
span_type=messages[messages.index('pub type RawSpan ='):messages.index('\n}',messages.index('pub struct Span'))+2]
probe='''#![feature(rustc_private)]
#![allow(dead_code)]
#![forbid(unsafe_code)]
extern crate rustc_driver;
extern crate rustc_span;
extern crate rustc_data_structures;
use rustc_span::{BytePos,Span,SpanData};
use rustc_span::source_map::SourceMap;
use std::collections::HashMap;
use std::sync::{Arc,Mutex};
use serde::{Serialize,Deserialize};
mod messages {use std::sync::Arc;use serde::{Serialize,Deserialize};
'''+messages[messages.index('fn empty_raw_span()'):messages.index('pub type RawSpan =')]+span_type+'\n}\nmod vir {pub(crate) use crate::messages;}\n'+types+metadata+'''\nstruct SpanContextX{imported_crates:HashMap<u64,CrateInfo>}
impl SpanContextX {\n'''+ranges+unpack+from_air+'\n}\n'+from_raw+'\nfn remaining_original_descriptors(remaining_crate_files:HashMap<u64,HashMap<Vec<u8>,FileStartEndPos>>)->HashMap<u64,CrateInfo>{let mut imported_crates=HashMap::new();\n'+remaining+'\nimported_crates}\n'+f'#[path={json.dumps(str(base/"tests.rs"))}]mod tests;\n'
(work/'probe.rs').write_text(probe);env={k:v for k,v in os.environ.items() if not k.startswith('SAIL_ENGINE')};env['RUSTC_BOOTSTRAP']='1';env['SAIL_TEST_METADATA_SUFFIX']=str(work/'metadata-suffix');sysroot=Path(subprocess.check_output(['rustc','+1.98.1','--print','sysroot'],text=True).strip());targetlib=Path(subprocess.check_output(['rustc','+1.98.1','--print','target-libdir'],text=True).strip());env['LD_LIBRARY_PATH']=str(sysroot/'lib')+':'+str(targetlib)
command=['rustc','+1.98.1','--edition=2024','--test','-C','prefer-dynamic',str(work/'probe.rs'),'-o',str(work/'probe'),'-L','dependency='+str(args.source/'target-verus/release/deps')]
for name,lib in libs.items():command+=['--extern',name+'='+str(lib)]
record={'scope':'Actual immutable pinned stock methods + exact sealed metadata; reduced SourceMap namespace fixture, no candidate authority/proof/HIT qualification','metadata_preflight':facts,'pinned_dependencies':dependency,'root_profile_basis':'Actual prior pinned source metadata graph release profile1241562680900052086; excludes separately recorded Clippy check profile','stock_source_sha256':{name:hashlib.sha256(text.encode()).hexdigest() for name,text in [('spans.rs',s),('import_export.rs',export),('messages.rs',messages)]},'stock_extracted_sha256':{name:hashlib.sha256(text.encode()).hexdigest() for name,text in [('ranges',ranges),('unpack',unpack),('from_air',from_air),('from_raw',from_raw),('remaining',remaining),('types',types),('metadata',metadata)]},'phases':[],'status':'running'}
for phase,argv,limit in [('compile',command,60),('test',[str(work/'probe'),'--test-threads=1','--nocapture'],30)]:
 started=time.monotonic();timed_out=False
 with (work/(phase+'.stdout')).open('wb') as stdout,(work/(phase+'.stderr')).open('wb') as stderr:
  child=subprocess.Popen(argv,env=env,stdout=stdout,stderr=stderr,start_new_session=True)
  try:code=child.wait(timeout=limit)
  except subprocess.TimeoutExpired:timed_out=True;os.killpg(child.pid,signal.SIGKILL);code=child.wait()
 record['phases'].append({'phase':phase,'exit_code':code,'timed_out':timed_out,'limit_seconds':limit,'elapsed_seconds':time.monotonic()-started});(work/'summary.json').write_text(json.dumps(record,indent=2)+'\n')
 if code or timed_out:break
record['status']='PASS' if len(record['phases'])==2 and all(p['exit_code']==0 and not p['timed_out'] for p in record['phases']) else 'FAIL';record['post_vstd_sha256']=sha(args.vstd);assert record['post_vstd_sha256']==facts['input_sha256'];record['raw']={p.name:{'sha256':sha(p),'bytes':p.stat().st_size} for p in work.iterdir() if p.is_file() and p.name!='summary.json'};(work/'summary.json').write_text(json.dumps(record,indent=2)+'\n');print(json.dumps({'status':record['status'],'phases':record['phases']}));raise SystemExit(0 if record['status']=='PASS' else 1)
