#!/usr/bin/env python3
"""Local-only source-engine differential gate; never builds Verus or runs in CI.

Each fixture runs three ways: stock Verus, the fork runtime without the engine
("fork") and the fork runtime with the engine ("engine"). The engine must match
the plain fork run exactly, raw SMT included. Against stock, exit status,
diagnostics, counters, verdicts and the SMT session inventory must match; the
raw SMT is only recorded, because the fork's accepted divergences rename
solver symbols (ADR 0054, spec parameters; ADR 0056, trait function spellings).
"""
import argparse
import hashlib
import json
import os
import pathlib
import platform
import subprocess

FIXTURES = {
    "long_worker_span": "proof fn selected() { /*" + "x" * 40000 + "*/ assert(true); }\nproof fn other() { }",
    "success": "proof fn selected(x: int) ensures x + 1 > x { }\nproof fn other(x: int) ensures x == x { }",
    "failed_focus": "proof fn selected(x: int) ensures x > x { }\nproof fn other(x: int) ensures x == x { }",
    "failed_other": "proof fn selected(x: int) ensures x == x { }\nproof fn other(x: int) ensures x > x { }",
    "rust_frontend_error": "proof fn selected() { }\nfn broken() { let value: u8 = true; }",
    "late_exec": "proof fn selected() { }\n#[verifier::external_body] fn late() { let mut x = 0u64; let a = &mut x; let b = &mut x; *a = *b; }",
    "bitvector": "proof fn selected(x: u64) ensures x & x == x { assert(x & x == x) by(bit_vector); }",
    "recursive": "spec fn count(n: nat) -> nat decreases n { if n == 0 { 0 } else { 1 + count((n-1) as nat) } }\nproof fn selected() ensures count(1) == 1 { }",
    "macro_spans": "proof fn selected() { assert(identity!(false)); }",
    "missing_function": "proof fn other() { }",
    "ambiguous_function": "struct Holder<T> { value:T } impl Holder<u8> { proof fn selected(&self) {} } impl Holder<u16> { proof fn selected(&self) {} }",
}

def pipe(path):
    raw = path.read_bytes()
    return b"".join(part.split(b";;;<<<\n", 1)[0] + b'\n(echo "<<DONE>>")\n'
                    for part in raw.split(b";;;>>> QUERY\n")[1:])

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def run(args):
    out = pathlib.Path(args.out).absolute()
    out.mkdir(parents=True, exist_ok=True)
    results = []
    for name, body in FIXTURES.items():
        work = out / name
        work.mkdir(exist_ok=True)
        source = work / "lib.rs"
        source.write_text("use verus_builtin::*;\nuse verus_builtin_macros::*;\n" + ("macro_rules! identity {($x:expr)=>{$x}}\n" if name == "macro_spans" else "") + "verus! {\n" + body + "\n}\nfn main() {}\n")
        records = {}
        for mode, executable in [("stock", args.stock), ("fork", args.engine), ("engine", args.engine)]:
            environment = os.environ.copy()
            for key in list(environment):
                if key.startswith("SAIL_ENGINE"):
                    del environment[key]
            environment["VERUS_Z3_PATH"] = args.z3
            if mode == "engine":
                events = work / "events.jsonl"
                events.unlink(missing_ok=True)
                (work / "focus.json").unlink(missing_ok=True)
                environment.update(SAIL_ENGINE="interactive", SAIL_ENGINE_FUNCTION="demo::Holder::selected" if name == "ambiguous_function" else "demo::selected",
                                   SAIL_ENGINE_EVENTS=str(events), SAIL_ENGINE_FOCUS_REPORT=str(work / "focus.json"))
            command = [executable, str(source), "--no-vstd", "--crate-name", "demo", "-V", "use-crate-name",
                       "--output-json", "--time", "--log", "air", "--log", "smt-transcript",
                       "--log-dir", str(work / mode), "--no-report-long-running"]
            result = subprocess.run(command, env=environment, capture_output=True, timeout=120)
            (work / (mode + ".stdout")).write_bytes(result.stdout)
            (work / (mode + ".stderr")).write_bytes(result.stderr)
            records[mode] = result
        engine = records["engine"]
        # Per-function success fields are the verdict oracle. Timing/effort remain outputs.
        def verdicts(document):
            modules = document.get("times-ms", {}).get("smt", {}).get("smt-run-module-times", [])
            return sorted((entry["function"], entry["success"]) for module in modules
                          for entry in module.get("function-breakdown", []))
        def agree(reference, label):
            left, right = records[reference], records["engine"]
            assert left.returncode == right.returncode, (name, label, left.returncode, right.returncode)
            assert left.stderr == right.stderr, (name, label, "diagnostics differ")
            left_json, right_json = json.loads(left.stdout), json.loads(right.stdout)
            assert left_json["verification-results"] == right_json["verification-results"], (name, label)
            assert left_json["func-details"] == right_json["func-details"], (name, label)
            assert verdicts(left_json) == verdicts(right_json), (name, label, "function verdicts differ")
            logs = sorted((work / reference).glob("*.smt_transcript"))
            assert [p.name for p in logs] == [p.name for p in engine_logs], (name, label, "session inventory")
            return logs
        engine_logs = sorted((work / "engine").glob("*.smt_transcript"))
        for left, right in zip(agree("fork", "engine vs fork"), engine_logs):
            assert pipe(left) == pipe(right), (name, "engine vs fork", "full SMT bytes differ", left.name)
        # Raw SMT against stock is recorded, not asserted (ADR 0054, ADR 0056).
        stock_sessions = [{"name": left.name, "equal": pipe(left) == pipe(right),
                           "stock_input_sha256": hashlib.sha256(pipe(left)).hexdigest(),
                           "engine_input_sha256": hashlib.sha256(pipe(right)).hexdigest()}
                          for left, right in zip(agree("stock", "engine vs stock"), engine_logs)]
        events = [json.loads(line) for line in (work / "events.jsonl").read_text().splitlines()]
        assert events[-1]["event"] == "compiler-sealed", (name, "missing compiler terminal seal")
        assert events[-1]["payload"]["normal_termination"] is True, (name, "missing normal driver completion")
        if name not in ["missing_function","ambiguous_function","rust_frontend_error"]:
            assert (work / "focus.json").is_file(), (name, "focused evidence missing")
            assert events.index(next(e for e in events if e["event"] == "phase-finished" and e["phase"] == "focus")) < len(events)-1
        elif name == "rust_frontend_error":
            assert not (work / "focus.json").exists(), "frontend failure retained focus evidence"
            assert engine.returncode != 0 and b"E0308" in engine.stderr, "frontend type error disappeared"
        else:
            assert not (work / "focus.json").exists()
            unavailable = next(e for e in events if e["event"] == "focus-unavailable")
            assert unavailable["payload"]["matches"] == (2 if name == "ambiguous_function" else 0)
        if name == "late_exec":
            assert engine.returncode != 0 and b"E0499" in engine.stderr, "late borrow error disappeared"
        results.append({"fixture": name, "exit_code": engine.returncode, "diagnostics_sha256": hashlib.sha256(engine.stderr).hexdigest(),
                        "full_sessions": [{"name": p.name, "bytes": len(pipe(p)), "input_sha256": hashlib.sha256(pipe(p)).hexdigest()} for p in engine_logs],
                        "stock_smt": stock_sessions,
                        "normal_termination": events[-1]["payload"]["normal_termination"]})
        differing = sum(not session["equal"] for session in stock_sessions)
        note = f" (raw SMT differs from stock in {differing} of {len(stock_sessions)} sessions)" if differing else ""
        print(name, "matches" + note, flush=True)
    summary = {"version": 1, "machine": platform.platform(), "engine_sha256": sha(pathlib.Path(args.engine)),
               "engine_verifier_sha256": sha(pathlib.Path(args.engine).with_name("rust_verify")),
               "stock_sha256": sha(pathlib.Path(args.stock)), "z3_sha256": sha(pathlib.Path(args.z3)), "results": results}
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ["engine", "stock", "z3", "out"]:
        parser.add_argument("--" + flag, required=True)
    run(parser.parse_args())
