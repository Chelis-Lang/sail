#!/usr/bin/env python3
"""Check the performance matrix's bars, output normalization, pairing and
report, and run its small-project sequence against stand-in tools."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import re
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("perf_matrix", Path(__file__).with_name("perf-matrix.py"))
pm = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(pm)

TEMPLATE = "\nuse vstd::prelude::*;\n\nverus! {\n\nfn foo() {\n    assert(1 == 0 + 1);\n}\n\n} // verus!\n"

# Stand-in `cargo` (stock) and `sail`: `new` writes the template, `verify`
# prints a Verus summary; stock prints nothing when no source changed since
# its last verification, as Cargo does.
TOOL = r'''#!/usr/bin/env python3
import hashlib, os, subprocess, sys
from pathlib import Path
args = sys.argv[1:]
if args[:1] == ["verus"]:
    args = args[1:]
    stock = True
else:
    stock = False
if args[:1] == ["new"]:
    d = Path(args[-1]); (d / "src").mkdir(parents=True)
    (d / "src" / "lib.rs").write_text(TEMPLATE)
    subprocess.run(["git", "init", "-q"], cwd=d, check=True)
    sys.exit(0)
if args[:1] == ["verify"]:
    src = Path("src/lib.rs").read_text()
    digest = hashlib.sha256(src.encode()).hexdigest()
    stamp = Path("target/stamp")
    if stock and stamp.exists() and stamp.read_text() == digest:
        print("    Finished `dev` profile in 0.01s", file=sys.stderr)
        sys.exit(0)
    stamp.parent.mkdir(exist_ok=True); stamp.write_text(digest)
    n = src.count("fn ")
    print("   Compiling demo v0.1.0", file=sys.stderr)
    print(f"verification results:: {n} verified, 0 errors")
    sys.exit(0)
sys.exit(3)
'''


class Bars(unittest.TestCase):
    def test_kinds(self):
        self.assertEqual(pm.bar_of("cold-first", "small"), ("speedup", 2.0))
        self.assertEqual(pm.bar_of("fresh-project", "small"), ("speedup", 2.0))
        self.assertEqual(pm.bar_of("module-edit", "vest"), ("speedup", 1.3))
        self.assertEqual(pm.bar_of("spec-wide", "vest"), ("not-slower", None))
        self.assertEqual(pm.bar_of("small-edit", "small"), ("not-slower", None))

    def test_thresholds(self):
        ns = ("not-slower", None)
        self.assertTrue(pm.meets(ns, 10.0, 10.5))
        self.assertFalse(pm.meets(ns, 10.0, 10.51))
        self.assertTrue(pm.meets(ns, 0.5, 0.6))
        self.assertFalse(pm.meets(ns, 0.5, 0.61))
        self.assertTrue(pm.meets(("speedup", 2.0), 30.0, 15.0))
        self.assertFalse(pm.meets(("speedup", 2.0), 30.0, 15.1))
        self.assertFalse(pm.meets(("speedup", 1.3), 30.0, 0.0))


class Normalize(unittest.TestCase):
    def test_keeps_verdicts_and_diagnostics_only(self):
        text = ("   Compiling vstd v0.0.0\n"
                "error: assertion failed\n   --> src/lib.rs:7:12\n    |\n7   |     assert(false);\n"
                "verification results:: 1 verified, 1 errors\n"
                "error: could not compile `demo` (lib) due to 1 previous error\n"
                "    Finished `dev` profile [unoptimized] target(s) in 3.21s\n")
        self.assertEqual(pm.normalize(text), ["error: assertion failed", "--> src/lib.rs:7:12",
                                              "verification results:: 1 verified, 1 errors"])

    @staticmethod
    def project_doc(verified=3, errors=0, outcomes=(("f", ["proved"]), ("g", ["proved", "proved"])),
                    failed=0, partial=True):
        """A sail-project-result shaped like anvil's: Verus counts checks
        (`crates[].verus`), the report lists functions and their obligations."""
        fns = [{"function": name, "verdict": "failed" if "failed" in obs else "verified",
                "obligations": [{"kind": "Function-Def", "advisory": False, "outcome": o} for o in obs]
                + [{"kind": "Function-Def", "advisory": True, "outcome": "proved"}]}
               for name, obs in outcomes]
        requested = {"requested": True, "target": "anvil", "complete": not partial, "report": {
            "summary": {"functions": len(fns), "verified": len(fns) - failed, "failed": failed},
            "crates": [{"verus": {"verified": verified, "errors": errors}, "functions": fns}]}}
        other = {"requested": False, "target": "vstd", "report": {
            "summary": {"verified": 9, "failed": 0}, "crates": [{"verus": {"verified": 9, "errors": 0}}]}}
        notes = ["Requested function/module scope is partial; whole-target reuse is disabled"] if partial else []
        return {"kind": "sail-project-result", "targets": [other, requested], "notes": notes}

    def test_project_result_uses_verus_check_counts_of_requested_targets(self):
        # Two functions, three checks (one function's loop is its own query):
        # Verus prints 3, as stock does, with the partial-verification suffix.
        text = "   Checking anvil\n" + json.dumps(self.project_doc(), indent=2) + "\n"
        self.assertEqual(pm.normalize(text), [
            "verification results:: 3 verified, 0 errors (partial verification with `--verify-*`)"])
        stock = "verification results:: 3 verified, 0 errors (partial verification with `--verify-*`)\n"
        self.assertEqual(pm.normalize(text), pm.normalize(stock))

    def test_project_result_counts_must_agree_with_function_verdicts(self):
        # A Verus count that the report's obligations do not account for (a
        # missing function verdict) is never papered over.
        text = json.dumps(self.project_doc(verified=4)) + "\n"
        got = pm.normalize(text)
        self.assertEqual(got[0], "verification results:: 4 verified, 0 errors (partial verification with `--verify-*`)")
        self.assertEqual(len(got), 2, got)
        self.assertTrue(got[1].startswith("sail-report-inconsistent: anvil: 3 proved obligations, Verus counted 4"), got)
        # Failures must agree too.
        text = json.dumps(self.project_doc(verified=2, errors=0, outcomes=(("f", ["proved"]), ("g", ["proved", "failed"])),
                                           failed=1)) + "\n"
        self.assertIn("1 failed functions, Verus counted 0 errors", pm.normalize(text)[1])
        # A report without Verus's counts is inconsistent, not zero.
        doc = self.project_doc()
        del doc["targets"][1]["report"]["crates"][0]["verus"]
        self.assertTrue(pm.normalize(json.dumps(doc) + "\n")[1].endswith("no Verus counts; 3 proved obligations, Verus counted 0 verified"))

    def test_whole_project_result_has_no_partial_suffix(self):
        text = json.dumps(self.project_doc(partial=False)) + "\n"
        self.assertEqual(pm.normalize(text), ["verification results:: 3 verified, 0 errors"])

    def test_copy_roots_are_normalized(self):
        warning = ("warning: the following packages contain code that will be rejected by a future version "
                   "of Rust: verismo v0.1.0 ({}/source/verismo)\n")
        with tempfile.TemporaryDirectory() as d:
            plain, sail = Path(d, "r1", "plain-proj"), Path(d, "r1", "sail-proj")
            a = pm.normalize(warning.format(plain), [plain])
            b = pm.normalize(warning.format(sail), [sail])
            self.assertEqual(a, b)
            self.assertIn("(<copy>/source/verismo)", a[0])
            # Only the arm's own root is replaced: another path still differs.
            self.assertNotEqual(pm.normalize(warning.format(sail), [plain]), a)


SHIPPED_LOCK = """version = 4

[[package]]
name = "sail-shipped-dependencies"
version = "0.1.0"

[[package]]
name = "syn"
version = "2.0.119"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "verus_builtin"
version = "0.0.0-b"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "vstd"
version = "0.0.0-v"
source = "registry+https://github.com/rust-lang/crates.io-index"
"""


class ShippedVstd(unittest.TestCase):
    def test_versions_and_lock_diff(self):
        self.assertEqual(pm.verus_versions(SHIPPED_LOCK), ["verus_builtin=0.0.0-b", "vstd=0.0.0-v"])
        self.assertEqual(pm.lock_diff(SHIPPED_LOCK, SHIPPED_LOCK.replace("sail-shipped-dependencies", "p")),
                         ["lock: vstd's closure resolves to the shipped versions"])
        older = SHIPPED_LOCK.replace('version = "2.0.119"', 'version = "2.0.100"')
        self.assertEqual(pm.lock_diff(SHIPPED_LOCK, older), ["lock: syn 2.0.100 (shipped 2.0.119)"])

    def test_proof_detection(self):
        with tempfile.TemporaryDirectory() as d:
            cache, log = Path(d, "cache"), Path(d, "cold.log")
            log.write_text("   Compiling vstd\n")
            self.assertEqual(pm.vstd_proof(cache, log), "not offered")
            log.write_text("note[SAIL3138]: the release ships a preverified vstd proof, but ... vstd that is not from crates.io\n")
            self.assertTrue(pm.vstd_proof(cache, log).startswith("miss: the release ships"))
            receipts = cache / "projects" / "x" / "receipts"
            receipts.mkdir(parents=True)
            (receipts / "a.json").write_text(json.dumps({"name": "demo", "invocation": {"dependency_build": None}}))
            (receipts / "b.json").write_text(json.dumps(
                {"name": "vstd", "invocation": {"dependency_build": {"kind": "release", "establishing_run": "r"}}}))
            self.assertEqual(pm.vstd_proof(cache, log), "served")

    def test_proof_summary(self):
        self.assertEqual(pm.proof_summary([]), "-")
        self.assertEqual(pm.proof_summary(["served", "served", "miss: x"]), "miss 1/3, served 2/3")

    def test_rust_targets_match_the_corpus(self):
        corpus = (pm.ROOT / "corpus" / "corpus.toml").read_text()
        entries = {"verismo": "verismo-main"}
        for project, entry in entries.items():
            block = corpus.split(f'name = "{entry}"', 1)[1].split("[[project]]", 1)[0]
            requires = re.search(r"(?m)^requires = \[(.*)\]", block).group(1)
            targets = re.findall(r'"rust-target:([^"]+)"', requires)
            self.assertEqual(pm.RUST_TARGETS.get(project, []), targets)


def sample(rnd, arm, scenario, wall, norm, ref=None, code=0, project="small", note=""):
    return dict(round=rnd, arm=arm, project=project, scenario=scenario, wall=wall, exit=code,
                log="x", note=note, normalized=norm, reference=ref if ref is not None else norm)


class Report(unittest.TestCase):
    V = ["verification results:: 1 verified, 0 errors"]

    def test_pairs_and_judges_rows(self):
        s = []
        for r in (1, 2, 3):
            s += [sample(r, "stock", "small-edit", 1.0, self.V), sample(r, "sail", "small-edit", 1.04, self.V)]
            s += [sample(r, "stock", "cold-first", 30.0, self.V), sample(r, "sail", "cold-first", 16.0, self.V)]
        rows = {r["scenario"]: r for r in pm.rows_of(s, 3)}
        self.assertTrue(rows["small-edit"]["pass"])
        self.assertFalse(rows["cold-first"]["pass"])  # 1.875x < 2x
        self.assertAlmostEqual(rows["cold-first"]["ratio"], 30.0 / 16.0)

    def test_a_different_output_or_missing_sample_fails_the_row(self):
        s = [sample(1, "stock", "unchanged", 0.2, [], ref=self.V),
             sample(1, "sail", "unchanged", 0.25, self.V),
             sample(2, "stock", "unchanged", 0.2, self.V),
             sample(2, "sail", "unchanged", 0.25, ["verification results:: 0 verified, 1 errors"]),
             sample(3, "stock", "unchanged", 0.2, self.V)]
        row = pm.rows_of(s, 3)[0]
        self.assertFalse(row["pass"])
        self.assertFalse(row["complete"])
        self.assertEqual({(f["round"], f["identity"]) for f in row["failures"]},
                         {(2, "different"), (3, "unpaired")})
        self.assertEqual(s[0]["identity"], "same")

    def test_a_stock_no_op_also_matches_a_sail_no_op(self):
        s = [sample(1, "stock", "unchanged", 0.1, [], ref=self.V), sample(1, "sail", "unchanged", 0.2, [])]
        pm.judge(s)
        self.assertEqual([x["identity"] for x in s], ["same", "same"])

    def test_refusal_counts_as_failure(self):
        s = []
        for r in (1, 2, 3):
            s += [sample(r, "stock", "revert", 1.0, self.V),
                  sample(r, "sail", "revert", 0.5, self.V, code=2 if r == 2 else 0)]
        row = pm.rows_of(s, 3)[0]
        self.assertFalse(row["pass"])
        self.assertEqual(len(row["failures"]), 2)  # both samples of round 2

    def test_markdown(self):
        with tempfile.TemporaryDirectory() as d:
            Path(d, "header.json").write_text(json.dumps({"rounds": 3, "machine": "m"}))
            with open(Path(d, "samples.jsonl"), "w") as f:
                for r in (1, 2, 3):
                    for arm, w in (("stock", 41.0), ("sail", 30.0)):
                        f.write(json.dumps(sample(r, arm, "module-edit", w, self.V, project="vest")) + "\n")
            out = Path(d, "m.md")
            code = pm.cmd_report(argparse.Namespace(dir=d, markdown=str(out)))
            text = out.read_text()
        self.assertEqual(code, 0)
        self.assertIn("| vest | module-edit | 41.00 s [41.00-41.00] | 30.00 s [30.00-30.00] | 1.37x | 3/3 | >= 1.3x | - | pass |", text)


class SmallSequence(unittest.TestCase):
    def test_runs_every_scenario_and_matches_stock(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            tools = d / "tools"; tools.mkdir()
            body = TOOL.replace("TEMPLATE)", repr(TEMPLATE) + ")")
            for name in ("cargo", "sail"):
                (tools / name).write_text(body); (tools / name).chmod(0o755)
            install = d / "install"; (install / "bin").mkdir(parents=True)
            stock = d / "stock"; stock.mkdir()
            args = argparse.Namespace(out=str(d / "out"), sail_install=str(install), env=None,
                                      stock_verus=str(stock), tools=str(d / "none"), settle=0.0,
                                      vstd="source")
            (d / "out").mkdir()
            r = pm.Runner(args)
            r.path = os.pathsep.join([str(tools), os.environ["PATH"]])
            for arm in ("stock", "sail"):
                r.small_round(1, arm, set(pm.SMALL))
            samples = pm.judge(pm.load(d / "out"))
        got = {(s["arm"], s["scenario"]): s for s in samples}
        self.assertEqual(len(got), 2 * len(pm.SMALL))
        for s in samples:
            self.assertTrue(s["ok"], s)
        # Stock's unchanged rerun is a Cargo no-op, judged against cold-first's output.
        self.assertEqual(got[("stock", "unchanged")]["normalized"], [])
        self.assertEqual(got[("stock", "unchanged")]["reference_from"], "previous verifying stock output")
        self.assertEqual(got[("sail", "small-edit")]["normalized"],
                         ["verification results:: 2 verified, 0 errors"])
        self.assertEqual(got[("sail", "cold-first")]["vstd_proof"], "not offered")


if __name__ == "__main__":
    unittest.main()
