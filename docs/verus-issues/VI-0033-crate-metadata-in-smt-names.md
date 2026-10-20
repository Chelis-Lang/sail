# VI-0033: The fork's closure and alias names carry Cargo's crate metadata

- **Status:** a defect of the fork's own renamings (VI-0019 closure names,
  VI-0030 function aliases), not of stock Verus. Fixed by fork commit
  `bd79859c` (first written as `912052eb` on `perf/7d82994`), in the Linux
  runtime pin `f8589ed4` (`engine-runtime-f8589ed4-linux`). Not applicable
  upstream.
- **Runtime:** fork `9c0ca7e75f20c8ae7120196efd760b64a61251aa` (Verus
  `0.2026.10.18.7d82994`, Rust 1.99.1); first observed on `cbbcdf44`.
- **Found:** 2026-10-19 (issue #247), reduced 2026-10-20.
- **Reproduction:** [verus-macro-profile-names](../../corpus/repros/verus-macro-profile-names/).
- **Impact:** default-mode cache keys bind exact solver bytes (ADR 0008).
  A project whose Cargo profile builds Verus's proc-macro crates differently
  (for example `[profile.dev.build-override] opt-level = 3`) gets different
  solver input for every query that mentions a closure type or an aliased
  trait function, so it misses every such cache entry. Each build is
  byte-stable on its own, so the determinism check cannot see it. Verdicts
  do not change.
- **Workaround:** none in Sail (no query normalization).

## Cause

Cargo computes each unit's `-C metadata` hash from its profile and from the
metadata of every dependency. Changing only the proc-macro crates' profile
therefore changes the metadata, and so rustc's stable crate ID, of every crate
that depends on them, including the verified crate. rustc seeds the local
half of each `DefPathHash` with that ID too.

The fork spelled both into SMT names:

- closure types as `anonymous_closure%<stable crate ID>_<local hash>` and
  `impl_closure&%<kind><stable crate ID>_<local hash>` (VI-0019, ADR 0049,
  `vir::def::prefix_closure_type` and `impl_closure`);
- function aliases as the hex of a key that contains each path's stable
  crate ID (VI-0030, ADR 0056 candidate, `vir::sail_smt_names`).

Stock Verus prints crate names without the ID (`NameCtxt` disambiguates
same-named crates by counter) and is unaffected: stock `7d82994` gives
byte-identical SMT logs for `-C metadata=aaa` and `-C metadata=bbb`, the fork
`9c0ca7e7` does not. A plain Cargo workspace confirms the propagation: adding
`[profile.dev.build-override] opt-level = 3` changed the dependent library's
`-C metadata` from `77cabe5ade4ebe80` to `bfb0fdbc41ace8b9`.

## Reproduction

`repro.sh <built Verus source dir>` verifies the one-crate `lib.rs` (one
closure, one trait impl) with `cargo verus verify` twice, with the
proc-macro crates at opt-level 0 and 3, and compares the `--log smt` trees.
On fork `9c0ca7e7` (local Linux x86-64, 2026-10-20) the crate's
`root.smt2` differs (both arms verify 2 functions, vstd's 427 logs are
equal):

```
< (declare-const fuel%%sail%fn%…69640000001076695f6d6163726f5f70726f66696c65000000086e59fd94872e55a4…
> (declare-const fuel%%sail%fn%…69640000001076695f6d6163726f5f70726f66696c65000000089bad7d20fb2b9298…
```

and the closure appears as `anonymous_closure%11217759827456135832_…` in
one arm and `anonymous_closure%7951665431464269220_…` in the other. Reruns
of either arm are byte-identical. With `912052eb` it prints
`SMT logs identical`.

## Fix

`912052eb` keeps the full `DefPathHash` pair for closure equality and
hashing (ADR 0049's identity) and spells names from the crate name and the
crate-relative definition path, encoded injectively
(`anonymous_closure%<crate>%<path>`, for example
`anonymous_closure%lib%v&apply.closure!1`). Two distinct closures can share
a spelling only when their crates share a name; the later one in
simplification order then keeps the identity pair in its name. Closure
declaration ties are broken by spelling before the ID. The alias key uses the
crate name only; same-named crates can then produce equal keys, which the
existing rule keeps on legacy names. The change is a renaming (below).

On Vest (native c7i.8xlarge, us-east-1, 2026-10-20; plain `cargo verus
verify`, one fresh target per arm, all 631 solver sessions captured), the
proc-macro crates at opt-level 0 and 3 give 103 of 631 differing sessions on
`9c0ca7e7` and identical session multisets on `912052eb` and on its
successor `51afcba6`; reruns of opt-level 0 are identical on every build.
Every arm reports 3,236 verified and the one known `parse_cbor_with_child`
rlimit error ([compatibility](../compatibility.md)). The opt-level 3 arms
take about 8 s longer, building the macros.

Across the fast tier's 147 direct units, `912052eb` changes the solver input
of 21 units and leaves stdout (build identity blanked), stderr and exit codes
identical to `9c0ca7e7`'s. Over every example's `--log smt` output, 335 files
are identical, 20 differ only by the renaming and 3 also swap the
`declare-sort` lines of their two closures; with that permutation the
commands are equal. Those sorts are emitted in name order (`prune` sorts
the abstract monomorphic types), so they follow the new names, as they
followed the hash names before; the `TYPE%` declarations keep `DefIndex`
order. See the [validation record](../validation/fork-perf-7d82994.md).

The fork commits predate this entry's number: commit messages, code
comments and the regression test name it VI-0032 (`sail_vi0032_metadata_names`;
VI-0032 here is the Vest rlimit entry). The rename lands with the next fork
change. The regression test verifies one source with
two `-C metadata` values and requires identical SMT logs; it fails on
`9c0ca7e7`.

This amends ADR 0049's naming rule and the ADR 0056 candidate's key; both
amendments are accepted by the lead for the R3 candidate; owner review is pending (see the notes in those ADRs).
