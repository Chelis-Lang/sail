#!/usr/bin/env python3
"""Replay one captured vstd query at its original and tenfold resource limit."""
import argparse
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('z3', help='the pinned engine runtime Z3 executable')
parser.add_argument('--runs', type=int, default=12)
args = parser.parse_args()
if args.runs < 1:
    parser.error('--runs must be positive')
source = Path(__file__).with_name('query.smt2').read_bytes()
limit = re.compile(rb'\(set-option :rlimit ([0-9]+)\)')
raised = limit.sub(lambda m: b'(set-option :rlimit ' + str(int(m[1]) * 10).encode() + b')', source)
answers, counts = set(), set()
for label, data in [('default', source), ('raised', raised)]:
    for index in range(args.runs):
        result = subprocess.run([args.z3, '-smt2', '-in'], input=data, capture_output=True,
                                timeout=60, check=True)
        answer = tuple(re.findall(rb'^(sat|unsat|unknown)$', result.stdout, re.M))
        count = tuple(int(n) for n in re.findall(rb':rlimit-count\s+([0-9]+)', result.stdout))
        if result.stderr or not answer or not count:
            raise SystemExit('unexpected solver output; inspect the pinned solver and input')
        answers.add(answer)
        counts.add(count)
        print(label, index, [a.decode() for a in answer], count, flush=True)
if len(answers) != 1:
    raise SystemExit('solver answers differ')
print('Resource-count variation observed' if len(counts) > 1 else 'Variation not observed in this sample')
raise SystemExit(0 if len(counts) > 1 else 1)
