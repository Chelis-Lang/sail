#!/usr/bin/env python3
"""Tables for scripts/perf-real.sh.

    perf-real-report.py OUT_DIR          timing, cache and time-breakdown tables
    perf-real-report.py --det DET_DIR    SMT-log comparison of a `det` run
"""
import os
import statistics
import sys

COLS = ["round", "mode", "phase", "secs", "exit", "sessions", "started", "served",
        "hellos", "gets", "hits", "puts", "total_ms", "rust_ms", "vir_ms",
        "vcrate_ms", "vsum_ms", "smt_ms", "load"]
MODE_ORDER = ["plain", "sail", "snap", "shim", "shim-snap", "incremental", "fork-plain", "fork-sail"]
LABEL = {"plain": "plain", "sail": "sail", "incremental": "sail --incr",
         "fork-plain": "fork plain", "fork-sail": "fork sail", "snap": "sail snapshots", "shim": "shim", "shim-snap": "shim snapshots"}


def fmt(x):
    return f"{x:.2f}" if x < 10 else f"{x:.1f}"


def med(xs):
    return statistics.median(xs) if xs else None


def load(out):
    rows = []
    with open(os.path.join(out, "results.tsv")) as f:
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) != len(COLS):
                continue
            r = dict(zip(COLS, parts))
            for k in COLS[3:-1]:
                r[k] = float(r[k])
            rows.append(r)
    return rows


def report(out):
    rows = load(out)
    modes = [m for m in MODE_ORDER if any(r["mode"] == m for r in rows)]
    phases = []
    for r in rows:
        if r["phase"] not in phases:
            phases.append(r["phase"])
    by = {}
    for r in rows:
        by.setdefault((r["mode"], r["phase"]), []).append(r)

    def col(mode, phase, key):
        return [r[key] for r in by.get((mode, phase), [])]

    rounds = len({r["round"] for r in rows})
    print(f"Wall seconds, median (min-max) of {rounds} round(s); ratio = plain / mode.\n")
    head = "| phase | " + " | ".join(LABEL[m] for m in modes)
    ratios = [m for m in modes if m != "plain" and "plain" in modes]
    head += "".join(f" | plain/{LABEL[m]}" for m in ratios) + " |"
    print(head)
    print("|---" * (1 + len(modes) + len(ratios)) + "|")
    for p in phases:
        cells = []
        for m in modes:
            xs = col(m, p, "secs")
            cells.append(f"{fmt(med(xs))} ({fmt(min(xs))}-{fmt(max(xs))})" if xs else "-")
        for m in ratios:
            a, b = med(col("plain", p, "secs")), med(col(m, p, "secs"))
            cells.append(f"{a / b:.2f}x" if a and b else "-")
        print(f"| {p} | " + " | ".join(cells) + " |")

    cached = [m for m in modes if m in ("sail", "snap", "shim", "shim-snap", "incremental", "fork-sail")]
    if cached:
        print("\nCache, median per run: solver sessions (Z3 processes) through the cache /"
              " sessions that started a solver (a miss), checks served from the cache (daemon"
              " hits), replies re-solved and stored (daemon puts).\n")
        print("| phase | " + " | ".join(f"{LABEL[m]} sessions/missed | {LABEL[m]} hits | {LABEL[m]} puts"
                                         for m in cached) + " |")
        print("|---" * (1 + 3 * len(cached)) + "|")
        for p in phases:
            cells = []
            for m in cached:
                rs = by.get((m, p), [])
                if not rs:
                    cells += ["-", "-", "-"]
                    continue
                sess = med([r["sessions"] for r in rs])
                miss = med([r["gets"] - r["hits"] for r in rs])
                started = med([r["started"] for r in rs])
                missed = started if sess else miss
                cells.append(f"{sess:.0f}/{missed:.0f}" if sess else f"-/{miss:.0f}")
                cells.append(f"{med([r['hits'] for r in rs]):.0f}")
                cells.append(f"{med([r['puts'] for r in rs]):.0f}")
            print(f"| {p} | " + " | ".join(cells) + " |")

    print("\nVerus's own time (--time-expanded), median, summed over the crates verified in"
          " the run. Solver share = smt-run / verify-time (both summed over threads);"
          " solver wall is verify-crate wall times that share; ceiling = wall / (wall -"
          " solver wall).\n")
    print("| mode | phase | wall s | total | rust | vir | verify-crate | solver share | solver wall | ceiling |")
    print("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|")
    for m in modes:
        for p in phases:
            rs = by.get((m, p), [])
            if not rs or not any(r["total_ms"] for r in rs):
                continue
            g = lambda k: med([r[k] for r in rs]) / 1000.0
            share = med([r["smt_ms"] / r["vsum_ms"] for r in rs if r["vsum_ms"]] or [0])
            wall = med([r["secs"] for r in rs])
            sw = g("vcrate_ms") * share
            ceil = wall / (wall - sw) if wall > sw else float("inf")
            print(f"| {LABEL[m]} | {p} | {fmt(wall)} | {fmt(g('total_ms'))} | {fmt(g('rust_ms'))} | "
                  f"{fmt(g('vir_ms'))} | {fmt(g('vcrate_ms'))} | {share:.0%} | {fmt(sw)} | {ceil:.2f}x |")

    loads, running = [], []
    for r in rows:
        a, _, b = r["load"].partition("/")
        try:
            loads.append(float(a))
            running.append(int(b) if b else 0)
        except ValueError:
            pass
    if loads:
        print(f"\nBefore each run (after a settle pause): 1-minute load average median"
              f" {statistics.median(loads):.2f}, max {max(loads):.2f} (it still carries the"
              f" previous run's own work); other processes running: max {max(running)}.")

    bad = []
    for p in phases:
        codes = {(r["round"], r["mode"]): r["exit"] for r in rows if r["phase"] == p}
        for rnd in {k[0] for k in codes}:
            vals = {v for k, v in codes.items() if k[0] == rnd}
            if len(vals) > 1:
                bad.append(f"round {rnd} phase {p}: {sorted(codes.items())}")
    print()
    print("EXIT CODES DIFFER: " + "; ".join(bad) if bad else "exit codes: identical across modes in every round and phase")


def checks(lines):
    return sum(1 for x in lines if x.lstrip().startswith("(check-sat"))


def smt_lines(f):
    """Lines of an SMT log without `;` comment lines, which Verus writes to
    the log (source positions) but which do not change what the solver does."""
    return [x for x in f.read().splitlines() if not x.lstrip().startswith(";")]


def det(d):
    runs = sorted(n[4:] for n in os.listdir(d) if n.startswith("smt-"))
    base_name = "rerun1"
    if base_name not in runs:
        print("no rerun1 logs")
        return
    def files(name):
        root = os.path.join(d, "smt-" + name)
        out = {}
        for dp, _, fs in os.walk(root):
            for f in fs:
                p = os.path.join(dp, f)
                out[os.path.relpath(p, root)] = p
        return out
    base = files(base_name)
    os.makedirs(os.path.join(d, "diffs"), exist_ok=True)
    order = [r for r in runs if r.startswith("rerun")] + [r for r in runs if not r.startswith("rerun")]
    print(f"SMT logs (comment lines removed) compared with {base_name}: files that differ, and checks (check-sat) at or after"
          " each file's first differing line (what a prefix-bound cache must re-solve).\n")
    print("| run | files | differ | only one side | checks | checks after divergence | first differing files |")
    print("|---|---:|---:|---:|---:|---:|---|")
    for run in order:
        if run == base_name:
            continue
        cur = files(run)
        names = sorted(set(base) | set(cur))
        differ, lone, total, after, examples = 0, 0, 0, 0, []
        for n in names:
            if n not in base or n not in cur:
                lone += 1
                if n in cur:
                    with open(cur[n], errors="replace") as f:
                        c = checks(smt_lines(f))
                    total += c
                    after += c
                continue
            with open(base[n], errors="replace") as f:
                a = smt_lines(f)
            with open(cur[n], errors="replace") as f:
                b = smt_lines(f)
            total += checks(b)
            if a == b:
                continue
            differ += 1
            i = next((k for k in range(min(len(a), len(b))) if a[k] != b[k]), min(len(a), len(b)))
            before = checks(b[:i])
            after += checks(b) - before
            examples.append(f"{n} ({before}/{checks(b)})")
            safe = n.replace("/", "_")
            with open(os.path.join(d, "diffs", f"{run}--{safe}.txt"), "w") as f:
                f.write(f"{n}: first difference at line {i + 1}; {before} of {checks(b)} checks before it\n")
                f.write("--- context (rerun1 then this run)\n")
                for k in range(max(0, i - 3), i):
                    f.write("  " + a[k][:400] + "\n")
                for k in range(i, min(len(a), i + 6)):
                    f.write("- " + a[k][:400] + "\n")
                for k in range(i, min(len(b), i + 6)):
                    f.write("+ " + b[k][:400] + "\n")
        ex = ", ".join(examples[:4]) + (" ..." if len(examples) > 4 else "")
        print(f"| {run} | {len(names)} | {differ} | {lone} | {total} | {after} | {ex} |")


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--det":
        det(sys.argv[2])
    elif len(sys.argv) == 2:
        report(sys.argv[1])
    else:
        print(__doc__)
        sys.exit(2)
