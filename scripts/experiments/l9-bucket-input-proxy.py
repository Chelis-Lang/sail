#!/usr/bin/env python3
"""L9 experiment E1: how many Verus buckets change their pruned VIR after an edit?

This is a proxy for the M8 bucket-memo hit rate. See
docs/reports/rescue/l9-m8-warm-frontend-design.md. For each step of the HumanEval
agent-loop catalogue and the HumanEval repair catalogue, it runs stock Verus with
`--no-verify --build-sst --log vir-poly --log vir-option=no_span`. It hashes each bucket's
pruned-VIR log in two forms:

- raw: the log bytes;
- canonical: `;;` location comments dropped, and every `:id N` token (crate-wide counters
  such as `UnfinalizedReadKind.id`) renumbered by its order-preserving rank within its
  top-level item (item-relative ids; imported and local items number ids independently).

It then reports how many buckets differ from the previous state. Verification and the solver
are not run.

The canonical digest is a proxy, not a key. The log omits the global SCC-order projection,
the datatype/bounds projections and the settings that a real bucket key must bind, and its
printer may omit fields. So it can over-estimate hits. It never establishes correctness.

Usage:
  python3.12 -I scripts/experiments/l9-bucket-input-proxy.py \
      --verus <verus> --project <fetched human-eval-verus tree> --out <scratch dir>
"""

import argparse
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import time
import tomllib

REPO = pathlib.Path(__file__).resolve().parents[2]
ID = re.compile(r":id (\d+)")


def canonical(text):
    """Drop location comments; rank ids within each top-level item (item-relative ids)."""
    items, current = [], []
    for line in text.splitlines():
        if line.lstrip().startswith(";;"):
            continue
        if line.startswith("(") and current:
            items.append(current)
            current = []
        current.append(line)
    if current:
        items.append(current)
    out = []
    for item in items:
        body = "\n".join(item)
        rank = {v: i for i, v in enumerate(sorted({int(m) for m in ID.findall(body)}))}
        out.append(ID.sub(lambda m: f":id {rank[int(m.group(1))]}", body))
    return "\n".join(out)


def run_verus(verus, tree, logdir):
    if logdir.exists():
        shutil.rmtree(logdir)
    cmd = [
        str(verus), "--crate-type=lib", str(tree / "tasks/lib.rs"), "--no-verify", "--build-sst",
        "--log", "vir-poly", "--log", "vir-option=no_span", "--log-dir", str(logdir),
        "--time-expanded",
    ]
    t0 = time.monotonic()
    proc = subprocess.run(cmd, cwd=tree, capture_output=True, text=True)
    wall = time.monotonic() - t0
    raw, canon = {}, {}
    for f in sorted(logdir.glob("*-poly.vir")):
        data = f.read_bytes()
        raw[f.name] = hashlib.sha256(data).hexdigest()
        canon[f.name] = hashlib.sha256(canonical(data.decode()).encode()).hexdigest()
    return {"status": proc.returncode, "wall_s": round(wall, 2), "raw": raw, "canon": canon,
            "timing": [l for l in (proc.stdout + proc.stderr).splitlines() if " ms" in l][:14]}


def changed(a, b):
    return sorted(k for k in set(a) | set(b) if a.get(k) != b.get(k))


def row(scenario, prev, r):
    return {"scenario": scenario, "buckets": len(r["raw"]),
            "changed_raw": changed(prev["raw"], r["raw"]),
            "changed_canonical": changed(prev["canon"], r["canon"]),
            "wall_s": r["wall_s"], "status": r["status"]}


def apply(tree, file, find, replace):
    path = tree / file
    text = path.read_text()
    if text.count(find) != 1:
        raise SystemExit(f"find text not unique in {file}")
    path.write_text(text.replace(find, replace))


def fresh_tree(project, out):
    # One path for every scenario: Verus prints the source path into the log.
    tree = out / "work"
    if tree.exists():
        shutil.rmtree(tree)
    shutil.copytree(project, tree, ignore=shutil.ignore_patterns(".git"))
    return tree


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--verus", required=True, type=pathlib.Path)
    ap.add_argument("--project", required=True, type=pathlib.Path)
    ap.add_argument("--out", required=True, type=pathlib.Path)
    a = ap.parse_args()
    a.out.mkdir(parents=True, exist_ok=True)
    logs = a.out / "logs"
    rows = []

    tree = fresh_tree(a.project, a.out)
    base = run_verus(a.verus, tree, logs / "baseline-1")
    for i in range(2, 6):
        rows.append(row(f"baseline rerun {i - 1} (no edit)", base,
                        run_verus(a.verus, tree, logs / f"baseline-{i}")))

    loops = tomllib.loads((REPO / "corpus/agent-loops.toml").read_text())
    for loop in loops["loop"]:
        if not loop["unit"].startswith("human-eval-verus/"):
            continue
        tree = fresh_tree(a.project, a.out)
        prev = base
        for step in loop["step"]:
            apply(tree, step["file"], step["find"], step["replace"])
            r = run_verus(a.verus, tree, logs / f"{loop['id']}-{step['id']}")
            rows.append(row(f"loop {loop['id']}/{step['id']}", prev, r))
            prev = r

    repair = json.loads((REPO / "corpus/repair-edits.json").read_text())
    for project in repair["projects"]:
        if project["id"] != "human-eval-verus":
            continue
        by_id = {c["id"]: c for c in project["cycles"]}
        for cycle in project["cycles"]:
            tree = fresh_tree(a.project, a.out)
            chain = [cycle]
            while chain[-1].get("from", "original") != "original":
                chain.append(by_id[chain[-1]["from"]])
            for c in reversed(chain[1:]):
                apply(tree, c["file"], c["find"], c["replace"])
            prev = base
            if len(chain) > 1:
                prev = run_verus(a.verus, tree, logs / f"repair-{cycle['id']}-pre")
            apply(tree, cycle["file"], cycle["find"], cycle["replace"])
            r = run_verus(a.verus, tree, logs / f"repair-{cycle['id']}")
            rows.append(row(f"repair {cycle['id']} ({cycle['kind']})", prev, r))

    summary = {"verus": str(a.verus), "baseline_timing": base["timing"], "rows": rows}
    (a.out / "summary.json").write_text(json.dumps(summary, indent=2))
    print(f"{'scenario':<46} {'buckets':>7} {'raw':>4} {'canon':>5}  canonical changes")
    for r in rows:
        names = ",".join(n.removesuffix("-poly.vir") for n in r["changed_canonical"])
        print(f"{r['scenario']:<46} {r['buckets']:>7} {len(r['changed_raw']):>4} "
              f"{len(r['changed_canonical']):>5}  {names[:60]}")


if __name__ == "__main__":
    main()
