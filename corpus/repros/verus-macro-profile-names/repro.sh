#!/usr/bin/env bash
# Verify one small crate twice with cargo verus, with Verus's proc-macro crates
# built at opt-level 0 and then 3 ([profile.dev.build-override]), and compare
# the SMT logs. VERUS_SOURCE is a built Verus source tree (its `source`
# directory: target-verus/release, vstd, builtin, builtin_macros and z3).
#   usage: repro.sh VERUS_SOURCE [WORK_DIR]
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
src=$(cd "${1:?VERUS_SOURCE}" && pwd)
work=${2:-$(mktemp -d)}
for opt in 0 3; do
  out="$work/o$opt"
  rm -rf "$out"; mkdir -p "$out/proj/src"
  cat >"$out/proj/Cargo.toml" <<TOML
[package]
name = "vi_macro_profile"
version = "0.1.0"
edition = "2021"

[dependencies]
vstd = { path = "$src/vstd", default-features = false, features = ["std"] }
verus_builtin = { path = "$src/builtin" }
verus_builtin_macros = { path = "$src/builtin_macros" }

[package.metadata.verus]
verify = true

[profile.dev.build-override]
opt-level = $opt
TOML
  cp "$here/lib.rs" "$out/proj/src/lib.rs"
  (cd "$out/proj" && VERUS_Z3_PATH="$src/z3" CARGO_TARGET_DIR="$out/target" \
    "$src/target-verus/release/cargo-verus" verus verify -p vi_macro_profile -- \
    --log smt --log-dir "$out/log" >"$out/out.txt" 2>&1)
  grep 'verification results' "$out/out.txt" | tail -1
done
if diff -r -q "$work/o0/log" "$work/o3/log"; then
  echo "SMT logs identical"
else
  diff "$work/o0/log/vi_macro_profile/root.smt2" "$work/o3/log/vi_macro_profile/root.smt2" | head -6 || true
fi
