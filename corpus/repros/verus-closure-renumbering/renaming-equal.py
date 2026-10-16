"""Check that two SMT log trees (`--log smt` directories of the same input)
are equal up to a one-to-one renaming of closure-type numbers
(anonymous_closure%N, impl_closure&%<Kind>N and their mangled forms),
consistent within each file, and up to whitespace (the printer wraps long
lines). Used for ADR 0048 / VI-0019: the fork before and after the fix.
usage: python3 -I renaming-equal.py A_DIR B_DIR
"""
import os, re, sys

pat = re.compile(r"(anonymous_closure(?:%|__)|impl_closure&(?:%|__)(?:FnOnce|FnMut|Fn))(\d+)")


def walk(d):
    return {os.path.relpath(os.path.join(r, f), d) for r, _, fs in os.walk(d) for f in fs}


def norm(text):
    ids = {}
    def sub(m):
        n = ids.setdefault(m.group(2), len(ids))
        return f"{m.group(1)}<{n}>"
    # The printer wraps long lines, so longer names move line breaks:
    # compare tokens, with `;;` comments kept as whole lines.
    out = []
    for line in pat.sub(sub, text).splitlines():
        out.append(line.strip() if line.lstrip().startswith(";;") else " ".join(line.split()))
    return _join(out), len(ids)


def _join(lines):
    toks = []
    for l in lines:
        if l.startswith(";;"):
            toks.append("\n" + l + "\n")
        elif l:
            toks.append(l)
    return " ".join(toks).replace("( ", "(").replace(" )", ")")


a, b = sys.argv[1], sys.argv[2]
files = sorted(walk(a) | walk(b))
same = raw_same = differ = closures = 0
for f in files:
    if not f.endswith(".smt2"):
        continue
    ta = open(os.path.join(a, f), errors="replace").read()
    tb = open(os.path.join(b, f), errors="replace").read()
    if ta == tb:
        raw_same += 1
        continue
    na, ka = norm(ta)
    nb, kb = norm(tb)
    if na == nb:
        same += 1
        closures += ka
    else:
        differ += 1
        la, lb = na.split(" "), nb.split(" ")
        i = next((i for i, (x, y) in enumerate(zip(la, lb)) if x != y), None)
        print(f"DIFFERENT {f}: line {i}: {la[i] if i is not None else ''!r} / {lb[i] if i is not None else ''!r}")
print(f"{raw_same} byte-identical, {same} identical after renaming closure numbers "
      f"({closures} closure numbers), {differ} different")
