# Files read only by ghost code are missing from the final dep-info

`lib.rs` has two ghost-only reads: `include!("value.rs")` inside a `proof fn`,
and `ghost.rs`, a module declared under `#[cfg(verus_keep_ghost)]`.
`repro.sh CARGO_VERUS [SAIL] [WORK_DIR]` takes each file in turn. It creates a
crate with `cargo verus new --lib` and verifies it, then edits the file so the
proof fails and verifies again, in the same target and in a new one. With
SAIL, it also runs `SAIL verify --output json` before and after the edit, with
a cache of its own.

Observed 2026-10-26 on native x86-64 Linux in the CI image. Stock is
`0.2026.10.18.7d82994` (`/opt/verus/verus-x86-linux/cargo-verus`). Sail is the
#436 head, before the #439 fix:

```
== value.rs
   plain, original:            exit 0, errors 0
   final dep-info names value.rs: 0
   sail, original:             exit 0, currentness current reused False complete True verifier_invocations 2
   plain, edited, same target: exit 0, errors 0
   plain, edited, new target:  exit 101, errors 2
   sail, edited:               exit 0, currentness current reused True complete True verifier_invocations 0
== ghost.rs
   plain, original:            exit 0, errors 0
   final dep-info names ghost.rs: 4
   sail, original:             exit 0, currentness current reused False complete True verifier_invocations 2
   plain, edited, same target: exit 101, errors 2
   plain, edited, new target:  exit 101, errors 2
   sail, edited:               exit 101, currentness current reused False complete False verifier_invocations 1
```

With the #439 fix, `sail, edited` for `value.rs` is `exit 101, ... reused
False complete False verifier_invocations 1`, as a new plain target fails. See
[VI-0037](../../../docs/verus-issues/VI-0037-ghost-file-reads-missing-from-dep-info.md).
