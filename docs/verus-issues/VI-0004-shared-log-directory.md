# VI-0004: every crate in a `cargo verus` run shares one log directory, and each clears it

- **Status:** open (tracked here)
- **Verus:** `0.2026.10.04.426d8b0` (pinned)
- **Found:** 2026-10-08, building function attribution (ADR 0014)
- **Impact on sail:** `sail verify --output json` requests Verus logs only
  when one Verus process will write them: a single workspace member with at
  most one lib or bin target. Otherwise attribution is `unavailable`.

## Description

- **Each process clears the directory.** On the first log file it creates in
  a run, Verus deletes every regular file already in its log directory
  (`--log-dir`, default `.verus-log`; see
  `rust_verify/src/verifier.rs:627-650` and `delete_dir_if_exists_and_is_dir`
  at `:2991-3008`).
- **`cargo verus` shares one directory.** It forwards the same Verus
  arguments, and so the same `--log-dir`, to every package it verifies
  (`cargo-verus/src/subcommands.rs:567-700`). Each lib and bin target is a
  separate Verus process.

So in one `cargo verus verify` with logging:
- each later process deletes the logs of the earlier ones;
- concurrent processes can delete each other's logs mid-run;
- in principle (from the source; not reproduced), with three or more
  concurrent processes, one can fail to remove a file another has just
  removed. Verus reports that as an error (`could not remove file`), which
  would change the run's outcome.

There is no per-crate log directory option, and the path cannot vary per
process.

## Workaround

sail runs `cargo metadata --no-deps` first. It adds `--log … --log-dir` only
when exactly one process will log. A fix in Verus could be a log
subdirectory per crate, or not clearing files it did not write. Either would
let attribution cover workspaces.
