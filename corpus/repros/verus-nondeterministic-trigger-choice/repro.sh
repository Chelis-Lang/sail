#!/usr/bin/env bash
# Runs the pinned Verus on examples/guide/traits.rs (from the pinned Verus
# source) N times with --log smt-transcript and reports, per run, the solver
# input digest and the automatically chosen triggers of the triangle-inequality
# quantifier `forall |x, y, z| x.dist(y) <= x.dist(z) + z.dist(y)`.
# See README.md.
# Usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]
set -euo pipefail
install="${1:?usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]}"
source_dir="${2:?usage: repro.sh <verus-install-dir> <verus-source-dir> [runs]}"
runs="${3:-6}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for i in $(seq 1 "$runs"); do
  VERUS_Z3_PATH="${VERUS_Z3_PATH:-$install/z3}" "$install/verus" --crate-type=lib \
    "$source_dir/examples/guide/traits.rs" --log-dir "$work/$i" --log smt-transcript \
    >/dev/null 2>&1
  python3 - "$work/$i/root.smt_transcript" <<'PY'
import hashlib, re, sys
raw = open(sys.argv[1], "rb").read()
# Only the bytes sent to Z3: drop the transcript's response sections.
sent = b"".join(part.split(b";;;<<<\n", 1)[0] for part in raw.split(b";;;>>> QUERY\n")[1:])
text = sent.decode(errors="replace")
pairs = re.findall(r":pattern \(\(traits!Distance\.dist\.\? Self%&\. Self%& (\w)\$ (\w)\$\)", text)
print(hashlib.sha256(sent).hexdigest()[:16], " ".join(a + b for a, b in pairs))
PY
done | sort | uniq -c
