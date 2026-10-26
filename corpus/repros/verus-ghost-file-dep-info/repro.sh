#!/usr/bin/env bash
# Files read only by Verus's verification phase (a `#[cfg(verus_keep_ghost)]`
# module, an `include!` inside a proof) are missing from the crate's final
# dep-info (VI-0037). For each file: verify, edit it so the proof fails, verify
# again in the same target and in a new one. With SAIL, also run
# `SAIL verify --output json` before and after the edit.
#   usage: repro.sh CARGO_VERUS [SAIL] [WORK_DIR]
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
cargo_verus=${1:?CARGO_VERUS}
sail=${2:-}
work=${3:-$(mktemp -d)}
mkdir -p "$work"

failures() { grep -cE '^error' "$1"; }
sail_summary() { # result JSON: exit, currentness, requested target's reused/complete
  python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); t=[t for t in d["targets"] if t["requested"]][0]
print("currentness", d["currentness"], "reused", t["reused"], "complete", t["complete"], "verifier_invocations", d["execution"]["verifier_invocations"])' "$1"
}

for file in value.rs ghost.rs; do
  case "$file" in
    value.rs) edit='s/2/3/' ;;
    ghost.rs) edit='s/1 + 1 == 2/1 + 1 == 3/' ;;
  esac
  dir="$work/$file"
  rm -rf "$dir"; mkdir -p "$dir"
  (cd "$dir" && "$cargo_verus" verus new --lib ghost_files >/dev/null 2>&1) || exit 1
  cp "$here/lib.rs" "$here/ghost.rs" "$here/value.rs" "$dir/ghost_files/src/"
  cd "$dir/ghost_files" || exit 1
  echo "== $file"
  "$cargo_verus" verus verify --target-dir "$dir/target" >"$dir/plain-1.txt" 2>&1
  echo "   plain, original:            exit $?, errors $(failures "$dir/plain-1.txt")"
  echo "   final dep-info names $file: $(grep -c "src/$file" "$dir"/target/debug/deps/ghost_files-*.d | paste -sd+ -)"
  if [ -n "$sail" ]; then
    SAIL_CACHE_DIR="$dir/cache" "$sail" verify --output json >"$dir/sail-1.json" 2>"$dir/sail-1.err"
    echo "   sail, original:             exit $?, $(sail_summary "$dir/sail-1.json")"
  fi
  sed -i.bak "$edit" "src/$file" && rm "src/$file.bak"
  "$cargo_verus" verus verify --target-dir "$dir/target" >"$dir/plain-2.txt" 2>&1
  echo "   plain, edited, same target: exit $?, errors $(failures "$dir/plain-2.txt")"
  "$cargo_verus" verus verify --target-dir "$dir/fresh" >"$dir/plain-3.txt" 2>&1
  echo "   plain, edited, new target:  exit $?, errors $(failures "$dir/plain-3.txt")"
  if [ -n "$sail" ]; then
    SAIL_CACHE_DIR="$dir/cache" "$sail" verify --output json >"$dir/sail-2.json" 2>"$dir/sail-2.err"
    echo "   sail, edited:               exit $?, $(sail_summary "$dir/sail-2.json")"
  fi
  cd "$work" || exit 1
done
