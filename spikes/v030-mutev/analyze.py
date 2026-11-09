#!/usr/bin/env python3
"""Spike A1: reads the kept stock runs of each mutant into one JSON record.

    analyze.py --catalogue SPIKE.toml --generated GEN.toml --scratch DIR --out FILE.json

SPIKE.toml is the catalogue the traced oracle rewrote (its sidecar holds the
exit codes of every run and why a mutant was dropped); GEN.toml is the same
catalogue before the oracle (every mutant's find/replace). DIR is the
oracle's scratch: `oracle-logs/<id>/run<k>/{stdout.txt,stderr.txt,trace/}`
and the worker copies `oracle-<w>/`.

Nothing is relabelled: solver answers and `reason-unknown` replies are the
solver's bytes, diagnostics are stock Verus's stderr.
"""
import argparse
import hashlib
import json
import re
import tomllib
from collections import Counter
from pathlib import Path

FN_HEADER = re.compile(
    r"^\s*(pub(\([^)]*\))?\s+)?((open|closed|broadcast|proof|spec|exec|const|unsafe|default|async|"
    r"uninterp|tracked|axiom|extern\s+\"C\")\s+)*fn\s+(\w+)")
SPAN = re.compile(r"^\s*(-->|:::) (.+?):(\d+):(\d+)\s*$")


def documents(text):
    out, dec, i = [], json.JSONDecoder(), 0
    while True:
        while i < len(text) and text[i].isspace():
            i += 1
        if i >= len(text) or text[i] != "{":
            return out
        try:
            doc, i = dec.raw_decode(text, i)
        except ValueError:
            return out
        out.append(doc)


def functions(docs):
    """(module, function, success, rlimit) from every document's --time breakdown."""
    out = []
    for n, doc in enumerate(docs):
        modules = (((doc.get("times-ms") or {}).get("smt") or {}).get("smt-run-module-times")) or []
        for m in modules:
            for f in m.get("function-breakdown") or []:
                out.append((n, m.get("module", ""), f.get("function", ""), bool(f.get("success")),
                            f.get("rlimit", 0)))
    return sorted(out)


def diagnostics(stderr):
    """Every `error`/`warning`/`note` block with its message, its spans
    (`-->` primary, `:::` secondary) and every source line its snippets show
    (`lines`: file and line number, which covers same-file secondary labels,
    which rustc prints without a `:::` line)."""
    blocks, cur, file = [], None, None
    for line in stderr.splitlines():
        m = re.match(r"^(error(\[E\d+\])?|warning|note): (.*)$", line)
        if m:
            cur = {"level": m.group(1), "message": m.group(3), "spans": [], "lines": [],
                   "text": [line]}
            blocks.append(cur)
            file = None
            continue
        if cur is None:
            continue
        cur["text"].append(line)
        s = SPAN.match(line)
        if s:
            file = s.group(2)
            cur["spans"].append([s.group(1), file, int(s.group(3)), int(s.group(4))])
            continue
        g = re.match(r"^\s*(\d+) \|", line)
        if g and file is not None:
            cur["lines"].append([file, int(g.group(1))])
    return blocks


def block_digest(stderr):
    """sha256 of stderr's diagnostic blocks as a multiset: stock prints the
    same blocks in an order that can change between runs."""
    blocks = sorted("\n".join(d["text"]) for d in diagnostics(stderr))
    return hashlib.sha256(json.dumps(blocks).encode()).hexdigest()


def sessions(trace):
    """Per solver session: sha256 of the input bytes, then every check's
    answer, its `reason-unknown` reply (verbatim) and the rlimit in force."""
    out = []
    for log in sorted(trace.glob("*.jsonl")):
        stem = log.name[: -len(".jsonl")]
        inp = trace / f"{stem}.in.smt2"
        digest = hashlib.sha256(inp.read_bytes()).hexdigest() if inp.exists() else None
        records = []
        for line in log.read_text().splitlines():
            try:
                records.append(json.loads(line))
            except ValueError:  # a trace still being written
                records.append({"type": "unreadable"})
        checks = [r for r in records if r.get("type") == "check"]
        end = next((r for r in records if r.get("type") == "end"), {})
        reasons = re.findall(rb'\(:reason-unknown "((?:[^"]|"")*)"\)',
                             (trace / f"{stem}.out").read_bytes())
        reasons = [r.decode("utf-8", "replace") for r in reasons]
        # Stock Verus asks for the reason after each answer that is not unsat.
        asked = [c for c in checks if c.get("answer") != "unsat"]
        paired = len(asked) == len(reasons)
        it = iter(reasons)
        answers = []
        for c in checks:
            reason = next(it, None) if (paired and c.get("answer") != "unsat") else None
            answers.append([c.get("answer"), reason, c.get("rlimit")])
        out.append({"input_sha256": digest, "answers": answers, "reasons_paired": paired,
                    "reasons": None if paired else reasons, "complete": end.get("complete"),
                    "paired": end.get("paired"), "solver_exit": end.get("exit_code")})
    return sorted(out, key=lambda s: (s["input_sha256"] or "", json.dumps(s["answers"])))


def fn_range(lines, name, line):
    """1-based [start, end] of the function `name` that contains `line`:
    from its header to the line before the next function header."""
    start = None
    for i in range(min(line, len(lines)) - 1, -1, -1):
        m = FN_HEADER.match(lines[i])
        if m and m.group(5) == name:
            start = i + 1
            break
    if start is None:
        return None
    end = len(lines)
    for i in range(line, len(lines)):
        if FN_HEADER.match(lines[i]):
            end = i
            break
    return [start, end]


def read_run(run):
    stdout = (run / "stdout.txt").read_text(errors="replace")
    stderr = (run / "stderr.txt").read_text(errors="replace")
    docs = documents(stdout)
    fns = functions(docs)
    results = [d.get("verification-results") for d in docs]
    trace = run / "trace"
    sess = sessions(trace) if trace.is_dir() else None
    return {
        "stderr_sha256": hashlib.sha256(stderr.encode()).hexdigest(),
        "stderr_blocks_sha256": block_digest(stderr),
        "stdout_docs": len(docs),
        "verification_results": results,
        "failing": [f"{n}#{m}::{f}" if m else f"{n}#{f}" for n, m, f, ok, _ in fns if not ok],
        "failing_rlimit": {f: r for _, _, f, ok, r in fns if not ok},
        "function_results": hashlib.sha256(
            json.dumps([(n, m, f, ok) for n, m, f, ok, _ in fns]).encode()).hexdigest(),
        "function_rlimits": hashlib.sha256(json.dumps(fns).encode()).hexdigest(),
        "diagnostics": [{k: v for k, v in d.items() if k != "text"}
                        for d in diagnostics(stderr) if d["level"] != "warning"],
        "sessions": sess,
        "stderr": stderr,
    }


def main():
    ap = argparse.ArgumentParser()
    for a in ("catalogue", "generated", "scratch", "out"):
        ap.add_argument(f"--{a}", required=True, type=Path)
    a = ap.parse_args()
    meta = json.loads(a.catalogue.with_suffix(".meta.json").read_text())
    oracle = meta.get("oracle") or {}
    edits = {e["id"]: e for e in tomllib.loads(a.generated.read_text())["edit"]}
    logs = a.scratch / "oracle-logs"
    originals = {}
    for d in sorted(logs.glob("original-*")):
        r = read_run(d)
        r.pop("stderr")
        originals[d.name] = r
    base_failing = set()
    for r in originals.values():
        base_failing |= set(r["failing"])
    base_sessions = Counter()
    for r in list(originals.values())[:1]:
        for s in r["sessions"] or []:
            base_sessions[s["input_sha256"]] += 1
    src = next(iter(sorted(a.scratch.glob("oracle-*/src"))), None)
    out = {"oracle": {k: v for k, v in oracle.items() if k != "results"}, "originals": originals,
           "spike_group": meta.get("spike_group"), "mutants": {}}
    for mid, info in sorted(meta["mutants"].items()):
        edit = edits[mid]
        rec = {"meta": info, "description": edit["description"],
               "groups": (meta.get("spike_group") or {}).get(mid),
               "oracle": (oracle.get("results") or {}).get(mid),
               "dropped": (oracle.get("dropped") or {}).get(mid), "runs": []}
        rng = None
        if src is not None:
            text = (src / info["file"]).read_text()
            if text.count(edit["find"]) == 1:
                mutated = text.replace(edit["find"], edit["replace"])
                rng = fn_range(mutated.splitlines(), info["function"], info["line"])
        rec["function_lines"] = rng
        for run in sorted((logs / mid).glob("run*")):
            r = read_run(run)
            stderr = r.pop("stderr")
            r["new_failing"] = sorted(set(r["failing"]) - base_failing)
            # Sessions whose input bytes the original run did not send: the
            # ones the mutation changed.
            r["changed_sessions"] = [s for s in (r["sessions"] or [])
                                     if s["input_sha256"] not in base_sessions]
            r["not_unsat"] = sorted(
                Counter(json.dumps(x[:2]) for s in (r["sessions"] or []) for x in s["answers"]
                        if x[0] != "unsat").items())
            sess = r.pop("sessions") or []
            # Stock Verus's solver input is not byte-stable between runs (the
            # fork fixes this, ADR 0039), so replies are compared separately
            # from the inputs: per session, every answer, reason and rlimit.
            r["answers_digest"] = hashlib.sha256(json.dumps(sorted(
                json.dumps([s["answers"], s["complete"], s["paired"], s["solver_exit"]])
                for s in sess)).encode()).hexdigest()
            r["inputs_digest"] = hashlib.sha256(json.dumps(sorted(
                s["input_sha256"] or "" for s in sess)).encode()).hexdigest()
            r["session_count"] = len(sess)
            r["check_count"] = sum(len(s["answers"]) for s in sess)
            r["unpaired_reasons"] = sum(1 for s in sess if not s["reasons_paired"])
            errors = [d for d in r["diagnostics"]
                      if d["level"].startswith("error") and not d["message"].startswith(
                          ("could not compile", "aborting due to"))]
            for d in errors:
                d["in_mutated_function"] = bool(rng) and any(
                    f.endswith(info["file"].split("/", 1)[-1]) and rng[0] <= ln <= rng[1]
                    for f, ln in d["lines"] + [x[1:3] for x in d["spans"]])
                d["primary_in_mutated_function"] = bool(rng) and any(
                    k == "-->" and f.endswith(info["file"].split("/", 1)[-1]) and rng[0] <= ln <= rng[1]
                    for k, f, ln, _ in d["spans"])
            r["errors"] = errors
            del r["diagnostics"]
            if run.name == "run0":
                r["stderr"] = stderr
            rec["runs"].append(r)
        keys = ("stderr_sha256", "stderr_blocks_sha256", "verification_results", "failing",
                "function_results",
                "function_rlimits", "answers_digest", "inputs_digest", "not_unsat")
        rec["identical"] = {k: len({json.dumps(r[k]) for r in rec["runs"]}) <= 1 for k in keys}
        exits = (rec["oracle"] or {}).get("exits")
        rec["identical"]["exit"] = exits is not None and len(set(exits)) <= 1
        out["mutants"][mid] = rec
    a.out.write_text(json.dumps(out, indent=1, sort_keys=True) + "\n")
    print("wrote", a.out, len(out["mutants"]), "mutants")


if __name__ == "__main__":
    main()
