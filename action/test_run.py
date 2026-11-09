#!/usr/bin/env python3
"""Tests the action's verification step against a real `sail` and recorded
result documents.

`sail verify` is replaced by a stand-in that prints a recorded document and
exits with a chosen code; `sail report` is the real one. Usage:

    python3 action/test_run.py path/to/sail
"""
import json
import os
import stat
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE.parent / "crates/sail-cli/tests/fixtures/ci-report"

STAND_IN = """#!/bin/sh
# `verify` replays a recorded document; everything else is the real sail.
if [ "$1" = verify ]; then
  echo "error: recorded diagnostics would appear here" >&2
  [ -n "$FAKE_DOCUMENT" ] && cat "$FAKE_DOCUMENT"
  exit "$FAKE_EXIT"
fi
exec "$REAL_SAIL" "$@"
"""


def run(real, root, document, code, **extra):
    """Runs the step; returns (process, outputs, summary text, out directory)."""
    directory = Path(tempfile.mkdtemp(prefix="case-", dir=root))
    fake = directory / "sail"
    fake.write_text(STAND_IN)
    fake.chmod(fake.stat().st_mode | stat.S_IEXEC)
    environment = {
        "PATH": os.environ["PATH"],
        "HOME": os.environ.get("HOME", ""),
        "SAIL_CACHE_DIR": str(directory / "cache"),
        "SAIL_ACTION_SAIL": str(fake),
        "REAL_SAIL": str(real),
        "FAKE_DOCUMENT": str(FIXTURES / document) if document else "",
        "FAKE_EXIT": str(code),
        "SAIL_ACTION_OUT": str(directory / "out"),
        "SAIL_ACTION_ANNOTATIONS": "true",
        "SAIL_ACTION_ARGS": "-p alpha -- --rlimit '20 30'",
        "GITHUB_WORKSPACE": "/work/repo",
        "GITHUB_OUTPUT": str(directory / "output"),
        "GITHUB_STEP_SUMMARY": str(directory / "summary"),
        "GITHUB_SERVER_URL": "https://github.com",
        "GITHUB_REPOSITORY": "example/demo",
        "GITHUB_SHA": "0123abc",
    }
    environment.update(extra)
    process = subprocess.run([sys.executable, str(HERE / "run.py")], env=environment,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                             check=False)
    outputs = {}
    if (directory / "output").exists():
        for line in (directory / "output").read_text().splitlines():
            name, _, value = line.partition("=")
            outputs[name] = value
    summary = (directory / "summary").read_text() if (directory / "summary").exists() else ""
    return process, outputs, summary, directory


def main():
    real = Path(sys.argv[1]).resolve()
    with tempfile.TemporaryDirectory(prefix="sail-action-test-") as root:
        # A failing verification: the step succeeds, the code is recorded,
        # every located error is annotated and the files exist.
        process, outputs, summary, directory = run(real, root, "failing.json", 101)
        assert process.returncode == 0, process.stderr
        assert outputs["exit-code"] == "101", outputs
        annotations = [l for l in process.stdout.splitlines() if l.startswith("::error ")]
        assert len(annotations) == 6, process.stdout
        assert all(l.startswith("::error file=proofs/alpha/src/lib.rs,line=") for l in annotations)
        assert summary.startswith("## Sail verification: failed\n"), summary
        assert "(https://github.com/example/demo/blob/0123abc/proofs/alpha/src/lib.rs#L17)" in summary
        assert "Compared with the baseline" not in summary
        sarif = json.loads(Path(outputs["sarif"]).read_text())
        assert len(sarif["runs"][0]["results"]) == 6
        assert json.loads(Path(outputs["result"]).read_text())["exit_code"] == 101
        assert "recorded diagnostics would appear here" in process.stderr

        # A push saves its result; a later run compares with it and says so.
        saved = directory / "baseline" / "result.json"
        process, outputs, summary, _ = run(real, root, "passing.json", 0,
                                           SAIL_ACTION_SAVE_BASELINE=str(saved),
                                           SAIL_ACTION_BASELINE=str(saved))
        assert outputs["exit-code"] == "0" and saved.is_file(), outputs
        assert "::error" not in process.stdout, process.stdout
        assert "No saved result was found for the base commit" in summary, summary
        commit = "f" * 40
        process, outputs, summary, _ = run(real, root, "failing.json", 101,
                                           SAIL_ACTION_BASELINE=str(saved),
                                           SAIL_ACTION_BASELINE_KEY="sail-result-Linux-X64-" + commit,
                                           SAIL_ACTION_BASELINE_COMMIT=commit)
        assert "**5 regressed, 0 became unknown, 0 recovered**" in summary, summary
        assert "_Baseline: the result saved for base commit `ffffffffffff`._" in summary, summary
        process, outputs, summary, _ = run(real, root, "failing.json", 101,
                                           SAIL_ACTION_BASELINE=str(saved),
                                           SAIL_ACTION_BASELINE_KEY="sail-result-Linux-X64-older",
                                           SAIL_ACTION_BASELINE_COMMIT=commit)
        assert "_Baseline: the most recent saved result (`sail-result-Linux-X64-older`)" in summary

        # No document at all is an error, never a pass, whatever the code.
        for code in (0, 2):
            process, outputs, summary, _ = run(real, root, "", code)
            assert process.returncode == 0, process.stderr
            assert outputs["exit-code"] == str(code or 1), outputs
            assert outputs["result"] == "" and outputs["sarif"] == "", outputs
            assert "::error title=Sail action::sail verify wrote no result document" in process.stdout

        # A document `sail report` refuses: a warning, and the code is kept.
        broken = Path(root) / "broken.json"
        broken.write_text('{"kind": "other"}\n')
        process, outputs, summary, _ = run(real, root, "failing.json", 101,
                                           FAKE_DOCUMENT=str(broken))
        assert outputs["exit-code"] == "101", outputs
        assert "::warning title=Sail action::sail report --format github-annotations failed (exit 2)" in process.stdout
        assert outputs["sarif"] == "", outputs

        # The helpers the install step uses.
        version = subprocess.check_output(
            [sys.executable, str(HERE / "run.py"), "--workspace-version",
             str(HERE.parent / "Cargo.toml")], text=True).strip()
        assert version and version[0].isdigit(), version
        scopes = {subprocess.check_output([sys.executable, str(HERE / "run.py"), "--scope"],
                                          env={**os.environ, "SAIL_ACTION_ARGS": args},
                                          text=True).strip() for args in ("-p a", "-p b")}
        assert len(scopes) == 2 and all(len(s) == 16 for s in scopes), scopes
    print("action step: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
