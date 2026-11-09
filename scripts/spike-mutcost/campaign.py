#!/usr/bin/env python3
"""v0.3.0 spike A3+A4: scoped mutant verification and campaign cost.

Hand-scripted prototype, not a product surface. Runs inside the CI image on a
native host (repo at /work, release binaries built), one arm per container so
the container cgroup's CPU counter covers exactly that arm, daemon included.

  campaign.py select  --source SRC --out LIST.json CATALOGUE.toml...
  campaign.py naive   --list LIST.json --root DIR --out OUT   stock full run per mutant
  campaign.py sscoped --list LIST.json --root DIR --out OUT   stock `focus` scoped run per mutant
  campaign.py a3      --list LIST.json --root DIR --out OUT   Sail scoped + Sail full per mutant, 2nd-copy hits
  campaign.py camp    --list LIST.json --root DIR --out OUT [--workers 4]
"""
import argparse
import json
import os
import queue
import subprocess
import sys
import threading
import time
import tomllib

CARGO_ENV_REMOVED = [
    "CARGO", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_TARGET_DIR", "CARGO_ENCODED_RUSTFLAGS", "CARGO_MAKEFLAGS",
    "CARGO_MANIFEST_DIR", "CARGO_PKG_NAME", "CARGO_PRIMARY_PACKAGE", "CARGO_TARGET_DIR", "RUSTC",
    "RUSTC_WORKSPACE_WRAPPER", "RUSTC_WRAPPER", "RUSTDOC", "RUSTFLAGS", "RUSTUP_TOOLCHAIN",
]
TIMEOUT = 1800


# ---------------------------------------------------------------- metering
def cpu_usec():
    for line in open("/sys/fs/cgroup/cpu.stat"):
        k, v = line.split()
        if k == "usage_usec":
            return int(v)
    return 0


def rss_sum_kb():
    total = 0
    for pid in os.listdir("/proc"):
        if pid.isdigit():
            try:
                with open(f"/proc/{pid}/statm") as f:
                    total += int(f.read().split()[1]) * 4
            except OSError:
                pass
    return total


class Meter:
    """Wall, container CPU and peak summed RSS (sampled at 4 Hz) for a phase."""

    def __init__(self):
        self.peak = 0
        self.stop = False
        self.t0 = time.monotonic()
        self.c0 = cpu_usec()
        self.thread = threading.Thread(target=self._sample, daemon=True)
        self.thread.start()

    def _sample(self):
        while not self.stop:
            self.peak = max(self.peak, rss_sum_kb())
            time.sleep(0.25)

    def done(self):
        self.stop = True
        self.thread.join()
        return {
            "wall_s": round(time.monotonic() - self.t0, 3),
            "cpu_s": round((cpu_usec() - self.c0) / 1e6, 3),
            "peak_rss_mb": round(self.peak / 1024, 1),
        }


# ---------------------------------------------------------------- workspace
def prepare(catalogue, root):
    """One prepared working copy, exactly as `sail-bench mutants run` makes it."""
    out = subprocess.run(
        ["/work/target/release/sail-bench", "mutants", "workspace", "--catalogue", catalogue,
         "--vstd-lock", "scripts/shipped-deps/Cargo.lock", "--root", root],
        cwd="/work", capture_output=True, text=True, check=False)
    if out.returncode != 0:
        raise SystemExit(f"workspace {root}: {out.stderr}")
    ws = json.loads(out.stdout.strip().splitlines()[-1])
    if os.environ.get("SPIKE_SAIL"):
        ws["sail"] = os.environ["SPIKE_SAIL"]
    return ws


def base_env(ws):
    env = {k: v for k, v in os.environ.items()
           if k not in CARGO_ENV_REMOVED and not k.startswith(("SAIL_", "VERUS_"))}
    env.update(CARGO_TERM_QUIET="true", CARGO_TERM_COLOR="never", CARGO_TERM_PROGRESS_WHEN="never")
    env.update(ws["env"])
    env["RUSTUP_TOOLCHAIN"] = ws["rust_toolchain"]
    return env


def stock_cmd(ws, scope=None):
    env = base_env(ws)
    env["CARGO_TARGET_DIR"] = os.path.join(ws["root"], "target-stock")
    if not ws["z3_bundled"]:
        env["VERUS_Z3_PATH"] = ws["z3"]
    if scope:
        argv = [ws["cargo_verus"], "verus", "focus", *ws["cargo_args"], "--", *ws["verus_args"],
                *scope, *ws["flags"]]
    else:
        argv = [ws["cargo_verus"], "verus", "verify", *ws["cargo_args"], "--fwd-verus-args-to", "roots",
                "--", *ws["verus_args"], *ws["flags"]]
    return argv, env


def sail_cmd(ws, cache, scope=None):
    env = base_env(ws)
    env.update(SAIL_CACHE_DIR=cache, SAIL_DAEMON_IDLE_TIMEOUT="0", SAIL_EXPLAIN="1",
               CARGO_TARGET_DIR=os.path.join(ws["root"], "target-sail"))
    if ws.get("verus_build"):
        env["SAIL_VERUS_BUILD"] = ws["verus_build"]
    if scope:
        # The managed focus path: JSON callers keep their focused, partial report.
        argv = [ws["sail"], "verify", "--output", "json", *ws["cargo_args"], "--", *ws["verus_args"], *scope]
    else:
        argv = [ws["sail"], "verify", *ws["cargo_args"], "--fwd-verus-args-to", "roots", "--",
                *ws["verus_args"], *ws["flags"]]
    return argv, env


def sail_stats(ws, cache):
    argv, env = sail_cmd(ws, cache)
    out = subprocess.run([ws["sail"], "status", "--json"], env=env, cwd=ws["package"],
                         capture_output=True, text=True, check=False)
    try:
        return json.loads(out.stdout).get("stats") or {}
    except ValueError:
        return {}


def sail_stop(ws, cache):
    _, env = sail_cmd(ws, cache)
    out = subprocess.run([ws["sail"], "status", "--json"], env=env, cwd=ws["package"],
                         capture_output=True, text=True, check=False)
    try:
        pid = (json.loads(out.stdout).get("info") or {}).get("pid")
        if pid:
            os.kill(int(pid), 15)
    except (ValueError, OSError):
        pass


def delta(a, b):
    return {k: b.get(k, 0) - a.get(k, 0) for k in ("gets", "hits", "puts", "hellos")}


def execute(argv, env, cwd, log):
    os.makedirs(log, exist_ok=True)
    t0 = time.monotonic()
    with open(os.path.join(log, "stdout.txt"), "wb") as o, open(os.path.join(log, "stderr.txt"), "wb") as e:
        try:
            p = subprocess.run(argv, env=env, cwd=cwd, stdin=subprocess.DEVNULL, stdout=o, stderr=e,
                               timeout=TIMEOUT, check=False)
            code = p.returncode
        except subprocess.TimeoutExpired:
            code = None
    wall = time.monotonic() - t0
    with open(os.path.join(log, "argv.json"), "w") as f:
        json.dump(argv, f)
    return {"exit": code, "wall_s": round(wall, 3), "log": log}


# ---------------------------------------------------------------- parsing
def json_docs(text):
    dec = json.JSONDecoder()
    docs, i = [], 0
    while True:
        j = text.find("{", i)
        if j < 0:
            break
        # Only documents that start a line.
        if j > 0 and text[j - 1] != "\n":
            i = j + 1
            continue
        try:
            d, end = dec.raw_decode(text[j:])
            docs.append(d)
            i = j + end
        except ValueError:
            i = j + 1
    return docs


def read(log, name):
    with open(os.path.join(log, name), errors="replace") as f:
        return f.read()


def classify_verus(run):
    """Outcome of a run whose stdout carries Verus `--output-json` documents."""
    docs = json_docs(read(run["log"], "stdout.txt"))
    stderr = read(run["log"], "stderr.txt")
    failed, reported = [], 0
    vir = False
    for d in docs:
        r = d.get("verification-results", {})
        vir = vir or bool(r.get("encountered-vir-error"))
        for m in d.get("times-ms", {}).get("smt", {}).get("smt-run-module-times", []):
            for f in m.get("function-breakdown", []):
                reported += 1
                if not f.get("success"):
                    failed.append(f"{m['module']}#{f['function']}")
    if run["exit"] is None:
        outcome = "timeout"
    elif not docs:
        outcome = "stillborn" if run["exit"] != 0 else "not-rebuilt"
    elif vir or any(line.startswith("error[E") for line in stderr.splitlines()):
        outcome = "stillborn"
    elif run["exit"] == 0:
        outcome = "verified"
    else:
        outcome = "errors"
    return {"outcome": outcome, "failed": sorted(failed), "reported": reported}


def classify_sail_json(run):
    """Outcome of a `sail verify --output json` run (project result, schema 2)."""
    text = read(run["log"], "stdout.txt")
    try:
        doc = json.loads(text)
    except ValueError:
        return {"outcome": "no-document", "failed": [], "reported": 0}
    failed, reported, vir, notes = [], 0, False, []
    reports = [t.get("report") or {} for t in doc.get("targets", [])] if "targets" in doc else [doc]
    for rep in reports:
        notes += (rep.get("attribution") or {}).get("notes", [])
        for c in rep.get("crates", []):
            vir = vir or bool((c.get("verus") or {}).get("encountered_vir_error"))
            for f in c.get("functions", []):
                reported += 1
                if f.get("verdict") != "verified":
                    failed.append(f"{f.get('module')}#{f.get('function')}:{f.get('verdict')}")
    stderr = read(run["log"], "stderr.txt")
    if run["exit"] is None:
        outcome = "timeout"
    elif vir or any(line.startswith("error[E") for line in stderr.splitlines()):
        outcome = "stillborn"
    elif reported == 0:
        outcome = "nothing-selected" if run["exit"] == 0 else "stillborn"
    elif run["exit"] == 0 and not failed:
        outcome = "verified"
    else:
        outcome = "errors"
    return {"outcome": outcome, "failed": sorted(failed), "reported": reported,
            "currentness": doc.get("currentness"), "notes": notes[:4]}


# ---------------------------------------------------------------- mutants
def module_of(ws, file):
    """`ironsht/src/a/b.rs` -> `a::b` for the package at `ironsht`."""
    path = os.path.join(ws["src"], file)
    rel = os.path.relpath(path, os.path.join(ws["package"], "src"))
    parts = rel[:-3].split(os.sep)
    if parts[-1] in ("mod", "lib"):
        parts.pop()
    return "::".join(parts)


def scope_of(ws, m):
    module = module_of(ws, m["file"])
    # Friendly names do not identify one impl (`alloc::vec::Vec::serialized_size`
    # is two functions), so select every function in the module ending in the
    # name: a superset of the mutated function.
    return ["--verify-only-module", module, "--verify-function", "*" + m["function"]] if module else \
        ["--verify-root", "--verify-function", "*" + m["function"]]


class Files:
    """Applies one mutant to a copy and restores the original bytes."""

    def __init__(self, ws):
        self.ws = ws
        self.originals = {}

    def apply(self, m):
        path = os.path.join(self.ws["src"], m["file"])
        if path not in self.originals:
            with open(path, encoding="utf-8") as f:
                self.originals[path] = f.read()
        text = self.originals[path]
        if text.count(m["find"]) != 1:
            raise SystemExit(f"{m['id']}: find occurs {text.count(m['find'])} times")
        self._write(path, text.replace(m["find"], m["replace"]))

    def restore(self, m):
        path = os.path.join(self.ws["src"], m["file"])
        self._write(path, self.originals[path])

    @staticmethod
    def _write(path, text):
        tmp = path + ".spike-tmp"
        with open(tmp, "w", encoding="utf-8") as f:
            f.write(text)
        os.replace(tmp, path)


class Out:
    def __init__(self, directory):
        os.makedirs(directory, exist_ok=True)
        self.dir = directory
        self.lock = threading.Lock()
        self.f = open(os.path.join(directory, "samples.jsonl"), "a")

    def write(self, **row):
        with self.lock:
            self.f.write(json.dumps(row) + "\n")
            self.f.flush()
        brief = {k: v for k, v in row.items() if k in ("phase", "id", "worker", "wall_s", "outcome", "meter", "cache")}
        print(json.dumps(brief), flush=True)


# ---------------------------------------------------------------- arms
def cmd_select(a):
    """The mutant list: body mutants in exec functions, stratified by class."""
    picked, seen = [], set()
    want = json.load(open(a.want)) if a.want else None
    for cat in a.catalogues:
        meta = json.load(open(cat[:-5] + ".meta.json"))["mutants"]
        for e in tomllib.load(open(cat, "rb"))["edit"]:
            m = meta.get(e["id"], {})
            if want is not None and e["id"] not in want:
                continue
            if want is None and not (m.get("kind") == "body" and m.get("mode") == "exec"):
                continue
            key = (e["file"], e["find"], e["replace"])
            if key in seen:
                continue
            seen.add(key)
            picked.append({"id": e["id"], "file": e["file"], "find": e["find"], "replace": e["replace"],
                           "description": e["description"], "class": m.get("class", "hand"),
                           "function": m.get("function"), "line": m.get("line"), "mode": m.get("mode"),
                           "catalogue_expected": e.get("expected"), "catalogue": cat})
    if want is None and a.per_class:
        # First N of each class in catalogue order (the generator already
        # orders one-per-function first).
        taken, kept = {}, []
        for p in picked:
            taken[p["class"]] = taken.get(p["class"], 0) + 1
            if taken[p["class"]] <= a.per_class:
                kept.append(p)
        picked = kept
    if want is not None:
        picked.sort(key=lambda p: want.index(p["id"]))
        missing = [w for w in want if w not in {p["id"] for p in picked}]
        if missing:
            raise SystemExit(f"not found: {missing}")
    json.dump({"catalogue": a.catalogues[0], "mutants": picked}, open(a.out, "w"), indent=1)
    print(len(picked), "mutants")


def cmd_naive(a):
    lst = json.load(open(a.list))
    out = Out(a.out)
    total = Meter()
    m0 = Meter()
    ws = prepare(lst["catalogue"], a.root)
    out.write(phase="setup-copy", meter=m0.done())
    argv, env = stock_cmd(ws)
    m0 = Meter()
    run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", "cold"))
    out.write(phase="cold", **run, **classify_verus(run), meter=m0.done())
    files = Files(ws)
    for m in lst["mutants"]:
        files.apply(m)
        m0 = Meter()
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"]))
        out.write(phase="mutant", id=m["id"], **run, **classify_verus(run), meter=m0.done())
        files.restore(m)
    out.write(phase="total", meter=total.done(), mutants=len(lst["mutants"]))


def cmd_sscoped(a):
    lst = json.load(open(a.list))
    out = Out(a.out)
    total = Meter()
    ws = prepare(lst["catalogue"], a.root)
    files = Files(ws)
    first = lst["mutants"][0]
    argv, env = stock_cmd(ws, scope_of(ws, first))
    m0 = Meter()
    run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", "warm-scoped"))
    out.write(phase="warm-scoped", **run, **classify_verus(run), meter=m0.done())
    for m in lst["mutants"]:
        scope = scope_of(ws, m)
        argv, env = stock_cmd(ws, scope)
        # Baseline twin: same copy, same scope flags, unmutated.
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-twin"))
        out.write(phase="twin", id=m["id"], **run, **classify_verus(run))
        files.apply(m)
        m0 = Meter()
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"]))
        out.write(phase="mutant", id=m["id"], scope=scope,
                  **run, **classify_verus(run), meter=m0.done())
        files.restore(m)
    out.write(phase="total", meter=total.done(), mutants=len(lst["mutants"]))


def cmd_a3(a):
    lst = json.load(open(a.list))
    out = Out(a.out)
    cache = os.path.join(a.root, "cache")
    total = Meter()
    ws = prepare(lst["catalogue"], os.path.join(a.root, "A"))
    other = prepare(lst["catalogue"], os.path.join(a.root, "elsewhere", "deeper", "second-copy"))
    # Full baseline in copy A (cold), then in the second copy at another path.
    for name, w in (("full-A", ws), ("full-second-copy", other), ("full-second-copy-again", other)):
        argv, env = sail_cmd(w, cache)
        if name.endswith("again"):
            env["SAIL_MANAGED_FORCE"] = "1"
        before = sail_stats(ws, cache)
        m0 = Meter()
        run = execute(argv, env, w["package"], os.path.join(a.out, "logs", name))
        out.write(phase=name, **run, **classify_verus(run), meter=m0.done(),
                  cache=delta(before, sail_stats(ws, cache)))
    files = Files(ws)
    first = lst["mutants"][0]
    for name, w in (("warm-scoped-A", ws), ("warm-scoped-second-copy", other)):
        argv, env = sail_cmd(w, cache, scope_of(w, first))
        before = sail_stats(ws, cache)
        m0 = Meter()
        run = execute(argv, env, w["package"], os.path.join(a.out, "logs", name))
        out.write(phase=name, **run, **classify_sail_json(run), meter=m0.done(),
                  cache=delta(before, sail_stats(ws, cache)))
    for m in lst["mutants"]:
        scope = scope_of(ws, m)
        argv, env = sail_cmd(ws, cache, scope)
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-twin"))
        out.write(phase="twin", id=m["id"], **run, **classify_sail_json(run))
        files.apply(m)
        before = sail_stats(ws, cache)
        m0 = Meter()
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-scoped"))
        out.write(phase="scoped", id=m["id"], scope=scope, **run, **classify_sail_json(run), meter=m0.done(),
                  cache=delta(before, sail_stats(ws, cache)))
        argv, env = sail_cmd(ws, cache)
        before = sail_stats(ws, cache)
        m0 = Meter()
        run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-full"))
        out.write(phase="full", id=m["id"], **run, **classify_verus(run), meter=m0.done(),
                  cache=delta(before, sail_stats(ws, cache)))
        files.restore(m)
    sail_stop(ws, cache)
    out.write(phase="total", meter=total.done(), mutants=len(lst["mutants"]))


def baseline_costs(run):
    """Baseline SMT micros per (module, friendly function name), summed."""
    costs = {}
    for d in json_docs(read(run["log"], "stdout.txt")):
        for m in d.get("times-ms", {}).get("smt", {}).get("smt-run-module-times", []):
            for f in m.get("function-breakdown", []):
                key = (m["module"], f["function"].rsplit("::", 1)[-1])
                costs[key] = costs.get(key, 0) + f.get("time-micros", 0)
    return costs


def cmd_camp(a):
    lst = json.load(open(a.list))
    out = Out(a.out)
    cache = os.path.join(a.root, "cache")
    k = a.workers
    total = Meter()

    # Setup 1: K owned copies.
    m0 = Meter()
    copies = [prepare(lst["catalogue"], os.path.join(a.root, f"w{i}")) for i in range(k)]
    out.write(phase="setup-copies", meter=m0.done(), workers=k)

    # Setup 2: one unscoped baseline (cold: vstd, dependencies, the project).
    m0 = Meter()
    argv, env = sail_cmd(copies[0], cache)
    base = execute(argv, env, copies[0]["package"], os.path.join(a.out, "logs", "baseline-w0"))
    info = classify_verus(base)
    out.write(phase="setup-baseline", **base, **info, meter=m0.done(), cache=sail_stats(copies[0], cache))
    if info["outcome"] != "verified":
        raise SystemExit("the baseline does not verify")
    costs = baseline_costs(base)

    def cost(m):
        return costs.get((module_of(copies[0], m["file"]), m["function"]), 0)

    plan = sorted(lst["mutants"], key=lambda m: (cost(m), m["id"]))  # cheapest first

    scoped = a.strategy == "scoped"

    # Setup 3: warm every copy. Scoped: the focused layout (a scoped baseline
    # twin); the unscoped layout of copies 1.. is then built by a copy's first
    # confirmation. Unscoped: the unscoped baseline in copies 1.. (copy 0 has it).
    def warm(i):
        if scoped:
            argv, env = sail_cmd(copies[i], cache, scope_of(copies[i], plan[0]))
        elif i == 0:
            return
        else:
            argv, env = sail_cmd(copies[i], cache)
        run = execute(argv, env, copies[i]["package"], os.path.join(a.out, "logs", f"warm-w{i}"))
        info = classify_sail_json(run) if scoped else classify_verus(run)
        out.write(phase="setup-warm", worker=i, **run, **info)

    m0 = Meter()
    before = sail_stats(copies[0], cache)
    threads = [threading.Thread(target=warm, args=(i,)) for i in range(k)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    out.write(phase="setup-warm-all", meter=m0.done(), cache=delta(before, sail_stats(copies[0], cache)))

    # Scoped phase: a shared cheapest-first queue, one mutant at a time per copy.
    todo = queue.Queue()
    for m in plan:
        todo.put(m)
    survivors = []
    lock = threading.Lock()

    def scoped_worker(i):
        ws = copies[i]
        files = Files(ws)
        while True:
            try:
                m = todo.get_nowait()
            except queue.Empty:
                return
            files.apply(m)
            scope = scope_of(ws, m) if scoped else None
            argv, env = sail_cmd(ws, cache, scope)
            run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-scoped"))
            info = classify_sail_json(run) if scoped else classify_verus(run)
            files.restore(m)
            out.write(phase="scoped" if scoped else "unscoped", worker=i, id=m["id"], cost_micros=cost(m),
                      scope=scope, **run, **info)
            if scoped and info["outcome"] in ("verified", "nothing-selected"):
                with lock:
                    survivors.append(m)

    m0 = Meter()
    before = sail_stats(copies[0], cache)
    threads = [threading.Thread(target=scoped_worker, args=(i,)) for i in range(k)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    out.write(phase="mutants-all", meter=m0.done(), cache=delta(before, sail_stats(copies[0], cache)),
              survivors=len(survivors))

    # Confirmation phase: every scoped survivor gets an unscoped run.
    confirm = queue.Queue()
    for m in sorted(survivors, key=lambda m: (cost(m), m["id"])):
        confirm.put(m)

    def confirm_worker(i):
        ws = copies[i]
        files = Files(ws)
        while True:
            try:
                m = confirm.get_nowait()
            except queue.Empty:
                return
            files.apply(m)
            argv, env = sail_cmd(ws, cache)
            run = execute(argv, env, ws["package"], os.path.join(a.out, "logs", m["id"] + "-confirm"))
            info = classify_verus(run)
            files.restore(m)
            out.write(phase="confirm", worker=i, id=m["id"], **run, **info)

    m0 = Meter()
    before = sail_stats(copies[0], cache)
    threads = [threading.Thread(target=confirm_worker, args=(i,)) for i in range(k)]
    [t.start() for t in threads]
    [t.join() for t in threads]
    out.write(phase="confirm-all", meter=m0.done(), cache=delta(before, sail_stats(copies[0], cache)))
    sail_stop(copies[0], cache)
    out.write(phase="total", meter=total.done(), mutants=len(plan), workers=k)


def main():
    p = argparse.ArgumentParser()
    sub = p.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("select")
    s.add_argument("--out", required=True)
    s.add_argument("--want")
    s.add_argument("--per-class", type=int, default=0)
    s.add_argument("catalogues", nargs="+")
    for name in ("naive", "sscoped", "a3", "camp"):
        s = sub.add_parser(name)
        s.add_argument("--list", required=True)
        s.add_argument("--root", required=True)
        s.add_argument("--out", required=True)
        s.add_argument("--workers", type=int, default=4)
        s.add_argument("--strategy", choices=("scoped", "unscoped"), default="scoped")
    a = p.parse_args()
    {"select": cmd_select, "naive": cmd_naive, "sscoped": cmd_sscoped, "a3": cmd_a3, "camp": cmd_camp}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
