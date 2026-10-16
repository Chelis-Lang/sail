# VI-0019: Inserting an item renames closure types crate-wide

- **Status:** open in stock Verus (tracked here; not filed upstream); fixed
  on Sail's Verus fork branch `m11-5/stable-closure-ids` by `8d03ab54` (fix:
  name closure types by definition path, not crate-wide index); not yet in
  Sail's pin. The fix renames symbols, so it is governed by
  [ADR 0049](../decisions/0049-stable-closure-names.md) rather than ADR
  0039's order rule.
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-15 (lane L40, [real-project report](../reports/2026-10-15-real-project-performance.md));
  reduced 2026-10-16 (lane M11-5, issue #118)
- **Reproduction:** `corpus/repros/verus-closure-renumbering/`
- **Impact on sail:** an edit that adds an item (a lemma, a helper function)
  changes the solver input of every module with a closure defined later in
  the crate, so those modules miss Sail's cache from their first check and Z3
  re-solves them whole. This is not run-to-run nondeterminism: the numbers
  are the same on every run of the same source.
- **Workaround:** none in Sail (no query normalization, charter rule 5). The
  fork fix removes the cause.

The description below is written so it could be filed upstream unchanged.

## Description

**Title:** Closure type names use the crate-wide `DefIndex`, so adding an
item changes the SMT encoding of unrelated modules

**Body:**

`rust_to_vir_base.rs` (`TyKind::Closure`) identifies a closure type by
`def.as_local().unwrap().local_def_index.index()`, and `vir` turns that into
the names `anonymous_closure%<N>` (`def::prefix_closure_type`) and
`impl_closure&%<kind><N>` (`def::impl_closure`). rustc assigns `DefIndex`
values crate-wide in definition order, so adding any item before a closure,
in any module, gives that closure a new number. Every module whose SMT
encoding mentions the closure type gets different solver input, although
nothing in it changed.

With release `0.2026.10.04.426d8b0`, `corpus/repros/verus-closure-renumbering/`
adds one empty function to module `m1`; module `m2`'s SMT log changes:

```
< (declare-sort anonymous_closure%9. 0)
> (declare-sort anonymous_closure%10. 0)
< (declare-const TYPE%anonymous_closure%9. Type)
```

On vest (`vest_lib`), inserting one small lemma in `asn1/der_ord.rs`
(`scripts/perf-real.sh` edit `lemma-mid`) changes three of 298 SMT logs.
Besides the edited module, `cbor::format` (25 checks) and
`combinators::recursive::exec` (6 checks) differ before their first check,
and their only differing tokens are closure-type numbers
(`TYPE%anonymous_closure%N.` and the `impl_closure&__FnN` axiom names).

**Impact:** tools that cache or compare solver input by bytes (Sail, SMT log
diffs) treat unrelated modules as changed after an edit elsewhere.

**Suggested fix:** name a closure type by something that depends only on its
own definition, for example the local half of its `DefPathHash`, which rustc
already keeps unique within a crate; keep sorting the declarations by
`DefIndex` so the output order does not change.

## Which counters change

`scripts/perf-real.sh det` compares each SMT log, with `;;` comment lines
removed, against an unchanged rerun, and lists the differing tokens by
family. Edit `lemma-mid` (a new `proof fn` mid-file), fork runtime `73dea021`:

| Project | Logs that differ | Checks after the first difference | Families in unedited modules |
|---|---|---|---|
| vest (macOS arm64) | 3 of 298: `asn1::der_ord` (edited; 1 of 149), `cbor::format` (25 of 25), `combinators::recursive::exec` (6 of 6) | 32 of 3,238 | `anonymous_closure%N` and `impl_closure&%FnN` only |
| capybarakv (Linux) | 1 of 111: `kv2::rwkv_v` (edited; 1 of 43) | 1 of 733 | none (no closure types in the crate's SMT) |
| ironkv (Linux) | 1 of 35: `host_impl_v` (edited; 1 of 25) | 1 of 361 | none (its only closures are in the edited module) |

No other crate-wide counter appeared: no fresh-name, lambda or temporary
counter crossed a module boundary. `impl&%<N>` names are numbered per parent
module by rustc's disambiguator, so a new `impl` block could rename later
impls of the same module; no measured edit showed that.

## Sail fork

Fixed on [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) branch
`m11-5/stable-closure-ids` by `8d03ab54`, one commit with its regression test
`source/rust_verify_test/tests/sail_vi0019_closure_ids.rs` (the repro's two
crates in one test: module `m2`'s SMT, without comments, must not change
when an item is added to `m1`). The test fails with stock numbering
(`anonymous_closure%11` becoming `%14`) and passes with the fix.

The closure's identity becomes `vir::ast::ClosureId { name, order }`: `name`
is `tcx.def_path_hash(def).local_hash()`, used in every SMT name and for
equality and hashing; `order` is the old `DefIndex`, used only to sort the
closure declarations. Imported closures can have equal indices; follow-up
`3f7f6c51` uses the stable name to break those ties, removing a separately
reproduced run-to-run ordering defect. The regression exercises two imported
closures with the same original index in 16 fresh processes. For the same input the fixed fork's SMT
equals the previous fork's up to a one-to-one renaming of closure numbers and
line wrapping (`renaming-equal.py` in the repro: vest, all 298 logs).

**Measured on vest `lemma-mid`** (see [Gain and qualification](#gain-and-qualification)).

## Gain and qualification

Native c7i.4xlarge, us-east-1, isolated `8d03ab54` candidate versus the
previous fork: Vest's `lemma-mid` edit fell from 38.2 s to 27.5 s; unchanged
runs were 26.5 s versus 27.4 s, and cold runs 79.3 s versus 80.3 s. Cache
puts fell from 323 to 11. These are the inherited experiment's samples,
not acceptance of the later combined runtime.

The fast tier passed 147 units in every arm. The isolated standard run
passed 156 units in every arm with two verismo projects skipped for missing
build tools/targets; it is not a full standard-tier pass. Raw inherited
standard archive: SHA-256
`12ff0a5ca420bc25f61236ba1fcafa1627f46e27a3a580c2ac0c9652585c5202`.

Independent review initially found one reported IronKV mismatch: the
renaming checker recognized `%` closure names but missed their mangled `__`
forms. Correcting the checker gives 34 identical logs and one consistently
alpha-renamed log, zero other differences. This checks only the permitted
symbol renaming; it does not normalize Sail's solver input or establish
verdict equivalence.

The imported-closure ordering regression passes on macOS; its test log is
SHA-256 `18b1cfa6f2ea7e3cfb02cdc4a0bea63c3604abf97795a50206d7702c3df13937`.
The combined source candidate `8a373f81` passes the fork CI and the selected
macOS runtime tests, including all nine `output_json` tests. Full native
corpus conformance, determinism, the full source suite, and renewed measured
gain on the final runtime remain gates. Sail's runtime pin is unchanged.
