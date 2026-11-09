#!/usr/bin/env python3
"""Spike A1: writes #664's minimal cases as plain `verus!` crate roots.

Run from the repository root. None imports `sail-contracts`; each is a whole
`src/lib.rs`. `repro_body_edit` is `corpus/repros/neutral-contract-unknown/
body_edit_verus.rs` byte for byte (emitted Verus source, vstd only), and
`repro_base` is the same file with the body returning `input + 1`.
"""
from pathlib import Path

OUT = Path("spikes/v030-mutev/minimal")
IDENTITY = "pub fn identity(b: bool) -> (r: bool)\n    ensures r == b,\n{ %s }"
INCREMENT = ("pub fn increment(x: u8) -> (r: u8)\n    requires x < 100,\n"
             "    ensures r == x + 1,\n{ %s }")
CASES = {
    "bool_id_base": IDENTITY % "b",
    "bool_id_false": IDENTITY % "false",
    "bool_id_true": IDENTITY % "true",
    "inc_base": INCREMENT % "x + 1",
    "inc_zero": INCREMENT % "0",
    "inc_one": INCREMENT % "1",
    # Not one of #664's cases: a false bit-vector assertion, whose query has
    # no quantified context, to see what a native `sat` looks like.
    "zz_bitvector_false": "pub fn low_bit(x: u8)\n{\n    assert(x & 1 == 1) by (bit_vector);\n}",
}

OUT.mkdir(parents=True, exist_ok=True)
for name, body in CASES.items():
    (OUT / f"{name}.rs").write_text(
        f"use vstd::prelude::*;\n\nverus! {{\n\n{body}\n\n}} // verus!\n")
repro = Path("corpus/repros/neutral-contract-unknown/body_edit_verus.rs").read_text()
(OUT / "repro_body_edit.rs").write_text(repro)
edit = "{ if input == 0 { return 1 ; } input }"
assert repro.count(edit) == 1
(OUT / "repro_base.rs").write_text(repro.replace(edit, "{ if input == 0 { return 1 ; } input + 1 }"))
