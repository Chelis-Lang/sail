# L17: error-code registry and remaining audit items

- **Date:** 2026-10-13
- **Base:** `origin/rescue/m7` at `e20d849`
- **Scope:** the error-code part of the [L13 audit](l13-simplification-audit.md#error-code-sprawl),
  audit items #13, #15, #21 and #23, and docs that described removed
  mechanisms. Left alone as instructed: #4, #8, #19 (VI-0011 is not fixed in
  the fork), `engine/verus`, `fork.toml`, `ci/engine-artifact.py` and the CI
  image inputs.
- **Total:** 6 commits before this report, 582 lines added and 3,135 removed
  (`git diff --shortstat e20d849`).

## Error codes

| | Before | After |
|---|---:|---:|
| Distinct codes in `crates/`, `xtask/`, `ci/`, `scripts/` and `bench/` sources | 126 | 110 |
| Codes with a registry entry | 0 | 110 |

- **Registry.** [docs/errors.md](../../errors.md) lists every code with its
  meaning and what to do, one row each. `cargo xtask check-error-codes`
  (`xtask/src/error_codes.rs`) fails when a code in the sources is not listed,
  or a listed code is no longer used. Its test runs in `ci/local.sh gate` and
  `ci/run.sh host`, which both run `cargo nextest run --workspace`.
- **Folded** (17 codes removed, 1 added; listed in `CHANGELOG.md`):
  - `0801` into `0802` (job protocol error);
  - `0809` into `0808` (job failed or its result could not be saved);
  - `0901` into `0902`. `0901` only ever validated events internally and was
    never shown;
  - `1002`–`1008` into `1001`. SMT-LIB parse errors never reach a user, because
    the shim forwards bytes unchanged;
  - `2005` and `2006` into `2004` (cannot start or keep track of the solver);
  - `3110` into `3111` (the managed adapter failed);
  - `3121` into `3123`. Adapter faults already surface as an engine fault with
    the stock fallback;
  - `0804` and the "missing RPC response" use of `0814` into the new `0899`,
    an internal job-service error. Proof-journal lock and load failures moved
    from `0902` to `0999`, which is now the internal proof-stream error. These
    are the one-per-subsystem internal codes that AGENTS.md asks for;
  - `sail verify`'s missing cache directory now reports `SAIL3001` instead of
    printing `SAIL0104` and `SAIL3001` together. `0104` stays for the
    fragility trace directory;
  - `0998` and `0999` were used only as fake daemon replies in a test, which
    now uses registered codes;
  - `7105` is retired and survived only in a comment.
- **Kept, and why:**
  - every code a user can act on (usage, setup, I/O, Currentness, engine);
  - the developer-tool codes `3010`–`3043`. Their users are Sail developers,
    and each names a different fix;
  - the repair-gate labels `7101`–`7110`. Gate reports and
    `docs/bench/m7-repair-acceptance.md` match on them;
  - `0816` and the per-read seal check (audit #23). L14 kept them deliberately,
    with a test. Dropping the check would save about 1.6 ms per read, but a
    damaged store would then serve a stale frame instead of an error;
  - `ci/engine-artifact.py`'s `0944` (out of scope) is registered, not changed.

## Audit items

| # | Item | Result |
|---:|---|---|
| 13 | `sail-bench history` | Deleted. It stays in tag `archive/l11-warm-sessions-spike`. ADR 0040's future no-history replay gate says to restore it from there |
| 15 | `sail-bench seeds` | Deleted. No gate or guide used it, and the survey results stay in `bench/results/`. The ignored solver-tier tests that exercised only these tools went with them. The rlimit test stays |
| 21 | Engine event relay | Kept as polling. The audit's alternative (the engine writes `InputEvent` itself, or uses a pipe) needs a fork change, which is outside this lane. The five repeated failure paths now share one closure (−20 lines) |
| 23 | `examples/managed.rs`, `scripts/experiments/l9-bucket-input-proxy.py`, `ci/test-repair-lifecycle.py` | Deleted |
| 23 | `ci/test-repair-engine-provenance.py` | Deleted. Its project, report and per-function engine provenance assertions, and its no-`engine_fault` check, are now in `ci/test-repair.py` and run in CI's e2e repair phase rather than only locally. Its `batch.full` and `preparation` checks were stale after L15 and L7a. `ci/run.sh release` had nothing left and is removed, along with its AGENTS.md tier |
| 23 | `ci/test-repair.py` evidence handling | Removed the summary digests, the worker-stream copies and the failure on evidence-write errors. Actor cleanup and every scenario assertion are unchanged |
| 23 | `check-engine-forwarding.py` and `forwarding-oracle.json` | Deleted. Archived outside the repository (sha256 in VI-0011). `reproduce.py` stays |
| 23 | `check_mcp` copied schemas | Replaced by an exact comparison of the served `tools/list` with this build's definitions, plus the five contract properties that the old mutation tests pinned (required identity fields, cursor minimum and engine enum). This is stricter: all 15 schemas are now compared, not 7 |
| 23 | `ToolResult::to_json` | Deleted. The test now checks the expected `into_json` document directly |
| 23 | `engine.rs` triple validation | `Installed::at` reuses `preflight`, which also adds the schema-version check that `at` lacked. One `invalid()` and one `runtime_dir()` helper |
| 23 | `store.rs` `project_bytes` | Kept; see `0816` above |
| 23 | `managed/absent.rs` | Kept. Comparing existence before and after a run misses a configuration file created and removed during it, which Cargo may have read. That would label a result Current wrongly (G6) |

## Docs

- `docs/trust.md`: the 100-line kernel-lease section is replaced by a short
  user-level summary of [ADR 0041](../../decisions/0041-content-and-stat-currentness.md).
- `docs/workspace-verification.md`: the sysroot-handoff paragraph and the
  `SAIL3114` namespace-observer labels are replaced by the single user note.
- `docs/workspace-hygiene.md`: rewritten as "Disk use and cleanup": where
  managed targets live and how to reclaim them. It drops the agent process and
  lease text.
- Grep after these changes: no live doc, script or crate mentions leases, the
  observer as a currentness mechanism, the oracle, preparation reuse, guarded
  output, the sysroot handoff, `SAIL_JOB_RPC_TIMINGS` or `bench/validation`,
  except:
  - ADRs, reports, validation indexes and VI entries, which record history;
  - `docs/freshness.md`'s one-line note that earlier versions used leases;
  - `bench/acceptance/*.json` (pre-M7, from main). These are raw dumps that ADR
    0038 would keep outside the repository. They were not in scope and are
    left for a follow-up.

## Recommendation for #4 (compiler-artifact families)

Measure before simplifying. On a native host, take the L7c HumanEval and Vest
repair catalogues (`sail-bench repair run`, three alternating rounds). Compare
the current per-selector `repair-targets-v1` generations with a branch that
uses one target directory per workspace and compiler identity, where focus and
repair runs publish no receipts and there is no negative generation. Record:

- the first repair after a failure;
- a repair of a second selector after the first;
- the warm repeat;
- the dependency rebuild count;
- disk use under `compiler-artifacts/`.

If the shared target is no slower on any of these and the conformance engine
arm stays at zero differences, land the simplification. That deletes about
300 production and 1,450 test lines and the B1 family lock. If a selector's
full run still rewrites shared artifacts, which is the original ADR 0036
trigger, keep the families until the fork's determinism work removes that
rewrite.

## Validation

- **Native host:** EC2 `c7i.4xlarge`, `us-east-1`, lane `l17`. The CI image
  `sail-ci:3f31e6fc827f00bf` was built natively from the committed inputs.
  The run used `1bc48d8`, which differs from this report's commit only by
  this report.
  - `ci/local.sh gate`: pass. 520 tests, plus fmt, clippy, rustdoc,
    layering, deny, the links check and the error-code check.
  - `solver`: 35/35.
  - `e2e`: all three phases pass. The repair phase printed "success with
    engine provenance".
  - `conformance fast pr`: 33/33 same in pass-through, cache fill and cache
    warm. The engine arm was also 33/33 same, with 0 unstable.

  The host was torn down. Logs are archived in `l17-host-logs.tar.gz`, sha256
  `a3271c309f4bb236e3da08f7bcdb732428fe29bb8656edccd1c8acf37f4d092c`.

- **macOS** (`ci/run.sh host`, Apple M3, at `1bc48d8`): clippy clean; 496
  tests passed, 35 skipped.

## Documentation impact

Changed:

- new: `docs/errors.md`;
- rewritten: `docs/trust.md`, `docs/workspace-verification.md` and
  `docs/workspace-hygiene.md`;
- code references updated: `docs/mcp.md`, `docs/result-client.md`, VI-0011,
  `docs/divergences.md`, `docs/bench/m7-repair-acceptance.md` and
  `corpus/repros/verus-cargo-no-vstd/README.md`;
- removal notes: `docs/survey/solver-behavior.md` and ADR 0040;
- links and tiers: `docs/README.md`, `README.md`, `docs/known-issues.md` and
  `AGENTS.md` (the release tier is removed);
- `CHANGELOG.md` and this report.
