#!/usr/bin/env python3
"""Solver-side edit cost through sail-z3, with and without solver snapshots.

Prototype evidence for ADR 0049 (draft). Replays Verus solver transcripts
(`<id>.in.smt2` from SAIL_TRACE_DIR) through `sail-z3` against a fresh cache
daemon, the way an edit loop does:

  1. base: every transcript of the unchanged run (fills the cache; with
     snapshots, takes them);
  2. edit: every transcript of the edited run, timed one at a time.

Each edit transcript is paired with the base transcript sharing its longest
prefix. For each edit transcript it records wall time, whether a solver
started, where a snapshot resumed (`snapshot_resumed_at` in the shim's
provenance) and whether the output equals a fresh pinned Z3's on the same
input once `:time`/`:memory`/`:max-memory` are masked (byte-identical
otherwise).

  scripts/snapshot-edit-replay.py --sail DIR --z3 Z3 [--lib LIB] [--gap-ms N]
      --base TRACE_DIR --edit TRACE_DIR --out OUT_DIR [--label L] [--only-changed]

`--lib` enables snapshots (mode `snap`); without it the mode is `off`.
Writes OUT_DIR/<label>.jsonl and prints a summary.
"""
import argparse
import glob
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time

MASK = re.compile(rb"(:(?:time|memory|max-memory)\s+)[0-9.]+")


def masked(b):
    return MASK.sub(rb"\1X", b)


def common_prefix(a, b):
    n = min(len(a), len(b))
    lo, hi = 0, n
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if a[:mid] == b[:mid]:
            lo = mid
        else:
            hi = mid - 1
    return lo


def load(d):
    out = {}
    for p in sorted(glob.glob(os.path.join(d, "**", "*.in.smt2"), recursive=True)):
        with open(p, "rb") as f:
            out[p] = f.read()
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sail", required=True, help="directory with sail and sail-z3")
    ap.add_argument("--z3", required=True)
    ap.add_argument("--lib")
    ap.add_argument("--gap-ms", default="0")
    ap.add_argument("--base", required=True)
    ap.add_argument("--edit", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--label", default=None)
    ap.add_argument("--only-changed", action="store_true",
                    help="time only edit transcripts that differ from their base")
    a = ap.parse_args()
    label = a.label or ("snap" if a.lib else "off")
    os.makedirs(a.out, exist_ok=True)
    work = os.path.join(a.out, "w-" + label)
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    sock = os.path.join(work, "d.sock")
    snapdir = os.path.join(work, "sn")
    prov = os.path.join(work, "prov")
    os.makedirs(prov)
    daemon = subprocess.Popen(
        [os.path.join(a.sail, "sail"), "serve", "--socket", sock, "--cache", os.path.join(work, "cache")],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for _ in range(200):
        if os.path.exists(sock):
            break
        time.sleep(0.05)
    env = {k: v for k, v in os.environ.items() if not k.startswith("SAIL_")}
    env.update(SAIL_Z3_PATH=a.z3, SAIL_DAEMON_SOCKET=sock, SAIL_PROVENANCE_DIR=prov)
    if a.lib:
        env.update(SAIL_SNAPSHOT_LIB=a.lib, SAIL_SNAPSHOT_DIR=snapdir, SAIL_SNAPSHOT_GAP_MS=a.gap_ms,
                   SAIL_SNAPSHOT_TTL_S="3600", SAIL_SNAPSHOT_MAX_PARKED="100000")
    shim = os.path.join(a.sail, "sail-z3")

    def through_shim(data):
        before = set(os.listdir(prov))
        t = time.time()
        p = subprocess.run([shim, "-smt2", "-in"], input=data, capture_output=True, env=env)
        secs = time.time() - t
        rec = {}
        for _ in range(100):
            new = set(os.listdir(prov)) - before
            new = [n for n in new if n.endswith(".json")]
            if new:
                with open(os.path.join(prov, new[0])) as f:
                    rec = json.load(f)
                break
            time.sleep(0.01)
        return p.stdout, p.returncode, secs, rec

    base = load(a.base)
    edit = load(a.edit)
    t = time.time()
    for data in base.values():
        through_shim(data)
    base_secs = time.time() - t
    time.sleep(1.0)
    parked = len([n for n in os.listdir(snapdir) if n.startswith("s") and not n.endswith(".pid")]) \
        if os.path.isdir(snapdir) else 0
    mem_pss = 0
    if sys.platform.startswith("linux") and os.path.isdir(snapdir):
        for n in os.listdir(snapdir):
            if n.endswith(".pid"):
                try:
                    pid = open(os.path.join(snapdir, n)).read().strip()
                    for line in open(f"/proc/{pid}/smaps_rollup"):
                        if line.startswith("Pss:"):
                            mem_pss += int(line.split()[1])
                except OSError:
                    pass
    rows = []
    for path, data in edit.items():
        best, bp = None, -1
        for bpath, bdata in base.items():
            c = common_prefix(bdata, data)
            if c > bp:
                best, bp = bpath, c
        changed = best is None or base[best] != data
        if a.only_changed and not changed:
            continue
        out, code, secs, rec = through_shim(data)
        fresh = subprocess.run([a.z3, "-smt2", "-in"], input=data, capture_output=True)
        rows.append(dict(
            file=path, base=best, common_prefix=bp, bytes=len(data), changed=changed,
            secs=round(secs, 3), exit=code, fresh_exit=fresh.returncode,
            identical=out == fresh.stdout, masked_identical=masked(out) == masked(fresh.stdout),
            solver_started=rec.get("solver_started"), replies_served=rec.get("replies_served"),
            resumed_at=rec.get("snapshot_resumed_at")))
    with open(os.path.join(a.out, label + ".jsonl"), "w") as f:
        for r in rows:
            f.write(json.dumps(r) + "\n")
    tot = sum(r["secs"] for r in rows)
    bad = [r for r in rows if not r["masked_identical"] or r["exit"] != r["fresh_exit"]]
    resumed = [r for r in rows if r["resumed_at"] is not None]
    print(f"{label}: base {len(base)} transcripts in {base_secs:.1f} s; parked {parked} "
          f"(pss {mem_pss} kB); edit {len(rows)} transcripts in {tot:.2f} s; "
          f"resumed {len(resumed)}; output different from fresh z3: {len(bad)}")
    for r in bad:
        print("  DIFF", r["file"])
    daemon.send_signal(signal.SIGTERM)
    daemon.wait(timeout=30)
    shutil.rmtree(snapdir, ignore_errors=True)


if __name__ == "__main__":
    main()
