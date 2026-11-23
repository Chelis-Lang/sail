#!/usr/bin/env python3
"""Run saved-witness selection with original Elevator and typed actor controls."""

import argparse
import hashlib
import json
from pathlib import Path
import signal
import subprocess
import tempfile
import time


def dump(path, document):
    path.write_text(json.dumps(document, separators=(",", ":")) + "\n")
    return path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def field(path, kind):
    return {"path": path.split("/"), "kind": kind}


def entry(module, config, word):
    return {"module": str(module), "config": str(config), "word": str(word)}


ELEVATOR = {"integer_carrier": "FloorCount", "fields": [
    field("PersonState/fcn/*/1/record/destination", "integer"),
    field("PersonState/fcn/*/1/record/location", "integer_or_model"),
    field("ElevatorState/fcn/*/1/record/floor", "integer"),
    field("ElevatorState/fcn/*/1/record/buttonsPressed/set/*", "integer"),
    field("ActiveElevatorCalls/set/*/record/floor", "integer"),
]}
ACTORS = {"integer_carrier": "N", "renames": {"old_actor": "new_actor"}, "fields": [
    field("x/seq/0/record/actor", "model"), field("x/seq/0/record/coordinate", "integer"),
]}
ACTOR_SOURCE = r"""---- MODULE Actors ----
EXTENDS Naturals, Sequences
CONSTANTS N, A
VARIABLE x
Init == x = <<[actor |-> A, coordinate |-> COORDINATE, noise |-> 2]>>
Progress == /\ x[1].coordinate /= 0
            /\ x' = <<[actor |-> A, coordinate |-> 0, noise |-> 2]>>
Next == Progress \/ UNCHANGED x
Spec == Init /\ [][Next]_x FAIRNESS
Live == <> (x[1].coordinate = 0)
====
"""


class Panel:
    def __init__(self, args, work):
        self.args, self.work, self.rows = args, work, []

    def run(self, argv):
        mark = time.perf_counter_ns()
        result = subprocess.run(["/usr/bin/time", "-qf", "%M", "-o", self.work / "rss", *argv],
                                capture_output=True, check=False, timeout=8)
        return (result.returncode, json.loads(result.stdout),
                (time.perf_counter_ns() - mark) / 1e6, int((self.work / "rss").read_text()))

    def dive(self, command, module, config, *options):
        return [self.args.dive, command, module, "--config", config,
                "--seed", "0", "--fp", "0", "--output", "json", *options]

    def job(self, module, config, prop, index, seconds=5):
        return [self.args.sail, "runtime", "witness-recheck", "--dive", self.args.dive,
                "--module", module, "--config", config, "--donor-index", index,
                "--property", prop, "--time-limit", str(seconds)]

    def check(self, name, module, config, prop, entries, layout, expected):
        index = dump(self.work / "index.json", {"version": 1, "donors": entries, "layout": layout})
        code, report, wall, rss = self.run(self.job(module, config, prop, index))
        route = report.get("route")
        assert route == expected, (name, code, report)
        row = {"case": name, "route": route, "exit": code, "wall_ms": wall, "peak_rss_kib": rss,
               "module_sha256": sha(module), "cfg_sha256": sha(config), "selection": report["selection"]}
        if route == "validated-witness":
            assert code == 13
            repair = report["validation"]["repair"]
            word = repair["trace"]
            trace = dump(self.work / "returned.json", word)
            rc, replay, wall, rss = self.run(self.dive("validate-trace", module, config, "--trace", trace))
            assert rc == 0 and replay["result"] == "valid"
            row["repair"] = {k: repair[k] for k in ("status", "gaps", "generated", "search_ms", "validation_ms")}
            row.update(word_states=word["length"], independent_replay_ms=wall, independent_replay_peak_rss_kib=rss)
        self.rows.append(row)
        return report

    def actor(self, name, coord, actor, *, fair=False, fixed=False):
        root = self.work / name
        root.mkdir()
        module, cfg = root / "Actors.tla", root / "Actors.cfg"
        source = ACTOR_SOURCE.replace("COORDINATE", str(coord)).replace("FAIRNESS", r"/\ WF_x(Progress)" if fair else "")
        module.write_text(source.replace("<> (x[1].coordinate = 0)", "[] (x[1].noise = 2)") if fixed else source)
        cfg.write_text(f"SPECIFICATION Spec\nCONSTANTS N = {coord + 1} A = {actor}\nPROPERTY Live\n")
        return module, cfg

    def collect(self):
        e, old, new, word = (self.args.elevator, self.args.cfg7, self.args.cfg8, self.args.word7)
        repaired = self.check("elevator-7-to-8", e, new, "TemporalInvariant",
                              [entry(e, new, word), entry(e, old, word)], ELEVATOR, "validated-witness")
        assert repaired["selection"]["chosen"]["donor"] == 1
        assert repaired["selection"]["chosen"]["mapping"]["integer_image"] == [1, 3, 4, 5, 6, 7, 8]
        module, cfg = self.actor("old", 2, "old_actor")
        current, changed = self.actor("new", 3, "new_actor")
        code, donor, _, _ = self.run(self.dive("check", module, cfg, "--workers", "1"))
        assert code == 13
        word = donor["counterexample"]["trace"]
        donor = dump(self.work / "actors.json", word)
        entries = [entry(module, cfg, donor)]
        renamed = self.check("typed-actors", current, changed, "Live", entries, ACTORS, "validated-witness")
        values = renamed["validation"]["repair"]["trace"]["states"][0]["values"]["x"]["seq"][0]["record"]
        assert values == {"actor": {"model": "new_actor"}, "coordinate": 3, "noise": 2}
        for name, flags in (("changed-fairness", {"fair": True}), ("fixed", {"fixed": True})):
            target, target_cfg = self.actor(name, 3, "new_actor", **flags)
            report = self.check(name, target, target_cfg, "Live", entries, ACTORS, "current-check")
            assert report["report"]["verdict"] == "success"
        ambiguous = {**ACTORS, "fields": ACTORS["fields"] + [field("x/seq/00/record/coordinate", "integer")]}
        self.check("ambiguous-layout", current, changed, "Live", entries, ambiguous, "current-check")
        for label, path in (("damaged-ending", dump(self.work / "damaged.json", {**word, "ending": {"kind": "lasso", "back_to": 9000}})),
                            ("missing-donor", self.work / "missing.json")):
            self.check(label, current, changed, "Live", [entry(module, cfg, path)], ACTORS, "current-check")
        corrupt = self.work / "corrupt.json"
        corrupt.write_text("{")
        self.check("corrupt-donor", current, changed, "Live", [entry(module, cfg, corrupt)], ACTORS, "current-check")
        missing = self.work / "missing-index.json"
        code, stopped, wall, rss = self.run(self.job(e, new, "TemporalInvariant", missing, seconds=1))
        assert (code == 4 and stopped["verdict"] == "stopped") or (code == 5 and stopped["report"]["verdict"] == "stopped")
        self.rows.append(dict(case="shared-deadline", exit=code, wall_ms=wall, peak_rss_kib=rss))
        with subprocess.Popen(self.job(e, new, "TemporalInvariant", missing), stdout=subprocess.PIPE) as job:
            time.sleep(0.1)
            job.send_signal(signal.SIGINT)
            result, _ = job.communicate(timeout=3)
            assert job.returncode == 4 and "cancelled" in json.loads(result)["reason"]
        self.rows.append(dict(case="cancelled", exit=4))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("sail", "dive", "elevator", "cfg7", "cfg8", "word7"):
        parser.add_argument(name, type=lambda name: Path(name).resolve())
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="sail-witness-selection-") as tmp:
        panel = Panel(args, Path(tmp))
        panel.collect()
        print(json.dumps({"sail_sha256": sha(args.sail), "dive_sha256": sha(args.dive),
                          "donor_sha256": sha(args.word7), "samples": panel.rows}, indent=2))


if __name__ == "__main__":
    main()
