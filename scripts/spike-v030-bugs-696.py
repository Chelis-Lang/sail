#!/usr/bin/env python3
"""SPIKE #696: an interrupted earlier run of a context withholds Current.

For each GAP: `sail new`, wait GAP seconds (the daemon's new-project
preparation runs meanwhile), then `sail verify --background`, which preempts
it. With --cancel: submit, cancel the job after GAP seconds, then resubmit.
Prints the final job's currentness and notes, and a third unchanged job.
"""
import argparse
import json
import re
import subprocess
import time
from pathlib import Path

LIB = "use vstd::prelude::*;\nverus! {\npub proof fn l(a: int) requires a > N ensures a + 1 > N {}\n}\n"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--sail", required=True)
    p.add_argument("--work", required=True)
    p.add_argument("--gaps", required=True)
    p.add_argument("--repeat", type=int, default=3)
    p.add_argument("--cancel", action="store_true")
    a = p.parse_args()
    work = Path(a.work)
    work.mkdir(parents=True, exist_ok=True)

    def run(cmd, cwd):
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=900)

    def job(project):
        out = run([a.sail, "verify", "--background"], project).stdout
        return re.search(r"\d{10,}-\d+-\d+", out).group(0)

    def finish(project, ident):
        while True:
            out = run([a.sail, "jobs", "status", ident, "--output", "json"], project).stdout
            if not out.strip():
                # A status call the loaded service did not answer; ask again.
                time.sleep(0.5)
                continue
            s = json.loads(out)["value"]
            if s["state"] not in ("queued", "running"):
                break
            time.sleep(0.2)
        r = json.loads(run([a.sail, "jobs", "result", ident, "--output", "json"], project).stdout)["value"]
        result = r.get("result") or {}
        return {"state": s["state"], "currentness": result.get("currentness"),
                "sail3114": any(n.startswith("SAIL3114") for n in result.get("notes", []))}

    n = 0
    for gap in [float(g) for g in a.gaps.split(",")]:
        for _ in range(a.repeat):
            n += 1
            name = f"spike_g{n}"
            run([a.sail, "new", "--lib", name], work)
            project = work / name
            (project / "src/lib.rs").write_text(LIB.replace("N", str(n)))
            if a.cancel:
                first = job(project)
                time.sleep(gap)
                run([a.sail, "jobs", "cancel", first], project)
                finish(project, first)
            else:
                time.sleep(gap)
            second = finish(project, job(project))
            third = finish(project, job(project))
            print(json.dumps({"gap_s": gap, "cancel": a.cancel, "project": name,
                              "job": second, "resubmitted": third}), flush=True)


main()
