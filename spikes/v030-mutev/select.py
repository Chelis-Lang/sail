#!/usr/bin/env python3
"""Spike A1 (v0.3.0, #664): pick the mutants to observe.

Usage: select.py GEN_DIR OUT_DIR   (run from the repository root)

GEN_DIR holds `NAME.toml` and `NAME.meta.json` from
`cargo xtask gen-mutants --project P --name NAME --seed 20261020 --per-class 60`
(the checked-in catalogues' seed; a larger --per-class extends them, the
shared ids name the same sites).

Two groups per project, written as one catalogue `OUT_DIR/NAME.toml` plus its
sidecar (the sidecar's `spike_group` records membership):

- `catalogue`: 20 checked-in `kind = "body"` mutants of the classes arith,
  compare, constant, logic and statement: every one whose function the
  generator calls exec, then the others in id order.
- `exec-token`: 20 mutants of the same seed whose changed token is executable
  code (not an assertion, invariant, proof block or spec expression), chosen
  by reading each diff: the first four per class in id order, topped up from
  arith, constant and logic where a class has fewer than four.
"""
import json
import sys
import tomllib
from pathlib import Path

CLASSES = ("arith", "compare", "constant", "logic", "statement")

EXEC_TOKEN = {
    "ironkv": [
        "ironkv-arith-006", "ironkv-arith-013", "ironkv-arith-014", "ironkv-arith-019",
        "ironkv-compare-033", "ironkv-compare-043",
        "ironkv-constant-003", "ironkv-constant-012", "ironkv-constant-024", "ironkv-constant-038",
        "ironkv-logic-011", "ironkv-logic-012", "ironkv-logic-013", "ironkv-logic-021",
        "ironkv-statement-021", "ironkv-statement-043",
        # top-ups (compare and statement have two executable sites each)
        "ironkv-logic-024", "ironkv-logic-025", "ironkv-arith-029", "ironkv-constant-053",
    ],
    "vest": [
        "vest-arith-003", "vest-arith-004", "vest-arith-010", "vest-arith-020",
        "vest-compare-003", "vest-compare-005", "vest-compare-006", "vest-compare-009",
        "vest-constant-002", "vest-constant-004", "vest-constant-005", "vest-constant-011",
        "vest-logic-002", "vest-logic-004", "vest-logic-024", "vest-logic-029",
        "vest-statement-013", "vest-statement-026", "vest-statement-053",
        # top-up (statement has three executable sites that still compile)
        "vest-arith-021",
    ],
}


def blocks(text):
    head, *rest = text.split("[[edit]]\n")
    return head, ["[[edit]]\n" + b for b in rest]


def main():
    gen, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    for name, exec_ids in EXEC_TOKEN.items():
        checked = tomllib.loads(Path(f"corpus/mutants/{name}.toml").read_text())["edit"]
        checked_meta = json.loads(Path(f"corpus/mutants/{name}.meta.json").read_text())["mutants"]
        body = [e["id"] for e in checked
                if e["kind"] == "body" and checked_meta[e["id"]]["class"] in CLASSES]
        exec_mode = [i for i in body if checked_meta[i]["mode"] == "exec"]
        others = [i for i in body if checked_meta[i]["mode"] != "exec"]
        catalogue = (exec_mode + others)[:20]
        meta = json.loads((gen / f"{name}.meta.json").read_text())
        for i in catalogue + exec_ids:
            got, want = meta["mutants"][i], checked_meta.get(i)
            if want and any(got[k] != want[k] for k in ("file", "line", "column", "function")):
                raise SystemExit(f"{i}: the regenerated site differs from the checked-in one")
        chosen = sorted(set(catalogue) | set(exec_ids))
        head, edits = blocks((gen / f"{name}.toml").read_text())
        by_id = {tomllib.loads(b)["edit"][0]["id"]: b for b in edits}
        (out / f"{name}.toml").write_text(head + "".join(by_id[i] for i in chosen))
        meta["mutants"] = {i: meta["mutants"][i] for i in chosen}
        meta["spike_group"] = {
            i: [g for g, ids in (("catalogue", catalogue), ("exec-token", exec_ids)) if i in ids]
            for i in chosen
        }
        (out / f"{name}.meta.json").write_text(json.dumps(meta, indent=2, sort_keys=True) + "\n")
        print(name, len(chosen), "mutants:", len(catalogue), "catalogue,", len(exec_ids),
              "exec-token,", len(set(catalogue) & set(exec_ids)), "in both")


main()
