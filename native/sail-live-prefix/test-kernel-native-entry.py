"""Actual mandatory-control entry and owned children; native image APIs doubled.

The corrected Linux twelve-control run remains mandatory and unrun. This fixture
checks its command entry, raw/result serialization and case-count admission.
"""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import select
from types import SimpleNamespace
import sys
import tempfile
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
import live_prefix

with tempfile.TemporaryDirectory() as td:
 root=Path(td);worker=root/'solver'
 worker.write_text('#!'+sys.executable+'\n'+'import sys\nfor line in sys.stdin.buffer:\n if line == b"(definitely-not-an-smt-command)\\n":\n  print("unsupported",flush=True);print("; definitely-not-an-smt-command line: 1 position: 31",file=sys.stderr,flush=True)\n elif line == b")\\n":\n  print(\'(error "line 1 column 1: invalid command, \\\'(\\\' expected")\',flush=True);sys.exit(1)\n elif b"check-sat" in line:print("unsat",flush=True)\n elif b"(exit)" in line:break\n')
 worker.chmod(0o700)
 spec=importlib.util.spec_from_file_location('kernel_native_controls',HERE/'test-kernel-owner-native.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 module.SOLVER=hashlib.sha256(worker.read_bytes()).hexdigest()
 descriptors={}
 actual_wait4=os.wait4
 def waitid(kind,fd,flags):
  assert kind==4242 and flags==os.WEXITED|os.WNOHANG
  os.fstat(fd);pid,status,_=actual_wait4(descriptors[fd],os.WNOHANG)
  if not pid:return None
  return SimpleNamespace(si_pid=pid,si_signo=signal.SIGCHLD,si_code=os.CLD_EXITED if os.WIFEXITED(status) else os.CLD_KILLED,si_status=os.WEXITSTATUS(status) if os.WIFEXITED(status) else os.WTERMSIG(status))
 class Poll:
  def register(self,fd,flags):self.fd=fd
  def poll(self,timeout=0):return [(self.fd,1)]
 def capture(pid,expected):
  if expected=='0'*64:raise ValueError('explicit fixture actual-image mismatch')
  first,second=os.open(os.devnull,os.O_RDONLY),os.open(os.devnull,os.O_RDONLY)
  descriptors[first]=pid
  return first,second,None
 actual_dup=os.dup
 def duplicate(fd):
  new=actual_dup(fd);descriptors[new]=descriptors[fd];return new
 def send(fd,sig):os.fstat(fd);os.kill(descriptors[fd],sig)
 with patch.object(sys,'argv',['test-kernel-owner-native.py','--z3',str(worker),'--out',str(root/'out')]),patch.object(os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))),patch.object(os,'pidfd_open',lambda *_:None,create=True),patch.object(signal,'pidfd_send_signal',send,create=True),patch.object(live_prefix,'capture_running_image',capture),patch.object(live_prefix.Entry,'_image_ok',lambda _:True),patch.object(os,'P_PIDFD',4242,create=True),patch.object(os,'waitid',waitid,create=True),patch.object(os,'dup',duplicate),patch.object(select,'poll',Poll,create=True):
  code=module.main()
 data=json.loads((root/'out/controls.json').read_text())
 assert code==0 and data['passed'] and len(data['cases'])==12,data
 assert all(r['passed'] for r in data['cases'])
 assert len(data['owner_releases'])==10
 assert (root/'out/fresh.stdout').read_bytes()==b'unsat\n'
 assert (root/'out/unsupported.stdout').read_bytes()==b'unsupported\n'
 assert (root/'out/unsupported.stderr').read_bytes()==b'; definitely-not-an-smt-command line: 1 position: 31\n'
 assert (root/'out/nonzero.stdout').read_bytes().startswith(b'(error')
 print(json.dumps(dict(passed=True,cases=12,scope=__doc__,rows=data['cases'])))
