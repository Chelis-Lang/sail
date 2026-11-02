#!/usr/bin/env python3
"""Run a Verus command several times on identical input and fail on any
byte difference (docs/validation/determinism.md).

    scripts/verus-determinism.py [--runs K] [--out DIR] [--inputs FILE...] -- COMMAND...

--classify-consumed-resources explicitly permits only ADR 0097 consumption
metadata differences, reporting their original values and retaining captures.
The default remains strict; solver streams, requested limits, verdicts,
diagnostics, stderr and exits remain exact in both modes.

COMMAND runs K times (default 3) in the current directory; with --inputs it
runs K times per input, `{}` in COMMAND naming the input. Each run records:
- the exact bytes sent to every solver process (VERUS_Z3_PATH becomes a `tee`
  wrapper around the real solver, taken from VERUS_Z3_PATH or SAIL_REAL_Z3);
- stdout with the wall-clock numbers inside `times-ms` blanked (rlimit counts,
  key order and array order are kept), stderr with Cargo's elapsed time
  blanked, and the exit code.

Solver processes are matched across runs by their declared names (sorted), so
a bucket whose bytes change in any way is reported, whatever order the
processes started in. Exit status: 0 if every run of every input agreed, 1 if
anything differed (the first differing line is printed), 2 on usage errors.
"""
import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter

TEE = """#!/bin/sh
f=$(mktemp "$SAIL_DETERMINISM_DIR/z3-XXXXXXXXXX") || exit 125
tee "$f" | "$SAIL_DETERMINISM_Z3" "$@"
"""
DECL = re.compile(rb"\((?:declare-fun|declare-const|declare-sort|declare-datatypes \() ?\(?([^\s()]+)")
CARGO_TIME = re.compile(r"^(\s*Finished .*) in [0-9.]+m?s$", re.M)


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blank_timing(text):
    """Zero every number in a `times-ms` object except rlimit* and num-threads."""
    out, stack, i, n = [], [], 0, len(text)
    key, pending_key = None, False
    while i < n:
        c = text[i]
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            s = text[i:j + 1]
            if stack and stack[-1][0] == "{" and stack[-1][2]:
                stack[-1][1] = s[1:-1]
            out.append(s)
            i = j + 1
            continue
        if c in "{[":
            parent = stack[-1] if stack else None
            timing = bool(parent) and (parent[3] or parent[1] == "times-ms")
            stack.append([c, parent[1] if (c == "[" and parent) else None, c == "{", timing])
        elif c in "}]":
            if stack:
                stack.pop()
        elif c == ":" and stack:
            stack[-1][2] = False
        elif c == "," and stack:
            stack[-1][2] = stack[-1][0] == "{"
        elif (c == "-" or c.isdigit()) and stack:
            m = re.match(r"-?[0-9.eE+-]+", text[i:])
            k = stack[-1][1] or ""
            blank = stack[-1][3] and not k.startswith("rlimit") and k != "num-threads"
            out.append("0" if blank else m.group(0))
            i += len(m.group(0))
            continue
        out.append(c)
        i += 1
    return "".join(out)


def first_difference(a, b, label):
    la, lb = a.splitlines(), b.splitlines()
    k = next((i for i, (x, y) in enumerate(zip(la, lb)) if x != y), min(len(la), len(lb)))
    show = lambda l: (l[k][:300] if k < len(l) else "<end>")
    return f"  {label}: first difference at line {k + 1}\n    a: {show(la)!r}\n    b: {show(lb)!r}"


def run_once(command, cap, env):
    os.makedirs(cap)
    env = dict(env, SAIL_DETERMINISM_DIR=cap)
    # Keep metadata beside the capture directory: only solver streams belong
    # inside it and participate in bucket matching.
    with open(cap + ".command.json", "w") as file:
        json.dump({"command": command}, file, indent=2)
    p = subprocess.run(command, env=env, stdin=subprocess.DEVNULL, capture_output=True)
    for suffix, data in [("stdout", p.stdout), ("stderr", p.stderr)]:
        with open(cap + "." + suffix, "wb") as file:
            file.write(data)
    with open(cap + ".result.json", "w") as file:
        json.dump({"command": command, "exit": p.returncode}, file, indent=2)
    streams = []
    for name in sorted(os.listdir(cap)):
        data = open(os.path.join(cap, name), "rb").read()
        names = sorted(set(DECL.findall(data)))
        streams.append((sha(b"\n".join(names)), sha(data), os.path.join(cap, name)))
    stdout = blank_timing(p.stdout.decode(errors="replace"))
    stderr = CARGO_TIME.sub(r"\1 in <time>", p.stderr.decode(errors="replace"))
    return {"exit": p.returncode, "stdout": stdout, "stderr": stderr, "streams": streams}


def check(command, runs, out, env, classify_consumption=False, classifications=None):
    results = [run_once(command, os.path.join(out, f"run-{i}"), env) for i in range(runs)]
    problems = []
    metadata = []
    if classify_consumption:
        from verus_resource_statistics import classify_stdout
        # A stdout classification never authorizes another field or stream to
        # change. Preserve exact stream bytes and multiplicity, ignoring only
        # the capture filenames as the strict bucket comparison already does.
        streams = [Counter((sig, digest) for sig, digest, _ in r["streams"])
                   for r in results]
        other_fields_same = (all(r["exit"] == results[0]["exit"]
                                 and r["stderr"] == results[0]["stderr"]
                                 for r in results)
                             and all(value == streams[0] for value in streams))
        if other_fields_same:
            for index, result in enumerate(results[1:], 1):
                classified = classify_stdout(results[0]["stdout"], result["stdout"])
                if classified["classification"] == "strict-different":
                    metadata = []
                    break
                if classified["classification"] == "consumed-resource-only":
                    metadata.append({"reference_run": 0, "candidate_run": index,
                                     "differences": classified["differences"]})
    for field in ("exit", "stdout", "stderr"):
        values = [r[field] for r in results]
        if len(set(values)) > 1:
            if field == "stdout" and metadata:
                continue
            j = next(i for i, v in enumerate(values) if v != values[0])
            detail = first_difference(values[0], values[j], f"{field} run 0 vs {j}") if field != "exit" else f"  exit codes {values}"
            problems.append(f"{field} differs\n{detail}")
    buckets = {}
    for i, r in enumerate(results):
        for sig, digest, path in r["streams"]:
            buckets.setdefault(sig, [Counter() for _ in results])[i][digest] += 1
    paths = {d: p for r in results for _, d, p in r["streams"]}
    varying = 0
    for sig, per_run in buckets.items():
        if any(c != per_run[0] for c in per_run):
            varying += 1
            digests = sorted({d for c in per_run for d in c})
            a, b = (open(paths[d], "rb").read().decode(errors="replace") for d in digests[:2]) if len(digests) > 1 else ("", "")
            problems.append(f"solver bucket {sig[:12]} varies ({len(digests)} streams)\n" + first_difference(a, b, "stream"))
    if not problems and metadata and classifications is not None:
        classifications.extend(metadata)
    return len(buckets), varying, problems


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--classify-consumed-resources", action="store_true",
                    help="ADR 0097: report only known consumed-counter differences separately")
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--out", help="keep captures here (default: a temporary directory)")
    ap.add_argument("--inputs", nargs="*", default=[None])
    ap.add_argument("command", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    command = a.command[1:] if a.command[:1] == ["--"] else a.command
    real = os.environ.get("SAIL_REAL_Z3") or os.environ.get("VERUS_Z3_PATH")
    if not command or a.runs < 2 or not real:
        ap.error("need a COMMAND, --runs >= 2 and the solver in VERUS_Z3_PATH or SAIL_REAL_Z3")
    out = a.out or tempfile.mkdtemp(prefix="verus-determinism-")
    os.makedirs(out, exist_ok=True)
    tee = os.path.join(out, "tee-z3.sh")
    with open(tee, "w") as f:
        f.write(TEE)
    os.chmod(tee, 0o755)
    env = dict(os.environ, VERUS_Z3_PATH=tee, SAIL_DETERMINISM_Z3=os.path.abspath(real))
    failed = 0
    classified_inputs = []
    for n, item in enumerate(a.inputs):
        cmd = [c.replace("{}", item) for c in command] if item else command
        metadata = []
        buckets, varying, problems = check(cmd, a.runs, os.path.join(out, f"input-{n}"), env,
                                          a.classify_consumed_resources, metadata)
        if metadata:
            classified_inputs.append({"input": item, "index": n, "comparisons": metadata})
        status = "varies" if problems else ("stats" if metadata else "same")
        print(f"{status:<7} {item or ' '.join(cmd)}  ({a.runs} runs, {buckets} buckets, {varying} varying)")
        for record in metadata:
            print("  consumed-resource statistics: " + json.dumps(record, sort_keys=True))
        for p in problems:
            print("  " + p.replace("\n", "\n  "))
        failed += bool(problems)
    if a.classify_consumed_resources:
        report = {"schema_version": 1, "policy": "ADR 0097",
                  "inputs": len(a.inputs), "failed": failed,
                  "strict_same": len(a.inputs) - failed - len(classified_inputs),
                  "consumed_resource_only": classified_inputs}
        with open(os.path.join(out, "consumed-resource-classifications.json"), "w") as file:
            json.dump(report, file, indent=2)
        print(f"{len(a.inputs) - failed} of {len(a.inputs)} qualify over {a.runs} runs; "
              f"{report['strict_same']} strict-same, {len(classified_inputs)} consumed-resource-only; captures in {out}")
    else:
        print(f"{len(a.inputs) - failed} of {len(a.inputs)} deterministic over {a.runs} runs; captures in {out}")
    if not a.out and not failed and not classified_inputs:
        shutil.rmtree(out, ignore_errors=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
