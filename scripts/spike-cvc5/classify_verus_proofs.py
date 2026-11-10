#!/usr/bin/env python3
"""SPIKE: classify the Verus-obligation certificate attempts.

  classify_verus_proofs.py OUT_DIR

measure.py's `cert` mode hands Carcara whatever cvc5 printed, including the
one-line `(error "Proof unsupported by Alethe: ...")`, and its plain solve
omits the arguments Verus passes cvc5. This reads the saved outputs again:
what cvc5 actually printed per obligation, and the proof-producing run
against the plain run with the same Verus arguments
(`vq-cvc5.cvc5-1.4.2.json`).
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

out = Path(sys.argv[1])
flag = {r["file"]: r for r in json.loads((out / "vq-cvc5.cvc5-1.4.2.json").read_text())["rows"]}
z3 = {r["file"]: r for r in json.loads((out / "vq-compat.z3.json").read_text())["rows"]}
for name in ("vq-cvc5.cert-dsl", "vq-cvc5.cert-default"):
    rows = json.loads((out / f"{name}.json").read_text())["rows"]
    kinds, real, hole_free = Counter(), [], 0
    for row in rows:
        if "proof_bytes" not in row:
            kinds[f"no proof run (plain answer {row['answer']})"] += 1
            continue
        text = (out / f"{name}.proofs" / row["file"].replace(".smt2", ".alethe")).read_text()
        if "(error" in text[:200]:
            kinds[re.sub(r"term @\w+", "term @..", text.strip().splitlines()[-1])[:90]] += 1
        elif row["proof_bytes"] == 0:
            kinds[f"empty output exit={row['proof_exit']}"] += 1
        else:
            kinds["alethe proof produced"] += 1
            real.append(row)
            hole_free += text.count(":rule hole") == 0
    print("==", name)
    for kind, count in kinds.most_common():
        print(f"  {count:4d} {kind}")
    print(f"  produced proofs with zero `hole` steps: {hole_free} of {len(real)};",
          "carcara verdicts on them:", dict(Counter(r.get("carcara") for r in real)))
    for label, subset in (("all proof runs", [r for r in rows if "proof_seconds" in r]), ("produced proofs", real)):
        subset = [r for r in subset if flag[r["file"]]["answers"] == ["unsat"]]
        plain = sum(flag[r["file"]]["seconds"] for r in subset)
        proof = sum(r["proof_seconds"] for r in subset)
        if subset:
            print(f"  {label}: proof-producing {proof:.1f}s vs same-argument plain {plain:.1f}s"
                  f" = {proof / plain:.2f}x over {len(subset)} obligations")
unsat = [f for f in flag if flag[f]["answers"] == ["unsat"]]
print(f"cvc5 1.4.2 unsat {len(unsat)}: {sum(flag[f]['seconds'] for f in unsat):.1f}s;",
      f"Z3 on the same obligations (cvc5-smtlib bytes): {sum(z3[f]['seconds'] for f in unsat):.1f}s")
