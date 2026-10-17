"""Actual mandatory-control entry and owned children; native image APIs doubled.

The actual Linux eight-control run remains mandatory and unrun. This fixture
checks its command entry, raw/result serialization and case-count admission.
"""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import signal
import sys
import tempfile
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
import live_prefix

with tempfile.TemporaryDirectory() as td:
 root=Path(td);worker=root/'solver'
 worker.write_text('#!'+sys.executable+'\nimport sys\nfor line in sys.stdin.buffer:\n if b"definitely-not" in line:\n  print(\'(error "fixture malformed input")\',flush=True);sys.exit(7)\n if b"check-sat" in line:print("unsat",flush=True)\n if b"(exit)" in line:break\n')
 worker.chmod(0o700)
 spec=importlib.util.spec_from_file_location('kernel_native_controls',HERE/'test-kernel-owner-native.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
 module.SOLVER=hashlib.sha256(worker.read_bytes()).hexdigest()
 descriptors={}
 def capture(pid,expected):
  if expected=='0'*64:raise ValueError('explicit fixture actual-image mismatch')
  first,second=os.open(os.devnull,os.O_RDONLY),os.open(os.devnull,os.O_RDONLY)
  descriptors[first]=pid
  return first,second,None
 def send(fd,sig):os.kill(descriptors[fd],sig)
 with patch.object(sys,'argv',['test-kernel-owner-native.py','--z3',str(worker),'--out',str(root/'out')]),patch.object(os,'uname',lambda:os.uname_result(('Linux','','','','x86_64'))),patch.object(os,'pidfd_open',lambda *_:None,create=True),patch.object(signal,'pidfd_send_signal',send,create=True),patch.object(live_prefix,'capture_running_image',capture),patch.object(live_prefix.Entry,'_image_ok',lambda _:True):
  code=module.main()
 data=json.loads((root/'out/controls.json').read_text())
 assert code==0 and data['passed'] and len(data['cases'])==8,data
 assert all(r['passed'] for r in data['cases'])
 assert len(data['owner_releases'])==6
 assert (root/'out/fresh.stdout').read_bytes()==b'unsat\n'
 assert (root/'out/nonzero.stdout').read_bytes().startswith(b'(error')
 print(json.dumps(dict(passed=True,cases=8,scope=__doc__,rows=data['cases'])))
