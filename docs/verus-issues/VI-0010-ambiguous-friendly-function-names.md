# VI-0010: Distinct trait implementations share a friendly function name

- Status: open, tracked locally; no upstream report filed
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
