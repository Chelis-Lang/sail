#!/usr/bin/env python3
"""Two sealed stock-only filtered controls; no qualification or retry policy."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time


EXPECTED = {
    "verus": "2fe2fca366846ec5aaaaaf80edf061f7d19564c097cb9840c7638719e8bc1854",
    "rust_verify": "b8c11e7b626cc85d8a46b90036f0adfec443b7cde1c69d8b182175b92c708af7",
    "z3": "e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765",
}


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def inventory(root):
    return [{"path": str(path.relative_to(root)), "bytes": path.stat().st_size,
             "sha256": sha(path)} for path in sorted(root.rglob("*")) if path.is_file()]


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def run(args):
    root = args.out.absolute()
    root.mkdir(parents=True, exist_ok=False)
    shutil.copytree(args.tasks, root / "project/tasks")
    source = root / "project/tasks"
    before = inventory(source)
    binaries = {name: args.stock.parent / name for name in EXPECTED}
    runtime = {name: {"bytes": path.stat().st_size, "sha256": sha(path)}
               for name, path in binaries.items()}
    assert all(runtime[name]["sha256"] == expected for name, expected in EXPECTED.items()), "sealed stock runtime changed"
    assert sha(source / "lib.rs") == "dc3453e0f9681725a27af19460a1dc08450097c7fb2cbd06844b57f808fee6b8"
    assert sha(source / "human_eval_013.rs") == "32e44a6ce4ca51c0c894bbffda850b14cbb8569a7fb2a9eaf5964226ac818d28"
    environment = {key: value for key, value in os.environ.items() if not key.startswith("SAIL_ENGINE")}
    environment.update(VERUS_Z3_PATH=str(binaries["z3"]), RUSTUP_TOOLCHAIN="1.98.1")
    command = [str(args.stock), str(source / "lib.rs"), "--crate-type", "lib", "--crate-name", "human_eval_verus",
               "-V", "use-crate-name", "--verify-only-module", "human_eval_013", "--output-json", "--time",
               "--no-report-long-running", "--log", "air", "--log", "smt-transcript", "--log-dir", str(root / "logs")]
    summary = {"version": 1, "scope": "Two fresh stock-only processes; filtered progress-order observation, not source qualification",
               "controller_sha256": sha(Path(__file__)), "runtime": runtime, "source_inventory": before,
               "command": command, "runs": [], "controls_completed": False}
    save(root / "summary.json", summary)
    documents, errors = [], []
    for index in range(2):
        work = root / str(index)
        work.mkdir()
        assert inventory(source) == before, "source changed before actor"
        started = time.monotonic()
        with subprocess.Popen(command, cwd=root, env=environment, stdout=subprocess.PIPE,
                              stderr=subprocess.PIPE, start_new_session=True) as process:
            stat = Path(f"/proc/{process.pid}/stat").read_text().rsplit(") ", 1)[1].split()
            save(work / "actor.json", {"pid": process.pid, "starttime_ticks": int(stat[19]),
                                      "started_monotonic_ns": int(started * 1_000_000_000), "command": command})
            timed_out = False
            try:
                stdout, stderr = process.communicate(timeout=max(0, 600 - (time.monotonic() - started)))
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGKILL)
                stdout, stderr = process.communicate()
            (work / "stdout").write_bytes(stdout)
            (work / "stderr").write_bytes(stderr)
            item = {"index": index, "exit_code": process.returncode, "timed_out": timed_out,
                    "deadline_seconds": 600, "elapsed_seconds": time.monotonic() - started,
                    "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                    "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                    "stderr_sha256": hashlib.sha256(stderr).hexdigest(), "source_unchanged": inventory(source) == before}
            summary["runs"].append(item)
            save(root / "summary.json", summary)
            if (root / "logs").exists():
                (root / "logs").rename(work / "logs")
            assert not timed_out and process.returncode == 0, "stock control failed; raw evidence retained"
            assert item["source_unchanged"], "source changed during stock control"
            documents.append(json.loads(stdout))
            errors.append(stderr)
    assert all(sha(path) == EXPECTED[name] for name, path in binaries.items()), "runtime changed during controls"
    assert documents[0]["verification-results"] == documents[1]["verification-results"]
    assert documents[0]["func-details"] == documents[1]["func-details"]
    summary.update(controls_completed=True, runtime_unchanged=True,
                   verdicts_equal=True, exact_stderr_equal=errors[0] == errors[1],
                   observed_progress_order_difference=errors[0] != errors[1])
    save(root / "summary.json", summary)
    print(json.dumps({key: value for key, value in summary.items() if key not in ["command", "source_inventory"]}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock", type=Path, required=True)
    parser.add_argument("--tasks", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    run(parser.parse_args())
