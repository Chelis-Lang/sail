"""Actual parent and child exits; solver/resource manifests are explicit doubles."""
import contextlib
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
from unittest.mock import patch

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE))
WORKER=r'''import argparse,json,os,pathlib,sys
p=argparse.ArgumentParser();p.add_argument('--out',required=True);a,extra=p.parse_known_args();o=pathlib.Path(a.out);o.mkdir();case=os.environ['SAIL_KERNEL_TEST_CASE'];fresh='--fresh-only' in extra
assert '--experiment-kernel-accounting-owner' in extra and '--experiment-numeric-time-only' in extra
m={'resource_policy':'owned-wait4-lifetime-peak-v1'}
if fresh:
 for i in range(3):(o/('fresh-'+str(i)+'.stdout')).write_bytes(b'unsat\n(:time 123.45)\n<<DONE>>\n')
 m.update(experiment_fresh_control_pass=True,fresh_control_pass=False,fresh_samples=[{'kernel_resource_gate_pass':True} for _ in range(3)])
 if case=='missing-fresh':m.pop('fresh_samples')
 if case=='short-fresh':m['fresh_samples'].pop()
 if case=='bad-fresh-resource':m['fresh_samples'][2]['kernel_resource_gate_pass']=False
else:
 m.update(experiment_candidate_gate_pass=True,candidate_kernel_resource_gate_pass=case!='bad-live-resource',fresh_kernel_resource_gate_pass=True)
if case=='missing-policy':m.pop('resource_policy')
(o/'manifest.json').write_text(json.dumps(m))
sys.exit(7 if case=='fresh-nonzero' and fresh else 0)
'''
rows=[]
for case in ('success','missing-policy','missing-fresh','short-fresh','bad-fresh-resource','bad-live-resource','fresh-nonzero'):
 with tempfile.TemporaryDirectory() as td:
  root=Path(td);src=root/'source';src.mkdir();(src/'run.py').write_text(WORKER)
  z3=root/'solver';z3.write_bytes(b'not executed, explicit fixture')
  selected=[]
  for project in ('ironkv','anvil'):
   d=src/project;d.mkdir();(d/'prefix.smt2').write_bytes(b'prefix');(d/'suffix.smt2').write_bytes(b'suffix');(d/'prefix.stdout').write_bytes(b'unsat\n(:time 0.01)\n')
   selected.append(dict(project=project,**{'edited.smt2_sha256':hashlib.sha256(b'prefixsuffix').hexdigest()}))
  (src/'pairs.json').write_text(json.dumps(dict(selected=selected)))
  hashes={str(p.relative_to(src)):hashlib.sha256(p.read_bytes()).hexdigest() for p in src.rglob('*') if p.is_file()}
  (src/'source-hashes.json').write_text(json.dumps(hashes))
  spec=importlib.util.spec_from_file_location('kernel_parent',HERE/'run-real.py');module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.SOLVER=hashlib.sha256(z3.read_bytes()).hexdigest()
  with patch.object(sys,'argv',['run-real.py','--source',str(src),'--out',str(root/'out'),'--z3',str(z3),'--experiment-numeric-time-only','--experiment-kernel-accounting-owner']),patch.object(module.platform,'system',lambda:'Linux'),patch.object(module.platform,'machine',lambda:'x86_64'),patch.dict(os.environ,SAIL_KERNEL_TEST_CASE=case):
   code=module.main()
  m=json.loads((root/'out/manifest.json').read_text())
  assert (code==0)==(case=='success'),(case,m)
  assert not m['qualified'] and not m['selected'] and m['resource_policy']=='owned-wait4-lifetime-peak-v1'
  assert set(m['projects'])=={'ironkv','anvil'}
  if case not in ('success','bad-live-resource'):
   assert all('one_use_skipped' in r for r in m['projects'].values()),(case,m)
   assert not (root/'out/ironkv-one-use').exists()
  if case=='fresh-nonzero':assert all(r['fresh_exit']==7 for r in m['projects'].values())
  rows.append(dict(case=case,passed=True))
print(json.dumps(dict(passed=True,cases=len(rows),rows=rows,scope=__doc__)))
