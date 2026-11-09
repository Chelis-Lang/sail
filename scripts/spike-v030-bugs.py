#!/usr/bin/env python3
"""SPIKE (v0.3.0 Track D, #696/#683): ordinary background jobs in sequence.

Each job is a separate small project (a Cargo-Verus library or a standalone
file) submitted to the one `sail serve` selected by SAIL_CACHE_DIR and
SAIL_DAEMON_SOCKET. One JSON line per job: the job's read-time currentness,
the runner's notes, target reuse and per-check provenance counts.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

LIB = """use vstd::prelude::*;

verus! {

pub open spec fn bounded(x: int) -> bool { 0 <= x < BOUND }

pub proof fn lemma_sum(a: int, b: int)
    requires bounded(a), bounded(b),
    ensures 0 <= a + b < 2 * BOUND,
{
}

pub fn add(a: u32, b: u32) -> (r: u32)
    requires a < BOUND, b < BOUND,
    ensures r == a + b,
{
    proof { lemma_sum(a as int, b as int); }
    a + b
}

pub fn first(v: &Vec<u32>) -> (r: u32)
    requires v@.len() > 0,
    ensures r == v@[0],
{
    v[0]
}

} // verus!
"""

FILE = """use vstd::prelude::*;
verus! {
proof fn selected(a: int, b: int)
    requires 0 <= a < BOUND, 0 <= b < BOUND,
    ensures a + b < 2 * BOUND,
{
}
fn first(v: &Vec<u32>) -> (r: u32)
    requires v@.len() > 0,
    ensures r == v@[0],
{
    v[0]
}
}
fn main() {}
"""


def walk(value, counts):
    if isinstance(value, dict):
        for key in ("checks_solved", "checks_served", "checks_unrecorded"):
            if isinstance(value.get(key), int):
                counts[key] = counts.get(key, 0) + value[key]
        if "answer" in value and "source" in value:
            counts["checks"] = counts.get("checks", 0) + 1
            if value["source"] is None:
                counts["source_null"] = counts.get("source_null", 0) + 1
        for item in value.values():
            walk(item, counts)
    elif isinstance(value, list):
        for item in value:
            walk(item, counts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sail", required=True)
    parser.add_argument("--mode", choices=("cargo", "standalone"), required=True)
    parser.add_argument("--count", type=int, required=True)
    parser.add_argument("--start", type=int, default=0)
    parser.add_argument("--work", required=True)
    parser.add_argument("--resubmit", type=int, default=0,
                        help="extra unchanged submissions of each project")
    parser.add_argument("--verus-arg", action="append", default=[])
    args = parser.parse_args()
    work = Path(args.work).absolute()
    work.mkdir(parents=True, exist_ok=True)

    def run(command, cwd, timeout=900):
        return subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=timeout)

    for index in range(args.start, args.start + args.count):
        bound = 1000 + index
        if args.mode == "cargo":
            name = f"spike_p{index}"
            created = run([args.sail, "new", "--lib", name], work)
            project = work / name
            if created.returncode != 0 or not project.is_dir():
                print(json.dumps({"index": index, "error": "new", "stderr": created.stderr[-400:]}), flush=True)
                continue
            (project / "src/lib.rs").write_text(LIB.replace("BOUND", str(bound)))
            submit = [args.sail, "verify", "--background"]
            if args.verus_arg:
                submit += ["--", *args.verus_arg]
        else:
            project = work / f"s{index}"
            project.mkdir(exist_ok=True)
            (project / "proof.rs").write_text(FILE.replace("BOUND", str(bound)))
            submit = [args.sail, "verify", "proof.rs", "--background"]
            if args.verus_arg:
                submit += ["--", *args.verus_arg]
        for attempt in range(1 + args.resubmit):
            began = time.time()
            submitted = run(submit, project)
            found = re.search(r"\b\d{10,}-\d+-\d+\b", submitted.stdout)
            job = found.group(0) if found else ""
            if submitted.returncode != 0 or not job:
                print(json.dumps({"index": index, "attempt": attempt, "error": "submit",
                                  "stdout": submitted.stdout[-300:], "stderr": submitted.stderr[-300:]}), flush=True)
                continue
            status = {}
            first_terminal = None
            while time.time() - began < 900:
                polled = run([args.sail, "jobs", "status", job, "--output", "json"], project)
                try:
                    status = json.loads(polled.stdout)
                    status = status.get("value", status)
                except ValueError:
                    status = {"raw": polled.stdout[-200:], "stderr": polled.stderr[-200:]}
                if status.get("state") in ("completed", "failed", "cancelled", "infrastructure-error",
                                           "infrastructure_error", "Completed"):
                    first_terminal = status
                    break
                time.sleep(0.2)
            result = run([args.sail, "jobs", "result", job, "--output", "json"], project)
            try:
                document = json.loads(result.stdout)
            except ValueError:
                document = {"raw": result.stdout[-300:], "stderr": result.stderr[-300:]}
            body = document.get("value", document) if isinstance(document, dict) else document
            project_result = body.get("result") if isinstance(body, dict) else None
            counts = {}
            walk(project_result, counts)
            line = {
                "index": index,
                "attempt": attempt,
                "mode": args.mode,
                "job": job,
                "wall_s": round(time.time() - began, 2),
                "loadavg": os.getloadavg()[0],
                "status_state": (first_terminal or status).get("state"),
                "status_currentness": (first_terminal or status).get("currentness"),
                "result_job_currentness": (body.get("job") or {}).get("currentness") if isinstance(body, dict) else None,
                "result_currentness": (project_result or {}).get("currentness"),
                "exit_code": (project_result or {}).get("exit_code"),
                "notes": (project_result or {}).get("notes"),
                "targets": [
                    {"package_id": t.get("package_id", "")[:60], "reused": t.get("reused"),
                     "complete": t.get("complete")}
                    for t in (project_result or {}).get("targets", [])
                ],
                "counts": counts,
            }
            if project_result is None:
                line["document"] = json.dumps(document)[:600]
            print(json.dumps(line), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
