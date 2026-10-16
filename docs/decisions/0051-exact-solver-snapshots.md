# 0051: Exact solver snapshots for edit-loop misses

- **Status:** proposed (prototype measured on branch `perf/solver-snapshots`;
  not built into any default)
- **Date:** 2026-10-16
- **Relates to:** [ADR 0008](0008-default-mode-keys-bind-the-process-prefix.md)
  (keys bind the process prefix), [ADR 0011](0011-local-cache-and-lazy-solver-sessions.md)
  (lazy sessions replay the whole prefix), [ADR 0040](0040-warm-solver-sessions.md)
  (warm solver sessions), issue #158

## Context

- **A miss costs the whole bucket.** Verus checks a module in one Z3
  process. Default-mode keys bind every byte the process received (ADR 0008),
  so after an edit the bucket hits up to the first changed byte. At the first
  miss `sail-z3` starts the pinned Z3 and writes the whole input so far,
  which re-solves every earlier `check-sat` (ADR 0011). The real-project
  report measured the cost: an anvil one-function edit took 104 s against a
  38 s all-hits floor, and edit position did not matter
  ([report](../reports/2026-10-15-real-project-performance.md)).
- **Smaller buckets are not the fix.** `-V spinoff-all` raised the all-hits
  floor 1.7–1.8× on capybarakv and vest, and one ironkv lemma exceeded its
  rlimit under it: a verdict difference from stock's default buckets.
- **History-tolerant replay changes replies.** Skipping earlier checks keeps
  answers (ADR 0008: 3,645 of 3,645) but changes `rlimit-count` for 2,617 of
  3,644 checks and model bytes for 64 of 97, so it can only be a labelled
  opt-in mode (charter rule 8).
- **What exactness needs.** The hypothesis is that resuming an identical logical and implementation
  state can preserve stock's replies without replay. The prototype has not
  established raw equality: `:num-allocs` differences remain visible, so state
  copying alone is insufficient evidence of the complete byte contract.

Lane M11-2 investigated whether Z3 4.16.0 can be snapshotted exactly, built
a prototype, and measured it. Evidence:
[solver snapshots report](../reports/2026-10-16-solver-snapshots.md).

## Options for taking a snapshot

| Option | Exact? | Platforms | Verdict |
|---|---|---|---|
| (a) `fork()` inside the unmodified pinned Z3, through a preload library (`LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`) that interposes `getc` on stdin | state-copy candidate; raw reply equality pending, including observed statistics differences | Linux, macOS (Z3 is not hardened-runtime signed) | **prototyped** |
| (b) a Z3 patch adding a `(sail-fork)` command | hypothesis requiring qualification of a different binary and explicit identity | both | not pursued: evaluate the unmodified-binary candidate first |
| (c) libz3 in a helper process that forks | the C API's text front end is not the `z3` binary's (`Z3_eval_smtlib2_string`, non-interactive parser, different error behaviour) | both | rejected: replies are not provably the binary's |
| (d) CRIU checkpoint and restore | state-copy candidate; unqualified | Linux only, needs `CAP_CHECKPOINT_RESTORE` or root, restore costs a process image write and read | rejected: no macOS, privileges |
| (e) an existing Z3 facility | none: Z3 has no state serialisation; `push`/`pop` work only inside one process | — | — |

**How (a) works.** Z3's `-smt2 -in` front end reads stdin one character at
a time through `std::cin.get()`; synced with stdio, that is one
`getc(stdin)` call per character (libstdc++ `stdio_sync_filebuf`, libc++
`__stdinbuf`; checked on both pinned binaries). `sail-snap`
(`native/sail-snap/sail_snap.c`) interposes `getc`/`ungetc` on stdin only.
`sail-z3` puts a marker `\0<name>\0` before each top-level `(push)`; the
library strips it, so Z3 reads exactly stock's bytes. When Z3 asks for the
character after the marker, the library forks. The parent continues. The
child is a byte-for-byte copy of Z3 that has consumed exactly the input
before the marker and is waiting for the next character. It drops its
stdio read-ahead, detaches, binds `<dir>/<name>` and parks. To resume, a
client connects three times (stdin/stdout, stderr, status); the parked
process forks again and the new child returns from `getc` with the first
byte of the new input. The parked process stays parked, so one snapshot
serves any number of edits. The name is derived from the v2 prefix key of
the input before the marker, so a snapshot is found only by a session whose
input starts with exactly those bytes.

## Evidence (summary; full tables in the report)

- **Exactness.** Pending. The exploratory comparisons mask nondeterministic statistics (including `:num-allocs`); these are not raw-byte acceptance.
- **Edit cost.** TBD
- **Memory.** TBD
- **Overheads.** TBD

## Decision (proposed)

TBD

## Consequences

TBD

## Prototype hardening (not qualification)

The version-2 transport doubles genuine NUL bytes; markers remain NUL,
nonempty snapshot name, NUL. The preload must acknowledge `sail-snap/2` before
any framed input is sent. Missing/incompatible acknowledgment kills that owned
child and starts a plain solver before forwarding any input. Snapshot names and
resume headers have a new namespace; old parked processes are not resumed.

Each snapshot fork takes a nonblocking directory-wide admission lock. It checks
all still-live parked process records, including evicted processes waiting for
active resumes, against `SAIL_SNAPSHOT_MAX_PARKED` (default 256) and
`SAIL_SNAPSHOT_MAX_BYTES` (default 536870912). Memory admission conservatively
charges every process's whole resident image plus one MiB of control reserve,
including the candidate, rather than relying on shared-page PSS savings. Missing
or unreadable process state refuses a snapshot. A pid record remains charged
until death; duplicate live generations are refused. The lock stays held through
socket and pid publication. Exceeding either cap skips the snapshot and the
current solver continues normally. This is a bound at snapshot admission;
ordinary active solver workers and future growth are not an OS memory limit.

Run correctness-only regression checks with:

```sh
native/sail-snap/build.sh
python3 native/sail-snap/test.py --lib target/sail-snap/libsail_snap.so --z3 PATH
```

Use `.dylib` on macOS. These checks cover genuine NULs, all input byte values,
resumed NUL input, a single solver exceeding the parked limit, concurrent solver
admission, a one-byte memory budget, and a zero-count budget. They do not
qualify default operation or establish an edit-loop performance gain. Native
Linux qualification, full-corpus output checks, raw-statistics exactness, and
active-worker resource policy remain pending.
