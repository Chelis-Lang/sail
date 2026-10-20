#!/usr/bin/env python3
"""Release performance matrix: plain `cargo verus verify` (stock Verus)
against `sail verify` from an installed release archive, in paired, rotated
rounds, with every sample recorded and its output checked against stock's.

    scripts/perf-matrix.py run --out DIR --sail-install DIR --stock-verus DIR
                               [--rounds 3] [--scenario S]... [--project P]...
                               [--machine TEXT] [--tools DIR] [--settle SECONDS]
    scripts/perf-matrix.py report DIR [--markdown FILE]
    scripts/perf-matrix.py list

`--sail-install` is an unpacked release archive (`bin/sail`, `bin/sail-z3`,
`engine/`, `deps/`), installed as the README says; `--stock-verus` holds the
stock release's `cargo-verus`, `verus` and `z3` (`cargo xtask fetch-verus`).
The pinned Rust toolchain (corpus/toolchain.toml) must be installed, with
`rust-src` (README step 2). `--tools` holds `sail-conformance`, which the
real-project rows need to fetch the corpus (default: `target/release`).

Scenarios (`list` prints them with their bars):

- the small project (the `sail new` / `cargo verus new --lib` template), per
  arm and round, in this order: `cold-first` (first use: a fresh Cargo home
  and Sail cache, then `new` and `verify`, timed together), `unchanged`,
  `small-edit` (one new function), `revert` (the template again),
  `fresh-project` (another `new` and `verify`, same Cargo home and cache) and
  `worktree` (`verify` in a new `git worktree` of the first project);
- each real project (vest, ironkv, capybarakv, anvil, verismo), through
  `scripts/perf-real.sh run` with both arms in the round's order: `cold`,
  `unchanged`, `module-edit` (a body edit in one module) and `spec-wide` (a
  spec parameter rename); vest also runs `worktree`.

Round r runs stock first when r is odd and Sail first when it is even. Every
arm of every round starts from fresh copies and an empty Sail cache.

Every sample is appended to `samples.jsonl` as it finishes, failures and
Sail refusals included, and none is ever dropped. A sample passes its
identity check when its exit code and normalized output (verification
summaries and diagnostic headlines and locations, without Cargo status lines
or timings) equal the paired stock sample's. When Cargo answers a stock
rerun without running Verus, Sail may match that no-op or the copy's
previous verifying stock output. Rows are judged when the report is generated: a row
passes only if every sample in it exists, exited as stock did and passed its
identity check, and the medians meet its bar.

Portable to Linux and macOS: Python 3 standard library, git and the tools
`scripts/perf-real.sh` needs (perl, jq).
"""
import argparse
import json
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

SMALL = ["cold-first", "unchanged", "small-edit", "revert", "fresh-project", "worktree"]
REAL = ["cold", "unchanged", "module-edit", "spec-wide"]
PROJECTS = ["vest", "ironkv", "capybarakv", "anvil", "verismo"]
WORKTREE_PROJECTS = {"vest"}
# perf-real.sh phase names for the real-project scenarios.
PERF_REAL_PHASE = {"cold": "cold", "unchanged": "noop1", "module-edit": "body-early",
                   "spec-wide": "spec-wide"}
# Bars (ROADMAP goal and #168): "speedup" needs stock/sail >= the factor;
# "not-slower" allows Sail +5%, or +0.1 s when stock is sub-second.
FIRST_USE = 2.0
REAL_EDIT = 1.3


def bar_of(scenario, project):
    if scenario in ("cold-first", "fresh-project"):
        return ("speedup", FIRST_USE)
    if scenario == "module-edit" and project != "small":
        return ("speedup", REAL_EDIT)
    return ("not-slower", None)


def bar_text(bar):
    kind, factor = bar
    return f">= {factor:g}x" if kind == "speedup" else "<= +5% (+0.1 s sub-second)"


def meets(bar, stock, sail):
    kind, factor = bar
    if kind == "speedup":
        return sail > 0 and stock / sail >= factor
    if stock < 1.0:
        return sail <= stock + 0.1
    return sail <= stock * 1.05


# --- Output normalization ----------------------------------------------------

CARGO_STATUS = re.compile(
    r"^\s*(Compiling|Checking|Finished|Fresh|Running|Blocking|Locking|Adding|Updating|"
    r"Downloaded|Downloading|Unpacking|Packaging|Verifying|Building|Creating|Created)\b")
KEEP = [
    re.compile(r"^verification results:: \d+ verified, \d+ errors"),
    re.compile(r"^(error|warning)(\[[A-Za-z0-9]+\])?: "),
    re.compile(r"^\s*--> \S"),
]
COUNT_LINE = re.compile(r"^(error|warning): .*\b(generated|due to)\b.*\d+ (warning|previous error)")


def project_result(text):
    """`sail verify --output json` (partial selectors) prints one
    sail-project-result document instead of Verus's text summary. Return the
    text without it and each requested target's counts, as Verus prints them."""
    dec = json.JSONDecoder()
    for m in re.finditer(r"(?m)^\{", text):
        try:
            doc, end = dec.raw_decode(text, m.start())
        except ValueError:
            continue
        if isinstance(doc, dict) and doc.get("kind") == "sail-project-result":
            summaries = []
            for t in doc.get("targets", []):
                if t.get("requested"):
                    summ = (t.get("report") or {}).get("summary") or {}
                    summaries.append(f"verification results:: {summ.get('verified', 0)} verified, "
                                     f"{summ.get('failed', 0)} errors")
            return text[:m.start()] + text[end:], summaries
    return text, []


def normalize(text):
    """The comparable part of one run's combined output."""
    text, summaries = project_result(text)
    out = []
    for line in text.splitlines():
        line = line.rstrip()
        if CARGO_STATUS.match(line) or COUNT_LINE.match(line):
            continue
        if any(p.match(line) for p in KEEP):
            out.append(re.sub(r"\s+", " ", line.strip()))
    return out + summaries


def verified(norm):
    return any(l.startswith("verification results::") for l in norm)


# --- Running -----------------------------------------------------------------

def scrubbed_env(extra=None, keep=None):
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("SAIL_", "VERUS_")) and k != "CARGO_TARGET_DIR"}
    env.update(keep or {})
    env.update(extra or {})
    return env


class Runner:
    def __init__(self, args):
        self.out = Path(args.out).resolve()
        self.install = Path(args.sail_install).resolve()
        self.stock = Path(args.stock_verus).resolve()
        self.tools = Path(args.tools).resolve()
        self.settle = args.settle
        self.samples = self.out / "samples.jsonl"
        self.path = os.pathsep.join([str(self.install / "bin"), str(self.stock),
                                     str(self.tools), os.environ.get("PATH", "")])
        self.last_stock = {}
        # Extra variables for both arms (for example SAIL_VERUS_BUILD for a
        # trial with a source build instead of a release archive).
        self.extra = dict(kv.split("=", 1) for kv in (args.env or []))

    def record(self, **sample):
        with open(self.samples, "a") as f:
            f.write(json.dumps(sample, sort_keys=True) + "\n")
        note = f"  {sample['note']}" if sample.get("note") else ""
        print(f"  r{sample['round']} {sample['arm']:5} {sample['project']:10} "
              f"{sample['scenario']:13} {sample['wall']:8.2f} s exit {sample['exit']}{note}",
              flush=True)

    def timed(self, steps, env, log):
        """Run (command, directory) steps in order, timed together."""
        time.sleep(self.settle)
        code = 0
        t0 = time.monotonic()
        with open(log, "w") as f:
            for cmd, cwd in steps:
                p = subprocess.run(cmd, cwd=cwd, env=env, stdout=f, stderr=subprocess.STDOUT,
                                   stdin=subprocess.DEVNULL)
                code = p.returncode
                if code != 0:
                    break
        return time.monotonic() - t0, code

    # Small project ------------------------------------------------------------

    def small_round(self, rnd, arm, scenarios):
        base = self.out / f"r{rnd}" / f"small-{arm}"
        shutil.rmtree(base, ignore_errors=True)
        base.mkdir(parents=True)
        env = scrubbed_env(keep=self.extra, extra={"PATH": self.path, "CARGO_HOME": str(base / "cargo-home"),
                            "SAIL_CACHE_DIR": str(base / "sail-cache"),
                            "GIT_AUTHOR_NAME": "perf", "GIT_AUTHOR_EMAIL": "perf@example.invalid",
                            "GIT_COMMITTER_NAME": "perf", "GIT_COMMITTER_EMAIL": "perf@example.invalid"})
        # A fresh Cargo home still needs the user's rustup proxies (on PATH).
        new = (lambda name: ["sail", "new", "--lib", name]) if arm == "sail" else \
              (lambda name: ["cargo", "verus", "new", "--lib", name])
        verify = ["sail", "verify"] if arm == "sail" else ["cargo", "verus", "verify"]
        demo, demo2 = base / "demo", base / "demo2"
        lib = demo / "src" / "lib.rs"
        template = None
        steps = {
            "cold-first": [(new("demo"), base), (verify, demo)],
            "unchanged": [(verify, demo)],
            "small-edit": [(verify, demo)],
            "revert": [(verify, demo)],
            "fresh-project": [(new("demo2"), base), (verify, demo2)],
            "worktree": [(verify, base / "demo-wt")],
        }
        for scenario in SMALL:
            if scenario not in scenarios:
                continue
            log = base / f"{scenario}.log"
            note = ""
            try:
                if scenario != "cold-first" and not demo.exists():
                    raise RuntimeError("the first project was not created")
                if scenario == "small-edit":
                    template = lib.read_text()
                    marker = "} // verus!"
                    if template.count(marker) != 1:
                        raise RuntimeError("template has no single `} // verus!`")
                    lib.write_text(template.replace(marker, MIN_FN + marker))
                if scenario == "revert":
                    if template is None:
                        raise RuntimeError("revert needs small-edit first")
                    lib.write_text(template)
                if scenario == "worktree":
                    self.git_worktree(demo, base / "demo-wt", env)
                wall, code = self.timed(steps[scenario], env, log)
            except (RuntimeError, OSError, subprocess.SubprocessError) as e:
                wall, code, note = 0.0, None, str(e)
            self.finish(rnd, arm, "small", scenario, wall, code, log, note)

    def git_worktree(self, repo, target, env):
        def git(*a, cwd=repo):
            subprocess.run(["git", *a], cwd=cwd, env=env, check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if not (repo / ".git").exists():
            git("init", "-q")
        git("add", "-A")
        git("commit", "-q", "--allow-empty", "-m", "perf-matrix")
        shutil.rmtree(target, ignore_errors=True)
        git("worktree", "add", "-q", str(target), "HEAD")

    # Real projects --------------------------------------------------------------

    def real_round(self, rnd, project, order, scenarios):
        out = self.out / f"r{rnd}" / f"real-{project}"
        shutil.rmtree(out, ignore_errors=True)
        modes = " ".join("plain" if a == "stock" else "sail" for a in order)
        env = scrubbed_env(keep=self.extra, extra={"PATH": self.path, "SAIL_PERF_OUT": str(out), "SAIL_PERF_MODES": modes,
                            "SAIL_PERF_EDITS": "body-early spec-wide", "SAIL_PERF_NOOPS": "1",
                            "SAIL_PERF_SETTLE": str(self.settle),
                            "SAIL_PERF_MACHINE": os.environ.get("SAIL_PERF_MACHINE", "")})
        log = self.out / f"r{rnd}" / f"perf-real-{project}.log"
        log.parent.mkdir(parents=True, exist_ok=True)
        with open(log, "w") as f:
            p = subprocess.run([str(HERE / "perf-real.sh"), "run", project, "1"], env=env,
                               stdout=f, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        rows = {}
        tsv = out / "results.tsv"
        if tsv.exists():
            for line in tsv.read_text().splitlines():
                c = line.split("\t")
                if len(c) >= 5 and c[0] == "1":
                    rows[(c[1], c[2])] = (float(c[3]), int(c[4]))
        for arm in order:
            mode = "plain" if arm == "stock" else "sail"
            for scenario in REAL:
                if scenario not in scenarios:
                    continue
                phase = PERF_REAL_PHASE[scenario]
                sample_log = out / "r1" / f"{mode}.{phase}.log"
                if (mode, phase) in rows:
                    wall, code = rows[(mode, phase)]
                    note = ""
                else:
                    wall, code, note = 0.0, None, f"perf-real.sh produced no {phase} row (exit {p.returncode})"
                self.finish(rnd, arm, project, scenario, wall, code, sample_log, note)
        if project in WORKTREE_PROJECTS and "worktree" in scenarios:
            for arm in order:
                self.real_worktree(rnd, project, arm, out)

    def real_worktree(self, rnd, project, arm, real_out):
        base = self.out / f"r{rnd}" / f"wt-{project}-{arm}"
        shutil.rmtree(base, ignore_errors=True)
        base.mkdir(parents=True)
        pristine = real_out / "pristine"
        pkg = {"vest": "vest_lib"}[project]
        env = scrubbed_env(keep=self.extra, extra={"PATH": self.path, "SAIL_CACHE_DIR": str(base / "sail-cache"),
                            "GIT_AUTHOR_NAME": "perf", "GIT_AUTHOR_EMAIL": "perf@example.invalid",
                            "GIT_COMMITTER_NAME": "perf", "GIT_COMMITTER_EMAIL": "perf@example.invalid"})
        verify = ["sail", "verify"] if arm == "sail" else ["cargo", "verus", "verify"]
        log = base / "worktree.log"
        note, wall, code = "", 0.0, None
        try:
            if not pristine.is_dir():
                raise RuntimeError("perf-real.sh left no pristine copy")
            shutil.copytree(pristine, base / "main", symlinks=True)
            # The first checkout is verified untimed; the new worktree is timed.
            with open(base / "main.log", "w") as f:
                subprocess.run(verify, cwd=base / "main" / pkg, env=env, stdout=f,
                               stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
            self.git_worktree(base / "main", base / "wt", env)
            wall, code = self.timed([(verify, base / "wt" / pkg)], env, log)
        except (RuntimeError, OSError, subprocess.SubprocessError) as e:
            note = str(e)
        self.finish(rnd, arm, project, "worktree", wall, code, log, note)

    # Identity -------------------------------------------------------------------

    def finish(self, rnd, arm, project, scenario, wall, code, log, note):
        log = Path(log)
        text = log.read_text(errors="replace") if log.exists() else ""
        norm = normalize(text)
        rel = str(log.relative_to(self.out)) if log.is_relative_to(self.out) else str(log)
        sample = dict(round=rnd, arm=arm, project=project, scenario=scenario, wall=wall,
                      exit=code, log=rel, note=note, normalized=norm, reference=norm)
        if arm == "stock":
            seq = (rnd, project)
            if code == 0 and not verified(norm) and scenario in ("unchanged", "revert", "worktree"):
                # Cargo answered without running Verus: compare with this
                # copy's previous verifying stock output.
                sample["reference"] = self.last_stock.get(seq, norm)
                sample["reference_from"] = "previous verifying stock output"
            elif verified(norm):
                self.last_stock[seq] = norm
        self.record(**sample)


MIN_FN = """pub fn min(a: u64, b: u64) -> (r: u64)
    ensures
        r <= a && r <= b,
        r == a || r == b,
{
    if a <= b { a } else { b }
}

"""


def cmd_run(args):
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    # Sail's daemon socket lives under the cache; Unix socket paths are short.
    if len(str(out.resolve() / "r1" / "small-stock" / "sail-cache" / "daemon.sock")) > 100:
        sys.exit(f"--out {out} is too long for Sail's daemon socket; use a shorter path")
    for need in (Path(args.sail_install) / "bin" / "sail", Path(args.stock_verus) / "cargo-verus"):
        if not need.exists():
            sys.exit(f"missing {need}")
    if args.rounds < 3 and not args.allow_fewer_rounds:
        sys.exit("the matrix needs at least 3 rounds (--allow-fewer-rounds for a trial)")
    scenarios = set(args.scenario or SMALL + REAL)
    projects = args.project or ["small"] + PROJECTS
    r = Runner(args)
    header = dict(started_unix=int(time.time()), machine=args.machine or platform.platform(),
                  host=platform.machine(), system=platform.system(), rounds=args.rounds,
                  sail=version([str(Path(args.sail_install) / "bin" / "sail"), "--version"]),
                  stock=version([str(Path(args.stock_verus) / "verus"), "--version"]),
                  scenarios=sorted(scenarios), projects=projects, env=args.env or [])
    (out / "header.json").write_text(json.dumps(header, indent=1) + "\n")
    for rnd in range(1, args.rounds + 1):
        order = ["stock", "sail"] if rnd % 2 == 1 else ["sail", "stock"]
        print(f"round {rnd}: {' then '.join(order)}", flush=True)
        if "small" in projects:
            for arm in order:
                r.small_round(rnd, arm, scenarios)
        for project in projects:
            if project != "small":
                r.real_round(rnd, project, order, scenarios)
    cmd_report(argparse.Namespace(dir=str(out), markdown=str(out / "matrix.md")))


def version(cmd):
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=60).stdout
        return " ".join(l.strip() for l in out.splitlines() if l.strip())[:200] or "unknown"
    except (OSError, subprocess.SubprocessError):
        return "unknown"


# --- Report --------------------------------------------------------------------

def load(dirpath):
    samples = []
    path = Path(dirpath) / "samples.jsonl"
    if path.exists():
        for line in path.read_text().splitlines():
            if line.strip():
                samples.append(json.loads(line))
    return samples


def judge(samples):
    """Mark each sample ok when it ran, and its exit code and normalized
    output equal its round's stock sample's (stock's reference output)."""
    stock = {(s["round"], s["project"], s["scenario"]): s for s in samples if s["arm"] == "stock"}
    sail = {(s["round"], s["project"], s["scenario"]): s for s in samples if s["arm"] == "sail"}
    for key in set(stock) | set(sail):
        st, sa = stock.get(key), sail.get(key)
        # A stock rerun that Cargo answered without Verus matches either
        # the same no-op or that copy's previous verifying output.
        same = bool(st and sa and st["exit"] is not None and st["exit"] == sa["exit"]
                    and sa["normalized"] in (st["normalized"], st["reference"]))
        for s in (st, sa):
            if s is not None:
                s["identity"] = "same" if same else ("unpaired" if not (st and sa) else "different")
                s["ok"] = same and s["exit"] is not None and not s.get("note")
    return samples


def rows_of(samples, rounds):
    judge(samples)
    groups = {}
    for s in samples:
        groups.setdefault((s["project"], s["scenario"]), {}).setdefault(s["arm"], []).append(s)
    order = {p: i for i, p in enumerate(["small"] + PROJECTS)}
    small_sc = {s: i for i, s in enumerate(SMALL)}
    real_sc = {s: i for i, s in enumerate(REAL + ["worktree"])}
    sc = lambda project, scenario: (small_sc if project == "small" else real_sc).get(scenario, 99)
    rows = []
    for (project, scenario), arms in sorted(groups.items(), key=lambda kv: (order.get(kv[0][0], 99), sc(*kv[0]))):
        stock, sail = arms.get("stock", []), arms.get("sail", [])
        bar = bar_of(scenario, project)
        failures = [s for s in stock + sail if not s.get("ok")]
        complete = len(stock) >= rounds and len(sail) >= rounds
        row = dict(project=project, scenario=scenario, n=(len(stock), len(sail)), bar=bar,
                   failures=failures, complete=complete)
        if stock and sail:
            ws, wa = [s["wall"] for s in stock], [s["wall"] for s in sail]
            row.update(stock_med=statistics.median(ws), stock_rng=(min(ws), max(ws)),
                       sail_med=statistics.median(wa), sail_rng=(min(wa), max(wa)))
            row["ratio"] = row["stock_med"] / row["sail_med"] if row["sail_med"] > 0 else 0.0
            row["meets"] = meets(bar, row["stock_med"], row["sail_med"])
        else:
            row["meets"] = False
        row["pass"] = row["meets"] and complete and not failures
        rows.append(row)
    return rows


def cmd_report(args):
    d = Path(args.dir)
    header = json.loads((d / "header.json").read_text()) if (d / "header.json").exists() else {}
    rounds = header.get("rounds", 3)
    rows = rows_of(load(d), rounds)
    lines = [f"# Performance matrix: stock Verus vs Sail", "",
             f"- Machine: {header.get('machine', '?')} ({header.get('system', '?')} {header.get('host', '?')})",
             f"- Sail: `{header.get('sail', '?')}`; stock: `{header.get('stock', '?')}`",
             f"- Rounds: {rounds}, paired and rotated (stock first in odd rounds)", "",
             "Ratio = stock median / Sail median (above 1: Sail faster). Ranges are min-max.", "",
             "| Project | Scenario | Stock median [range] | Sail median [range] | Ratio | n | Bar | Result |",
             "|---|---|---:|---:|---:|---:|---|---|"]
    for r in rows:
        if "stock_med" in r:
            st = f"{r['stock_med']:.2f} s [{r['stock_rng'][0]:.2f}-{r['stock_rng'][1]:.2f}]"
            sa = f"{r['sail_med']:.2f} s [{r['sail_rng'][0]:.2f}-{r['sail_rng'][1]:.2f}]"
            ratio = f"{r['ratio']:.2f}x"
        else:
            st = sa = ratio = "-"
        why = []
        if not r["complete"]:
            why.append("incomplete")
        if r["failures"]:
            why.append(f"{len(r['failures'])} failed sample(s)")
        if "stock_med" in r and not r["meets"]:
            why.append("misses bar")
        result = "pass" if r["pass"] else "**FAIL**" + (f" ({', '.join(why)})" if why else "")
        lines.append(f"| {r['project']} | {r['scenario']} | {st} | {sa} | {ratio} | "
                     f"{r['n'][0]}/{r['n'][1]} | {bar_text(r['bar'])} | {result} |")
    failed = [s for r in rows for s in r["failures"]]
    if failed:
        lines += ["", "Failed samples:", ""]
        for s in failed:
            what = s.get("note") or (f"identity {s.get('identity')}" if s.get("identity") != "same" else f"exit {s['exit']}")
            lines.append(f"- r{s['round']} {s['arm']} {s['project']} {s['scenario']}: {what} (`{s['log']}`)")
    passed = sum(r["pass"] for r in rows)
    lines += ["", f"{passed} of {len(rows)} rows pass."]
    text = "\n".join(lines) + "\n"
    if getattr(args, "markdown", None):
        Path(args.markdown).write_text(text)
    print(text, end="")
    return 0 if rows and passed == len(rows) else 1


def cmd_list(_args):
    for s in SMALL:
        print(f"small      {s:13} {bar_text(bar_of(s, 'small'))}")
    for p in PROJECTS:
        for s in REAL + (["worktree"] if p in WORKTREE_PROJECTS else []):
            print(f"{p:10} {s:13} {bar_text(bar_of(s, p))}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    run = sub.add_parser("run")
    run.add_argument("--out", required=True)
    run.add_argument("--sail-install", required=True)
    run.add_argument("--stock-verus", required=True)
    run.add_argument("--rounds", type=int, default=3)
    run.add_argument("--allow-fewer-rounds", action="store_true")
    run.add_argument("--scenario", action="append", choices=sorted(set(SMALL + REAL)))
    run.add_argument("--project", action="append", choices=["small"] + PROJECTS)
    run.add_argument("--machine")
    run.add_argument("--tools", default=str(ROOT / "target" / "release"))
    run.add_argument("--settle", type=float, default=5.0)
    run.add_argument("--env", action="append", metavar="KEY=VALUE",
                     help="pass a variable to both arms (trials only; recorded in header.json)")
    rep = sub.add_parser("report")
    rep.add_argument("dir")
    rep.add_argument("--markdown")
    sub.add_parser("list")
    a = ap.parse_args()
    if a.cmd == "run":
        return cmd_run(a)
    if a.cmd == "report":
        return cmd_report(a)
    return cmd_list(a)


if __name__ == "__main__":
    sys.exit(main() or 0)
