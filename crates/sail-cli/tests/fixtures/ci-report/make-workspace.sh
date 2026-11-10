#!/usr/bin/env bash
# Writes the two-member Verus workspace used to produce real result documents.
# Usage: make-workspace.sh DIR VARIANT   (VARIANT: pass | fail | compile | vir)
set -euo pipefail
root=$1
variant=$2
mkdir -p "$root/alpha/src" "$root/beta/src"
cat > "$root/Cargo.toml" <<'E'
[workspace]
members = ["alpha", "beta"]
resolver = "2"
E
for p in alpha beta; do
cat > "$root/$p/Cargo.toml" <<E
[package]
name = "$p"
version = "0.1.0"
edition = "2021"

[dependencies]
vstd = "=0.0.0-2026-10-18-0147"

[package.metadata.verus]
verify = true

[lints.rust]
unexpected_cfgs = { level = "warn", check-cfg = ['cfg(verus_only)'] }
E
done

cat > "$root/beta/src/lib.rs" <<'E'
use vstd::prelude::*;

verus! {

pub fn clamp(x: u32, lo: u32, hi: u32) -> (r: u32)
    requires
        lo <= hi,
    ensures
        lo <= r <= hi,
{
    if x < lo { lo } else if x > hi { hi } else { x }
}

} // verus!
E

case "$variant" in
pass)
cat > "$root/alpha/src/lib.rs" <<'E'
use vstd::prelude::*;

verus! {

pub open spec fn double(x: int) -> int {
    2 * x
}

pub fn max(a: u64, b: u64) -> (r: u64)
    ensures
        r >= a,
        r >= b,
{
    if a > b { a } else { b }
}

pub fn add_one(x: u64) -> (r: u64)
    requires
        x < 100,
    ensures
        r == x + 1,
{
    x + 1
}

pub fn count_to(n: u64) -> (r: u64)
    requires
        n < 1000,
    ensures
        r == n,
{
    let mut i: u64 = 0;
    while i < n
        invariant
            i <= n,
        decreases n - i,
    {
        i = i + 1;
    }
    i
}

pub fn call_add_one(x: u64) -> (r: u64)
    requires
        x < 50,
{
    add_one(x)
}

pub proof fn lemma_double_monotone(a: int, b: int)
    requires
        a <= b,
    ensures
        double(a) <= double(b),
{
}

} // verus!
E
;;
fail)
# Same functions; five distinct Verus failures and one rustc warning.
cat > "$root/alpha/src/lib.rs" <<'E'
use vstd::prelude::*;

verus! {

pub open spec fn double(x: int) -> int {
    2 * x
}

pub fn max(a: u64, b: u64) -> (r: u64)
    ensures
        r >= a,
        r > b,
{
    if a > b { a } else { b }
}

pub fn add_one(x: u64) -> (r: u64)
    ensures
        r == x + 1,
{
    x + 1
}

pub fn count_to(n: u64) -> (r: u64)
    requires
        n < 1000,
    ensures
        r == n,
{
    let mut i: u64 = 0;
    let unused = 3u8;
    while i < n
        invariant
            i < n,
        decreases n - i,
    {
        i = i + 1;
    }
    i
}

pub fn call_add_one(x: u64) -> (r: u64)
{
    let r = max(x, 7);
    assert(r == x);
    r
}

pub proof fn lemma_double_monotone(a: int, b: int)
    ensures
        double(a) <= double(b),
{
}

} // verus!
E
;;
compile)
cat > "$root/alpha/src/lib.rs" <<'E'
use vstd::prelude::*;

verus! {

pub fn max(a: u64, b: u64) -> (r: u64)
    ensures
        r >= a,
        r >= b,
{
    if a > b { a } else { "b" }
}

} // verus!
E
;;
vir)
# Compiles as Rust; Verus rejects it (a proof function calls an exec one).
cat > "$root/alpha/src/lib.rs" <<'E'
use vstd::prelude::*;

verus! {

pub fn add_one(x: u64) -> (r: u64)
    requires
        x < 100,
    ensures
        r == x + 1,
{
    x + 1
}

pub proof fn lemma_calls_exec(x: u64)
    requires
        x < 50,
{
    let y = add_one(x);
}

} // verus!
E
;;
*) echo "unknown variant $variant" >&2; exit 2 ;;
esac
