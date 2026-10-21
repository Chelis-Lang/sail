#!/usr/bin/env python3
"""Release performance matrix: plain `cargo verus verify` (stock Verus)
against `sail verify` from an installed release archive, in paired, rotated
rounds, with every sample recorded and its output checked against stock's.

    scripts/perf-matrix.py run --out DIR --sail-install DIR --stock-verus DIR
                               [--rounds 3] [--scenario S]... [--project P]...
                               [--machine TEXT] [--tools DIR] [--settle SECONDS]
                               [--vstd registry|source]
    scripts/perf-matrix.py report DIR [--markdown FILE]
    scripts/perf-matrix.py list

`--out` must be outside any Cargo project, so generated projects are independent.
`--sail-install` is an unpacked release archive (`bin/sail`, `bin/sail-z3`,
`engine/`, `deps/`), installed as the README says; `--stock-verus` holds the
stock release's `cargo-verus`, `verus` and `z3` (`cargo xtask fetch-verus`).
The pinned Rust toolchain (corpus/toolchain.toml) must be installed, with
`rust-src` (README step 2). `--tools` holds `sail-conformance`, which the
real-project rows need to fetch the corpus (default: `target/release`).

Scenarios (`list` prints them):

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

Real projects get vstd as a user of the release would (`--vstd registry`,
the default): from crates.io, at the versions of the release's shipped vstd
build (`deps/*/Cargo.lock` in `--sail-install`). A project that pins other
Verus versions, or takes Verus from git, is adjusted to those versions, and
the report lists each adjustment. `--vstd source` keeps the older setup that
patches every project to the pinned Verus source on disk, which the shipped
vstd proof can never match. Every Sail `cold`/`cold-first` run records
whether Sail served the shipped vstd proof (a vstd receipt with a release
`dependency_build`), or the SAIL3138 note that says why not, and the report
shows it per row. Rust targets a project needs (verismo:
x86_64-unknown-none) are added to the pinned toolchain for that project only
and removed after it when the harness added them; while installed they
change the toolchain's identity, so the shipped proof cannot apply, and the
report says so.

Every sample is appended to `samples.jsonl` as it finishes, failures and
Sail refusals included, and none is ever dropped. A sample passes its
identity check when its exit code and normalized output (verification
summaries and diagnostic headlines and locations, without Cargo status lines
or timings, with each arm's own copy directory replaced by `<copy>`) equal
the paired stock sample's. For partial verification (anvil), Sail's JSON
report is compared through the counts its Verus run printed (Verus counts
checks, not functions: a function with a separate loop query counts twice),
after checking them against the report's per-function verdicts. When Cargo answers a stock
rerun without running Verus, Sail may match that no-op or the copy's
previous verifying stock output. A comparison is valid only when each round
has exactly one sample per arm, both exited as stock did and both passed
identity checks. Speed is reported separately under the owner's relative bar;
every slower median requires review, including tiny overheads.

Portable to Linux and macOS: Python 3 standard library, git and the tools
`scripts/perf-real.sh` needs (perl, jq).
"""
import argparse
import hashlib
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

import perf_matrix_mutants

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent

SMALL = ["cold-first", "unchanged", "small-edit", "revert", "fresh-project", "worktree"]
REAL = ["cold", "unchanged", "module-edit", "spec-wide"]
PROJECTS = ["vest", "ironkv", "capybarakv", "anvil", "verismo"]
WORKTREE_PROJECTS = {"vest"}
# perf-real.sh phase names for the real-project scenarios.
PERF_REAL_PHASE = {"cold": "cold", "unchanged": "noop1", "module-edit": "body-early",
                   "spec-wide": "spec-wide"}
def performance(stock, sail):
    """Describe measured direction; release acceptance requires review."""
    if stock <= 0 or sail <= 0:
        return "invalid timing"
    return "faster" if sail < stock else "equal" if sail == stock else "slower: review"


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


PARTIAL_SUFFIX = " (partial verification with `--verify-*`)"
PARTIAL_NOTE = "Requested function/module scope is partial"


def target_summary(target, partial):
    """The `verification results::` line Verus printed for one requested
    target of a sail-project-result, from the counts of Sail's own Verus run
    (`crates[].verus`), after checking them against the report's function
    verdicts. Verus counts checks (one per query: a function whose loop is
    checked separately counts twice), the report's summary counts functions,
    so the two are compared through the obligations, not directly. Any
    inconsistency is returned as an extra line, so the identity check fails."""
    report = target.get("report") or {}
    crates = report.get("crates") or []
    verified = sum(((c.get("verus") or {}).get("verified") or 0) for c in crates)
    errors = sum(((c.get("verus") or {}).get("errors") or 0) for c in crates)
    line = f"verification results:: {verified} verified, {errors} errors" + (PARTIAL_SUFFIX if partial else "")
    functions = [f for c in crates for f in (c.get("functions") or [])]
    proved = sum(1 for f in functions for o in (f.get("obligations") or [])
                 if not o.get("advisory") and o.get("outcome") == "proved")
    failed = ((report.get("summary") or {}).get("failed") or 0)
    problems = []
    if not crates or any("verified" not in (c.get("verus") or {}) for c in crates):
        problems.append("no Verus counts")
    if proved != verified:
        problems.append(f"{proved} proved obligations, Verus counted {verified} verified")
    if (failed > 0) != (errors > 0):
        problems.append(f"{failed} failed functions, Verus counted {errors} errors")
    extra = [f"sail-report-inconsistent: {target.get('target')}: {'; '.join(problems)}"] if problems else []
    return [line] + extra


def project_result(text):
    """`sail verify --output json` (partial selectors) prints one
    sail-project-result document instead of Verus's text summary. Return the
    text without it and each requested target's summary, as Verus prints it."""
    dec = json.JSONDecoder()
    for m in re.finditer(r"(?m)^\{", text):
        try:
            doc, end = dec.raw_decode(text, m.start())
        except ValueError:
            continue
        if isinstance(doc, dict) and doc.get("kind") == "sail-project-result":
            partial = any(str(n).startswith(PARTIAL_NOTE) for n in doc.get("notes") or [])
            summaries = []
            for t in doc.get("targets", []):
                if t.get("requested"):
                    summaries += target_summary(t, partial)
            return text[:m.start()] + text[end:], summaries
    return text, []


def path_forms(paths):
    """Each path as given and fully resolved, longest first."""
    forms = set()
    for p in paths:
        forms.add(str(p))
        forms.add(os.path.realpath(str(p)))
    return sorted((f for f in forms if f and f != os.sep), key=len, reverse=True)


def normalize(text, roots=()):
    """The comparable part of one run's combined output. `roots` are the
    arm's own copy directories: messages that embed them (Cargo's
    future-incompatibility warning names the package's path) differ between
    arms only by that prefix."""
    for root in path_forms(roots):
        text = text.replace(root, "<copy>")
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


# --- The shipped vstd proof -----------------------------------------------------

SAIL3138 = re.compile(r"note\[SAIL3138\]: (.*)")


def vstd_proof(cache, log):
    """Whether a Sail run served the release's shipped vstd proof: `served`
    when a vstd receipt in its cache replays a release dependency build,
    `miss: REASON` with Sail's SAIL3138 note, else `not offered` (for
    example partial verification, which never uses it)."""
    for path in sorted(Path(cache).glob("**/receipts/*.json")):
        try:
            receipt = json.loads(path.read_text())
        except (OSError, ValueError):
            continue
        if not isinstance(receipt, dict) or receipt.get("name") != "vstd":
            continue
        origin = (receipt.get("invocation") or {}).get("dependency_build")
        if isinstance(origin, dict):
            kind = str(origin.get("kind", "?")).lower().replace("-", "_")
            return "served" if kind == "release" else f"served ({kind})"
    try:
        text = Path(log).read_text(errors="replace")
    except OSError:
        text = ""
    m = SAIL3138.search(text)
    if m:
        return f"miss: {m.group(1).strip()}"
    return "not offered"


def lock_packages(text):
    """(name, version, source) of every package in a Cargo.lock."""
    out = []
    for block in text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'(?m)^(name|version|source) = "([^"]*)"', block))
        if "name" in fields:
            out.append((fields["name"], fields.get("version", ""), fields.get("source", "")))
    return out


def verus_versions(lock_text):
    """`name=version` for vstd and every verus_* crate in a shipped lock."""
    return [f"{n}={v}" for n, v, _ in sorted(lock_packages(lock_text))
            if n == "vstd" or n.startswith("verus_")]


def lock_diff(shipped_text, project_text):
    """One line per package of the shipped vstd build whose version the
    project's resolution does not contain (each such difference keeps the
    shipped vstd proof from applying, #238)."""
    shipped = [(n, v) for n, v, src in lock_packages(shipped_text) if src.startswith("registry+")]
    have = {}
    for n, v, src in lock_packages(project_text):
        have.setdefault(n, set()).add(v)
    lines = [f"lock: {n} {', '.join(sorted(have[n]))} (shipped {v})"
             for n, v in shipped if n in have and v not in have[n]]
    return lines or ["lock: vstd's closure resolves to the shipped versions"]


def proof_summary(statuses):
    """`served 3/3`, `miss 3/3`, ... for one row's Sail samples."""
    if not statuses:
        return "-"
    counts = {}
    for st in statuses:
        key = (st or "unrecorded").split(":")[0].split(" (")[0]
        counts[key] = counts.get(key, 0) + 1
    return ", ".join(f"{k} {n}/{len(statuses)}" for k, n in sorted(counts.items()))


# --- Rust targets --------------------------------------------------------------

# Rust targets each project needs on the pinned toolchain (corpus `requires`).
RUST_TARGETS = {"verismo": ["x86_64-unknown-none"]}


def pinned_toolchain():
    m = re.search(r'(?m)^rust_toolchain = "([^"]+)"', (ROOT / "corpus" / "toolchain.toml").read_text())
    return m.group(1) if m else None


def installed_targets(toolchain):
    p = subprocess.run(["rustup", "target", "list", "--installed", "--toolchain", toolchain],
                       capture_output=True, text=True)
    if p.returncode != 0:
        raise RuntimeError(f"rustup target list failed: {p.stderr.strip()}")
    return set(p.stdout.split())


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
        self.vstd = args.vstd
        self.vstd_lock = shipped_lock(self.install) if args.vstd == "registry" else None
        self.toolchain = pinned_toolchain()
        if self.toolchain:
            self.extra.setdefault("RUSTUP_TOOLCHAIN", self.toolchain)

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
            extra = {}
            if arm == "sail" and scenario == "cold-first":
                extra["vstd_proof"] = vstd_proof(base / "sail-cache", log)
            self.finish(rnd, arm, "small", scenario, wall, code, log, note, roots=[base], **extra)

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
                            "SAIL_PERF_MACHINE": os.environ.get("SAIL_PERF_MACHINE", ""),
                            "SAIL_PERF_VSTD": self.vstd,
                            "SAIL_PERF_STOCK_HOME": str(self.stock.parent),
                            # No Verus flags after `--`: cargo-verus forwards
                            # them to vstd too, which a user's run does not do.
                            "SAIL_PERF_VERUS_TIMES": "0"})
        if self.vstd_lock:
            env["SAIL_PERF_VSTD_LOCK"] = str(self.vstd_lock)
        log = self.out / f"r{rnd}" / f"perf-real-{project}.log"
        log.parent.mkdir(parents=True, exist_ok=True)
        added, targets_note = [], ""
        try:
            added = self.add_targets(project)
        except (RuntimeError, OSError, subprocess.SubprocessError) as e:
            targets_note = f"rust targets: {e}"
        try:
            with open(log, "w") as f:
                p = subprocess.run([str(HERE / "perf-real.sh"), "run", project, "1"], env=env,
                                   stdout=f, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL)
        finally:
            self.remove_targets(added)
        setup = dict(rust_targets=RUST_TARGETS.get(project, []), vstd_mode=self.vstd,
                     vstd_adjustments=read_lines(out / "vstd-adjustments.txt"),
                     vstd_setup=read_lines(out / "vstd-setup.txt"))
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
                    note = targets_note
                else:
                    wall, code, note = 0.0, None, f"perf-real.sh produced no {phase} row (exit {p.returncode})"
                extra = dict(setup)
                if arm == "sail":
                    proof = read_lines(out / "r1" / f"{mode}.vstd-proof.txt")
                    extra["vstd_proof"] = proof[0] if proof else "unrecorded"
                self.finish(rnd, arm, project, scenario, wall, code, sample_log, note,
                            roots=[out / "r1" / f"{mode}-proj"], **extra)
        if project in WORKTREE_PROJECTS and "worktree" in scenarios:
            for arm in order:
                self.real_worktree(rnd, project, arm, out)

    def add_targets(self, project):
        """Add the project's Rust targets to the pinned toolchain; return
        those this call added (removed again after the project)."""
        need = RUST_TARGETS.get(project, [])
        if not need:
            return []
        if not self.toolchain:
            raise RuntimeError("no rust_toolchain in corpus/toolchain.toml")
        have = installed_targets(self.toolchain)
        added = []
        for t in need:
            if t not in have:
                subprocess.run(["rustup", "target", "add", "--toolchain", self.toolchain, t], check=True,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                added.append(t)
        return added

    def remove_targets(self, added):
        for t in added:
            subprocess.run(["rustup", "target", "remove", "--toolchain", self.toolchain, t],
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

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
        self.finish(rnd, arm, project, "worktree", wall, code, log, note, roots=[base / "main", base / "wt"])

    # Identity -------------------------------------------------------------------

    def finish(self, rnd, arm, project, scenario, wall, code, log, note, roots=(), **extra):
        log = Path(log)
        text = log.read_text(errors="replace") if log.exists() else ""
        norm = normalize(text, roots)
        rel = str(log.relative_to(self.out)) if log.is_relative_to(self.out) else str(log)
        sample = dict(round=rnd, arm=arm, project=project, scenario=scenario, wall=wall,
                      exit=code, log=rel, note=note, normalized=norm, reference=norm, **extra)
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


def read_lines(path):
    try:
        return [l for l in Path(path).read_text().splitlines() if l.strip()]
    except OSError:
        return []


def shipped_lock(install):
    """The Cargo.lock of the release's shipped vstd build."""
    locks = sorted((Path(install) / "deps").glob("*/Cargo.lock"))
    if len(locks) != 1:
        sys.exit(f"--vstd registry needs exactly one {install}/deps/*/Cargo.lock (found {len(locks)}); "
                 "use --vstd source for a release without a shipped vstd proof")
    return locks[0].resolve()


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
    if any((parent / "Cargo.toml").is_file() for parent in [out.resolve(), *out.resolve().parents]):
        sys.exit("--out must be outside a Cargo project so fresh projects form independent workspaces")
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
    if any(project != "small" for project in projects):
        for tool in ("sail-conformance", "jq", "perl", "git"):
            if not shutil.which(tool, path=r.path):
                sys.exit(f"missing {tool}; build the release tools or add it to PATH before real-project runs")
    header = dict(started_unix=int(time.time()), machine=args.machine or platform.platform(),
                  host=platform.machine(), system=platform.system(), rounds=args.rounds,
                  sail=version([str(Path(args.sail_install) / "bin" / "sail"), "--version"]),
                  stock=version([str(Path(args.stock_verus) / "verus"), "--version"]),
                  scenarios=sorted(scenarios), projects=projects, env=args.env or [],
                  platform=("aarch64-apple-darwin" if platform.system() == "Darwin" and platform.machine() in ("arm64", "aarch64")
                            else platform.machine() + "-unknown-linux-gnu"),
                  sail_sha256=file_sha256(r.install / "bin/sail"),
                  stock_cargo_verus_sha256=file_sha256(r.stock / "cargo-verus"),
                  runtime_manifest_sha256=file_sha256(runtime_manifest(r.install, r.extra)),
                  corpus_sha256=file_sha256(ROOT / "corpus/corpus.toml"),
                  vstd_lock_sha256=file_sha256(r.vstd_lock),
                  required_mutations=(["human-eval", "ironkv", "vest", "capybarakv"] if not args.project
                                      else [p for p in projects if p in ("ironkv", "vest", "capybarakv")]),
                  vstd=args.vstd, vstd_lock=str(r.vstd_lock) if r.vstd_lock else None,
                  vstd_versions=verus_versions(r.vstd_lock.read_text()) if r.vstd_lock else [],
                  rust_toolchain=r.toolchain, cargo_toolchain=r.extra.get("RUSTUP_TOOLCHAIN"),
                  rust_targets={p: RUST_TARGETS[p] for p in projects if p in RUST_TARGETS})
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
    return cmd_report(argparse.Namespace(dir=str(out), markdown=str(out / "matrix.md")))


def version(cmd):
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=60).stdout
        return " ".join(l.strip() for l in out.splitlines() if l.strip())[:200] or "unknown"
    except (OSError, subprocess.SubprocessError):
        return "unknown"


def file_sha256(path):
    if path is None:
        return None
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def runtime_manifest(install, extra):
    if extra.get("SAIL_VERUS_BUILD"):
        return Path(extra["SAIL_VERUS_BUILD"]) / "sail-build.json"
    manifests = list((Path(install) / "engine").glob("*/sail-build.json"))
    return manifests[0] if len(manifests) == 1 else None


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


def rows_of(samples, rounds, expected=()):
    judge(samples)
    groups = {}
    for s in samples:
        groups.setdefault((s["project"], s["scenario"]), {}).setdefault(s["arm"], []).append(s)
    for key in expected:
        groups.setdefault(key, {})
    order = {p: i for i, p in enumerate(["small"] + PROJECTS)}
    small_sc = {s: i for i, s in enumerate(SMALL)}
    real_sc = {s: i for i, s in enumerate(REAL + ["worktree"])}
    sc = lambda project, scenario: (small_sc if project == "small" else real_sc).get(scenario, 99)
    rows = []
    for (project, scenario), arms in sorted(groups.items(), key=lambda kv: (order.get(kv[0][0], 99), sc(*kv[0]))):
        stock, sail = arms.get("stock", []), arms.get("sail", [])
        failures = [s for s in stock + sail if not s.get("ok")]
        expected = list(range(1, rounds + 1))
        complete = all(sorted(s["round"] for s in arm) == expected for arm in (stock, sail))
        row = dict(project=project, scenario=scenario, n=(len(stock), len(sail)),
                   failures=failures, complete=complete,
                   vstd_proof=proof_summary([s["vstd_proof"] for s in sail if "vstd_proof" in s]))
        if stock and sail:
            ws, wa = [s["wall"] for s in stock], [s["wall"] for s in sail]
            row.update(stock_med=statistics.median(ws), stock_rng=(min(ws), max(ws)),
                       sail_med=statistics.median(wa), sail_rng=(min(wa), max(wa)))
            row["ratio"] = row["stock_med"] / row["sail_med"] if row["sail_med"] > 0 else 0.0
            row["performance"] = performance(row["stock_med"], row["sail_med"])
            row["delta_s"] = row["sail_med"] - row["stock_med"]
        else:
            row["performance"] = "unmeasured"
        row["valid"] = complete and not failures and row["performance"] != "invalid timing"
        row["pass"] = row["valid"] and row["performance"] in ("faster", "equal")
        rows.append(row)
    return rows


def missing_mutation_row(project, reason):
    return dict(project=project, scenario="mutation-loop", n=(0, 0), complete=False,
                failures=[], vstd_proof="-", valid=False, performance="unmeasured",
                mutation_issues=[reason], **{"pass": False})


def cmd_report(args):
    d = Path(args.dir)
    header = json.loads((d / "header.json").read_text()) if (d / "header.json").exists() else {}
    rounds = header.get("rounds", 3)
    expected = []
    for project in header.get("projects", []):
        offered = SMALL if project == "small" else REAL + (["worktree"] if project in WORKTREE_PROJECTS else [])
        expected.extend((project, scenario) for scenario in offered
                        if scenario in header.get("scenarios", offered))
    rows = rows_of(load(d), rounds, expected)
    mutation_rows = []
    for directory in getattr(args, "mutants", None) or []:
        try:
            mutation_rows.append(perf_matrix_mutants.analyse(directory, header))
        except (OSError, ValueError, TypeError, KeyError) as error:
            mutation_rows.append(missing_mutation_row(Path(directory).name, str(error)))
    for project in header.get("required_mutations", []):
        if not any(row["project"] == project for row in mutation_rows):
            mutation_rows.append(missing_mutation_row(project, "realistic mutation results not attached"))
    rows += mutation_rows
    lines = [f"# Performance matrix: stock Verus vs Sail", "",
             f"- Machine: {header.get('machine', '?')} ({header.get('system', '?')} {header.get('host', '?')})",
             f"- Sail: `{header.get('sail', '?')}`; stock: `{header.get('stock', '?')}`",
             f"- Rounds: {rounds}, paired and rotated (stock first in odd rounds)",
             f"- vstd: {vstd_text(header)}", "",
             "Ratio = stock median / Sail median (above 1: Sail faster). Ranges are min-max. "
             "Shipped vstd: whether Sail's first run of the copy (`cold`, `cold-first`) served the "
             "release's preverified vstd proof; later rows of a copy follow from it.", "",
             "The release bar is relative (owner, 2026-10-20): most scenarios faster, modest overhead "
             "only on tiny cases, no real slowdown. Every slower median requires review of samples "
             "and variability; this report does not grant release acceptance.", "",
             "| Project | Scenario | Stock median [range] | Sail median [range] | Ratio | n | Delta | Shipped vstd | Result |",
             "|---|---|---:|---:|---:|---:|---|---|---|"]
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
        why.extend(r.get("mutation_issues", [])[:3])
        if not r["valid"]:
            result = "**INVALID**" + (f" ({', '.join(why)})" if why else "")
        else:
            result = r["performance"]
            if r.get("slower_mutants"):
                result += f" ({len(r['slower_mutants'])} edits slower: review)"
        delta = f"{r['delta_s']:+.2f} s" if "delta_s" in r else "-"
        scenario = "scripted-" + r["scenario"] if r["scenario"] in ("module-edit", "spec-wide") else r["scenario"]
        lines.append(f"| {r['project']} | {scenario} | {st} | {sa} | {ratio} | "
                     f"{r['n'][0]}/{r['n'][1]} | {delta} | {r['vstd_proof']} | {result} |")
    lines += setup_notes(load(d))
    lines += ["", "Scripted edits are diagnostic. Edit-loop claims require the corrected mutation suite; "
              "mutation-loop times are totals for the full set of edits per round."]
    for row in mutation_rows:
        if row.get("mutation_source"):
            lines += ["", f"Mutation details ({row['project']}): `{row['mutation_source']}/report.md`."]
        if row.get("slower_mutants"):
            lines += ["", f"Slower edits requiring review ({row['project']}):", ""]
            for edit, stock, sail in row["slower_mutants"]:
                lines.append(f"- {edit}: stock {stock:.3f} s, Sail {sail:.3f} s ({sail - stock:+.3f} s)")
    failed = [s for r in rows for s in r["failures"]]
    if failed:
        lines += ["", "Failed samples:", ""]
        for s in failed:
            what = s.get("note") or (f"identity {s.get('identity')}" if s.get("identity") != "same" else f"exit {s['exit']}")
            lines.append(f"- r{s['round']} {s['arm']} {s['project']} {s['scenario']}: {what} (`{s['log']}`)")
    valid = sum(r["valid"] for r in rows)
    review = sum(r["valid"] and not r["pass"] for r in rows)
    lines += ["", f"{valid} of {len(rows)} rows have valid comparisons; {review} slower rows require review."]
    text = "\n".join(lines) + "\n"
    if getattr(args, "markdown", None):
        Path(args.markdown).write_text(text)
    print(text, end="")
    if not rows or valid != len(rows):
        return 1
    return 2 if review else 0


def vstd_text(header):
    mode = header.get("vstd")
    if mode == "registry":
        versions = ", ".join(header.get("vstd_versions") or []) or "?"
        return f"crates.io at the shipped build's versions ({versions})"
    if mode == "source":
        return "patched to the pinned Verus source (the shipped vstd proof cannot apply)"
    return "not recorded (before `--vstd`)"


def setup_notes(samples):
    """Per project: Verus dependency adjustments, lock differences from the
    shipped build, Rust targets and the reasons for shipped-proof misses."""
    lines = []
    by_project = {}
    for s in samples:
        by_project.setdefault(s["project"], []).append(s)
    for project, ss in by_project.items():
        notes = []
        first = next((s for s in ss if "vstd_mode" in s), None)
        if first:
            notes += [f"adjusted {a}" for a in first.get("vstd_adjustments") or []]
            notes += first.get("vstd_setup") or []
            if first.get("rust_targets"):
                notes.append(f"Rust targets {', '.join(first['rust_targets'])} installed on the pinned "
                              "toolchain for this project: they change the compiler identity, so the "
                              "shipped vstd proof cannot apply")
        misses = sorted({s["vstd_proof"] for s in ss if str(s.get("vstd_proof", "")).startswith("miss")})
        notes += misses
        if notes:
            lines += ["", f"{project} setup:", ""] + [f"- {n}" for n in notes]
    return lines


def cmd_list(_args):
    for s in SMALL:
        print(f"small      {s:13}")
    for p in PROJECTS:
        for s in REAL + (["worktree"] if p in WORKTREE_PROJECTS else []):
            print(f"{p:10} {s:13}")


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
    rep.add_argument("--mutants", action="append", metavar="DIR",
                     help="attach a realistic mutation run from the same candidate; repeat for each project")
    sub.add_parser("list")
    # Helpers for scripts/perf-real.sh.
    vv = sub.add_parser("vstd-versions")
    vv.add_argument("lock")
    ld = sub.add_parser("lock-diff")
    ld.add_argument("shipped")
    ld.add_argument("project")
    vp = sub.add_parser("vstd-proof")
    vp.add_argument("cache")
    vp.add_argument("log")
    run.add_argument("--vstd", choices=["registry", "source"], default="registry",
                     help="real projects' vstd: crates.io at the shipped build's versions (default), "
                          "or the pinned Verus source (the older setup)")
    a = ap.parse_args()
    if a.cmd == "run":
        return cmd_run(a)
    if a.cmd == "report":
        return cmd_report(a)
    if a.cmd == "vstd-versions":
        print("\n".join(verus_versions(Path(a.lock).read_text())))
        return 0
    if a.cmd == "lock-diff":
        project = Path(a.project).read_text() if Path(a.project).exists() else ""
        print("\n".join(lock_diff(Path(a.shipped).read_text(), project)))
        return 0
    if a.cmd == "vstd-proof":
        print(vstd_proof(a.cache, a.log))
        return 0
    return cmd_list(a)


if __name__ == "__main__":
    sys.exit(main() or 0)
