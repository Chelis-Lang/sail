#!/usr/bin/env python3
"""Reproduce the pinned Cargo launcher no-vstd mismatch without a workaround."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--source", required=True, help="complete pinned Verus source checkout")
parser.add_argument("--cargo-verus", required=True, help="pinned installed cargo-verus executable")
parser.add_argument("--out", required=True)
args = parser.parse_args()
source = Path(args.source).resolve() / "source"
out = Path(args.out).resolve()
out.mkdir(parents=True, exist_ok=True)
with tempfile.TemporaryDirectory(prefix="cargo-no-vstd-") as temporary:
    root = Path(temporary)
    (root / "src").mkdir()
    (root / "src/lib.rs").write_bytes(Path(__file__).with_name("input.rs").read_bytes())
    manifest = '[package]\nname="repair_demo"\nversion="0.1.0"\nedition="2021"\n[package.metadata.verus]\nverify=true\nno-vstd=true\n[dependencies]\n'
    for name, directory in (("verus_builtin", "builtin"), ("verus_builtin_macros", "builtin_macros")):
        manifest += f'{name}={{path={json.dumps(str(source / directory))}}}\n'
    (root / "Cargo.toml").write_text(manifest)
    environment = dict(os.environ, RUSTUP_TOOLCHAIN="1.98.1")
    environment.pop("VSTD_KIND", None)
    process = subprocess.Popen([str(Path(args.cargo_verus).resolve()), "verify", "--package", "repair_demo", "--lib", "--", "--output-json", "--time"], cwd=root, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(timeout=120)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        stdout, stderr = process.communicate(timeout=10)
        (out / "stdout").write_bytes(stdout)
        (out / "stderr").write_bytes(stderr)
        raise
    (out / "stdout").write_bytes(stdout)
    (out / "stderr").write_bytes(stderr)
    print(f"Cargo-Verus exit: {process.returncode}")
    raise SystemExit(process.returncode)
