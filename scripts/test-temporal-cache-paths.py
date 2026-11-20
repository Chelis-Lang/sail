#!/usr/bin/env python3
"""Check receipt sharing across the installed native CLI and its default daemon."""
from contextlib import contextmanager
import json
import os
import shutil
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def run(sail, base):
    cache = base / "cache"
    assert not cache.exists()
    env = dict(os.environ, SAIL_CACHE_DIR=str(cache))
    fixture = Path(__file__).resolve().parents[1] / "crates/sail-cli/src/temporal_warmup.json"
    doc = json.loads(fixture.read_bytes())
    doc["input"]["job"]["wall_time_ms"] = 10_000
    request = base / "job.json"

    def check(action, *args):
        output = subprocess.run(
            [sail, "runtime", action, "--file", request, *args],
            env=env, capture_output=True, check=True, timeout=20,
        )
        result = json.loads(output.stdout)["output"]["result"]
        assert result["outcome"] == "completed" and result["response"]["outcome"] == "holds", result
        return result

    @contextmanager
    def running(args, endpoint=None):
        with subprocess.Popen(
            [sail, "serve", *args], env=env, cwd=base, umask=0o002,
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
        ) as daemon:
            try:
                deadline = time.monotonic() + 15
                while time.monotonic() < deadline:
                    if daemon.poll() is not None:
                        raise RuntimeError(daemon.stderr.read().decode())
                    sockets = [endpoint] if endpoint else list(cache.glob("daemon-v*-workspace-v2-proof-v1.sock"))
                    if sockets:
                        [socket] = sockets
                        ready = subprocess.run(
                            [sail, "runtime", "capabilities", "--socket", socket],
                            env=env, capture_output=True, timeout=5,
                        )
                        if ready.returncode == 0:
                            assert json.loads(ready.stdout)["temporal"]["installed"], ready.stdout
                            yield socket
                            return
                    time.sleep(0.05)
                raise RuntimeError("native runtime endpoint did not start")
            finally:
                daemon.terminate()
                try:
                    daemon.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    daemon.kill()
                    daemon.wait()

    with running([]) as socket:
        assert cache.stat().st_mode & 0o777 == 0o700
        request.write_text(json.dumps(doc))
        assert not check("execute", "--socket", socket)["cache_hit"]
        embedded = check("execute")
        assert embedded["cache_hit"]
        key = embedded["cache_key"]
        assert (cache / "runtime/temporal-v1/v2" / key[:2] / key).is_file()

        # A new byte identity, prepared through the embedded installer path.
        doc["input"]["job"]["request"]["model"]["files"]["Warm.cfg"] += "\n"
        request.write_text(json.dumps(doc))
        assert not check("prepare-temporal")["cache_hit"]
        served = check("execute", "--socket", socket)
        assert served["cache_hit"] and served["cache_key"] != key
        assert not (cache / "results/runtime/temporal-v1").exists()

        custom = base / "explicit"
        assert not check("execute", "--cache", custom)["cache_hit"]
        assert check("execute", "--cache", custom)["cache_hit"]
        assert (custom / "temporal-v1").is_dir()

    # The regular solver cache is eligible for unsigned CI restore. Alternate
    # spellings of it select the same trusted temporal store on both interfaces.
    (base / "replies").symlink_to(cache / "results", target_is_directory=True)
    custom_socket = base / "alias.sock"
    with running(["--socket", custom_socket, "--cache", "./replies"], custom_socket):
        assert check("execute", "--socket", custom_socket)["cache_hit"]
    assert check("execute", "--cache", base / "replies")["cache_hit"]
    alternate = base / "replies" / "nested"
    assert not check("execute", "--cache", alternate)["cache_hit"]
    assert check("execute", "--cache", alternate)["cache_hit"]
    assert not list((cache / "results").rglob("temporal-v1"))


if __name__ == "__main__":
    with tempfile.TemporaryDirectory(prefix="sail-rt-", dir="/tmp") as directory:
        base = Path(directory)
        if len(sys.argv) == 2:
            sail = Path(sys.argv[1]).resolve()
        elif len(sys.argv) == 4 and sys.argv[1] == "--stage":
            install = base / "bin"
            install.mkdir()
            sail = install / "sail"
            shutil.copy2(sys.argv[2], sail)
            stage = Path(__file__).with_name("stage-temporal-worker.py")
            subprocess.run([sys.executable, stage, sys.argv[3], install], check=True)
        else:
            raise SystemExit("usage: test-temporal-cache-paths.py STAGED_SAIL | --stage SAIL WORKER")
        run(sail, base)
        subprocess.run([sys.executable, Path(__file__).with_name("benchmark-guarded-refutation.py"),
                        sail, "--smoke"], check=True)
    print("native daemon, embedded runtime and preparation share exact receipts")
