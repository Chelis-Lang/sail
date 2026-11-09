#!/usr/bin/env python3
"""Spike A1: reads `minimal.py`'s kept runs into one JSON record.

    minimal_report.py OUT_DIR RESULT.json

Per case: for stock, the exit code, the error diagnostics and the root
crate's solver answers (with verbatim `reason-unknown`) of each traced run,
whether the five traced runs agree, and whether the untraced stock run
printed the same bytes; for Sail, the exit code, whether stderr equals
stock's, the labels of every function not reported verified and its checks.
"""
import hashlib
import json
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from analyze import diagnostics, documents, functions  # noqa: E402
from sail_pass import summarise  # noqa: E402


def root_sessions(trace, crate):
    out = []
    for inp in sorted(trace.glob("*.in.smt2")):
        if f"{crate}!".encode() not in inp.read_bytes():
            continue
        stem = str(inp)[: -len(".in.smt2")]
        checks = []
        for line in Path(stem + ".jsonl").read_text().splitlines():
            r = json.loads(line)
            if r.get("type") == "check":
                checks.append([r.get("answer"), r.get("rlimit")])
        reasons = [m.decode() for m in re.findall(rb'\(:reason-unknown "((?:[^"]|"")*)"\)',
                                                  Path(stem + ".out").read_bytes())]
        out.append({"checks": checks, "reason_unknown_replies": reasons,
                    "input_sha256": hashlib.sha256(inp.read_bytes()).hexdigest()})
    return out


def stock(run, crate):
    stderr = (run / "stderr.txt").read_text()
    docs = documents((run / "stdout.txt").read_text())
    fns = functions(docs[-1:])
    r = {
        "exit": int((run / "exit.txt").read_text()),
        "stderr_sha256": hashlib.sha256(stderr.encode()).hexdigest(),
        "errors": [[d["message"], d["spans"]] for d in diagnostics(stderr)
                   if d["level"].startswith("error")],
        "verification_results": docs[-1].get("verification-results") if docs else None,
        "functions": [[f, ok, rl] for _, _, f, ok, rl in fns],
    }
    if (run / "trace").is_dir():
        r["root_sessions"] = root_sessions(run / "trace", crate)
    return r, stderr


def stable(s):
    """A Sail result without wall-clock fields: exit, labels, and each
    check's answer, reason and rlimit use."""
    return [s["exit"], [[t["target"], t["complete"], t["report_exit_code"], t["verus"], [
        [f["function"], f["verdict"], f["establishment"], f["evidence_tier"], f["unstable"],
         (f["timing"] or {}).get("rlimit"),
         [[o["id"], o["outcome"], o["has_counterexample"],
           [[k["answer"], k["reason_unknown"], k["rlimit_limit"], k["rlimit_used"]]
            for k in o["checks"]]] for o in f["obligations"]]]
        for f in t["not_verified"]]] for t in s["targets"]]]


def main():
    out = Path(sys.argv[1])
    result = {"identity": json.loads((out / "identity.json").read_text()), "cases": {}}
    for case in sorted((out / "logs").iterdir()):
        traced = []
        for run in sorted(case.glob("stock-traced-*")):
            r, stderr = stock(run, case.name)
            traced.append(r)
        plain, plain_stderr = stock(case / "stock-plain", case.name)
        sail = []
        for run in sorted(case.glob("sail-*")):
            s = summarise(run)
            s["exit"] = int((run / "exit.txt").read_text())
            s["stderr_equals_stock_plain"] = (run / "stderr.txt").read_text() == plain_stderr
            sail.append(s)
        answers = [json.dumps([[s["checks"], s["reason_unknown_replies"]] for s in r["root_sessions"]])
                   for r in traced]
        result["cases"][case.name] = {
            "stock_traced": traced,
            "stock_plain": plain,
            "stock_stderr": plain_stderr,
            "sail": sail,
            "stock_identical": {
                "exit": len({r["exit"] for r in traced} | {plain["exit"]}) == 1,
                "stderr": len({r["stderr_sha256"] for r in traced} | {plain["stderr_sha256"]}) == 1,
                "functions": len({json.dumps(r["functions"]) for r in traced + [plain]}) == 1,
                "solver_answers": len(set(answers)) == 1,
                "solver_input": len({json.dumps([s["input_sha256"] for s in r["root_sessions"]])
                                     for r in traced}) == 1,
            },
            "sail_identical": len({json.dumps(stable(s), sort_keys=True) for s in sail}) == 1,
        }
    Path(sys.argv[2]).write_text(json.dumps(result, indent=1, sort_keys=True) + "\n")
    for name, c in result["cases"].items():
        t = c["stock_traced"][0]
        print(name, "stock exit", t["exit"], [e[0] for e in t["errors"]],
              [[s["checks"], s["reason_unknown_replies"]] for s in t["root_sessions"]],
              c["stock_identical"], "| sail", [s["exit"] for s in c["sail"]],
              [(f["function"], f["verdict"], f["establishment"], f["evidence_tier"],
                [[(k["answer"], k["reason_unknown"]) for k in o["checks"]] for o in f["obligations"]])
               for s in c["sail"][:1] for t2 in s["targets"] for f in t2["not_verified"]],
              "sail identical", c["sail_identical"],
              "stderr=stock", [s["stderr_equals_stock_plain"] for s in c["sail"]])


main()
