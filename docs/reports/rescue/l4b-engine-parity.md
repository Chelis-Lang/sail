# Rescue lane L4b: engine parity

- Branch: `worktree-agent-ae975e5ad4d404736`, rebased on `rescue/m7` at `e0ab21b`.
- Host: EC2 `c7i.8xlarge` (32 vCPU, us-east-1), lane `l4b`, native x86-64
  Linux, CI image `sail-ci:8e24fb5f7f490c01` (pinned engine manifest
  `54109b76e8bf…`). The host is down. macOS checks ran on the lead's Apple M3
  under heavy load from other lanes (load average 90–170).
- Raw evidence: `~/sail-raw-evidence/l4b/` on the lead's machine, cited by
  file name and SHA-256 prefix.

**Rebuild needed.** Commits `dbade7e`, `84225d0` and `96bfa65` change the
engine patch or build, so the committed artifact pin no longer matches and
the CI image cannot be built from them until the engine runtime is rebuilt
and re-pinned. Per the lead's instruction, nothing was packed, uploaded or
pinned. The gates below ran at `302fc05`, the last commit before them.
Candidates built from these sources were validated locally (not pinned).

## 1. vstd diagnostic paths (must fix)

**Cause.** Neither the Sail adapter nor diagnostic replay. The pinned
runtime's `vstd.vir` and `libvstd.rlib` record `vstd/seq.rs`; the release's
record `seq.rs` (`engine-build-1-runtime.txt` `bd0b0a3a`). vargo's
`vstd_build` compiles `vstd/vstd.rs` from `source/`. The Verus release
workflow (`.github/workflows/build.yml` at the pinned commit) instead builds
vstd with `cargo-verus build --release --manifest-path vstd/Cargo.toml` from
`source/vstd`.

**Fix (`84225d0`, build procedure, no patch change).** After `vargo build`,
`cargo xtask build-verus-source --interactive-engine` rebuilds vstd the
release's way and removes Cargo's build-dir leftovers. The runtime has the
same thirteen files. `rust_verify` and `verus` are byte-identical to the
pinned runtime; `vstd.vir`, `libvstd.rlib`, the builtin libraries and
`cargo-verus` change. The variant gains the feature
`vstd-build:cargo-verus-release`, and `ci/engine-artifact.py` requires it.

**Evidence.** The candidate records `seq.rs` (`candidate-144f87f8-runtime.txt`
`79a2e648`); its build verified vstd (2060 verified, 0 errors;
`engine-build-7-keyfirst.log` `efaa904a`). `basic_failure.rs` is `same`
(`basic-failure-candidate.log` `c3f605c2`). The 13-stage qualification was
not run on these candidates; the combined rebuild needs it.

## 2. Oracle mismatches under VI-0015

**Decision implemented** (open question 15 answered; ADR 0038 amended).

**First attempt rejected.** Initially I took the second fresh arm from a
repeat engine process. That is unsound: on `guide/datatypes.rs`, stock
declaration order (VI-0001) differs between processes but not within one,
so an injected `effects` fault was classified unstable
(`oracle-negative-control-effects.log` `e4bcce21`,
`datatypes-fresh-arms-diff.txt` `edb9edb6`). It was never committed in that
form.

**Fix.**
- Patch (`dbade7e`, rebuild needed): after a mismatch, the oracle forks a
  pristine owner of the same compiler input in the same callback. It lowers
  and verifies the bucket again (`SAIL_ENGINE_ORACLE_FRESH_ARMS`, default 1,
  at most 8), stopping at the first arm that differs from the first fresh
  arm. It records ordered wire, diagnostic, outcome and effect equality as
  `second_fresh`. Passing summaries and production paths are unchanged.
- Sail (`482f251`): `confirm()` classifies:
  - differing arm: unstable. Verdicts and diagnostics then come from an
    engine run without the oracle and the same focus, which must match plain
    Verus.
  - agreeing arms, a missing record or an incomplete record: hard failure.
- `--oracle-fresh-arms N` passes the count to the engine. Summary lines add
  hard and unstable counts. The fault negative control now also requires at
  least one confirmed hard failure. The unit tests include the negative
  control (agreeing arms stay hard) and malformed or missing evidence.
- With the pinned runtime, which has no `second_fresh`, every mismatch stays
  hard.

**Evidence** (candidate `144f87f8…`, engine manifest `0d4e59aeaf26`;
`batch2-all.log` `ac4a5589`, `batch1-all.log` `4322aaa3`):

| Run | Result |
|---|---|
| Fast tier, 147 units | 145 same, 2 unstable (`traits.rs`, `chapter-6-1.rs`), 0 different; 110 hits, 110 checks, 2 failures (0 hard, 2 unstable) |
| Negative control `effects`, guide | 22/22 detected: 21 hard, 1 unstable (`traits.rs`); `datatypes.rs` is hard |
| Negative control `body`, guide (earlier candidate, same oracle logic) | 22/22 detected, 22 hard |
| `traits.rs` ×10, 1 confirming arm, three batches | 24 unstable, 4 hard, 2 passed |
| `traits.rs` ×10, 4 confirming arms | 10 unstable, 0 hard |

The `traits.rs` fresh lowerings produced 8–9 distinct streams per 10 runs.

**For the lead.** With the default of one confirming arm, about 14% of
`traits.rs` runs (4 of 28 mismatches) end hard, because the confirming arm
happens to reproduce the first fresh lowering. The required Conformance job
would flake at roughly that rate. Four arms gave 0 of 10. I kept the default
at 1, as decided. I recommend `--oracle-fresh-arms 4` in `ci/run.sh`. A hard
failure then needs four confirming arms to agree. That can only excuse a
real defect in a bucket whose lowering is itself nondeterministic, which the
one-arm rule already allows.

## 3. `vest-generated` oracle could not start

**Cause.** A new diagnostic (`96bfa65`) reports "bounded baseline encoding
refused: original encoded false, independent encoded false"
(`vest-generated-diag.log` `695ef398`). The bucket's baseline exceeds the
key budget (`SAIL_KEY_LIMIT`, 32 MiB), so the two owners "differed". The
same refusal empties the bucket's preparation key, and `take_bucket` never
restores an empty key, so there was nothing for the oracle to check.

**Fix (`96bfa65`, rebuild needed).** The bucket key is computed first. The
oracle forks and checks the initial owner only when the key exists.
Otherwise the engine records `oracle-not-applicable`, which the arm counts
and reports (`302fc05`). Any remaining initial-state failure names the
refused encoding or the differing components.

**Evidence.** On the candidate, `vest-generated` is `same` (2,720 functions;
`vest-generated-v4-report.json.txt` `541c3734`), with 7 `oracle-not-applicable`
buckets and 0 restores (`vest-generated-v4-events.txt` `6453e177`).

## 4. Flaky `sail-api` decoded tests

- **`public_default_shared_family_captures_cold_admission…`** (the L4
  failure). `Cache` used `try_lock` on the one result slot. Under
  contention, admissions were captured empty, promotion checks failed and
  publications were dropped. Instrumentation showed a `try_lock` miss before
  every failure. The fix waits for the lock (`3ee7a10`). Every critical
  section only clones, compares, takes or replaces one `Arc`, and old
  entries drop after the guard, so the wait is bounded. A poisoned lock still
  means "decode fresh". No assertion or deadline changed. Isolated loops on
  macOS under load: before the fix 18/500 failed, after it 0/500 (`41c04a82`,
  `e92ec6f5`).
- **`rebound_endpoints_share_budget…`**, found while looping. The fixture's
  accepted sockets inherited the listener's `O_NONBLOCK` on macOS. The fix
  sets them blocking, as `cancellable_tests` already does. Before: 20/500
  failed; after: 0/500 (`7a6e9cde`, `38bc2e64`).
- **Seen, not fixed:** `jobs::cancellable::blocked_read_and_write_cancel…`
  missed its 2 s readiness wait in 5 of 40 full `sail-api` runs on the Mac
  at load average about 50. It never failed in isolation or on Linux.

## 5. macOS clippy

`37d92cf` gates the Linux-only code with `cfg(target_os = "linux")`, or
`any(linux, test)` where macOS unit tests use it. That covers the bootstrap
stage labels and mutators, `publish_records`, `idle_for_test` and the store
probes. Two non-Linux stubs that ignore `self` carry documented `allow`s.
`cargo clippy --workspace --all-targets -- -D warnings` passes on macOS; the
Linux gate below passes too.

## Gates (native, `302fc05`)

| Step | Result | Evidence |
|---|---|---|
| `gate` ×3 | pass, pass, pass (722 tests each) | `gate-1.log` `72123d6b`, `gate-2.log` `0c88c137`, `gate-3.log` `ceeec8b9` |
| `solver` | 41/41 | `solver-1.log` `f2c42212` |
| `e2e` (legacy, managed, repair) | pass | `e2e-1.log` `3834e4cd` |
| `conformance fast pr` | default arms 33/33 ×3; engine arm fails on `traits.rs` (1 hard), as expected without the rebuilt runtime | `conformance-fast-pr-1.log` `d773fa9d` |

Before rebasing, the base `e75581d` gate failed on the dead
`repair_contract` test that the lead has since restored
(`gate-base-e75581d.log` `44f59bd9`).

Summary lines:
```
engine conformance (fast tier): 33 units, 32 same, 0 unstable, 1 different, 0 infra errors; 311 functions compared; 0 units with rlimit drift; 0 projects skipped; engine 54109b76e8bf: 32 focused units, 22 units with 22 restored preparation hits, 22 oracle checks, 1 oracle failures (1 hard, 0 unstable in 0 units), 0 buckets without a preparation key
engine conformance (fast tier): 147 units, 145 same, 2 unstable, 0 different, 0 infra errors; 3450 functions compared; 0 units with rlimit drift; 0 projects skipped; engine 0d4e59aeaf26: 146 focused units, 110 units with 110 restored preparation hits, 110 oracle checks, 2 oracle failures (0 hard, 2 unstable in 2 units)
```
The first line is the pinned runtime. The second is the candidate, from a
binary built before the not-applicable count was added.

## Documentation

- Changed: `docs/proof-repair.md` (engine conformance), `engine/verus/README.md`
  (conformance arm), `docs/decisions/0038-m7-rescue.md` (amendment),
  `docs/open-questions.md` (question 15 answered),
  `docs/verus-issues/VI-0015…`, this report.
- **For the lead (not edited):**
  - `docs/known-issues.md`: the vstd span-path, VI-0015 and `vest-generated`
    entries should point to these fixes, pending the rebuild.
  - `CHANGELOG.md`: the oracle classification, `--oracle-fresh-arms` and the
    engine vstd build.
  - `docs/ROADMAP.md`: the engine runtime rebuild and its 13-stage
    qualification are an open gate.
