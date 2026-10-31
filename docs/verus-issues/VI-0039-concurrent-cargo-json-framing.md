# VI-0039: Concurrent Cargo-Verus JSON output is not a JSON stream

- **Found:** 2026-10-31
- **Runtime:** stock `7d829946`, Rust 1.99.1, pinned Z3; fork `47fb3be6`
  through the recorded Sail candidate also reproduces it
- **Status:** unresolved; no upstream report or runtime change selected

The [reproduction](../../corpus/repros/verus-parallel-json/README.md) records
valid generated chain, diamond and shared-dependency workspaces. Every arm
exits zero. Nevertheless, every repeated stock stdout fails JSON decoding:
only 1–7 whole JSON documents parse before the first malformed fragment.
Consequently the harness's parsed verified/function totals vary between runs.
They must not be interpreted as the complete verification scope.

The full workspace/all-targets comparison reports 3 different, 0 same, 0
unstable and 0 infrastructure errors. Both the primary compiler stdout and
the additional stock controls remain intact. The observations are consistent
with concurrent compiler writers interleaving records; no per-writer trace
has yet established the exact write sequence. A serial Cargo control is a
separate diagnostic configuration, not acceptance of the parallel command.

The separate `--jobs 1` rerun retains every workspace/test/binary target and
uses candidate `32f7bba6`. All streams parse completely, but the strict
comparison reports one same (chain) and two different (diamond/shared).
Independent crates appear at different document positions: for example,
document two describes package 0001 in one arm and package 0002 in another.
The comparator currently binds function details to document position rather
than a complete compiler-invocation identity. Equal aggregate verified totals
(2,187 / 2,203 / 2,199 respectively) do not establish per-function agreement.
Serial execution therefore does not resolve the capture/identity gate either.

Sail's conformance harness currently cannot establish its contract on this
combined output. Do not weaken comparison, discard malformed output or map
missing function data to success. A lossless record/diagnostic capture must
identify complete compiler invocations before this profile can qualify.
This does not demonstrate a wrong solver answer or attribute a source-level
verification disagreement. Ordinary runtime stdout remains unchanged.
