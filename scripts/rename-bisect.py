#!/usr/bin/env python3
"""Bisects which declared names change Z3's result when renamed.
usage: bisect.py BASE.smt2 Z3 SAIL_BENCH
"""
import re, subprocess, sys

base, z3, bench = sys.argv[1:4]
text = open(base, "rb").read()
names = []
seen = set()
for m in re.finditer(rb"\(declare-(?:const|fun|sort)\s+([^\s()]+)", text):
    n = m.group(1)
    if n not in seen and not n.startswith(b"%%location_label%%"):
        seen.add(n)
        names.append(n)
for m in re.finditer(rb"\(declare-datatypes\s.*", text):
    for n in re.findall(rb"[^\s()]+", m.group(0)):
        if n not in seen and not n.isdigit() and n != b"declare-datatypes":
            seen.add(n)
            names.append(n)


def signature(path):
    out = subprocess.run([z3, "-smt2", path], capture_output=True).stdout
    return re.findall(rb"^(?:unsat|sat|unknown)$|rlimit-count\s+\d+", out, re.M)


want = signature(base)


def differs(subset):
    open("only.txt", "wb").write(b"\n".join(subset) + b"\n")
    subprocess.run([bench, "rename-one", base, "try.smt2", "1", "declared", "only.txt"], check=True, capture_output=True)
    return signature("try.smt2") != want


print(len(names), "declared names; all differ:", differs(names))
# Greedy delta debugging to a minimal subset.
subset = names
n = 2
while len(subset) > 1:
    chunk = max(1, len(subset) // n)
    parts = [subset[i : i + chunk] for i in range(0, len(subset), chunk)]
    for part in parts:
        if differs(part):
            subset, n = part, 2
            break
    else:
        for part in parts:
            rest = [x for x in subset if x not in part]
            if rest and differs(rest):
                subset, n = rest, max(n - 1, 2)
                break
        else:
            if n >= len(subset):
                break
            n = min(len(subset), n * 2)
print("minimal:", [s.decode() for s in subset], "differs:", differs(subset))
