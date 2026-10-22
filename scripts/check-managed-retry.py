#!/usr/bin/env python3
"""A source edit during an actual receipt retry must produce a failed proof.

Usage: scripts/check-managed-retry.py SAIL_INSTALL NEW_OUTPUT_DIRECTORY
Requires Cargo and the pinned verifier toolchain. Uses an installed release,
a private project/cache, and an actual Cargo cleanup hook. Local acceptance
only: this is outside CI and timed benchmarks.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

if len(sys.argv) != 3:
    raise SystemExit("usage: check-managed-retry.py SAIL_INSTALL NEW_OUTPUT_DIRECTORY")
install, root = (Path(arg).resolve() for arg in sys.argv[1:])
root.mkdir(parents=True, exist_ok=False)
project = root / "project"
cache = root / "cache"
marker = root / "mutate-on-clean"
commands = root / "cargo-commands.jsonl"
cargo = shutil.which("cargo")
assert cargo, "Cargo is not on PATH"
wrapper = root / "sail-cargo-wrapper"
wrapper.write_text(f'''#!/usr/bin/env python3
import json, os
from pathlib import Path
cargo = {cargo!r}
marker = Path({str(marker)!r})
source = Path({str(project / 'src/lib.rs')!r})
commands = Path({str(commands)!r})
args = os.sys.argv[1:]
with commands.open('a') as log:
    log.write(json.dumps(args) + '\\n')
if 'clean' in args and marker.exists():
    text = source.read_text()
    assert 'assert(true)' in text
    source.write_text(text.replace('assert(true)', 'assert(false)', 1))
    marker.unlink()
os.execv(cargo, [cargo] + args)
''')
wrapper.chmod(0o755)
env = dict(os.environ, CARGO=str(wrapper), SAIL_CACHE_DIR=str(cache),
           RUSTUP_TOOLCHAIN="1.99.1")
for name in ("SAIL_ENGINE_BUILD", "SAIL_VERUS_BUILD", "CARGO_TARGET_DIR"):
    env.pop(name, None)
subprocess.run([str(install / "bin/sail"), "new", "--lib", str(project)],
               env=env, check=True, capture_output=True)
source = project / "src/lib.rs"
source.write_text("use vstd::prelude::*;\n"
                  "verus! { proof fn retry_guard() { assert(true); } }\n")


def run(label):
    """Keep the complete result and require the caller to check its verdict."""
    process = subprocess.run([str(install / "bin/sail"), "verify", "--output", "json"],
                             cwd=project, env=env, capture_output=True, timeout=300)
    (root / f"{label}.json").write_bytes(process.stdout)
    (root / f"{label}.stderr").write_bytes(process.stderr)
    result = json.loads(process.stdout)
    print(label, process.returncode, result["currentness"], result["execution"], flush=True)
    return process, result


process, result = run("baseline")
assert process.returncode == 0 and result["currentness"] == "current"
target = next(t for t in result["targets"] if t["requested"])
receipts = list((cache / "projects").glob(f'*/{result["context_sha256"]}/receipts/*.json'))
receipt = next(p for p in receipts
               if json.loads(p.read_text())["package_id"] == target["package_id"])
command_start = len(commands.read_text().splitlines())
receipt.unlink()
marker.touch()
process, result = run("retry-mutated")
assert not marker.exists(), "Control never reached actual Cargo cleanup"
calls = [json.loads(line) for line in commands.read_text().splitlines()[command_start:]]
clean = next(i for i, args in enumerate(calls) if args and args[0] == "clean")
checks = [i for i, args in enumerate(calls)
          if args and args[0] == "check" and "--unit-graph" not in args]
assert any(i < clean for i in checks) and any(i > clean for i in checks), \
    "Control did not bracket an actual receipt retry"
assert process.returncode != 0, "Source mutation during retry was falsely verified"
selected = [t for t in result["targets"] if t["requested"]]
assert len(selected) == 1 and not selected[0]["complete"]
assert selected[0]["report"]["summary"]["failed"] == 1
assert result["execution"]["verifier_invocations"] >= 1
assert result["currentness"] != "current"
assert "assert(false)" in source.read_text()
print("Actual retry source-mutation control passed", flush=True)
