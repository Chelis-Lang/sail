#!/usr/bin/env python3
"""Check that raising Verus's --rlimit leaves every solver query unchanged
apart from its rlimit, on given inputs (ADR 0060).

    scripts/rlimit-subsumption.py [--runs K] [--rlimit N] [--out DIR] [--inputs FILE...] -- COMMAND...

COMMAND runs K times (default 3) as given and K times with `--rlimit N`
(default 100) appended, per input (`{}` in COMMAND names it). VERUS_Z3_PATH
becomes a wrapper that records each solver process's input and output. Every
run of both settings must then agree on:
- each solver process's input, after replacing the value of every nonzero
  `(set-option :rlimit V)` with a placeholder (the only bytes allowed to
  differ), with processes matched by their declared names;
- each process's sequence of `sat`/`unsat`/`unknown` answers and of
  `:rlimit-count` values, the solver's own resource counter, so every query
  consumed exactly the same resources under both limits;
- exit code, stdout with wall-clock `times-ms` numbers blanked, and stderr with
  Cargo's elapsed time blanked.
With --unordered (for stock Verus, whose JSON object and timing-array order
and cross-module diagnostic order vary between runs: VI-0007, VI-0008,
VI-0017, VI-0021), JSON documents in stdout are compared with object keys and
arrays sorted, and stderr as the multiset of its diagnostics.

Under the default setting, every query must also have consumed less than the
limit set for it (the counter's growth across its `check-sat`, against the last
`(set-option :rlimit V)` before it); the largest consumption and its limit are
reported per input. Cargo's progress lines, whose order follows Cargo's
parallel scheduling, are left out of stderr. Exit status: 0 if everything agreed, 1 otherwise,
2 on usage errors.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from collections import Counter

TEE = """#!/bin/sh
f=$(mktemp "$SAIL_RLIMIT_DIR/z3-XXXXXXXXXX") || exit 125
tee "$f.in" | "$SAIL_RLIMIT_Z3" "$@" | tee "$f.out"
"""
DECL = re.compile(rb"\((?:declare-fun|declare-const|declare-sort|declare-datatypes \() ?\(?([^\s()]+)")
RLIMIT_SET = re.compile(rb"\(set-option :rlimit ([0-9]+)\)")
COUNT = re.compile(rb":rlimit-count\s+([0-9]+)")
ANSWER = re.compile(rb"^(sat|unsat|unknown)$", re.M)
CARGO_TIME = re.compile(r"^(\s*Finished .*) in [0-9.]+m?s$", re.M)
CARGO_PROGRESS = re.compile(r"^\s*(Compiling|Checking|Fresh|Running|Locking|Blocking|Downloaded|Downloading) .*\n?", re.M)
CHECK = re.compile(rb"\(set-option :rlimit ([0-9]+)\)|\(check-sat\)")


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blank(stdout):
    """Wall-clock numbers of `--time` JSON, as the determinism check blanks
    them; resource counts are kept."""
    spec = importlib.util.spec_from_file_location(
        "determinism", os.path.join(os.path.dirname(os.path.abspath(__file__)), "verus-determinism.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.blank_timing(stdout)


def solver_streams(cap):
    """Per solver process: (signature, normalized input digest, answers and
    counts, largest per-query count growth, smallest nonzero limit, the
    queries that consumed at least their own nonzero limit, and every query's
    (consumption, limit))."""
    streams = []
    for name in sorted(os.listdir(cap)):
        if not name.startswith("z3-") or not name.endswith(".in"):
            continue
        data = open(os.path.join(cap, name), "rb").read()
        out = open(os.path.join(cap, name[:-3] + ".out"), "rb").read()
        limits = [int(v) for v in RLIMIT_SET.findall(data) if int(v)]
        normal = RLIMIT_SET.sub(lambda m: b"(set-option :rlimit " + (b"0" if m.group(1) == b"0" else b"R") + b")", data)
        signature = sha(b"\n".join(sorted(set(DECL.findall(data)))))
        counts = [int(v) for v in COUNT.findall(out)]
        answers = ANSWER.findall(out)
        # Verus reads the counter right before and right after each check-sat
        # (air smt_check_query): a query's consumption is the counter's growth
        # from the last reading before its answer to the first one after it.
        growth, before = [], None
        for m in re.finditer(rb":rlimit-count\s+([0-9]+)|^(?:sat|unsat|unknown)$", out, re.M):
            if m.group(1) is not None:
                if before is not None and before[1]:
                    growth.append(int(m.group(1)) - before[0])
                before = (int(m.group(1)), False)
            elif before is not None:
                before = (before[0], True)
        # The limit each check-sat ran under: the last value set before it.
        query_limits, limit = [], 0
        for m in CHECK.finditer(data):
            if m.group(1) is not None:
                limit = int(m.group(1))
            else:
                query_limits.append(limit)
        over = [(g, l) for g, l in zip(growth, query_limits) if l and g >= l]
        streams.append((signature, sha(normal), (tuple(answers), tuple(counts)),
                        max(growth, default=0), min(limits, default=0), tuple(over),
                        tuple(zip(growth, query_limits))))
    return streams


def canonical_json(value):
    if isinstance(value, dict):
        return {k: canonical_json(v) for k, v in sorted(value.items())}
    if isinstance(value, list):
        return sorted((canonical_json(v) for v in value), key=lambda v: json.dumps(v, sort_keys=True))
    return value


def unordered_stdout(text):
    """Every JSON document with keys and arrays sorted; other text kept."""
    out, i, decoder = [], 0, json.JSONDecoder()
    while i < len(text):
        j = text.find("{", i)
        if j < 0:
            out.append(text[i:])
            break
        out.append(text[i:j])
        try:
            value, end = decoder.raw_decode(text, j)
        except ValueError:
            out.append(text[j])
            i = j + 1
            continue
        out.append(json.dumps(canonical_json(value), sort_keys=True))
        i = end
    return "".join(out)


def unordered_stderr(text):
    """The multiset of diagnostics: blocks starting at `error`/`warning`/`note`."""
    blocks, current = [], []
    for line in text.splitlines():
        if re.match(r"^(error|warning|note)(\[|:)", line) and current:
            blocks.append("\n".join(current))
            current = []
        current.append(line)
    if current:
        blocks.append("\n".join(current))
    return "\n\n".join(sorted(blocks))


def run_once(command, cap, env, unordered):
    os.makedirs(cap)
    p = subprocess.run(command, env=dict(env, SAIL_RLIMIT_DIR=cap), stdin=subprocess.DEVNULL, capture_output=True)
    result = {"exit": p.returncode, "stdout": blank(p.stdout.decode(errors="replace")),
              "stderr": CARGO_PROGRESS.sub("", CARGO_TIME.sub(r"\1 in <time>", p.stderr.decode(errors="replace"))),
              "streams": solver_streams(cap)}
    if unordered:
        result["stdout"] = unordered_stdout(result["stdout"])
        result["stderr"] = unordered_stderr(result["stderr"])
    for field in ("stdout", "stderr"):
        with open(os.path.join(cap, "verus." + field), "w") as f:
            f.write(result[field])
    return result


def check(base, raised, runs, out, env, unordered):
    results = [run_once(c, os.path.join(out, f"{label}-{i}"), env, unordered)
               for label, c in (("default", base), ("raised", raised)) for i in range(runs)]
    problems = []
    for field in ("exit", "stdout", "stderr"):
        if len({r[field] for r in results}) > 1:
            problems.append(f"{field} differs between runs or settings")
    per = [Counter((s[0], s[1], s[2]) for s in r["streams"]) for r in results]
    if any(c != per[0] for c in per):
        problems.append("solver processes differ beyond their rlimit values (input, answers or resource counts)")
    default = [s for r in results[:runs] for s in r["streams"]]
    growth = max((s[3] for s in default), default=0)
    limit = min((s[4] for s in default if s[4]), default=0)
    over = [o for s in default for o in s[5]]
    if over:
        problems.append(f"{len(over)} queries consumed at least their own limit, for example {over[0]}")
    return len(results[0]["streams"]), growth, limit, problems


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--rlimit", default="100")
    ap.add_argument("--unordered", action="store_true", help="compare stock's unordered output as multisets")
    ap.add_argument("--out", help="keep captures here (default: a temporary directory)")
    ap.add_argument("--inputs", nargs="*", default=[None])
    ap.add_argument("command", nargs=argparse.REMAINDER)
    a = ap.parse_args()
    command = a.command[1:] if a.command[:1] == ["--"] else a.command
    real = os.environ.get("SAIL_REAL_Z3") or os.environ.get("VERUS_Z3_PATH")
    if not command or a.runs < 1 or not real:
        ap.error("need a COMMAND, --runs >= 1 and the solver in VERUS_Z3_PATH or SAIL_REAL_Z3")
    out = a.out or tempfile.mkdtemp(prefix="rlimit-subsumption-")
    os.makedirs(out, exist_ok=True)
    tee = os.path.join(out, "tee-z3.sh")
    with open(tee, "w") as f:
        f.write(TEE)
    os.chmod(tee, 0o755)
    env = dict(os.environ, VERUS_Z3_PATH=tee, SAIL_RLIMIT_Z3=os.path.abspath(real))
    failed = 0
    for n, item in enumerate(a.inputs):
        base = [c.replace("{}", item) for c in command] if item else command
        processes, growth, limit, problems = check(base, base + ["--rlimit", a.rlimit], a.runs, os.path.join(out, f"input-{n}"), env, a.unordered)
        status = "differs" if problems else "same"
        print(f"{status:<7} {item or ' '.join(base)}  ({a.runs}+{a.runs} runs, {processes} solver processes, "
              f"largest query {growth}, limit {limit})", flush=True)
        for p in problems:
            print("  " + p)
        failed += bool(problems)
    print(f"{len(a.inputs) - failed} of {len(a.inputs)} unchanged by --rlimit {a.rlimit} over {a.runs}+{a.runs} runs; captures in {out}")
    if not a.out and not failed:
        shutil.rmtree(out, ignore_errors=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
