#!/usr/bin/env bash
# A variable read only in ghost code is missing from the crate's final
# dep-info, so Cargo-Verus keeps a verified crate fresh after it changes
# (VI-0036). CARGO_VERUS is the pinned stock `cargo-verus`, or Sail's fork.
#   usage: repro.sh CARGO_VERUS [WORK_DIR]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cargo_verus=${1:?CARGO_VERUS}
work=${2:-$(mktemp -d)}
mkdir -p "$work"
rm -rf "$work/ghost_env" "$work/target" "$work/fresh"
(cd "$work" && "$cargo_verus" verus new --lib ghost_env >/dev/null 2>&1) || exit 1
cp "$here/lib.rs" "$work/ghost_env/src/lib.rs"
cd "$work/ghost_env" || exit 1

# verify TARGET [VAR=VALUE]: verify into TARGET with the flag unset or set.
verify() {
  env -u ORDINARY_PROOF_FLAG "${@:2}" "$cargo_verus" verus verify --target-dir "$1" \
    >"$work/out.txt" 2>&1
  echo "   exit $?, assertion failures: $(grep -c 'assertion failed' "$work/out.txt")"
}

echo "1. flag present, new target:"
verify "$work/target" ORDINARY_PROOF_FLAG=7
echo "   final dep-info environment:"
grep -h '^# env-dep:' "$work"/target/debug/deps/ghost_env-*.d | sed 's/^/     /'
echo "2. flag removed, same target:"
verify "$work/target"
echo "3. flag removed, new target:"
verify "$work/fresh"
