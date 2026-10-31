#!/usr/bin/env python3
"""Bounded baseline-only control: external input plus an unrelated conventional target."""
import argparse,json,os,pathlib,shutil,subprocess,tempfile,time
P=pathlib.Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--sail',type=P,required=True,help='bundled baseline Sail executable')
parser.add_argument('--out',type=P,required=True,help='new raw-evidence directory')
args=parser.parse_args();sail=args.sail.expanduser().absolute();out=args.out.expanduser().absolute();out.mkdir(parents=True)

work=P(tempfile.mkdtemp(prefix='sail-external-repro-'));env=dict(os.environ)
for key in list(env):
 if key.startswith(('SAIL_','VERUS_')):env.pop(key)
env.update(RUSTUP_TOOLCHAIN='1.99.1')
env['PATH']=str(sail.parent)+':'+env['PATH']
subprocess.run(['cargo','verus','new','--lib','demo'],cwd=work,env=env,check=True,capture_output=True)
source='''use vstd::prelude::*;
verus! {
pub fn one() -> (r:u32) ensures r == 1, { 1 }
}
'''
(work/'outside.txt').write_text('first\n')
(work/'demo/src/lib.rs').write_text(source+'pub const PROBE: Option<&str> = option_env!("NOOP_PROBE_VALUE");\npub const OUTSIDE: &str = include_str!('+json.dumps(str(work/'outside.txt'))+');\n')
(work/'output-writer/src').mkdir(parents=True)
(work/'output-writer/Cargo.toml').write_text('[package]\nname="sail-output-writer"\nversion="0.1.0"\nedition="2024"\n[workspace]\n')
(work/'output-writer/src/main.rs').write_text('fn main() {}\n')
subprocess.run(['cargo','build','--manifest-path',str(work/'output-writer/Cargo.toml'),'--target-dir',str(work/'demo/target')],env=env,check=True,capture_output=True)
records=[]
try:
 for round in (1,2,3):
  for mode in ('plain','sail'):
   e=dict(env,CARGO_TARGET_DIR=str(work/('target-'+mode)))
   if mode=='plain':argv=['cargo','verus','verify']
   else:
    e.update(SAIL_CACHE_DIR=str(work/'cache'));argv=[str(sail),'verify']
   t=time.monotonic();r=subprocess.run(argv,cwd=work/'demo',env=e,capture_output=True)
   (out/f'{mode}-{round}.out').write_bytes(r.stdout);(out/f'{mode}-{round}.err').write_bytes(r.stderr)
   records.append({'round':round,'mode':mode,'exit':r.returncode,'wall_s':time.monotonic()-t,'stdout':r.stdout.decode(errors='replace'),'argv':argv})
   print(records[-1],flush=True)
  (out/'results.json').write_text(json.dumps({'scope':'bundled Sail, ordinary TMPDIR, three unchanged plain/Sail rounds; syntax/output control, not performance acceptance','logical_root':str(work),'physical_root':str(work.resolve()),'records':records},indent=2)+'\n')
 for file in ('Cargo.toml','Cargo.lock','src/lib.rs'):
  target=out/'source'/file;target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(work/'demo'/file,target)
 for n,p in enumerate((work/'cache').glob('projects/*/*/runs/*/result.json')):(out/f'managed-{n}.json').write_bytes(p.read_bytes())
finally:
 e=dict(env,SAIL_CACHE_DIR=str(work/'cache'))
 subprocess.run([str(P(__file__).resolve().parents[3]/'scripts/sail-stop-daemon.sh')],env=e,capture_output=True)
 shutil.rmtree(work)
