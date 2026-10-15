# VI-0010: Distinct trait implementations share a friendly function name

- Status: open in stock Verus (tracked here; not filed upstream);
  fixed in Sail's Verus fork by `fbde5ea3` (feat: add function-path to the timing function breakdown).
- Found: 2026-10-10
- Verus: `0.2026.10.04.426d8b0`
- Reproduction: `corpus/repros/verus-ambiguous-friendly-name/`

Two traits with a method named `check`, implemented by the same struct in
separate modules, produce two timing entries and two AIR operations named
`friendly_collision::S::check`. The module and source spans distinguish the
operations, but the friendly function name does not identify the trait impl.
More than one implementation can also occur in the same module (observed in
Vest), so a module/name join alone does not resolve the ambiguity.

A Linux full stock-Verus run of the pinned Vest corpus verified 3,237 functions
and returned exit zero. Sail's obligation IDs collided because the ordinal
counter restarted per solver log; it also attached all three implementations'
operations to the first matching timing entry. The unchanged duplicate-ID
invariant correctly refused a complete receipt. This is an attribution defect
in Sail triggered by an ambiguous upstream display identity.

The conservative workaround declines obligation attribution for every repeated
friendly-name timing entry. Original Verus verdicts, modules, timings and raw
logs are retained, with an explicit partial-attribution note. Successful
whole-target Verus results remain authoritative; a failing crate with incomplete
attribution continues to use the conservative unknown policy. No IDs are
invented and no invariant is relaxed. Exact focused selection must reject an
ambiguous display name rather than choose one implementation.

A future upstream identity could include the trait and concrete implementation
identity consistently in AIR, timing JSON and diagnostics.

## Sail fork

Fixed in [Chelis-Lang/verus](https://github.com/Chelis-Lang/verus) by
`fbde5ea3` (feat: add function-path to the timing function breakdown), one
commit with its regression test
`source/rust_verify_test/tests/sail_vi0010_function_path.rs`, which fails on
the pinned stock source and passes on the fork. Each `function-breakdown`
entry gains a `function-path` field: the crate and the full VIR path,
including the impl segment (`impl&%N`) that the friendly name replaces, so
distinct impls are told apart, including several in one module. The `function`
field is unchanged. `func-details` stays keyed by friendly name.

Since [ADR 0043](../decisions/0043-default-mode-runs-the-fork.md) Sail runs the fork for every verification, default mode included. Stock runs remain only as the conformance oracle and under the `SAIL_CARGO_VERUS` development override. Still needed: Sail declines
attribution for repeated friendly names, and does not yet read
`function-path` from the fork's output. Comparisons
compare a repeated name as one group only when every function under it has
the same result on each side and the counts match; otherwise the name is
`incomparable` (vest has 95 such names, shared by 207 functions).
