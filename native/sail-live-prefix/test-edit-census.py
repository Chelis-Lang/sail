#!/usr/bin/env python3
"""Literal parser controls only; these fixture bytes are not native strace evidence."""
import copy
import json
from edit_census import census

wrapper = '/owned/research/edit-interposer.py'
encoded = ''.join('\\x' + format(byte, '02x') for byte in wrapper.encode())
execution = ('1.000000 execve("' + encoded + '", [], 0x0 /* 0 vars */) = 0\n').encode()
normal = execution + b'1.100000 exit_group(0) = ?\n1.100100 +++ exited with 0 +++\n'
identity = dict(pid=42, attempt='edited', session='a'*32)
starts = {'a'*32: dict(identity=identity, context_sha256='bound')}
finishes = {'a'*32: dict(identity=identity, exit=0, error=None, context_sha256='bound')}
sessions = [dict(identity=identity)]
rows = []
for name in ('exact', 'missing-start', 'missing-broker', 'ignored-late7',
             'duplicate-exec', 'failed-exec', 'partial-exec', 'missing-terminal',
             'actual-signal', 'empty-census'):
    traces, begin, end, joined = {42:normal}, copy.deepcopy(starts), copy.deepcopy(finishes), copy.deepcopy(sessions)
    if name == 'missing-start':begin.clear()
    if name == 'missing-broker':joined.clear()
    if name == 'ignored-late7':traces[42]=execution+b'1.100100 +++ exited with 7 +++\n'
    if name == 'duplicate-exec':traces[42]=execution+normal
    if name == 'failed-exec':traces[42]=normal.replace(b') = 0',b') = -1 ENOENT (No such file or directory)')
    if name == 'partial-exec':traces[42]=normal.replace(b') = 0',b' <unfinished ...>')
    if name == 'missing-terminal':traces[42]=execution
    if name == 'actual-signal':traces[42]=execution+b'1.100100 +++ killed by SIGKILL +++\n'
    if name == 'empty-census':traces.clear()
    result=census(traces,wrapper,joined,begin,end)
    assert result['passed'] == (name=='exact'), (name,result)
    rows.append(dict(name=name,passed=True,refused=not result['passed']))
print(json.dumps(dict(passed=True,rows=rows,scope=__doc__),indent=2))
