#!/usr/bin/env python3
"""SPIKE: cross-file summaries of measure.py outputs.

  analyze.py agree A.json B.json...      per-file answer agreement between solve runs
  analyze.py families CERT.json          Carcara verdicts per query family, with error kinds
"""
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path


def kind(row):
    if row["exit"] == "timeout":
        return "timeout"
    if row["errors"] or row["exit"] != 0 or not row["answers"]:
        return "error"
    return "+".join(row["answers"])


def agree(names):
    runs = [{r["file"]: kind(r) for r in json.loads(Path(n).read_text())["rows"]} for n in names]
    files = sorted(runs[0])
    same = sum(1 for f in files if len({run.get(f) for run in runs}) == 1)
    print(f"{same}/{len(files)} files have identical outcomes across {len(names)} runs")
    pairs = Counter(tuple(run.get(f) for run in runs) for f in files)
    for combo, count in pairs.most_common(12):
        print(f"  {count:4d}  {' | '.join(str(c) for c in combo)}")


def families(name):
    rows = json.loads(Path(name).read_text())["rows"]
    table = defaultdict(Counter)
    errors = Counter()
    for row in rows:
        family = re.sub(r"^[a-z]\d+-", "", row["file"]).removesuffix(".smt2")
        if row["answer"] != "unsat":
            table[family][row["answer"]] += 1
            continue
        verdict = row.get("carcara") or "no-proof"
        table[family][verdict] += 1
        if verdict not in ("valid", "holey"):
            log = row.get("carcara_log", "") or row.get("proof_error", "")
            found = re.search(r"with rule '([^']+)'", log)
            message = ("rule " + found.group(1)) if found else re.sub(r"\s+", " ", log)[-110:]
            errors[message] += 1
    for family, counts in sorted(table.items()):
        print(f"  {family:32s} {dict(counts)}")
    for message, count in errors.most_common(12):
        print(f"  not accepted x{count}: {message}")


if __name__ == "__main__":
    if sys.argv[1] == "agree":
        agree(sys.argv[2:])
    else:
        for item in sys.argv[2:]:
            print("==", item)
            families(item)
