from pathlib import Path
import subprocess,os,time,json,hashlib
import argparse
parser=argparse.ArgumentParser(description="Check pinned rustc diagnostic SourceMap owner and narrow lint allowance")
parser.add_argument("--work-dir",type=Path,required=True)
args=parser.parse_args()
p=args.work_dir.resolve()
p.mkdir(parents=True,exist_ok=True)
(p/'probe.rs').write_bytes(Path(__file__).with_name('owner-api.rs').read_bytes())
def sha(x):return hashlib.sha256(x.read_bytes()).hexdigest()
env=dict(os.environ,RUSTC_BOOTSTRAP='1')
sysroot=Path(subprocess.check_output(['rustc','+1.98.1','--print','sysroot'],text=True).strip())
version=subprocess.check_output(['rustc','+1.98.1','-vV'],text=True)
host=next(line.removeprefix('host: ') for line in version.splitlines() if line.startswith('host: '))
env['DYLD_LIBRARY_PATH' if 'apple' in host else 'LD_LIBRARY_PATH']=str(sysroot/'lib')+':'+str(sysroot/'lib/rustlib'/host/'lib')
version=subprocess.check_output(['rustc','+1.98.1','-vV'],text=True)
source=(p/'probe.rs').read_text()
(p/'unallowed.rs').write_text(source.replace('            #[allow(clippy::arc_with_non_send_sync)]\n',''))
(p/'rc.rs').write_text(source.replace('map: Arc<SourceMap>', 'map: std::rc::Rc<SourceMap>').replace('let map = Arc::new(map);','let map = std::rc::Rc::new(map);').replace('            rustc_data_structures::marker::assert_dyn_send_val(&map);\n','').replace('Arc::strong_count(&map)','std::rc::Rc::strong_count(&map)'))
records=[]
for name,compiler,file in [('compile',['rustc','+1.98.1'],'probe.rs'),('strict-clippy',['rustup','run','1.98.1','clippy-driver'],'probe.rs'),('unallowed-arc',['rustup','run','1.98.1','clippy-driver'],'unallowed.rs'),('rc-negative',['rustc','+1.98.1'],'rc.rs')]:
 cmd=compiler+['--edition=2024','--test',str(p/file),'-C','prefer-dynamic','-o',str(p/name)]
 if name in ['strict-clippy','unallowed-arc']:cmd+=['-D','warnings','-D','clippy::arc_with_non_send_sync']
 started=time.monotonic();r=subprocess.run(cmd,env=env,capture_output=True,timeout=60)
 (p/(name+'.stdout')).write_bytes(r.stdout);(p/(name+'.stderr')).write_bytes(r.stderr)
 records.append(dict(name=name,exit_code=r.returncode,elapsed_seconds=time.monotonic()-started,source_sha256=sha(p/file),stdout_sha256=sha(p/(name+'.stdout')),stderr_sha256=sha(p/(name+'.stderr'))))
 print(name,r.returncode,r.stderr.decode()[:2000])
 if name in ['compile','strict-clippy']:assert r.returncode==0
 elif name=='unallowed-arc':assert r.returncode!=0 and b'arc_with_non_send_sync' in r.stderr
 else:assert r.returncode!=0 and b'DynSend' in r.stderr
r=subprocess.run([str(p/'compile'),'--test-threads=1','--nocapture'],env=env,capture_output=True,timeout=30)
(p/'test.stdout').write_bytes(r.stdout);(p/'test.stderr').write_bytes(r.stderr)
print('test',r.returncode,r.stdout.decode(),r.stderr.decode());assert r.returncode==0
records.append(dict(name='test',exit_code=r.returncode,stdout_sha256=sha(p/'test.stdout'),stderr_sha256=sha(p/'test.stderr'),binary_sha256=sha(p/'compile')))
(p/'summary.json').write_text(json.dumps(dict(version=1,scope='Actual pinned compiler Emitter/DynSend ownership control only; no source qualification or proof',compiler=version,records=records),indent=2)+'\n')
