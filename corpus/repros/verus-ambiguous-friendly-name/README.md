# Ambiguous Verus friendly function names

The pinned Verus release names both distinct trait implementations
`friendly_collision::S::check` in its timing JSON and AIR queries. The
functions are verified in different module processes (`first` and `second`).
The source has no failing proofs: plain Verus reports two verified, zero errors.

Run the pinned Verus directly, from this directory:

```sh
verus input.rs --crate-name friendly_collision --output-json --time \
  --log air --log smt-transcript --log-dir logs
```

`observed.json` preserves the verdict and identity fields from a macOS arm64
run on 2026-10-10. Timings and machine paths are omitted. Both `first.air` and
`second.air` contain `;; Function-Def friendly_collision::S::check` followed
by distinct source spans. Sail's previous per-file ordinal counter assigned
both operations `friendly_collision::S::check#Function-Def/0`. Its function
join attached both operations to whichever timing entry appeared first.

This reduced case comes from a Linux stock-Verus Vest run: three trait
implementation entries all named
`vest_lib::combinators::mapped::Mapped::lemma_productive`, with distinct
spans in `mapped/proof.rs` and `mapped/spec.rs`.

The same displayed name is insufficient to identify one implementation. Sail
must retain the original function records and logs, report partial attribution,
and decline an ambiguous obligation join rather than inventing a stable ID.


The same-module `Vec<u8>`/`Vec<u16>` controls are
`same-module-positive.rs` and `same-module-failing.rs`; only the unrelated
proof's assertion changes. `same-module-observed.json` records original
stock and Sail outcomes. The unselected ADR0140 producer supplies an explicit
AIR path that a consumer can corroborate with timing JSON; the ordinary
selected runtime still uses the conservative legacy rule above. A fixture
pass does not select or qualify that runtime.
