# Rescue lane L4: engine correctness

- Branch: `rescue/l4-engine-correctness`, cut from `rescue/m7` at `541d3e7`.
- Host: EC2 `c7i.4xlarge` (16 vCPU, Linux 7.0.0-1014-aws x86_64), lane `l4`,
  CI image `sail-ci:8e24fb5f7f490c01` with the pinned prebuilt engine
  (manifest `54109b76e8bf…`, patch `aaa3b2e4…`). Not emulated.
- Raw evidence: `~/sail-raw-evidence/l4/` on the lead's machine, cited below
  by file name and SHA-256 prefix.

## Findings

### 1. Engine faults and stock fallback

**Verified.** Per target, `adapter::engine_output` already reran the exact
stock invocation when the engine's completion check failed, but
`engine_events::completed` only checked shape: a seal whose `success`
contradicted the JSON document or the exit status was accepted. An engine
fault that escaped the per-target path (for example a crash while the engine
identified its toolchain, inside `run_engine`) made `engine::repair` return an
error, so the private worker and the CLI failed the repair outright. The
fallback also kept persistent preparation entries and could publish a receipt
for the fallback's stock output under the engine context.

**Choice:** follow ADR 0027 (fault falls back to stock, invalidates
preparation, never manufactures success), and make the CLI agree.

**Change:**
- `completed()` rejects a seal that contradicts the exit status
  (`(exit == 0) != seal.success`), the document's `success`, or a VIR error.
- A fallback target's report is labelled `stock-fallback`, publishes no
  receipt (`receipt_success`), cannot accept a repair (`repair::accepted`),
  and adds a `SAIL3123` project note. Persistent preparation entries are
  removed under the engine's `cache.lock`.
- `engine::verify`/`engine::repair` fall back to an independent stock full run
  for any engine-run error except `SAIL3120` (missing or incompatible
  install, still rejected). Repair records `batch.engine_fault` (`SAIL3123`),
  keeps the stock `full` report, and is never accepted; an error that also
  stops stock is returned unchanged. The CLI prints the fault.
- Tests: `contradictory_seal_is_not_completion`,
  `engine_fault_invalidates_only_persistent_preparation_entries` (injected
  exit-101 crash returns the stock output and removes `.prepared`/`.tmp`
  entries; a completed control keeps them),
  `stock_fallback_reports_engine_provenance_without_a_receipt`,
  `engine_fault_and_stock_fallback_never_accept_a_repair`; existing fallback
  tests unchanged.

### 2. Engine provenance

**Verified.** Engine reports carried `provenance.mode: "default"` and nothing
else; `ProjectResult` had no engine field.

**Change** (additive within schema v1, per `docs/output-schema.md`; no version
bump because fields are only added and are omitted for stock):
- `Report.engine` and `ProvenanceReport.engine`: `{name, manifest_sha256,
  established_by: engine|stock-fallback, persistent_preparation}`;
  `ProjectResult.engine`: `{name, manifest_sha256, persistent_preparation}`;
  `BatchResult.engine_fault`. `mode` stays `default` (not a verdict-changing
  mode). Provisional proof-event function observations also name the engine.
- JSON Schemas (`verify-output-v1`, `project-result-v1`, `repair-result-v1`),
  the ownership audit, MCP lookups and summaries (only when present).
- Stock output: serialization skips `None`, and a test checks the stock golden
  has no `engine` key. The unchanged e2e legacy and managed phases (byte
  comparison with `cargo verus verify`, schema-v1 checks) passed.
- Real-engine control `ci/test-repair-engine-provenance.py` (e2e repair phase)
  checks project, report and every function provenance against the installed
  manifest.

### 3. Persistent preparation off by default

**Verified.** `request()` always passed `<cache>/engine-prepared` as
`SAIL_ENGINE_PREPARED_DIR`; the patch's `disk_root()` reads only that variable.

**Change (no runtime rebuild):** `engine::Request.prepared` is now
`Option<PathBuf>`, set only with `SAIL_ENGINE_PERSISTENT_PREPARATION=1`. By
default the adapter passes a per-invocation directory inside the target's
sidecar and removes it after the driver exits. The opt-in is part of the proof
context (new context-hash variant test), excluded from the environment input
list, and reported as `persistent_preparation: true`.

**Impact on in-batch reuse:** none observed. In-memory and same-invocation
file reuse both remain: e2e default run `batch_hits 1, misses 1`, identical to
the opt-in run (`e2e-1-engine-provenance-summary.json`, `88c392e5`). On the
fast tier 110 of 146 focused units restored a hit in the default
configuration. Only reuse across invocations and edits is lost.

**Controls:** with the opt-in, corrupting every persistent entry left verdicts
and completeness unchanged (`persistent_hits 0`, focus miss, e2e above).
`engine/verus/test-prepared.py` on the selected runtime passed all eight
revisions, including corrupt, misfiled and oversized entries not restored and
stock-identical SMT bytes (`test-prepared.log`, `07c3a5b9`).

### 4. Engine conformance arm

`sail-conformance engine` reuses the harness's selection, plain runs and
`classify` (ADR 0009 confirmation runs). Per unit it runs the engine with the
plain run's first proved function focused, an invocation-local preparation
directory and the same-input oracle on (`SAIL_ENGINE_ORACLE_DIR/_Z3`), then
adds engine-only checks: normal, consistent seals, and an oracle summary that
passed for every restored hit. `--oracle-fault` turns it into a negative
control. `ci/run.sh conformance` runs it after the cache rounds on every tier
(required in CI; a host run may opt out with `SAIL_CONFORMANCE_ENGINE=0`).

Results (Linux x86-64 native; code at `8b884dd`, later commits are documentation only; the first `pr` run used `c55c6dd`, whose arm differs only in test assertions):

| Scope | Result | Evidence |
|---|---|---|
| `conformance fast pr` (33 guide units), first run | pass-through, fill, warm 33/33 same; engine 32 same, 1 different (`guide/traits.rs` oracle mismatch); 22 hits, 22 oracle checks; whole step 89 s including build | `conformance-fast-pr-1.log` `9a9b2647` |
| `conformance fast` (147 units, 3,450 functions) | pass-through, fill, warm 147/147 same; engine 145 same, 2 different; 146 focused, 110 hits, 110 oracle checks, 1 failure | `conformance-fast-full.log` `0a65ceb9`, report `conformance-fast-full-engine-report.json` |
| `guide/traits.rs` repeated, two batches | oracle mismatch in 7 of 10 runs (4/5, then 3/5); same as plain in 6/6 runs without the oracle and 6/6 without focus | `traits-oracle-repeat.log` `7d40fe00` (second batch; the first was read from the terminal and not retained) |
| Negative control `--oracle-fault effects` (guide) | 22/22 restored hits detected | `oracle-negative-controls.log` `da8fabf6` |
| Negative control `--oracle-fault order` (guide) | 11/22 detected; the other 11 swaps left bytes and effects identical (no-op fault), so `order` is not a complete control | same |
| Standard-only projects (9 projects, 12 units, 9,276 functions), engine arm only, `--jobs 3`, one project at a time | 11 same, 1 different (`vest-generated`); 3 hits, 3 oracle checks, 0 failures; 46 min | `engine-standard.log` `c7e4f0f4`, `engine-standard-reports.tar.gz` `afebab91` |
| `vest-generated` without the oracle | same as plain (2,720 functions) | `vest-generated-no-oracle.log` `d950b56c` |

With the fast tier, the engine arm covered all 159 units of the standard
scope. The default pass-through and cache arms were run on the fast tier only;
the standard default arms were not rerun in this lane. Most cargo units had no
restored hit (the focused bucket was not the full phase's first lookup), so the
oracle exercised 113 hits in total.

`vest-generated` fails before any hit: with the oracle enabled, the oracle's
own initial-state fork (`sail_oracle_fork`) differs from the original context
for some bucket (`error: oracle initial state: independent baseline differs`,
exit 101). The engine without the oracle matches plain Verus there. This is an
oracle coverage limit, recorded as a failure, not reclassified.

The two fast-tier differences:
- `basic_failure.rs`: a vstd span renders `vstd/seq.rs:980:12` from the engine
  runtime and `seq.rs:980:12` from the stock release, also with no
  `SAIL_ENGINE` (`basic-failure-vstd-span.log`, `6cda421d`). The source
  build's vstd records a different path. Engine diagnostics pointing into vstd
  differ from stock. The e2e fixture is no-vstd and never reaches this.
- `summer_school/chapter-6-1.rs` (and `guide/traits.rs` in most runs): the
  restored lowering's automatic triggers differ from fresh lowering in the same
  callback. Stock itself varies: 12 stock runs of `traits.rs` gave 8 streams
  and two trigger choices; 10 runs of `chapter-6-1.rs` gave 2 streams
  (`vi-0015-repro.log` `98150343`, `chapter-6-1-stock-variation.log`
  `900252f5`). Cause by source reading: `triggers_auto.rs` breaks ties in
  `HashSet`/`HashMap` order. Recorded as VI-0015 with a reproduction; every
  engine transcript without the oracle equalled a stock transcript
  (`traits-stock-engine-transcripts.log`, `16749c7f`). No normalization or
  reclassification was made; open question 15 asks the owner.

**Consequence:** the required `Conformance` CI job fails on this branch in most
runs (`guide/traits.rs`). Landing without that requires an owner decision or
holding the engine block in `ci/run.sh` `conformance`.

### 5. Docs reconciled with code

- Codec: `engine/verus/README.md` now says portable checkpoints are version 3
  (patch `portable.version!=3`) and names the `.../v10` constructor key domain.
- `engine/verus/qualification-host.json` is marked `superseded` (it records
  patch `b34639a5…`, codec 2) and names the selected `aaa3b2e4…`; the
  acceptance table row says so. Evidence fields unchanged.
- **For the lead (not edited by me):** `docs/known-issues.md` says the 512 MiB
  preparation bound is not established. ADR 0027 said "The global preparation
  budget is 512 MiB"; I changed it to "configured" and pointed to known issues.
  Known issues should also gain the vstd span-path difference and VI-0015, and
  AGENTS.md's `conformance fast pr` comment should mention the engine arm.

ADR 0027 was edited only to match implemented behaviour: per-target and
batch-level fallback, invocation-local preparation by default with the named
opt-in, additive provenance fields, the configured (not established) budget,
and the conformance arm. Also updated: `docs/output-schema.md`,
`docs/workspace-verification.md`, `docs/proof-repair.md` (fallback, opt-in,
engine conformance), `docs/proof-repair-walkthrough.md`, `docs/divergences.md`,
`docs/open-questions.md` (question 15), `docs/verus-issues/` (VI-0015),
`engine/verus/README.md`, `docs/bench/m7-repair-acceptance.md`.

## Gates

| Step | Result | Evidence |
|---|---|---|
| `ci/local.sh gate` | gate-1: clippy failure in new tests (fixed). gate-2 and gate-3 each failed one timing test (`sail-api jobs::decoded::…cold_admission…`, then `sail-daemon output_service::overlap_tests::…`). gate-4: all 687 tests, clippy, docs, layering, deny pass | `gate-2.log` `c414f4b0`, `gate-3.log` `b4a02a4f`, `gate-4.log` `f5b05981` |
| Flake comparison: four full `nextest --profile ci` runs per tree, separate targets | base `541d3e7`: 3 of 4 runs fail 1–2 `output_service` timing tests; this branch: 3 of 4 runs fail 1–3 of the same family. The `decoded` failure did not recur in 8 runs (5/5 in isolation). The `output_service` flakes predate this branch; the `decoded` one is unexplained | `nextest-stress-base.log` `5bcf669d`, `nextest-stress-mine.log` `b6401267`, `gate-base-541d3e7.log` `dd1b8a96` (base gate, one run, pass) |
| `ci/local.sh solver` | 41/41 pass | `solver-1.log` `3894f7bc` |
| `ci/local.sh e2e` (legacy, managed, repair) | all pass, including the new provenance controls | `e2e-1.log` `59de88cc` |
| `ci/local.sh conformance fast` | default arms 147/147 ×3; engine arm 2 different (above) | `conformance-fast-full.log` |

## Open risks

- The same-input oracle cannot pass deterministically on corpus units whose
  stock lowering varies within a process (VI-0015). Until the owner decides,
  the engine arm and therefore the conformance CI job fail.
- Engine diagnostics into vstd differ from the stock release (`vstd/` prefix).
  Fixing it needs an engine runtime rebuild or a stock source-build baseline;
  neither was done.
- `sail-daemon` `output_service` timing tests fail intermittently on the base
  and this branch alike; the required `Workspace gate` can go red without a
  code cause. One `sail-api` `decoded` failure on this branch is unexplained.
- The oracle cannot fork every crate's initial state (`vest-generated`), so
  it cannot check some real hits at all.
- The batch-level stock fallback is unit-tested by construction only; no real
  engine crash before target execution was injected end to end.
- Persistent preparation remains available by opt-in with its rule-6 caveat;
  real hits are still not compared with fresh lowering outside the oracle.
