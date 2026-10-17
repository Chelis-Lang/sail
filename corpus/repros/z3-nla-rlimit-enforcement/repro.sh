#!/usr/bin/env bash
# Runs Verus on one crate next to this script, N times, each run capped at
# CAP seconds by killing its whole process group (Verus and Z3). Prints each
# run's wall time and outcome. See README.md.
# Usage: repro.sh <verus-install-dir> <crate.rs> [runs] [cap-secs] [verus args...]
set -euo pipefail
usage="usage: repro.sh <verus-install-dir> <crate.rs> [runs] [cap-secs] [verus args...]"
install="${1:?$usage}"; crate="${2:?$usage}"; runs="${3:-3}"; cap="${4:-900}"
shift $(( $# < 4 ? $# : 4 ))
here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
cp "$here/$crate" "$work/$crate"
cd "$work"
"$install/z3" --version
for _ in $(seq 1 "$runs"); do
  python3 - "$cap" "$install/verus" --crate-type=lib "$@" "$crate" <<'PY'
import os, re, signal, subprocess, sys, time
cap, cmd = float(sys.argv[1]), sys.argv[2:]
start = time.monotonic()
p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                     stdin=subprocess.DEVNULL, start_new_session=True)
try:
    out, _ = p.communicate(timeout=cap)
except subprocess.TimeoutExpired:
    os.killpg(p.pid, signal.SIGKILL)
    p.communicate()
    print(f"{time.monotonic() - start:.1f}s did not finish within {cap:.0f}s")
    sys.exit(0)
found = sorted(set(re.findall(rb"rlimit|verification results:: \d+ verified, \d+ errors", out)))
print(f"{time.monotonic() - start:.1f}s " + " ".join(f.decode() for f in found))
PY
done
