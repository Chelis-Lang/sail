#!/usr/bin/env python3
"""Check ordinary parent-variable changes against the actual managed proof path."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time
import tomllib


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new evidence directory")
    parser.add_argument("--sail", required=True, type=Path, help="candidate binary beside sail-z3")
    parser.add_argument("--build", required=True, type=Path, help="existing pinned engine build")
    parser.add_argument("--cargo-home", required=True, type=Path, help="prepared registry home")
    args = parser.parse_args()
    fixture = Path(__file__).resolve().parent
    repo = fixture.parents[2]
    pin = tomllib.loads((repo / "corpus/toolchain.toml").read_text())["verus"]
    fork = tomllib.loads((repo / "engine/verus/fork.toml").read_text())["fork"]
    build, binary = args.build.resolve(), args.sail.resolve()
    manifest = json.loads((build / "sail-build.json").read_text())
    assert manifest["commit"] == pin["commit"]
    assert manifest["rust_toolchain"] == pin["rust_toolchain"]
    assert "sail-fork:" + fork["commit"] in manifest["features"]
    for relative, expected in manifest["artifacts"].items():
        assert digest(build / relative) == expected, relative
    runtime = build / "verus/source/target-verus/release"
    out = args.out.resolve()
    out.mkdir()
    project = out / "project"
    project.mkdir()
    for name in ["Cargo.toml", "Cargo.lock"]:
        shutil.copyfile(fixture / name, project / name)
    shutil.copytree(fixture / "src", project / "src")
    cache = out / "cache"
    cache.mkdir()
    sock = out / "cache.sock"
    env = {key: value for key, value in os.environ.items()
           if key in {"HOME", "USER", "PATH", "TMPDIR", "RUSTUP_HOME"}}
    env.update(CARGO_HOME=str(args.cargo_home.resolve()), RUSTUP_TOOLCHAIN=pin["rust_toolchain"],
               CARGO_NET_OFFLINE="true", CARGO_TARGET_DIR=str(out / "plain-target"),
               SAIL_CACHE_DIR=str(cache), SAIL_DAEMON_SOCKET=str(sock),
               SAIL_CARGO_VERUS=str(runtime / "cargo-verus"),
               VERUS_Z3_PATH=str(runtime / "z3"), SAIL_Z3_PATH=str(runtime / "z3"))
    report = {"scope": "actual managed ordinary-parent-variable regression; no complete compiler I/O claim",
              "binary_sha256": digest(binary), "shim_sha256": digest(binary.with_name("sail-z3")),
              "runtime_manifest_sha256": digest(build / "sail-build.json"),
              "source_sha256": digest(project / "src/lib.rs"), "fork_commit": fork["commit"], "cases": []}

    def save():
        (out / "report.json").write_text(json.dumps(report, indent=2))

    def run(name, command, current):
        proc = subprocess.Popen(command, cwd=project, env=current, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, start_new_session=True)
        timed_out = False
        try:
            stdout, stderr = proc.communicate(timeout=300)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(proc.pid, signal.SIGKILL)
            stdout, stderr = proc.communicate()
        (out / (name + ".stdout")).write_bytes(stdout)
        (out / (name + ".stderr")).write_bytes(stderr)
        row = {"name": name, "command": command, "exit": proc.returncode,
               "timed_out": timed_out, "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
               "stderr_sha256": hashlib.sha256(stderr).hexdigest()}
        report["cases"].append(row)
        save()
        assert not timed_out, name
        return proc.returncode, stdout

    with (out / "serve.stdout").open("wb") as stdout, (out / "serve.stderr").open("wb") as stderr:
        server = subprocess.Popen([str(binary), "serve", "--socket", str(sock), "--cache", str(cache / "solver"),
                                   "--idle-timeout", "1800"], cwd=project, env=env, stdout=stdout,
                                  stderr=stderr, start_new_session=True)
        try:
            deadline = time.monotonic() + 5
            while not sock.exists():
                assert server.poll() is None, "owned cache daemon exited"
                assert time.monotonic() < deadline, "cache daemon did not become ready"
                time.sleep(0.05)
            contexts = []
            for name, value in [("establish", "7"), ("unchanged", "7"), ("absent", None)]:
                current = dict(env)
                if value is not None:
                    current["ORDINARY_PROOF_FLAG"] = value
                status, raw = run(name, [str(binary), "verify", "--output", "json", "--lib"], current)
                result = json.loads(raw)
                targets = [target for target in result["targets"] if target["requested"]]
                assert len(targets) == 1
                target = targets[0]
                contexts.append(result["context_sha256"])
                assert result["currentness"] == "current"
                if value is not None:
                    assert status == 0 and target["complete"]
                    assert target["reused"] == (name == "unchanged")
                    assert any(crate["verus"]["verified"] == 1 and crate["verus"]["errors"] == 0
                               for crate in target["report"]["crates"])
                else:
                    assert status == 101 and not target["complete"] and not target["reused"]
                    assert any(crate["verus"]["verified"] == 0 and crate["verus"]["errors"] == 1
                               for crate in target["report"]["crates"])
            assert contexts[0] == contexts[1] and contexts[0] != contexts[2]
            status, _ = run("plain-absent", [str(runtime / "cargo-verus"), "verus", "verify",
                                              "--lib", "--", "--output-json", "--time"], env)
            assert status == 101
            report["regression_passed"] = True
        finally:
            if server.poll() is None:
                os.killpg(server.pid, signal.SIGTERM)
                try:
                    server.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(server.pid, signal.SIGKILL)
                    server.wait()
            report["owned_server_exit"] = server.returncode
            save()
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
