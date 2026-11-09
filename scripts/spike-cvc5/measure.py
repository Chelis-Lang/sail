#!/usr/bin/env python3
"""SPIKE: per-file solver answers, Alethe proof production and Carcara checks.

  measure.py solve  --solver z3|cvc5 --bin PATH --out JSON [--extra ARG]... FILES_DIR
  measure.py cert   --cvc5 PATH --carcara PATH --out JSON FILES_DIR
  measure.py report JSON...

Timing is wall time of one fresh process per file, measured sequentially.
Nothing here decides equivalence: every answer is the solver's own stdout.
"""
import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

ENV = {"LC_ALL": "C"}
ANSWER = re.compile(r"^(sat|unsat|unknown)\s*$", re.M)


def run(argv, stdin_path=None, timeout=60):
    start = time.perf_counter()
    try:
        with (open(stdin_path, "rb") if stdin_path else open("/dev/null", "rb")) as src:
            done = subprocess.run(argv, stdin=src, capture_output=True, timeout=timeout, env=ENV)
        code, out, err = done.returncode, done.stdout, done.stderr
    except subprocess.TimeoutExpired as expired:
        code, out, err = "timeout", expired.stdout or b"", expired.stderr or b""
    return {"exit": code, "seconds": time.perf_counter() - start,
            "stdout": out.decode("utf-8", "replace"), "stderr": err.decode("utf-8", "replace")}


def solve(args):
    files = sorted(Path(args.dir).glob("*.smt2"))
    if args.solver == "z3":
        base = [args.bin, "-smt2", "-in", "smt.random_seed=0", "sat.random_seed=0"]
    else:
        base = [args.bin, "--lang=smt2", "-q", "--incremental", "--seed=0", "--sat-random-seed=0"]
    base += args.extra
    rows = []
    for path in files:
        result = run(base, path, args.timeout)
        answers = ANSWER.findall(result["stdout"])
        errors = result["stdout"].count("(error") + result["stdout"].count("unsupported")
        rows.append({"file": path.name, "exit": result["exit"], "seconds": result["seconds"],
                     "answers": answers, "errors": errors,
                     "first_error": next((line for line in result["stdout"].splitlines()
                                          if "(error" in line or line == "unsupported"), "")[:240],
                     "stderr": result["stderr"][:240], "stdout_bytes": len(result["stdout"])})
    Path(args.out).write_text(json.dumps({"mode": "solve", "argv": base, "rows": rows}, indent=1))


def cert(args):
    files = sorted(Path(args.dir).glob("*.smt2"))
    work = Path(args.out).with_suffix(".proofs")
    work.mkdir(parents=True, exist_ok=True)
    plain = [args.cvc5, "--lang=smt2", "-q", "--seed=0", "--sat-random-seed=0"]
    proof = plain + ["--dump-proofs", "--proof-format-mode=alethe"] + args.proof_extra
    rows = []
    for path in files:
        base = run(plain, path, args.timeout)
        answers = ANSWER.findall(base["stdout"])
        row = {"file": path.name, "answer": answers[0] if answers else "error",
               "solve_seconds": base["seconds"]}
        if row["answer"] == "unsat":
            made = run(proof, path, args.timeout)
            text = made["stdout"]
            body = text.split("\n", 1)[1] if text.startswith("unsat") else ""
            # cvc5 wraps the proof in one outer pair of parentheses.
            stripped = body.strip()
            if stripped.startswith("(\n") and stripped.endswith("\n)"):
                stripped = stripped[2:-2]
            proof_path = work / (path.stem + ".alethe")
            proof_path.write_text(stripped + "\n")
            row.update(proof_seconds=made["seconds"], proof_exit=made["exit"],
                       proof_bytes=len(stripped), proof_steps=stripped.count("\n(step "),
                       proof_error=next((line for line in text.splitlines() if "(error" in line), "")[:240])
            if made["exit"] == 0 and stripped:
                checked = run([args.carcara, "check"] + args.carcara_extra + [str(proof_path), str(path)],
                              None, args.timeout)
                verdict = (checked["stdout"].strip().splitlines() or [""])[-1]
                row.update(check_seconds=checked["seconds"], check_exit=checked["exit"],
                           carcara=verdict[:120],
                           carcara_log=(checked["stderr"] or checked["stdout"])[-400:])
        rows.append(row)
    Path(args.out).write_text(json.dumps(
        {"mode": "cert", "plain": plain, "proof": proof, "carcara_extra": args.carcara_extra,
         "rows": rows}, indent=1))


def report(args):
    for name in args.json:
        data = json.loads(Path(name).read_text())
        rows = data["rows"]
        print(f"== {name} ({len(rows)} files)")
        if data["mode"] == "solve":
            kinds = {}
            for row in rows:
                if row["exit"] == "timeout":
                    kind = "timeout"
                elif row["errors"] or row["exit"] != 0 or not row["answers"]:
                    kind = "error"
                else:
                    kind = "+".join(sorted(set(row["answers"])))
                kinds[kind] = kinds.get(kind, 0) + 1
            print("  outcome:", dict(sorted(kinds.items())))
            print("  total seconds: %.2f" % sum(row["seconds"] for row in rows))
            firsts = {}
            for row in rows:
                if row["first_error"]:
                    key = re.sub(r"<stdin>:\d+\.\d+", "<stdin>", row["first_error"])[:110]
                    firsts[key] = firsts.get(key, 0) + 1
            for key, count in sorted(firsts.items(), key=lambda item: -item[1])[:8]:
                print(f"  first error x{count}: {key}")
        else:
            unsat = [row for row in rows if row["answer"] == "unsat"]
            answers = {}
            for row in rows:
                answers[row["answer"]] = answers.get(row["answer"], 0) + 1
            verdicts = {}
            for row in unsat:
                verdict = row.get("carcara") or ("no-proof: " + row.get("proof_error", ""))[:80]
                verdicts[verdict] = verdicts.get(verdict, 0) + 1
            print("  cvc5 answers:", answers)
            print("  carcara verdicts over unsat:", verdicts)
            if unsat:
                solve_s = sum(row["solve_seconds"] for row in unsat)
                proof_s = sum(row.get("proof_seconds", 0) for row in unsat)
                check_s = sum(row.get("check_seconds", 0) for row in unsat)
                print("  unsat solve %.3fs, proof-producing solve %.3fs, carcara %.3fs, "
                      "cost multiple (proof+check)/solve %.2fx"
                      % (solve_s, proof_s, check_s, (proof_s + check_s) / solve_s))
                print("  proof bytes total %d, steps total %d"
                      % (sum(row.get("proof_bytes", 0) for row in unsat),
                         sum(row.get("proof_steps", 0) for row in unsat)))


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("solve")
    s.add_argument("--solver", required=True, choices=["z3", "cvc5"])
    s.add_argument("--bin", required=True)
    s.add_argument("--out", required=True)
    s.add_argument("--extra", action="append", default=[])
    s.add_argument("--timeout", type=float, default=60)
    s.add_argument("dir")
    c = sub.add_parser("cert")
    c.add_argument("--cvc5", required=True)
    c.add_argument("--carcara", required=True)
    c.add_argument("--out", required=True)
    c.add_argument("--proof-extra", action="append", default=[])
    c.add_argument("--carcara-extra", action="append", default=[])
    c.add_argument("--timeout", type=float, default=60)
    c.add_argument("dir")
    r = sub.add_parser("report")
    r.add_argument("json", nargs="+")
    args = parser.parse_args()
    {"solve": solve, "cert": cert, "report": report}[args.cmd](args)


if __name__ == "__main__":
    main()
