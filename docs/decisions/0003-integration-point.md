# 0003: Integration point: a Z3-compatible shim named by `VERUS_Z3_PATH`

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The charter expects S.A.I.L. to sit between Verus and Z3 without modifying
Verus, and asks that the integration point be confirmed against the pinned
release before anything is built on it. The pinned release is Verus
`0.2026.10.04.426d8b0` with Z3 4.16.0 (`corpus/toolchain.toml`).

**Source evidence** (`docs/survey/verus-z3.md`):
- `rust_verify` starts `$VERUS_Z3_PATH -smt2 -in`, falling back to `z3` on
  `PATH`.
- The `verus` launcher sets the variable only when it is unset.
- `cargo verus` passes it through and records it as a fingerprint input.
- Verus checks `(get-info :version)` against `4.16.0` before checks. It never
  kills the solver; it closes stdin and waits.
- Each process is one bucket (a module or a spun-off function) or one
  spun-off query.

**Empirical evidence** (2026-10-08, macOS arm64, the pinned release
binaries):
- A logging wrapper named by `VERUS_Z3_PATH` received argv `-smt2 -in` for
  `corpus/fixtures/toy/toy.rs`, and Verus produced identical output through
  it. The captured session is
  `crates/sail-smtlib/tests/fixtures/verus-toy.{in.smt2,out.txt}`.
- The captured session confirms the survey's batch protocol:
  - 554 commands;
  - 52 `(echo "<<DONE>>")` markers;
  - 11 checks in `(push)`/`(pop)`, each preceded by
    `get-info :all-statistics`, `get-info :version` and
    `set-option :rlimit 30000000`, and followed by `set-option :rlimit 0`;
  - `get-model` and `get-info :reason-unknown` after the three `unknown`
    answers.

  It also confirms survey items 3 and 12: the version reply is
  `(:version "4.16.0")`, and every statistics reply carries `rlimit-count`.
- Verus's pipe bytes for the toy crate were byte-identical across two runs
  (survey item 13, for this input only; the corpus run in M3 checks it at
  scale).
- Replaying the captured input into Z3 directly, three times, gave replies
  that differ only in the `:time` and `:memory` figures of statistics
  replies. Verdicts, models, version and `rlimit-count` were identical.
- With `VERUS_Z3_PATH=sail-z3` (pass-through, M2), Verus's stderr was
  byte-identical to plain Verus. Its `--output-json` was identical apart
  from timing fields. Exit status was the same (1, from the deliberately
  failing function). The shim's trace recorded 11 checks with every
  response paired.

**Alternatives considered:**
- *Patching Verus*, or a fork that links sail: rejected by the charter
  (no changes to the user's Verus). It would also need re-porting on every
  Verus upgrade.
- *Passing options with `--smt-option`*: this only adds solver options, so it
  cannot intercept queries.
- *Replaying from `--log smt` files*: an offline analysis aid only. It
  changes pipe whitespace (survey §8), and it cannot answer Verus.
- *A `z3` earlier on `PATH`*: works only when `VERUS_Z3_PATH` is unset, and
  the `verus` launcher sets it whenever a bundled z3 exists, so the shim would
  be silently bypassed. An explicit `VERUS_Z3_PATH` is strictly more reliable.
- *A wrapper around `verus` or `cargo verus`*: needed anyway for `sail verify`,
  but on its own it sees only Verus's output, not individual queries.

## Decision

- S.A.I.L. integrates through `sail-z3`, a Z3-compatible executable that
  Verus runs because `VERUS_Z3_PATH` names it. The pinned Z3 is named by
  `SAIL_Z3_PATH`.
- `sail verify` (a later wave) sets both variables around the user's
  `cargo verus verify` arguments. Switching back means dropping the wrapper.
- The shim forwards argv unchanged, including `--version`, which `--record`
  uses.
- Sessions that ask Z3 to write files relative to its working directory
  (`--profile*` trace files) must run on a local pinned Z3 in place. This is
  recorded now for the daemon wave.

## Consequences

- Nothing in Verus changes. Cargo re-verifies when `VERUS_Z3_PATH` changes,
  so switching to sail and back is clean (survey item 11 still needs
  confirming under `cargo verus`).
- **Function attribution cannot come from the pipe**, because function names
  never reach it. It needs Verus's log files or structural correlation.
  Recorded for the attribution wave (`sail-verus`).
- Every process is a separate shim instance, up to 2 × `--num-threads` at
  once. The daemon wave must handle that fan-in.
