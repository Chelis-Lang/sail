# VI-0037: Files read only by ghost code are missing from the final dep-info

- **Status:** open in stock Verus and in the fork (tracked here; not filed
  upstream). Sail works around it (#439).
- **Verus:** `0.2026.10.18.7d82994` (pinned stock) and fork `36d65c3f`.
- **Found:** 2026-10-26, issue #439, while fixing #386.
- **Reproduction:** [verus-ghost-file-dep-info](../../corpus/repros/verus-ghost-file-dep-info/)
  (`repro.sh CARGO_VERUS [SAIL]`).
- **Impact on Sail:** a receipt's per-target input revision counts the files
  the dep-info declares, so after a file included only by a proof was edited
  so the proof fails, Sail reused the old receipt as Current.

## Description

This is the file counterpart of
[VI-0036](VI-0036-ghost-env-reads-missing-from-dep-info.md). The erased
compilation never expands macros inside ghost code, and its dep-info replaces
the verification phase's. A file that only `include!`, `include_str!` or
`include_bytes!` inside a `proof fn` reads is therefore not declared:

```rust
proof fn included_value_is_two() {
    assert(include!("value.rs") == 2);   // src/value.rs contains `2`
}
```

Observed on native x86-64 Linux with the pinned stock release:

| Step | `include!` in a proof (`value.rs`) | `#[cfg(verus_keep_ghost)] mod ghost;` |
|---|---|---|
| final dep-info names the file | no | yes |
| edit so the proof fails, same target | exit 0 (stale) | exit 101 |
| edited, new target | exit 101 | exit 101 |

A module declared under `#[cfg(verus_keep_ghost)]` is not affected: rustc
already records the module file in the final dep-info.

## Sail workaround

The fork's compiler-input report (`SAIL_COMPILER_INPUTS`), which Sail requests
for every verified target for VI-0036, lists each phase's local source-map
files with the checksum rustc computed of the bytes it read, and the files it
tracked. After a successful verification the managed driver adds each missing
file to the unit's dep-info rules, and, under Cargo's checksum freshness, its
`# checksum:` line from the report. A file the report gives no checksum for
refuses the observation, since a checksum taken later could describe other
bytes. Imported VIR is left out: the invocation binds each extern's VIR by
content.

## Fork

Not fixed. A fork fix would carry the verification phase's file dependencies
into the erased compilation's dep-info, and would need a runtime re-pin.
