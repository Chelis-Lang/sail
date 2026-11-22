#!/usr/bin/env python3
"""Measure Sail's external witness route using the inputs from Dive's temporal repair panel."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv, work):
    mark = time.perf_counter_ns()
    result = subprocess.run(
        ["/usr/bin/time", "-qf", "%M", "-o", work / "rss", *argv], capture_output=True, check=False
    )
    return result, round((time.perf_counter_ns() - mark) / 1e6, 3), int((work / "rss").read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sail", type=Path)
    parser.add_argument("dive", type=Path)
    parser.add_argument("examples", type=Path, help="pinned Examples specifications directory")
    parser.add_argument("panel", type=Path, help="Dive temporal-repair-panel.py work directory")
    parser.add_argument(
        "anchors", type=Path, help="seven-floor donor mapped with --anchors-only for eight floors"
    )
    args = parser.parse_args()
    sail, dive, panel = args.sail.resolve(), args.dive.resolve(), args.panel.resolve()
    elevator = args.examples / "MultiCarElevator/Elevator.tla"
    clock = args.examples / "SpecifyingSystems/RealTime/MCRealTimeHourClock.tla"
    rows = []
    with tempfile.TemporaryDirectory() as temp:
        work = Path(temp)
        # Capture the original small public word, independently of the tested repair.
        baseline, _, _ = run(
            [
                dive,
                "check",
                clock,
                "--config",
                clock.with_suffix(".cfg"),
                "--output",
                "json",
                "--workers",
                "1",
                "--seed",
                "0",
                "--fp",
                "0",
            ],
            work,
        )
        assert baseline.returncode == 13
        donor = work / "clock.json"
        donor.write_text(json.dumps(json.loads(baseline.stdout)["counterexample"]["trace"]))
        damaged = work / "damaged.json"
        damaged.write_text("{")
        # The mapping stage in the Dive panel is explicit and can be repeated independently.
        for name, module, cfg, trace, route, code in [
            ("same-clock", clock, clock.with_suffix(".cfg"), donor, "validated-witness", 13),
            (
                "repair-clock",
                panel / "slower" / clock.name,
                panel / "slower" / clock.with_suffix(".cfg").name,
                donor,
                "validated-witness",
                13,
            ),
            (
                "fixed-clock",
                panel / "fixed" / clock.name,
                panel / "fixed" / clock.with_suffix(".cfg").name,
                donor,
                "current-check",
                0,
            ),
            (
                "stale-clock",
                panel / "stale" / clock.name,
                panel / "stale" / clock.with_suffix(".cfg").name,
                donor,
                "current-check",
                None,
            ),
            (
                "fair-clock",
                panel / "fairness" / clock.name,
                panel / "fairness" / clock.with_suffix(".cfg").name,
                donor,
                "current-check",
                None,
            ),
            (
                "malformed-donor",
                panel / "fixed" / clock.name,
                panel / "fixed" / clock.with_suffix(".cfg").name,
                damaged,
                "current-check",
                0,
            ),
            (
                "missing-donor",
                panel / "fixed" / clock.name,
                panel / "fixed" / clock.with_suffix(".cfg").name,
                work / "missing.json",
                "current-check",
                0,
            ),
            ("same-elevator", elevator, panel / "n7.cfg", panel / "seven.json", "validated-witness", 13),
            ("repair-elevator", elevator, panel / "n8.cfg", args.anchors, "validated-witness", 13),
        ]:
            proc, ms, mem = run(
                [
                    sail,
                    "runtime",
                    "witness-recheck",
                    "--dive",
                    dive,
                    "--module",
                    module,
                    "--config",
                    cfg,
                    "--trace",
                    trace,
                    "--time-limit",
                    "20",
                ],
                work,
            )
            doc = json.loads(proc.stdout)
            assert doc.get("route") == route and (code is None or proc.returncode == code), (
                name,
                doc,
                proc.stderr,
            )
            row = {
                "case": name,
                "route": route,
                "exit": proc.returncode,
                "wall_ms": ms,
                "peak_rss_kib": mem,
                "module_sha256": digest(module),
                "cfg_sha256": digest(cfg),
            }
            if trace.exists():
                row["donor_sha256"] = digest(trace)
            if route == "validated-witness":
                detail = doc["validation"]["repair"]
                candidate = work / "received.json"
                candidate.write_text(json.dumps(detail["trace"], sort_keys=True))
                replay, _, _ = run(
                    [
                        dive,
                        "validate-trace",
                        module,
                        "--config",
                        cfg,
                        "--trace",
                        candidate,
                        "--output",
                        "json",
                        "--seed",
                        "0",
                        "--fp",
                        "0",
                    ],
                    work,
                )
                assert replay.returncode == 0 and json.loads(replay.stdout)["result"] == "valid"
                row.update(
                    status=detail["status"],
                    word_sha256=digest(candidate),
                    states=detail["trace"]["length"],
                    generated=detail["generated"],
                    gaps=detail["gaps"],
                )
            else:
                row.update(verdict=doc["report"]["verdict"], stats=doc["report"]["statistics"])
            rows.append(row)
        # A missing donor sends seven floors into genuine cold checking, long enough
        # to exercise the one shared wall bound without a full duplicate cold run.
        stopped, ms, mem = run(
            [
                sail,
                "runtime",
                "witness-recheck",
                "--dive",
                dive,
                "--module",
                elevator,
                "--config",
                panel / "n7.cfg",
                "--trace",
                work / "missing.json",
                "--time-limit",
                "1",
            ],
            work,
        )
        doc = json.loads(stopped.stdout)
        assert (stopped.returncode == 4 and doc["verdict"] == "stopped") or (
            stopped.returncode == 5 and doc["report"]["verdict"] == "stopped"
        )
        rows.append(
            {
                "case": "cold-deadline",
                "exit": stopped.returncode,
                "wall_ms": ms,
                "peak_rss_kib": mem,
                "verdict": "stopped",
            }
        )
    print(json.dumps({"sail_sha256": digest(sail), "dive_sha256": digest(dive), "samples": rows}, indent=2))


if __name__ == "__main__":
    main()
