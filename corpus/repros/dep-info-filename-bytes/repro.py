#!/usr/bin/env python3
"""Show actual rustc filename bytes and the former Sail decoder's omission."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib


def old_dependencies(text):
    rule = text.replace("\\\n", "").splitlines()[0].split(": ", 1)[1]
    paths, word, escape = [], "", False
    for char in rule:
        if escape:
            word += char
            escape = False
        elif char == "\\":
            escape = True
        elif char.isspace():
            if word:
                paths.append(word)
                word = ""
        else:
            word += char
    if escape:
        return None
    if word:
        paths.append(word)
    return paths


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, type=Path, help="new evidence directory")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    pin = tomllib.loads((repo / "corpus/toolchain.toml").read_text())["verus"]
    out = args.out.resolve()
    out.mkdir()
    project = out / "project"
    project.mkdir()
    target = out / "target"
    target.mkdir()
    name = "left\u00a0right.txt"
    external = out / name
    external.write_text("7")
    (out / "left").write_text("unchanged decoy")
    (project / "right.txt").write_text("unchanged decoy")
    source = 'const _: () = assert!(include_str!("../left\\u{a0}right.txt").as_bytes()[0] == b\'7\');\n'
    (project / "lib.rs").write_text(source)
    env = {key: value for key, value in os.environ.items()
           if key in {"HOME", "USER", "PATH", "TMPDIR", "RUSTUP_HOME"}}
    env["RUSTUP_TOOLCHAIN"] = pin["rust_toolchain"]
    command = ["rustc", "--crate-name", "demo", "--crate-type", "lib",
               "--emit=dep-info,metadata", "--out-dir", str(target), "lib.rs"]
    version = subprocess.run(["rustc", "--version"], env=env, capture_output=True,
                             check=True, timeout=30).stdout.decode().strip()
    records = []
    for value in ["7", "9"]:
        external.write_text(value)
        result = subprocess.run(command, cwd=project, env=env, capture_output=True, timeout=60)
        (out / f"value-{value}.stdout").write_bytes(result.stdout)
        (out / f"value-{value}.stderr").write_bytes(result.stderr)
        records.append({"value": value, "exit": result.returncode,
                        "input_sha256": hashlib.sha256(external.read_bytes()).hexdigest()})
        if value == "7":
            assert result.returncode == 0, result.stderr.decode()
            raw = (target / "demo.d").read_text()
            (out / "successful.d").write_text(raw)
            decoded = old_dependencies(raw)
            assert f"../{name}" not in decoded
            assert "../left" in decoded and "right.txt" in decoded
            assert all((project / path).is_file() for path in decoded)
        else:
            assert result.returncode != 0 and b"error[E0080]" in result.stderr and b"assertion failed" in result.stderr
    report = {"scope": "actual rustc plus former decoder; no end-to-end Sail verdict claim",
              "rustc": version, "command": command, "source_sha256": hashlib.sha256(source.encode()).hexdigest(),
              "actual_input": f"../{name}", "former_decoded_inputs": decoded,
              "all_former_decoded_files_exist": True, "cases": records}
    (out / "report.json").write_text(json.dumps(report, indent=2))
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
