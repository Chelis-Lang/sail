# L9: M8 warm Verus frontend, design and ADR draft

- **Lane:** L9 (rescue, ADR 0038 item 6.1). **Date:** 2026-10-12. **Status:** draft for the lead; ADR number unassigned.
- **Scope:** this is design work plus one proxy experiment. Nothing here is built, qualified or accepted.
- **Verus:** pinned `426d8b0` (`corpus/toolchain.toml`). Paths below are relative to Verus `source/`.

## 1. Findings that shape the design

**F1. Where time goes.** Stock Verus ran on HumanEval with `--time-expanded`. It was cold, on an Apple M3 (8 cores, macOS 26.5), at load average 20 to 31 from other lanes, so wall times are inflated. Under Sail's warm cache the SMT row largely disappears ([ROADMAP](../../ROADMAP.md) vstd profile).

| Stage (cold run, 441 functions, 105 buckets) | Time |
|---|---:|
| rustc GHOST pass (`init-and-types`) / EXEC pass (`compile-time`) | 2.2 s / 1.05 s |
| VIR: HIR 0.59 s, vstd import 0.37 s, `rust_to_vir` 0.59 s | 2.3 s |
| Per-bucket non-solver CPU (`verify-time` minus SMT; AIR command time is 11.9 s of it) | 18.6 s CPU |
| SMT run (cold; served from cache when warm) | 141 s CPU |

Per-bucket work is about 177 ms CPU per bucket, even for tiny modules. Each bucket re-emits its pruned vstd context into a fresh solver session through the shim: on the toy crate, one bucket spent 42 ms in AIR. `--no-verify --build-sst` puts prune, `Ctx::new` and `ast_to_sst` (with logging) at no more than 33 ms per bucket, single-threaded.

**F2. The bucket is the unit of observational identity.**
- Each bucket gets its own Z3 process: `rust_verify/src/verifier.rs` `verify_bucket_outer` and `verify_bucket`. Spinoffs get their own processes too.
- ADR 0008 keys bind the whole process prefix, so skipping one query would change every later key and the solver's history.
- Inside a bucket, `OpGenerator::next` (`rust_verify/src/commands.rs`) emits operations in the order of the **global** SCC vector `ctx.global.func_call_sccs`, filtered to the pruned functions. A bucket key must therefore bind that projection.

**F3. Raw keys miss almost everything; canonical keys almost nothing.** Experiment E1 (`scripts/experiments/l9-bucket-input-proxy.py`) hashed each bucket's span-free pruned VIR (`--log vir-poly`, `vir-option=no_span`) after every HumanEval agent-loop step and repair cycle. The table shows the number of buckets, out of 105, whose digest changed:

| Edit | Raw digest | Item-relative canonical digest |
|---|---:|---:|
| No edit (4 reruns) | 0–1 | 0–1 (`human_eval_160` only) |
| In-line edits (`wrong-spec`, `fix-spec`, `strlen-failing-attempt`, `strlen-repaired-attempt`) | 1–2 | 1–2 |
| Line-count-changing edits (`refactor-body`, `drop-invariant`, `restore-invariant`, `split-mod`, `gcd-body`, `concatenate-spec`, `strlen-refactor`) | **76–91** | **1–2** |

- The cause of the raw misses is crate-wide counters. `UnfinalizedReadKind.id` comes from `unique_read_kind_id` (`rust_verify/src/context.rs`). Span `id` comes from the atomic `next_span_id`, and span `data` packs global `BytePos` (`rust_verify/src/spans.rs` `to_air_span`, `pack_span`).
- Ranking ids crate-wide is not enough. Imported vstd ids and local ids share one integer range, so a shift can reorder them. Ranking per top-level item works.
- This explains why M7's prepared-lowering keys got zero cross-edit reuse (ADR 0028).
- Evidence: summary sha256 `7400856b54d945c9216067fef2bc5167d2be23a5b3267e93ad46d9c6cf91c6b6`, kept outside the repository. Stock timing output sha256 `36c14d2acb317c623fc9b8ac7b134d64b75ec1a0a44132142ae53e5954714350`.

**F4. VI-0001 is visible.** `human_eval_160` flipped in 10 of 15 comparisons. Its cold SMT run takes 6.4 s CPU, so a flip that misses both the memo and the SMT prefix is a multi-second tail.

**F5. Bucket effects are separable.**
- In the parallel path, each bucket gets `global_ctx.from_self_with_log` and writes back only `qid_map` and `chosen_triggers` (`vir/src/context.rs` `GlobalCtx::merge`).
- It adds counters, `func_details`, `bucket_stats` and `deferred_errors` (`Verifier::merge`), and messages through `QueuedReporter`.
- `vir/` and `air/` contain no statics, thread-locals or atomics.
- SMT bytes carry no spans. Labels are per-query counters (`%%location_label%%N`; `crates/sail-smtlib/tests/fixtures/verus-toy.in.smt2`).

## 2. ADR draft: NNNN, M8 warm Verus frontend via hermetic bucket memo

- **Status:** proposed. **Context:** F1 to F5, charter roadmap ("A warm Verus frontend reusing compilation state across invocations"), ADR 0027, 0028 and 0038.

### Decision

1. **Skip whole unchanged buckets** in the opt-in Sail engine. A bucket whose complete input matches a memo entry is not lowered, emits nothing to a solver, spawns no shim, and restores its recorded effects. Granularity is the bucket, not the function (F2). The SMT cache stays the second line of defence for every bucket that runs.
2. **Hermetic input: the key is the only argument.**
   - Define `BucketInput`, an owned, serde-derived struct, and lower a bucket only from it: `GlobalCtx::for_bucket(&BucketInput)`, then the stock `Ctx::new`, `ast_to_sst_krate`, `poly_krate_for_module`, `verify_bucket` and `OpGenerator`.
   - The key is SHA-256 over the domain tag plus the whole serialization of `BucketInput`. No field is hand-picked, so adding a field keys it automatically.
   - The bucket worker never sees the full krate or the crate `GlobalCtx`. Missing data cannot silently go stale; it shows up as an engine-versus-stock difference on every cold run (§5).
3. **`BucketInput` contents:**
   - **Runtime identity:** Verus commit, patch digest, Rust toolchain, and imported `.vir` digests.
   - **Solver identity and mode:** Sail solver identity (name, version, binary SHA-256, platform) and Sail mode label, so keys cannot collide across solvers or verdict-changing modes (charter rules 2 and 8).
   - **Settings:** the complete engine argv, the relevant environment and `error_format`.
   - **Bucket:** the `BucketId`, the full `Fun` list and the user filter.
   - **Pruned krate:** the canonical pruned krate from `vir::prune::prune_krate_for_module_or_krate`, plus all `PruneInfo` fields in their original order.
   - **Global projection:** `datatypes`, `fun_bounds` and `fun_attrs` for the names present; the ordered SCC subsequence intersecting the bucket, with full ordered node lists and cycle data; trait-extension maps; the datatype-graph projection; and the scalar `GlobalCtx` options.
   - **Refused:** logging, profiling, debugger, interpreter-log and capture flags disable the memo outright.
4. **Canonical coordinates.**
   - Every span becomes (item ordinal, local ordinal). Every crate-wide counter (span id, read-kind id) becomes its order-preserving rank within its top-level item.
   - Hermetic lowering runs **on canonical spans**. Emitted messages and chosen triggers come out in canonical coordinates. They are bound to real spans only through the table built from the **current** callback's pruned krate.
   - On a hit, this binding is exactly what fresh hermetic lowering of the current revision would have produced, by purity. No compiler context, source map or raw span crosses a revision, which meets ADR 0027.
5. **Memo value.** The memo stores the bucket's effects in canonical coordinates:
   - counters, `func_fails`, `func_details` rlimits and proof notes;
   - chosen triggers, `qid_map` entries and queued messages, with level and report method;
   - `deferred_errors`;
   - for each solver process, the stream digest and reply digests.

   Timing-dependent `report_now` long-running notes are neither recorded nor replayed. Stock also omits them when queries are fast.
6. **Daemon vouching (rule 7).**
   - The engine writes entries only after the bucket completes, to a local, checksummed, sole-writer store. Entries are never shared-tier.
   - The engine uses an entry only if the daemon's allow-list for the run names it. The daemon lists an entry only when its ledger shows every check of the establishing run confirmed by the pinned solver or a confirmed cache hit, and those SMT entries are still present.
   - A hit reports Confirmed, with provenance `engine-bucket-memo{memo, establishing_run}`. Results flagged unstable (ADR 0009) are never memoised. Cache eviction or `sail cache clear` turns hits into misses.
7. **Warm process, not a resident compiler.**
   - The daemon keeps one pre-spawned engine process per workspace. While idle it has finished dynamic loading and has preloaded and deserialised the vstd `.vir`, checked against its digest.
   - Each revision consumes the spare, which runs one compiler and exits. Isolation, fail-safe exits and ADR 0027 hold trivially.
8. **Scheduling and streaming (§6).** Dependency information orders work only (rule 6). Final acceptance still requires a complete Current result (ADR 0027).
9. **Retire M7's persistent prepared-lowering checkpoint** (ADR 0028 candidates) once M8.1 qualifies. It is off by default (ADR 0038) and superseded.

### Consequences

- Memo correctness reduces to **purity of hermetic lowering**. Equivalence with stock is a separate, always-observable engine property, gated like the rest of the engine.
- About 3k lines of patch replace the 11k-line checkpoint direction.
- M8 stays opt-in and labelled. Making it default would need a separate owner decision.

## 3. Options, savings and risk

Savings are per warm HumanEval cycle (natively 6–12 s). They are estimates from F1 scaled to 7–8 threads; L1's native profile replaces them (§8).

| Option | What it removes | Est. saving / cycle | Risk | Rank |
|---|---|---:|---|---|
| **(c) Hermetic bucket memo** | Lowering, AIR, SMT text, shim spawns and IPC for unchanged buckets: 103–104 of 105 (F3) | 2.5–4 s (35–50%); ~18 s CPU | Medium: hermetic `GlobalCtx` must equal stock (caught by the cold engine-vs-stock arm) | **1** |
| (d1) Spare process with vstd preload | Startup, 0.15–0.37 s import | 0.2–0.5 s | Low | 2 |
| (d2) Cargo bypass when only target sources changed | `cargo` metadata, fingerprinting and build scripts; replays recorded driver argv and env, falls back to Cargo on any mismatch | unknown (L1) | Medium: exact argv/env equality | 3 |
| (d3) Shim IPC batching | Only the edited bucket's stream remains after (c) | < 0.1 s | Low | 6 |
| (b1) rustc incremental on the EXEC pass only | Part of 0.5–1.05 s | 0.3–0.7 s | Low to medium: stock rustc path; check whether `cargo verus` already passes `-C incremental` | 4 |
| (b2) rustc incremental or query reuse on the GHOST pass | Part of 0.9–2.2 s; `verus!` expansion still reruns | 0.5–1.5 s | **High**: overridden providers (`mir_built`, `mir_borrowck`, `lower_to_hir`), forked `rustc_*_verus`, and diagnostics replayed from rustc's cache | 7 |
| (a) Long-lived in-process driver with per-function products | Beyond (c), only the edited bucket's own generation | ≤ 0.2 s on HumanEval | High: `std::process::exit` and panics in the driver, rustc globals, ADR 0027 | 8 |
| (a′) Per-function context products inside a missed bucket | Context generation for large modules (Vest, Anvil) | Measure first | Medium | 5 |

(a) as framed is subsumed. Content-addressed products live at bucket granularity (F2). The "long-lived" part becomes the spare process (d1). Per-function keys remain for attribution and scheduling.

## 4. Charter compliance

- **Rule 6.**
  - Option (c): `BucketInput` is the sole input and the key covers it completely, so invalidation is correct by construction. Item digests from the previous revision only reorder work.
  - Options (d1) and (b1): no reuse beyond immutable, digest-checked vstd bytes, or rustc's own incremental contract.
  - Option (b2) would trust rustc's dependency graph under Verus's overridden providers, which is why it is ranked last.
- **Rules 1 and 10.**
  - Verdicts and diagnostics must equal stock: the oracles in §5 are mandatory for every option.
  - Any memo decode error, missing allow-list entry, key failure or unsupported flag is a bucket miss.
  - An engine fault, a malformed seal or a missing seal means stock fallback (ADR 0038).
  - Restored output is emitted in bucket order. That is a member of stock's run-dependent order set (VI-0007), and chosen-trigger merge order is likewise thread-dependent in stock.
- **ADR 0027 rule.** Only content-addressed bytes in canonical coordinates survive a revision (§2.4). Spare processes never run a second compiler.
- **VI-0001.**
  - With the memo, the flip costs a bucket miss in up to about half of cycles, and while only one order is cached it also costs an SMT miss: 6.4 s for 160 (F4).
  - A deterministic order in the engine would make memo and SMT hits steady from the first run, help shared and CI tiers, and make the oracles reproducible. The order would be declaration order in `builtin_macros/src/enum_synthesize.rs`, about 5 lines, rebuilding the engine's own vstd.
  - Steady-state local gain is small once both orders are cached. This needs owner sign-off (Q1).

## 5. Conformance oracles

1. **Engine versus stock, cold (memo off), on the fast and standard corpus.**
   - Verdicts and diagnostics must be identical (ADR 0009 instability classes apply).
   - Solver streams must be byte-identical under same-order controls (ADR 0027 amendment).
   - This qualifies `GlobalCtx::for_bucket` and canonical-span lowering.
2. **Lower from key.**
   - In qualification, serialise the `BucketInput`, deserialise it in a fresh worker, lower, and compare streams and effects with in-memory lowering.
   - This proves the hashed bytes are a sufficient input.
   - Negative control: a test build that drops the SCC projection must fail on a fixture where an unrelated added function reorders SCCs.
3. **Cached versus fresh.**
   - In oracle mode, every hit also runs the bucket fresh with independent owners. Compare stream digests, effects and messages after remapping.
   - Run this over every step of `corpus/agent-loops.toml`, both repair catalogues (`corpus/repair-edits.json`) and history replay, with stock verdicts and diagnostics checked at every step.
   - In production, a sampled background shadow re-check reports a stable `SAILnnnn` bug and disables the memo for the session on a mismatch.
4. **Fault injection** (`docs/fault-injection.md`): corrupt, truncated, misfiled and old-version entries; a body-digest mismatch under a matching key; an evicted SMT entry or missing allow-list entry; a kill during write; concurrent writers; cancellation mid-bucket; and a memo written by a cancelled revision used by the next. Each must end in a miss or a correct answer.

## 6. Fail-fast, edit-prioritised streaming

- **Pre-pass.** Workers compute all bucket keys in parallel (prune, canonicalise, hash: tens of ms per bucket). They restore hits at once.
- **Miss ordering.**
  1. Buckets containing an item whose own canonical digest changed (the edit). The previous revision's digest table is scheduling-only.
  2. Their dependants.
  3. Everything else.
- **Order inside a bucket stays stock.** Bytes must match stock, so the engine does not reorder within a bucket. For large buckets, an optional speculative filtered process (M7 focus) is provisional only.
- **Events** go to the proof-event journal (ADR 0027), as new versioned variants:
  - `bucket-memo-hit`;
  - provisional `function-result`, with source `smt-cache` or `solver`;
  - `first-failure`, with diagnostics rendered by the current compiler;
  - `compiler-sealed`.

  The daemon promotes results to Confirmed only after a complete Current result. MCP event reads and `sail watch` show the first actionable failure immediately.
- **Cancellation.** On a newer revision, the daemon signals the engine. The engine stops dequeuing, shims abandon their solver work, and the process exits unsealed. Completed buckets' memo entries remain valid: they are content-addressed, and vouching depends on solver confirmation, not on revision currentness. The next revision starts on a fresh spare.

## 7. Implementation plan

**Verus patch surface** (about 3k lines including tests; the pinned commit only; never filed upstream):
- `rust_verify/src/verifier.rs`:
  - `verify_crate_inner`: key pre-pass, `tasks` order, ordered restore and merge, cancellation checks;
  - `verify_bucket_outer`: build input, look up, lower hermetically;
  - a capturing reporter for canonical messages.
- New `rust_verify/src/bucket_memo.rs`: `BucketInput`, canonicalisation, hashing, store, effects codec, allow-list.
- `vir/src/context.rs`: `GlobalCtx::for_bucket`.
- `vir/src/ast_visitor.rs`: krate-wide span and id mapping.
- `rust_verify/src/import_export.rs`: preloaded imports.
- `rust_verify/src/main.rs` and `driver.rs`: spare mode.
- `builtin_macros/src/enum_synthesize.rs`: deterministic order, only if Q1 is approved.

**Sail crates touched:**
- `sail-verus`: engine mode, memo directory, allow-list, events, fallback.
- `sail-daemon`: spare pool, vouching, cancellation, memo GC in `sail cache gc`, Cargo bypass.
- `sail-cache`: read-only presence and provenance probe.
- `sail-ledger`: memo provenance.
- `sail-api`: schema and events, versioned.
- `sail-mcp` and `sail-cli`: streaming and `watch`.
- `sail-conformance`: the four oracle arms.
- `sail-bench`: the engine-warm arm.

No new layering edges are expected; check with `cargo xtask check-layering`.

**Milestones.** Every exit is measured on native x86-64 Linux (ADR 0038), over at least 3 alternating rounds against M6 stock-managed, on both catalogues.

| Milestone | Content | Size | Exit |
|---|---|---|---|
| M8.0 | Per-bucket stage timers (prune, key, `Ctx::new`, `ast_to_sst`, poly, op generation, AIR command, shim wait), crate-level timers, Cargo and Sail overhead; E1 on Vest (Cargo) | S: ~150 Verus lines + script | Stage table for both catalogues. Proceed with (c) only if unchanged-bucket work is ≥ 25% of the median cycle |
| M8.1 | Hermetic bucket memo for verified buckets, including chosen-trigger and note replay; vouching; oracles 1–4 | L: ~2k Verus, ~1.2k Sail | All arms at zero differences. Memo hits ≥ 95% of unchanged buckets on every HumanEval step (E1 predicts 103–104 of 105). HumanEval median whole loop ≥ 1.5× vs M6; Vest reported |
| M8.2 | Failing buckets, expand-errors, recommends and deferred errors replayed in canonical coordinates | M: ~800 Verus | Failure and repair cycles plus ADR 0027's failure fixtures (bitvector, recursion, late EXEC, duplicated diagnostics) identical to stock |
| M8.3 | (d1), (d2), (b1) if M8.0 supports it | M: ~400 Verus, ~800 Sail | ≥ 2× median whole loop vs M6 on **each** catalogue (the ADR 0027 gate); `ci/local.sh e2e` identical |
| M8.4 | §6 streaming, prioritisation, cancellation | M: ~300 Verus, ~1k Sail | Median time to first actionable failure on failing steps ≤ 1.2× (fixed overhead + edited-bucket time). Cancel to idle ≤ 200 ms p95. No provisional result reported as verified |
| M8.5 | (a′) or (b2), only if M8.0 or M8.3 profiles justify it | TBD | Its own ADR |

## 8. Re-ranking with L1's profile, and open questions

**Re-ranking rule.** Score each option as (measured stage share × fraction removable) divided by a risk weight: low 1, medium 2, high 4.
- If unchanged-bucket work is below 25% of the cycle, (c) drops below (d1) and (d2).
- If Cargo or Sail overhead dominates, M8.3 moves ahead of M8.1.
- If the GHOST rustc pass dominates, revisit (b2) with a spike.

**Owner and lead questions:**
- **Q1.** May the engine emit one fixed `arrow_*` order (VI-0001)?
- **Q2.** Are vouched memo hits acceptable as Confirmed engine provenance?
- **Q3.** Should M8 drop M7's separate focused phase in favour of prioritised scheduling?

**Answered 2026-10-12 (lead, owner agreed to build the better cache design):**
- Q1: yes. The engine emits declarations in source order and picks triggers in a fixed order. Stock Verus can already produce those orders, and determinism makes cached-versus-fresh an exact test.
- Q2: no allow-list vouching. A bucket is memoised only after every one of its checks was answered by the pinned Z3, fresh or from the result cache; a hit is then Confirmed with memo provenance.
- Q3: yes. The edited functions are scheduled first within one run instead of a separate focused phase.
- M7's prepared-lowering reuse and its oracle are deleted, not retained behind a flag.

**Limits.**
- E1 is a proxy. The log omits the SCC and global projections and may omit printer fields, so it can over-estimate hits.
- Timings come from one loaded M3 and are not reference measurements.
- Vest was not measured.
