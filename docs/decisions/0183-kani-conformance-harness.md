# 0183: Kani conformance harness, fault arm, corpus tiers and secondary pins

- **Status:** accepted for implementation (Kani phase checkpoint, owner approval 2026-11-28)
- **Date:** 2026-11-28
- **Extends:** [ADR 0181](0181-kani-parity-and-outcomes.md), [ADR 0013](0013-ci-image.md), [ADR 0038](0038-m7-rescue.md)

## Context

Every Kani behaviour change, and every reuse layer after it, needs the same
check: Sail against direct Kani, per harness and per check, on pinned real
projects, plus proof that faults never become successes. ADR 0156's paired
runner was an external script over one project. The spikes (S1 to S7) used
their own scripts. The design (workstream W3) asks for one in-tree harness
beside `sail-conformance`.

## Decision

**Corpus.** `corpus/kani-corpus.toml` pins every project by full commit and
names the package directory, the project's own CI arguments, the unwind
bound for harnesses without `#[kani::unwind]` (the current CLI requires
`--unwind`), host requirements and a recorded `Cargo.lock` with its sha256
where the project commits none (`corpus/kani/`).

- *Fast tier* (minutes, every Kani behaviour change): the arrayvec pilot,
  rust-osdev/x86_64 and DataDog/lading `lading_throttle`.
- *Standard tier* (hours, memory-bound): aws/s2n-quic `s2n-codec`,
  `s2n-quic-core` and `s2n-quic-platform`, and Firecracker. Firecracker needs
  libseccomp 2.6.0 on Ubuntu 24.04 and at most `-j4` on 61 GiB.
- *Secondary pins* are listed in the corpus (`[[secondary]]`): Firecracker's
  own CI pins Kani 0.67.0. Every arm runs the primary pin, 0.68.0, until a
  secondary pin has its own staged bundle.

**Harness.** `crates/sail-conformance/src/kani/`, driven by
`sail-conformance kani run` and wired as `ci/local.sh conformance kani TIER`
(local-only; `ci/run.sh` refuses it in CI). For each harness and round it
runs direct, then Sail, then direct again, in one job slot with its own copy
of the prepared Cargo home, so concurrent Cargo processes never print
lock-wait lines into each other's output. Every run has a fresh private
directory for its target, home, tmp and export. The direct arm runs the
original driver from Sail's staged bundle, whose bytes are the pinned stock
release, with the executor's argv and environment built independently of
Sail; so tool and library paths in check locations match and only the run
directory differs. Harnesses are discovered from the `kani-metadata.json`
files of an `--only-codegen` run (S5: `cargo kani list` misses real
harnesses). Records (`KaniRunRecord`) keep argv, environment, exit, streams
and the strictly parsed export (duplicate keys refused); targets are
removed after reading.

**Sail side as an adapter.** `SailSide` returns one record per harness.
`Cli` runs one `sail verify --backend kani` per harness. A batch executor
(ADR 0186) needs a new adapter only; the direct oracle then becomes one
`-j N` driver run per batch, split per harness.

**Fault arm.** `sail-conformance kani fault`, on an in-tree fixture crate
(`crates/sail-conformance/tests/fixtures/kani-fault`). A stand-in `cbmc`
(after S6's) injects: the last result cut then exit 6, complete results then
`SIGKILL`, `SIGKILL` before any result, and a hang. The direct arm runs a
stage copy holding the stand-in. Sail runs a private copy of its original
tools, staged by Sail itself in a control row with real tools that must match
direct; the stand-in is then installed in that stage. Real resource faults
run Sail's real tools: a cgroup memory cap (`sudo systemd-run --scope`,
swap off) that only the harness's CBMC run exceeds, and Sail's own
deadline. The rule: the control row matches; under every fault Sail makes no
claim and leaves no entry in its state directory other than attempt
captures. Each direct fault capture is also given to the product decoder
alone, as information for integrity supervision (ADR 0182).

**Benchmarks.** `sail-bench kani cold|warm|edit|history-replay|ci-rerun`.
The baseline is one original driver over all harnesses with `-j N` on the
staged tools (ADR 0180). The Sail side is measured where its layer exists:
today only `cold` (the per-harness CLI). The other arms emit a `not-built`
Sail row naming the layer that will fill it: closure receipts (ADR 0189)
for warm, the CBMC-boundary store (ADR 0188) for edit and history replay,
the signed tier (ADR 0190) for a CI rerun.

**Dependency.** `sail-conformance` gains the internal edge to `sail-kani`,
used only by the fault arm's decoder column, and the workspace `toml` crate.

**Errors.** `SAIL6320` corpus, `SAIL6321` project preparation, `SAIL6322`
arm infrastructure. None of them is a conformance result.

## Consequences

- The fast tier runs in minutes on one c7i.8xlarge; standard needs the fleet
  tooling (ADR 0191) and memory-aware slots.
- x86_64 builds only with `--all-features`, which the current CLI cannot
  pass, so it is reported as blocked until the CLI passes Cargo features.
  `lading_throttle`'s CI argument `--solver cadical` is Kani's default and
  changes nothing.
- The `ci/local.sh` container has no systemd scopes; the memory-cap case is
  skipped there and says so. Native lanes run it.
- Not yet covered: harness kinds the decoder refuses (should-panic, stubbing,
  contracts, pinned solvers) run as `no-claim`, with their raw driver output
  still compared, until ADR 0184 admits them.
