#!/usr/bin/env python3
"""Reproduce the stock/fork HumanEval proof-search disagreement (#337)."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

PIN = "bbe93f73b0a5802c438501f9a2106f9126bd2eea"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stock", type=Path, required=True, help="stock runtime directory")
    parser.add_argument("--fork", type=Path, required=True, help="fork runtime directory")
    parser.add_argument("--work", type=Path, required=True, help="new private scratch directory")
    args = parser.parse_args()
    work = args.work.resolve()
    work.mkdir(parents=True, exist_ok=False)
    checkout = work / "checkout"
    subprocess.run(["git", "clone", "--quiet", "https://github.com/secure-foundations/human-eval-verus", str(checkout)], check=True)
    subprocess.run(["git", "-C", str(checkout), "checkout", "--quiet", PIN], check=True)
    src = work / "src"
    shutil.copytree(checkout, src, ignore=shutil.ignore_patterns(".git", "target"))
    path = src / "tasks/human_eval_110.rs"
    text = path.read_text()
    start = text.index("proof fn exists_even_index(")
    before, body = text[:start], text[start:]
    assert body.count("if lst.len() == 0 {") >= 1
    path.write_text(before + body.replace("if lst.len() == 0 {", "if lst.len() == 1 {", 1))
    (src / "Cargo.toml").write_text('''[package]
name = "human_eval_verus_mutants"
version = "0.0.0"
edition = "2021"
publish = false
[lib]
path = "tasks/lib.rs"
[package.metadata.verus]
verify = true
[workspace]
[dependencies]
vstd = { version = "=0.0.0-2026-10-18-0147" }
''')
    shutil.copyfile(Path(__file__).with_name("Cargo.lock"), src / "Cargo.lock")
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("SAIL_", "VERUS_")) and k not in (
               "CARGO", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
               "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS",
               "CARGO_BUILD_TARGET_DIR", "CARGO_ENCODED_RUSTFLAGS",
               "CARGO_MAKEFLAGS", "CARGO_MANIFEST_DIR", "CARGO_PKG_NAME",
               "CARGO_PRIMARY_PACKAGE", "CARGO_TARGET_DIR", "RUSTC",
               "RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "RUSTDOC")}
    env.update(RUSTUP_TOOLCHAIN="1.99.1", CARGO_TERM_QUIET="true",
               CARGO_TERM_COLOR="never", CARGO_TERM_PROGRESS_WHEN="never")
    rows = []
    for arm, runtime in (("stock", args.stock.resolve()), ("fork", args.fork.resolve())):
        command = [str(runtime / "cargo-verus"), "verus", "verify", "--locked",
                   "--fwd-verus-args-to", "roots", "--", "--output-json", "--time",
                   "--no-report-long-running", "--log", "smt", "--log-dir", str(work / arm)]
        with (work / (arm + ".stdout")).open("w") as out, (work / (arm + ".stderr")).open("w") as err:
            result = subprocess.run(command, cwd=src, env=env | {"CARGO_TARGET_DIR": str(work / ("target-" + arm))}, stdout=out, stderr=err, check=False)
        row = dict(arm=arm, exit=result.returncode, command=command)
        rows.append(row)
        (work / "results.json").write_text(json.dumps(rows, indent=2) + "\n")
        print(json.dumps(row), flush=True)
    return 0 if rows[0]["exit"] == rows[1]["exit"] == 0 else 2


if __name__ == "__main__":
    raise SystemExit(main())
