#!/usr/bin/env python3
"""Run the unchanged repair fixture once, retaining its owned scratch and timers."""

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import signal
import subprocess
import tempfile
import time

FIXTURE_SHA = "9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7"
PRODUCT_SHA = "36d018f1a52b95a5df158c58f7f66adabfa2b4979003eae45927b7f3ea0fbf53"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ("fixture", "sail", "engine-build", "verus-source", "out"):
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--environment-label", required=True)
    args = parser.parse_args()
    output = Path(args.out)
    output.mkdir(mode=0o700)
    assert hashlib.sha256(Path(args.fixture).read_bytes()).hexdigest() == FIXTURE_SHA
    assert hashlib.sha256(Path(args.sail).read_bytes()).hexdigest() == PRODUCT_SHA
    spec = importlib.util.spec_from_file_location("original_repair_fixture", args.fixture)
    fixture = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(fixture)
    original_temporary = tempfile.TemporaryDirectory
    original_popen = subprocess.Popen
    owned_daemons = []
    retained = []

    class RetainedTemporary:
        def __init__(self, *positional, **keywords):
            assert not positional and keywords == {"prefix": "sr-", "dir": "/tmp"}
            self.path = tempfile.mkdtemp(**keywords)
            retained.append(self.path)
            (output / "retained-fixture.json").write_text(json.dumps(retained) + "\n")

        def __enter__(self):
            return self.path

        def __exit__(self, *_):
            # Preserve the same paths in place, after the fixture's own cleanup.
            return False

    def observed_popen(command, *positional, **keywords):
        child = original_popen(command, *positional, **keywords)
        if command[:2] == [args.sail, "serve"]:
            owned_daemons.append(child)
            stat = Path(f"/proc/{child.pid}/stat").read_text()
            birth = stat[stat.rfind(")") + 2:].split()[19]
            (output / "daemon-birth.json").write_text(json.dumps({
                "pid": child.pid, "start_ticks": birth,
                "observation": "owned unreaped child immediately after original spawn",
            }) + "\n")
        return child

    def interrupted(_signum, _frame):
        raise KeyboardInterrupt

    for signum in (signal.SIGTERM, signal.SIGHUP):
        signal.signal(signum, interrupted)
    tempfile.TemporaryDirectory = RetainedTemporary
    subprocess.Popen = observed_popen
    started = time.monotonic()
    status = "FAIL"
    error = None
    cleanup = []
    try:
        fixture.run(argparse.Namespace(
            sail=args.sail, engine_build=args.engine_build,
            verus_source=args.verus_source, out=str(output / "public"),
            timeout=120, environment_label=args.environment_label,
        ))
        status = "PASS"
    except BaseException as failure:
        error = {"type": type(failure).__name__, "message": str(failure)}
        raise
    finally:
        # Original cleanup normally already reaped these actual child handles.
        # Covers an exception between original daemon spawn and its inner try.
        for child in owned_daemons:
            if child.poll() is None:
                try:
                    fixture.stop_owned(child, 10)
                except BaseException as failure:
                    cleanup.append(type(failure).__name__)
        tempfile.TemporaryDirectory = original_temporary
        subprocess.Popen = original_popen
        (output / "diagnostic-result.json").write_text(json.dumps({
            "status": status, "error": error, "elapsed_seconds": time.monotonic() - started,
            "fixture_sha256": FIXTURE_SHA, "product_sha256": PRODUCT_SHA,
            "fixture_paths": retained, "cleanup_errors": cleanup,
            "owned_daemon_returncodes": [p.returncode for p in owned_daemons],
            "qualification_credit": False, "orphan_absence_certified": False,
            "change": "retain original scratch; observe owned daemon birth; outer interruption cleanup",
        }, indent=2) + "\n")


if __name__ == "__main__":
    main()
