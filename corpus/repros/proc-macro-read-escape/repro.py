#!/usr/bin/env python3
"""Exercise ordinary proc-macro I/O and optional actual pinned-fork observations."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import tomllib


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="fresh evidence directory")
    parser.add_argument("--build", type=Path, help="optional existing pinned engine build root")
    args = parser.parse_args()
    fixture = Path(__file__).resolve().parent
    repo = fixture.parents[2]
    pin = tomllib.loads((repo / "corpus/toolchain.toml").read_text())["verus"]
    out = args.out.resolve()
    out.mkdir()
    shutil.copytree(fixture / "project", out / "project")
    shutil.copyfile(fixture / "proof.rs", out / "proof.rs")
    shutil.copyfile(fixture / "external-value.txt", out / "external-value.txt")
    project = out / "project"
    cargo_home = out / "cargo-home"
    cargo_home.mkdir()
    env = {key: value for key, value in os.environ.items()
           if key in {"HOME", "USER", "PATH", "TMPDIR", "RUSTUP_HOME"}}
    env.update(RUSTUP_TOOLCHAIN=pin["rust_toolchain"], CARGO_HOME=str(cargo_home),
               CARGO_TARGET_DIR=str(out / "target"))
    report = {
        "scope": "ordinary Cargo and optional standalone compiler; no default Sail acceptance claim",
        "platform": platform.platform(),
        "fixture_sha256": {str(path.relative_to(fixture)): digest(path)
                           for path in sorted(fixture.rglob("*")) if path.is_file()},
        "rust_toolchain": pin["rust_toolchain"],
        "cases": [],
    }

    def save():
        (out / "report.json").write_text(json.dumps(report, indent=2))

    def run(name, command, actual_env=None):
        process = subprocess.Popen(command, cwd=project, env=actual_env or env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   start_new_session=True)
        timeout = False
        try:
            stdout, stderr = process.communicate(timeout=120)
        except subprocess.TimeoutExpired:
            timeout = True
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
        row = {"name": name, "command": command, "exit": process.returncode, "timeout": timeout, "pid": process.pid}
        for stream, raw in [("stdout", stdout), ("stderr", stderr)]:
            path = out / (name + "." + stream)
            path.write_bytes(raw)
            row[stream] = {"path": str(path), "sha256": digest(path)}
        report["cases"].append(row)
        save()
        assert not timeout, f"{name}: process timeout; raw evidence retained"
        return row, stdout, stderr

    for name, expected in [("seven", b"7\n"), ("nine-cached", b"7\n"), ("nine-forced", b"9\n")]:
        if name == "nine-cached":
            (out / "external-value.txt").write_text("9\n")
        elif name == "nine-forced":
            (project / "src/main.rs").touch()
        row, stdout, _ = run(name, ["cargo", "run", "--offline", "--locked"])
        assert row["exit"] == 0 and stdout == expected, f"{name}: unexpected Cargo result"
    print("ordinary Cargo: 7 -> edited input, cached 7 -> forced expansion 9")
    declared_env = dict(env, CARGO_TARGET_DIR=str(out / "declared-target"))
    for value in [7, 9]:
        (out / "external-value.txt").write_text(f"{value}\n")
        row, stdout, _ = run(f"declared-{value}",
                             ["cargo", "run", "--offline", "--locked", "--features", "declared-input"],
                             declared_env)
        assert row["exit"] == 0 and stdout == f"{value}\n".encode(), "declared read did not rebuild"
    print("declared include_str! control: 7 -> edited input, rebuilt 9")
    if args.build is None:
        return

    build = args.build.resolve()
    host = "macos" if platform.system() == "Darwin" else "linux"
    artifact = json.loads((repo / f"engine/verus/artifact-{host}.json").read_bytes())
    manifest_path = build / "sail-build.json"
    assert digest(manifest_path) == artifact["manifest_sha256"], "unqualified runtime manifest"
    manifest = json.loads(manifest_path.read_bytes())
    for relative, expected in manifest["artifacts"].items():
        assert digest(build / relative) == expected, f"runtime file differs: {relative}"
    runtime = build / "verus/source/target-verus/release"
    assert digest(runtime / "z3") == artifact["z3_sha256"], "wrong Z3"
    report["runtime_sha256"] = {name: digest(runtime / name) for name in ["verus", "rust_verify", "z3"]}
    report["proof_sha256"] = digest(out / "proof.rs")
    report["macro_sha256"] = {}
    for variant, target in [("untracked", out / "target"), ("declared", out / "declared-target")]:
        libraries = list((target / "debug/deps").glob("libsail_read_escape-*.dylib" if host == "macos" else "libsail_read_escape-*.so"))
        assert len(libraries) == 1, "ambiguous actual proc-macro library"
        library = libraries[0]
        report["macro_sha256"][variant] = digest(library)
        for value, success in [(7, True), (9, False)]:
            (out / "external-value.txt").write_text(f"{value}\n")
            name = f"fork-{variant}-{value}"
            directory = out / name
            output = directory / "output"
            observation = directory / "observation"
            output.mkdir(parents=True)
            observation.mkdir()
            actual_env = dict(env, VERUS_Z3_PATH=str(runtime / "z3"),
                              SAIL_COMPILER_INPUTS=str(observation), SAIL_COMPILER_INPUTS_BINDING="0" * 64)
            command = [str(runtime / "verus"), str(out / "proof.rs"), "--crate-type=lib", "--compile",
                       "--output-json", "--time", "-Zchecksum-hash-algorithm=sha256",
                       "--emit=dep-info,metadata,link", "--out-dir", str(output),
                       "--extern", "sail_read_escape=" + str(library)]
            row, stdout, stderr = run(name, command, actual_env)
            result = json.loads(stdout)
            assert result["verus"]["commit"] == artifact["fork_commit"], "wrong actual verifier"
            verification = result["verification-results"]
            assert verification["is-verifying-entire-crate"] and not verification["encountered-vir-error"]
            assert verification["success"] == success
            assert verification["verified"] == int(success) and verification["errors"] == int(not success)
            assert (row["exit"] == 0) == success
            row["verification-results"] = verification
            if success:
                raw = observation / "inputs.json"
                document = json.loads(raw.read_bytes())
                assert document["pid"] == row["pid"], "wrong actual producer PID"
                assert document["complete"] and set(document["phases"]) == {"verification", "compilation"}
                row["phase_record_sha256"] = digest(raw)
                row["external_file_recorded"] = {
                    phase_name: str(project / "escape/../../external-value.txt") in list(phase["sources"]) + phase["extra_tracked_files"]
                    for phase_name, phase in document["phases"].items()
                }
                row["dep_info_contains_external_file"] = "external-value.txt" in (output / "proof.d").read_text()
                if variant == "declared":
                    assert row["external_file_recorded"]["verification"], "declared file omitted from verification observation"
                print(name, "records external file:", row["external_file_recorded"])
            else:
                assert b"assertion failed" in stderr, "failure is not the expected proof failure"
            assert digest(library) == report["macro_sha256"][variant] and digest(out / "proof.rs") == report["proof_sha256"]
            save()
    print("each unchanged actual macro library and proof: file 7 verifies; file 9 fails")



if __name__ == "__main__":
    main()
