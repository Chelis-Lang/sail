# L13: simplification audit of crates, scripts and CI tooling

- **Date:** 2026-10-12
- **Base:** `origin/rescue/m7` at `fc5416e`, compared with `origin/main` at `dd65c38`
- **Scope:** read-only review of crate code (M0 to M7), `scripts/`, `ci/`, `bench/validation/`, `engine/verus/*.py` and `corpus/repros/**/*.py`. No code was changed. Line counts come from `wc -l` and `git diff --numstat`. Reachability was checked with `grep` over `crates/`, `ci/`, `scripts/`, `.github/` and `docs/`.
- **Excluded:** the L7c managed-freshness area (summarised in [L7c area](#l7c-area-not-analysed)) and the conformance harness's comparison of verdicts and diagnostics with plain Verus.

The user goals are numbered as in the lane brief:

| Goal | Meaning |
|---|---|
| G1 | Trust: Sail reports the same per-function verdicts and diagnostics as `cargo verus verify` |
| G2 | Edit-to-verdict latency, and time to the first failure |
| G3 | No overhead compared with plain Verus |
| G4 | Many agents at a cost well below N times one agent |
| G5 | Opt-in stability modes |
| G6 | A result is shown as Current only when its code is unchanged |

## Ranked findings

The ranking score is lines removable multiplied by confidence (high 1.0, medium-high 0.8, medium 0.6, low-medium 0.4). Line counts are approximate and include tests.

| # | Finding | Removable lines | Verdict | Confidence | Score |
|---:|---|---:|---|---|---:|
| 1 | `bench/validation/` JSON dumps and their summariser scripts | 7,511 | delete | high | 7,511 |
| 2 | Python suite for M7 reader acceptance | 3,200 | delete | high | 3,200 |
| 3 | Guarded output owner (ADR 0035) | 1,500 | delete | high | 1,500 |
| 4 | Per-repair compiler-artifact families (ADR 0036) | 1,750 | simplify | medium | 1,050 |
| 5 | Client-side reuse of decoded results, plus exact heap accounting | 1,300 | delete | medium-high | 1,040 |
| 6 | Offline result and output profilers (examples) | 990 | delete | high | 990 |
| 7 | RPC timing diagnostics (`SAIL_JOB_RPC_TIMINGS`) | 900 | delete | high | 900 |
| 8 | Shared HTTP tier changes made to chase timing failures (on main) | 900 | simplify | medium | 540 |
| 9 | `job_read_bench` Rust example, which runs in every gate | 899 | delete | medium | 539 |
| 10 | Managed setup and bootstrap telemetry | 500 | simplify | high | 500 |
| 11 | Process-tree evidence captured on CI timeouts | 473 | delete | high | 473 |
| 12 | Cancellable job RPC | 700 | simplify | medium | 420 |
| 13 | `sail-bench history` | 1,004 | delete (archive) | low-medium | 402 |
| 14 | Exact allocator accounting in the result frame cache | 390 | simplify | high | 390 |
| 15 | `sail-bench seeds` | 670 | delete | medium | 402 |
| 16 | MCP read-client table and CLI error mimicry | 500 | simplify | medium | 300 |
| 17 | Runner pieces coupled to the L7c lease work | 340 | delete with L7c | medium-high | 272 |
| 18 | MCP live-service CI tests | 411 | delete | medium | 247 |
| 19 | VI-0011 launcher workaround in the adapter | 330 | move to fork, then delete | medium | 198 |
| 20 | Repair-bench preparation telemetry, RSS and raw sidecar | 250 | delete / simplify | high / medium | 190 |
| 21 | Engine event relay by file polling | 250 | simplify | medium | 150 |
| 22 | Hashing the runtime identity on every run | 150 | simplify | medium | 90 |
| 23 | Small items (see the list) | ~1,250 | mixed | mixed | ~800 |

### Totals by verdict

| Verdict | Lines | Of which non-Rust |
|---|---:|---|
| Delete, high confidence | ~14,900 | 7,511 lines of JSON and scripts in `bench/validation`, ~3,900 lines of Python |
| Delete, medium or lower confidence | ~5,000 | ~600 lines of Python |
| Simplify (lines removed) | ~6,400 | ~250 lines of Python |
| **Total** | **~26,300** | about 13,000 lines of Rust product and test code |

There are also three behaviour fixes. They remove little code but matter more than most deletions:

- **B1** (G4): the managed runner's compiler-family lock fails rather than waits.
- **B2** (G2): repair Results carry the full graph twice.
- **B3** (G3): the first managed verify re-verifies vstd.

See [Behaviour fixes](#behaviour-fixes-small-diff-large-effect).

Most of these lines trace to one internal target: four readers pulling a 4 MB Result in under 200 ms at p95 (ADR 0027). That gate now passes natively, with Rust p95 at about 35–70 ms ([L5](l5-readers.md)). L5 also measured where a read spends its time:

| Read phase | Time |
|---|---:|
| Input freshness (kernel leases, the L7c area) | 18–24 ms |
| Output seal and hash | 1.6 ms |
| Socket write | about 1.7 ms |

Findings 2, 3, 5, 6, 7, 9, 12, 14 and 16 therefore optimise or instrument phases that were never the bottleneck.

## Findings

### 1. `bench/validation/` dumps and summarisers: delete (high)

- **What:** 29 files and 7,511 lines on main: JSON run dumps, a README and six summarisers (`acceptance.py`, `anvil_acceptance.py`, `http_runtime.py`, `live_workspace.py`, `native_managed_acceptance.py`, `stderr_cost.py`).
- **Requirement:** none current. It contradicts [ADR 0038](../../decisions/0038-m7-rescue.md) §4: "Raw logs stay outside the repository and are cited by sha256."
- **Goal served:** none. They are past evidence and no gate reads them.
- **Alternative:** archive the files outside the repository. Cite each one's sha256 from `docs/validation/m7.md`, the 2026-10-09 and 2026-10-10 reports, the L2 report and `crates/sail-verus/tests/fixtures/README.md`.
- **Risk:** none.

### 2. Python suite for M7 reader acceptance: delete (high)

- **What:** about 3,200 lines:
  - `scripts/m7-reader-acceptance.py` (614) and its test (532);
  - `m7-retained-jobs-acceptance.py` (368) and its test (231);
  - `m7-job-read-bench.py` (261) and its test (94);
  - `m7-owned-actors.py` (160) and its test (100);
  - `m7-native-product.py` (277) and its test (198);
  - `reader-acceptance-run.py` (135);
  - `m7-result-frame-profile.py` (161) and its test (64).
- **Callers:** only `ci/run.sh release` (lines 233–268, local-only) and the docs.
- **Requirement:** [ADR 0027](../../decisions/0027-interactive-proof-engine-and-repair-workflow.md): "Established reads must meet less than 200 ms p95 on Linux under active verification and concurrent clients." [Reader acceptance](../../reader-acceptance.md) adds: "Two collectors run in turn, the Python probe… and the Rust probe."
- **Goal served:** G2, weakly. The gate is already recorded as passing natively. The scripts' own docstrings say they are "never a live-reader acceptance result" and "never freshness or proof authority". Each `test-m7-*.py` tests only its own script.
- **Alternative:** delete the suite. If the gate is ever rerun, add one `sail-bench reader` subcommand.
- **Risk:** reproducing the frozen gate exactly would need the archive tag.
- **Docs:** `reader-acceptance.md`, `native-product-setup.md`, `result-frame-profile.md` and `freshness.md` lose their script sections.

### 3. Guarded output owner (ADR 0035): delete (high)

- **What:** about 1,500 lines:
  - `crates/sail-daemon/src/jobs/output_service.rs` (506) and `output_overlap_tests.rs` (315);
  - the guarded branches and `output_fallback_tests` in `jobs/store.rs` (about lines 160–183, 315–330 and 388–640; about 300 lines);
  - `crates/sail-verus/src/managed/guarded_output.rs` (391);
  - `examples/guarded_output_profile.rs` (108).
  
  `managed/retained.rs` stays, because the L7c lease files use it.
- **Requirement:** [ADR 0035](../../decisions/0035-guarded-retained-output-digests.md): "Prototype an output-only `managed::GuardedOutput` primitive. It acquires the original bounded Linux namespace observer, mount-generation coverage and nonrenewable read leases before independently hashing the exact entire output." Its status is still "proposed … isolated shortcut benefit unmeasured".
- **Goal served:** none. It avoids re-hashing the daemon's own private stdout file. L5 found that 86 of 88 guarded checks fell back to the 1.6 ms full hash anyway.
- **Alternative:** keep the fallback, which re-reads the file and compares its SHA-256. That is the path that already runs.
- **Risk:** none.

### 4. Per-repair compiler-artifact families (ADR 0036): simplify (medium)

- **What, in `crates/sail-verus/src/managed/runner.rs`:**
  - two-lock ownership and the `repair-targets-v1` / `repair-observations-v1` layout in `prepare` (about lines 434–513);
  - `negative_compiler_generation` and `publish_compiler_generation` (672–789);
  - `record_observation_layout` (1271–1286);
  - the second compiler hash.
- **What, in `engine.rs`:** `completed_driver`, `completed_compiler_output` and `bounded_document` (636–706), which only the negative-generation check uses.
- **Tests:** about 750 lines of runner tests and all of `runner_artifact_isolation_tests.rs` (594). The total is about 300 production and 1,450 test lines.
- **Requirement:** [ADR 0036](../../decisions/0036-managed-compiler-artifact-families.md): "Repair requests with an explicit function now use a versioned target generation inside the compiler family." It was triggered by "three ready repair selectors had vstd receipts with the same input revision, but different artifact hashes at the same shared paths."
- **Goal served:** G2. The root cause is nondeterministic compiler output, which the fork exists to remove ([ADR 0039](../../decisions/0039-private-verus-fork.md): "Determinism in the engine makes cached-versus-fresh comparisons exact").
- **Alternative:**
  - one target directory per workspace and compiler identity;
  - focus and repair runs never publish proof receipts;
  - no negative generation, so a failed run simply re-establishes build-script declarations next time.
- **Risk:** the first repair after a failure pays one more build-script run. If a selector's full run still rewrites shared artifacts, dependencies may rebuild more often. Measure this on a native host before landing.

### 5. Client-side reuse of decoded results: delete (medium-high)

- **What:** about 1,300 lines:
  - `crates/sail-api/src/jobs/decoded.rs` (803);
  - `jobs/ownership.rs` (414), heap accounting against hard-coded `serde_json` layout sizes;
  - the `with_`/`without_result_decoding_cache` constructors in `jobs.rs` (about lines 405–460);
  - the shared "decoding cache family" in `sail-mcp/src/job_clients.rs`.
- **Requirement:** [Result client](../../result-client.md): "`jobs::Client::new(socket)` now stages decoding reuse … A second byte-identical response is parsed normally and can promote one independent decoded graph."
- **Goal served:** G2, marginally.
  - A single-shot CLI process can never hit the cache; the doc admits this.
  - In MCP, a hit still clones a 4 MB graph, so it saves only part of a roughly 37 ms decode.
- **Alternative:** a plain parse on every call.
- **Risk:** repeated MCP Result reads lose a few milliseconds.

### 6. Offline profilers: delete (high)

- **What:** about 990 lines:
  - `crates/sail-api/examples/result_frame_profile.rs`, `result_client_profile.rs` and `result_stream_profile.rs` (878), with their `ci/run.sh:250` test step;
  - `sail-verus/examples/guarded_output_profile.rs` (108), counted under finding 3 if that lands first;
  - `docs/result-frame-profile.md` (98).
- **Requirement:** the reader p95 bound (finding 2).
- **Goal served:** none. Nothing in the product reaches them.
- **Risk:** none.

### 7. RPC timing diagnostics: delete (high)

- **What:** about 900 lines:
  - `crates/sail-daemon/src/jobs/rpc_diagnostics.rs` (614);
  - the `Trace` plumbing in `jobs.rs` (about lines 240–290), `worker.rs` and `store.rs`;
  - the metadata test (about 70 lines);
  - the `SAIL_JOB_RPC_TIMINGS` environment filters in `sail-cli/src/jobs.rs` and `managed/runner.rs`;
  - `docs/result-rpc-diagnostics.md` (168).
- **Requirement:** [RPC diagnostics](../../result-rpc-diagnostics.md): "The ring retains the latest 256 fixed metadata rows … The initial allocator capacity is refused if it exceeds 512 rows."
- **Goal served:** none. The only consumer is `reader-acceptance-run.py --timings`.
- **Alternative:** one `tracing` debug line per RPC, if needed later.
- **Keep:** the Linux blocking-accept change (`configure_accept`) and its test. It was L5's largest real fix, removing a 10 ms accept sleep.
- **Risk:** none.

### 8. Shared HTTP tier changes made to chase timing failures: simplify (medium, on main)

- **What:** on main, `crates/sail-daemon/src/shared.rs` grew to 1,457 lines and `remote.rs` to 637 (both from #80), plus `shared/initialization.rs` (524) and `shared/presence.rs` (205). The additions are:
  - building the transport asynchronously at bind time;
  - reusing the loopback transport on refresh;
  - discovering presence and trust off the foreground path;
  - prepared small-body reads;
  - test-only `/proc/thread-self/stat` CPU and phase recorders.
- **Requirement:** amendments to [ADR 0025](../../decisions/0025-http-shared-cache-and-local-first-team-workflow.md), for example "A configured daemon now launches one optional HTTP constructor asynchronously at bind…". They were written against 250 ms deadline misses, which ADR 0038 attributes to x86 emulation. Known issue #76 is still open.
- **Goal served:** G4, but a cold first miss costs one Z3 solve, never a wrong answer.
- **Alternative:** build the client lazily on the lookup helper under one mutex, and reload configuration every 5 s.
- **Keep:** entry verification, trust and signatures, coalescing, the negative cache, backoff, the in-flight cap and `http_server.rs`.
- **Risk:** cold lookups after a configuration change miss more often. Correctness is unchanged. Rerun the #76 fixtures on a native host first.

### 9. `job_read_bench` example: delete (medium)

- **What:** `crates/sail-cli/examples/job_read_bench.rs` and `job_read_bench/{args,measure,process}.rs`, 899 lines. It has `test = true` in `Cargo.toml`, so it builds and tests in every workspace gate.
- **Requirement:** the reader p95 bound. Its `/proc` scans exist to prove "the same live verifier process before and after the RPC".
- **Goal served:** none in the product; this is a passed gate's probe.
- **Alternative:** delete it, or move a minimal latency loop into `sail-bench`. The bench-only code `SAIL0943` goes with it.
- **Risk:** low.

### 10. Managed setup and bootstrap telemetry: simplify (high)

- **What:** `crates/sail-verus/src/managed/diagnostics.rs` (574): 21 setup phases, about 40 bootstrap stages and a capped JSONL sink. It has about 60 call sites in `runner.rs` and about 42 in the L7c bootstrap files.
- **Requirement:** its header: "Opt-in bounded setup timings. These records never authorize a proof or reuse."
- **Consumers:** `docs/managed-cold-start.md` and `reader-acceptance-run.py`.
- **Goal served:** profiling only.
- **Alternative:** about five coarse phases (metadata, context, Cargo, verify, publish) written through the existing `files::event`.
- **Risk:** less detail when profiling cold starts.

### 11. Process-tree evidence on CI timeouts: delete (high)

- **What:** `ci/managed-evidence.py` (233) and `scripts/test-managed-timeout-evidence.py` (240). On a timeout, they walk `/proc` to record the process tree and a scratch inventory.
- **Requirement:** its own docstring: "Private timeout evidence … never proof authority."
- **Alternative:** `ci/test-managed.py` keeps its process-group kill and prints stdout and stderr on failure.
- **Risk:** none.

### 12. Cancellable job RPC: simplify (medium)

- **What:** `crates/sail-api/src/jobs/cancellable.rs` (308) and `cancellable_tests.rs` (505). The only caller is `sail-mcp/src/job_clients.rs:101`.
- **Requirement:** [Result client](../../result-client.md): "`Client::call_cancellable(request, cancelled)` uses one owned, joined monitor and an absolute five-second transport interval … A full listener backlog therefore cannot hold a blocking `UnixStream::connect`."
- **Goal served:** G4, marginally. Reads take about 50 ms natively and already have 5 s deadlines.
- **Alternative:** check cancellation before and after a plain `call` and drop a late reply. For a hard cancel, call `shutdown()` on the stream from the cancel path (about 30 lines).
- **Risk:** in a rare hang, a cancelled MCP read takes up to 5 s to give up.
- **Note:** `cancellable::blocked_read_and_write_cancel…` is one of the flaky macOS tests in `known-issues.md`, and this change removes it.

### 13. `sail-bench history`: delete or archive (low-medium)

- **What:** `crates/sail-bench/src/history.rs` (1,004, on main).
- **Requirement:** it answered ADR 0004's question about key design, which ADR 0008 settled. Its only results are frozen survey data.
- **Risk:** the analysis would need recreating if the key design changes. It stays reachable through git history.

### 14. Exact allocator accounting in the result frame cache: simplify (high)

- **What:** `crates/sail-daemon/src/jobs/result_cache.rs` (467): `Budget`, `Charge`, `ADMISSION_MARGIN` (sized from BTree nodes and Arc headers "for Rust 1.99"), `REFUND_GATE`, owner retention and `retained_size`.
- **Requirement:** [Result frame profile](../../result-frame-profile.md): "The daemon already retains three immutable, preencoded frames per cached job." The rescue review called out the "byte-exact allocator accounting".
- **Goal served:** G2. It avoids re-encoding a 4 MB frame per read, so keep the cache itself.
- **Alternative:** an LRU of about 16 entries mapping a key to `[Arc<Vec<u8>>; 3]`, capped by the sum of `frame.len()`, in about 80 lines.
- **Risk:** memory is bounded slightly less precisely.

### 15. `sail-bench seeds`: delete (medium)

- **What:** `crates/sail-bench/src/seeds.rs` (650, on main) and about 20 lines in `cli.rs`.
- **Problem:** it reseeds after the prelude, which is the same method the ADR 0018 fragility census already uses. `report` skips its output, and only `docs/survey/solver-behavior.md` cites its frozen results.
- **Risk:** low. The survey results remain in `bench/results`.

### 16. MCP read-client table and CLI error mimicry: simplify (medium)

- **What:** about 500 removable lines:
  - in `crates/sail-mcp/src/job_clients.rs` (270), an eight-entry LRU of endpoints, and `call`/`call_endpoint` repeating the same guards;
  - in `job_tools.rs`, `read_error`, `read_or_cli` and `response_value` (about 120 lines), which reproduce exact CLI error text, for example an empty `"SAIL0813: job operation failed: "`;
  - the matching tests in `job_tools_tests.rs` and `test_io.rs`.
- **Goal served:** G2 and G4. Reading over the socket instead of spawning a CLI child is worth keeping.
- **Alternative:** one `Client` for the configured socket, with the CLI as fallback, and plain error strings.
- **Keep:** `response_matches`, the job-identity check that stops an agent receiving a stale answer.
- **Risk:** low.

### 17. Runner pieces coupled to the L7c lease work: delete with L7c (medium-high)

- **What:** about 340 lines:
  - in `managed/files.rs`, `json_metadata` and `metadata_bytes_equal*` (about 215 with tests). They avoid rewriting an unchanged sidecar "on Linux" so that kernel watchers are not disturbed;
  - `receipts.rs` `publish_guarded` and runner lines 2004–2017 (about 80);
  - `selected_closure` and `artifacts_within` (runner lines 1042–1072).
- **Goal served:** none, once L7c replaces the leases.
- **Next step:** give this list to L7c rather than acting on it separately.

### 18. MCP live-service CI tests: delete (medium)

- **What:** `ci/test-mcp-live-service.py` (342) and `ci/test-mcp-live-service-controller.py` (69).
- **Requirement:** AGENTS.md: the controls "establish no proof or reader/performance qualification". They assert mechanism: socket counts, process-group identity, no CLI child spawned, and cancel cleanup under 1 s.
- **Coverage elsewhere:**
  - MCP cancellation is unit-tested in `sail-mcp` (`server.rs:333`, `output_tests.rs:120`, `job_clients.rs:223`);
  - MCP verdicts are covered by `check_mcp`, `test-managed.py` and `test-repair.py`;
  - the controller file tests only the harness.
- **Risk:** a cancel leak in the installed process would go unnoticed. That is low.
- **Docs:** remove the AGENTS.md Gates paragraph about these controls.

### 19. VI-0011 launcher workaround: move to the fork, then delete (medium)

- **What:** about 330 lines:
  - `crates/sail-verus/src/adapter.rs`: `cargo_driver_options*`, `relocate_no_vstd`, `launcher_arguments` (154–297), and the `launcher-options.json` writer (378–396);
  - `engine.rs`: `LauncherAdaptation` and `launcher_notes` (568–625);
  - the matching tests.
- **Requirement:** the code comment: "VI-0011: the pinned launcher chooses macro mode before decoding Cargo's effective per-package options."
- **Goal served:** G1 for engine runs. ADR 0039 makes the fork the right place for this fix. Nothing reads the `launcher-options.json` fields `cargo_identity`, `deferred_environment`, `original_arguments` and `effective_arguments`.
- **Risk:** medium until the fork commit lands with its regression test.

### 20. Repair-bench leftovers: delete or simplify

- **Preparation telemetry: delete (high), about 100 lines.**
  - `crates/sail-bench/src/repair/mod.rs`: `PREPARATION_BUDGET_BYTES`, `struct Preparation`, `Sample.preparation` and `fn preparation` (about lines 640–700);
  - the preparation column in `render.rs`;
  - the fixtures in `judge.rs`.

  ADR 0038's removal note says the `SAIL7105` check "is retired", and no producer of the field remains.
- **Peak RSS and the raw sidecar: simplify (medium), about 150 lines.**
  - `repair/process.rs`: `tree_rss`, `time_rss` and `/proc` sampling (214–265, 505–555);
  - `repair/mod.rs`: `RawRecord`/`RawFile` (427–495);
  - `run.rs`: `open_raw`.

  No gate check (SAIL7101–7110) reads them.
- **Keep:** the harness itself (record, then judge), the outcome comparison and the median gate. Gate 7 is still open.

### 21. Engine event relay: simplify (medium)

- **What:** `crates/sail-verus/src/engine_events.rs` lines 1–275. A thread polls a file every 20 ms and re-validates each event into `SAIL_PROOF_EVENTS`. This needs the extra `Runtime` fields `target_kinds`, `input_revision` and `packages`.
- **Goal served:** G2 (time to first failure), so keep the feature.
- **Alternative:** the fork writes `sail_api::proof::InputEvent` directly, or uses a pipe. That deletes `convert`, the polling loop and the three fields.
- **Keep as they are:** `completed()` and `terminal()`. They decide when a crashed engine falls back to stock Verus, which is a G1 property.

### 22. Hashing the runtime identity on every run: simplify (medium, a G3 issue)

- **What:** `runner.rs` `context()` (105–317), plus `runtime_matches` and `proof_runtime_current_with` afterwards.
- **The cost:** every managed run hashes the full contents of sail, cargo-verus, cargo, z3, the shim, rustc, rustup, rust_verify, the wrapper, engine artifacts and every sysroot file. The sysroot handoff, guard and barrier glue (185–245) exists only to make that affordable. This is a per-run tax.
- **Alternative:**
  - use a stat identity (device, inode, size, mtime, ctime) for installed toolchain and sysroot files;
  - hash only small configuration files;
  - treat any stat change as a reason to re-run.
  
  That satisfies G6 and removes the reason for much of the L7c sysroot machinery.
- **Risk:** an in-place rewrite that preserves mtime goes undetected. That is unusual for installed toolchains. Give this to L7c to decide.

### 23. Small items

| Item | Lines | Verdict |
|---|---:|---|
| `crates/sail-verus/examples/managed.rs`, unreferenced anywhere | 47 | delete (high) |
| `scripts/experiments/l9-bucket-input-proxy.py`, a one-off with no caller | 166 | delete (high) |
| `ci/test-repair-lifecycle.py`, which tests the test harness | 51 | delete (high) |
| `ci/test-repair-engine-provenance.py`: fold its provenance assertions into `test-repair.py`, and drop its check for the removed `preparation` field | 136 | simplify (high) |
| `ci/test-repair.py` evidence handling: summary digests, worker-evidence copies, failing on an evidence write error | 50 | simplify (high) |
| `corpus/repros/verus-cargo-no-vstd/check-engine-forwarding.py`, an engine oracle rather than a reproduction (keep `reproduce.py`) | 193 | delete (medium) |
| `crates/sail-cli/examples/check_mcp.rs`, which copies seven tool input schemas from `proof_tools.rs` (keep the verdict comparison) | 100 | simplify (high) |
| `crates/sail-mcp`: ADR 0034's `ToolResult::to_json`, used only by a test, and the old/new byte-comparison test (keep encoding outside the stdout lock) | 150 | simplify (medium) |
| `crates/sail-verus/src/engine.rs`: `Installed::at`, `preflight` and `validate_request` repeat the same checks; identify the engine by fork commit once `fork.toml` exists | 70 | simplify (high) |
| `crates/sail-daemon/src/jobs/store.rs` `project_bytes`: re-hashes stored stdout on every Status read (`SAIL0816`) to guard the daemon's own 0600 files against the same user | 40 | simplify (medium) |
| `crates/sail-verus/src/managed/absent.rs` (on main): an inotify observer for configuration files created and removed during a run; compare existence before and after instead | 200 | simplify (low-medium) |

## Behaviour fixes: small diff, large effect

- **B1 (G4): the compiler-family lock refuses instead of waiting.** `ownership_lock` (`runner.rs:439–451`) uses `try_lock` and fails with `SAIL3104` "managed target is in use". ADR 0036 made this lock span every focus selector in the compiler family. Two `sail verify --managed` processes, or two daemons, on one workspace now collide. Inside one daemon this is hidden, because jobs run on a single FIFO worker (`jobs/worker.rs:1`). That worker is itself a G4 limit: N agents on one daemon run serially. Fix: a blocking lock with a deadline, and then concurrency per target directory.
- **B2 (G2): repair Results carry the full graph twice.** `store.rs` sets both `result` and `repair.batch.full` from the same project (lines 145–148 and 200–205). `docs/result-frame-profile.md` records that `result` "exactly equalled `repair.batch.full`". Dropping one halves the 4 MB frame for every reader. This changes the contract (`ci/test-repair.py:147` asserts both), so it needs an ADR.
- **B3 (G3): the first managed verify re-verifies vstd** (known issue, about 49 s). It is also the only large CI saving: about 50 s in each of the e2e legacy and managed jobs. It belongs to the managed-overhead lane.

## Error-code sprawl

The crates use 133 distinct `SAILnnnn` codes (106 on main). There is no registry: codes are string literals at the call sites. Codes that name internal states users cannot act on:

- **Job transport, 08xx:** `0804` transport cancelled, `0805` deadline elapsed, `0806` missing decode admission or poisoned client table, `0807`, `0812`, four variants of `0813`, `0816` (finding 23), `0817` RPC diagnostics over their bound (finding 7). Users need one "daemon unavailable or failed; retried with the CLI" code and one protocol-mismatch code (`0802`).
- **Sysroot protocol, 0830–0835:** six codes for an internal daemon handoff (L7c area).
- **`0998`/`0999`:** internal states "original result-read failure" and "cleanup unavailable".
- **`SAIL3114`:** 14 sites, rendered as three notes. The note users see says "native coverage and kernel read leases must be established before claiming Current", which describes mechanism. Replace it with one note: "inputs could not be confirmed unchanged; re-run for a current result."
- **`SAIL3121`:** 22 sites, all engine internals (contradictory launcher modes, sidecar I/O). Fold them into the stock fallback, which is already labelled `SAIL3123`.
- **Benchmark codes:** `0943` and `7101`–`7110`, where `7105` is retired. These are refusal labels in a local benchmark, not user errors, and plain labels would do.

**Recommendation:**
- Add one `codes.rs` table in `sail-api`, with a test that every literal in the crates appears in it.
- Collapse about 30 internal codes into the generic internal-bug path (`SAIL0999`-style, with context) or into the nearest user-actionable code.
- Keep the codes users act on, for example `3001`, `3100`, `3112`, `3120`, `3122`, `3123`, `0650` and `0802`.

## Schema fields nobody consumes

| Field | Where | Note |
|---|---|---|
| `preparation` | `sail-bench` repair `Sample` | no producer remains (finding 20) |
| `cargo_identity`, `deferred_environment`, `original_arguments`, `effective_arguments` | `launcher-options.json` | written, never read (finding 19) |
| `Runtime.target_kinds`, `Runtime.input_revision` | `managed/receipts.rs` | only for the event relay (finding 21) |
| `sail_managed_observation_layout` | `input-state.json` | only for compiler families (finding 4) |
| `ArtifactRequest`, `sail-managed-compiler-artifacts-v1` | runner context hash | compiler families (finding 4) |
| `Projection.stdout_sha256` | `jobs/store.rs` | per-read self-check (finding 23) |
| per-function `provenance.engine` | `sail-api` output | repeats the report-level engine; low priority |
| `result` vs `repair.batch.full` | repair Result | duplicate graph (B2) |
| peak RSS, raw sidecar | repair bench | no gate reads them (finding 20) |

## Python that duplicates Rust

- **Reader probe:** `scripts/m7-job-read-bench.py` duplicates the `job_read_bench` example (findings 2 and 9). Delete both.
- **Engine comparison:**
  - `engine/verus/test-engine.py` (107) overlaps the engine conformance arm. Keep it until the fork runtime is re-pinned (ADR 0039), then retire it.
  - `test-cancel.py` is unique; keep it.
- **`ci/fetch-verus.py`:** duplicates `cargo xtask fetch-verus`, but the image build needs it before the workspace exists. Keep it.
- **`helper_runtime.py` and `lease.py`:** Python in the product runtime path (L7c area). They go if L7c drops kernel leases.

## Docs that describe mechanism rather than user behaviour

- **Delete with their mechanisms:**
  - `docs/result-rpc-diagnostics.md` (168, finding 7);
  - `docs/result-frame-profile.md` (98, finding 6);
  - most of `docs/result-client.md` (382; findings 5 and 12; keep a short "how to read a Result" section);
  - the script sections of `docs/reader-acceptance.md` (118).
- **Shorten to a few lines each:** the status headers and validation history in ADRs 0035 and 0037. ADR 0037's transport rewrite was rejected and left no code.
- **Rewrite after finding 10:** `docs/managed-cold-start.md` (190) as a user guide (what the first run costs and why), not a list of bootstrap stages.
- **Rewrite:** the user-visible `SAIL3114` note wording (above).
- **Remove:** the AGENTS.md Gates paragraph on "installed MCP/live-service read controls" (finding 18).

## What to keep

These serve G1 and G6 directly and are proportionate:

- **Managed runner core:**
  - argument refusal;
  - `expected_targets`, `target_records` and `completed_receipts`;
  - receipt checksum and input-revision binding;
  - the before/after snapshot that decides Current, Historical or Unknown;
  - engine fallback;
  - the requirement that every selected target was observed.
- **`adapter.rs`:** `engine_output` (re-runs stock Verus when the engine crashes), `receipt_success` (a fallback never gets a receipt) and `mark_engine` (provenance).
- **`engine_events::completed`/`terminal`:** the fallback trigger.
- **`attribution.rs` `remove_ambiguous_joins`:** prevents wrong per-function attribution (VI-0010).
- **Job store core:** `classify`, plus repair `accepted()`, which refuses fallback, unknown or provisional results.
- **Shared tier:** entry verification and trust.
- **Engine conformance arm:** comparison of verdicts and diagnostics.
- **CI tests:** `ci/test-managed.py` (assertions on user-visible behaviour), `ci/test-repair.py` assertions, `ci/engine-artifact.py`, `check-ai-markers.sh`, `perf-host.sh`, `check-links.py` and the CI completion guards.
- **Pre-M7 features:** portfolio, fragility and watch features.

## L7c area (not analysed)

L7c is reworking these, so this audit lists them only:

| Location | Lines |
|---|---:|
| `managed/freshness.rs` | 1,808 |
| `managed/lease.rs` and `lease.py` | 1,275 |
| `managed/precise.rs` | 1,173 |
| `managed/input.rs` | 824 |
| `managed/kernel_leases.rs` | 565 |
| `managed/lease_binding_batch.rs` | 319 |
| `managed/coherent.rs` | 262 |
| `managed/observe.rs` | 245 |
| `managed/kernel_mounts.rs` | 179 |
| `managed/freshness_identity_guard*.rs` | 375 |
| `managed/sysroot.rs`, `sysroot_cache.rs` and `sysroot_selection.rs` | 1,870 |
| `managed/build_inputs*.rs` | 3,626 |
| `sail-api/src/sysroot.rs` and `sail-daemon/src/sysroot.rs` | 968 |
| `tests/fixtures/kernel-locks-frozen.txt` | 2,883 |

That is about 13,500 lines of Rust plus a 2,883-line fixture, roughly a third of the rescue branch's crate diff.

One observation for L7c: L5 measured kernel-lease freshness at 18–24 ms of each read, the largest share. Findings 3, 17 and 22 and the `SAIL3114` wording all depend on what L7c chooses.

## Documentation impact

This report is new. No other documentation changed, because this lane makes no code changes. Each lane that removes a mechanism listed here updates the docs named in its finding.
