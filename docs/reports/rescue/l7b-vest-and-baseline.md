# Rescue lane L7b: Vest blockers and baseline validity

Status: four of the five defects are fixed. One of those fixes is an engine
patch, on a separate branch, that needs a new engine artifact. The fifth defect
(Vest's read-time currentness) is diagnosed, but fixing it needs an owner
decision. All evidence comes from a native host. This is a smoke run, not
acceptance evidence.

## Environment

- **Host.** AWS EC2 `c7i.4xlarge` (16 vCPU, native x86-64), Ubuntu 24.04.
  - Brought up with `scripts/perf-host.sh up l7b c7i.4xlarge`.
  - Taken down with `scripts/perf-host.sh down l7b`.
- **Container.** The CI image `sail-ci:8e24fb5f7f490c01`, built natively. Work
  trees, scratch and caches were on a Docker volume.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0 and Rust 1.98.1.
- **Base.** `rescue/m7` at `5fb311f`.

## Branches and commits

| Branch | Commit | Content |
|---|---|---|
| `l7b-vest-and-baseline` (for `rescue/m7`) | `b775638` | fix 1: clean `verus-partial` before a focused check |
| | `0a351c5` | fix 2: `SAIL7110`, an invalid M6 baseline sample |
| | `c026b31` | fix 4 (run time): coverage scoped to the selected dependency closure |
| | `256cafb` | docs: M6 arm and selection-scoped coverage |
| | `a12cf4f` | refactor: split the M6 judge checks (clippy) |
| | this report | |
| `l7b-engine-focus` (on top of the above) | branch tip (measured as `8d5be94`, same patch) | fix 3: engine focused report keeps only the selected function |
| `rescue/m6-baseline` (pushed to origin) | `26f124f` | `0fde777` plus fix 1 and its test |

`l7b-engine-focus` changes `engine/verus/interactive.patch`. That changes the
patch digest that `sail` binds into the engine identity, so it cannot merge
until a new engine runtime is built, qualified and published, and
`engine/verus/artifact-linux.json` is updated. L7a is also changing the engine,
so the lead should combine the two patches into one artifact.

## 1. M6 focus `SAIL3107` (product bug)

**Cause.**
- `cargo verus focus` sets `verify_deps = false`, so it builds in
  `<target-dir>/verus-partial` with `CARGO_UNSTABLE_CHECKSUM_FRESHNESS=true`
  (cargo-verus `src/plan.rs`, and `src/subcommands.rs:393-397` and `:588`).
- Sail's forced, missing-receipt and stale-script cleanups ran
  `cargo clean --target-dir <target-dir>`.
- With byte-identical source, the focus build stayed fresh by checksum, so no
  verifier ran and no receipt was written.
- The retry cleaned the same wrong directory, so the run failed with `SAIL3107`.
  Repeating `sail verify --incremental -- --verify-root --verify-function f`
  on unchanged source is enough to trigger it.
- With Cargo 1.99, cleaning an outer target that holds only `verus-partial/`
  fails outright (`missing or invalid CACHEDIR.TAG`).

**Fix.** `clean()` in `crates/sail-verus/src/managed/runner.rs` now cleans
`cargo_build_target()`, which is `<target>/verus-partial` for a focused
invocation and `<target>` otherwise.

**Tests.**
- `focused_cleanup_rebuilds_byte_identical_partial_source` is a real-Cargo test.
  It shows that repeated content stays fresh under checksum freshness, that
  cleaning the outer target does not help, and that the focused cleanup forces
  recompilation.
- `ci/test-managed.py`, which runs in `e2e managed`, now runs three focused
  checks over byte-identical source, and each must run a verifier.

**Native evidence** (the managed acceptance script, with the repeated-focus
step):

| Binary | Result |
|---|---|
| `rescue/m7` `5fb311f` | First focus passes with 2 drivers. The second fails: `SAIL3100`, wrapping `SAIL3107` |
| Fixed | All three focused checks pass with exit 0, Current, one driver each |

## 2. A valid M6 baseline arm

- **Branch.** `rescue/m6-baseline` is `0fde777` plus one commit, `26f124f`: the
  fix 1 backport and its two tests, cherry-picked cleanly. It was pushed to
  origin.
- **Acceptance doc.** `docs/bench/m7-repair-acceptance.md` now builds the M6
  arm from `26f124f`. The frozen catalogue's `baseline_commit` is unchanged.
- **Harness, building on what L3 implemented.**
  - L3 recorded only invocation failures (`SAIL7101`). A focus that printed a
    JSON document with `error: SAIL3107` was counted as M6 time.
  - `m6_facts` now records `baseline_error` when the focused or full document
    reports a Sail error.
  - `judge` raises `SAIL7110` for that.
  - `tally` excludes any M6 sample with `SAIL7110` or `SAIL7101` from M6's
    totals, so its round is incomplete.
  - Failing M7 samples keep their time.
  - Tests: `a_failed_m6_check_is_an_invalid_sample_never_m6_time` and
    `m6_and_m7_facts`.

## 3. Vest `SAIL3122` ("exact repair function was not established")

**Cause.**
- Selection worked: the engine matched the function, the focus phase ran and
  verified it.
- Building the focused report then failed with
  `focus-unavailable: focused report allocation budget`.
- A temporary diagnostic build showed why. The focused verifier's
  `func_details` holds 622 functions, and 17 friendly names repeat because of
  trait impls. Examples are
  `vest_lib::combinators::mapped::Mapped::{spec_parse, byte_len, lemma_parse_safe, …}`.
- `focused_document` refuses the whole document on a repeated key. The metadata
  charge was only 665 KB of 8 MiB, so the budget was not the cause.
- HumanEval has no repeated names, so it was not affected.

**Fix (`l7b-engine-focus`).** `focused_document` keeps only the entry whose
friendly name equals the selected function. That function's name is already
unique, because selection requires exactly one match. Sail kept only that entry
anyway (`adapter.rs` retains `item.function == function`).

**Evidence.**
- The engine was built from source on the host with
  `cargo xtask build-verus-source --interactive-engine`. The build took
  158–187 s, and its ordinary build verified vstd.
- With the fix, every Vest repair is `batched: true`, has no selection error,
  produces focused evidence and uses one driver.

## 4. Vest Unknown, `SAIL3114 native-observer-setup-unavailable`

### Run-time cause (fixed)

The run-time coherent guard (`coherent.rs`, new since M6) walks every root of
the input snapshot. `cargo metadata` resolves every workspace member's
dependencies, and Vest's members pull in many registry crates (`wasm-bindgen`
and others) that `-p vest_lib` never builds. Instrumentation showed the guard
reach the 8,192-watch bound inside
`/opt/cargo/registry/src/…/wasm-bindgen-0.2.127` (705 roots).

After that was fixed, a second cause appeared. `build_inputs::stale` required an
observation for every metadata package with a build script, including scripts
never scheduled for the selection, so script coverage was always incomplete.

The rescue/m7 binary was Unknown even without the engine. M6 has no coherent
guard, leases or this script rule, so it was Current.

### Run-time fix (`c026b31`)

- **Guard scope.** The coherent guard covers the dependency closure (every
  resolved dependency kind) of the selected targets. It also covers the
  workspace `Cargo.toml`, `Cargo.lock` and every member manifest. The original
  guard did not observe these for a virtual workspace.
- **Script coverage.** Scripts outside an explicit `-p` selection's closure are
  treated as never scheduled, which extends the existing `cfg(any())` rule.
- **Unit check.** After every invocation, each `compiler-artifact` and
  `build-script-executed` record from Cargo must name a package inside both
  scopes. Otherwise the guard fails with `native-observer-scope-exceeded`
  (Unknown).
- **Fallback.** Without an exact selection or a complete, unambiguous resolve
  graph, nothing is narrowed.
- **Unchanged.** The whole-workspace revision and stamps are unchanged.
- **Tests:**
  - `closure_scope_drops_only_unselected_package_directories`
  - `coherent_scope_is_the_selected_closure_and_cargo_must_stay_inside`
  - `unselected_workspace_scripts_do_not_require_an_observation`

**Native result.**
- Fixed `sail verify --incremental --package vest_lib`: Current on the first
  run, which established script declarations with 2 drivers.
- The second run was Current with 1 driver and vstd reused, which matches M6.
- Before the fix, every run was Unknown with 2 drivers.

### Read-time cause (not fixed; needs a decision)

`repair-check` results go through the daemon's job projection
(`sail-daemon/src/jobs/store.rs`, `projected_currentness`). That calls
`freshness::inspect`, which:

- observes every package directory in `observation_plan`;
- requires a kernel read lease on every content path of the whole-workspace
  revision (`cacheable`).

On Vest that is far above the documented bounds (8,192 watches per Observer and
4,096 leased files; ADRs 0031 and 0035). So a run that established Current is
projected Unknown, with the note "current input validation is unavailable or
still running". That note is only added when the stored result was Current. All
five measured M7 Vest samples carry it.

Under the current design that Unknown is correct: the revision covers files no
lease can cover. Getting Current requires one of two changes:

1. **Scope the context revision and read-time plan to the selection closure**,
   as receipts already do (`target_revision`). An edit to a package outside the
   closure, such as `vest_bench`, would then no longer make a `vest_lib` result
   Historical. That changes what Current means and needs an ADR. This is my
   recommendation, and the run-time unit check above already supplies the
   safety condition.
2. **Raise the watch and lease bounds.** That is an ADR change to resource
   limits (ADRs 0031 and 0035), and it is costly for large workspaces.

M6 differs because it has none of these mechanisms. Its Current rests only on
the before/after content revision and stamps.

## 5. Harness `SAIL0940` on Vest

L3's check (exactly one library target and no binary target) accepts `vest_lib`
with its `cbor_rfc` test target. `sail-bench repair prepare` printed
`prepared …/vest`, and the warm-ups and samples ran.

## Smoke numbers (native, one round each, warm SMT cache)

- **M6:** `26f124f`. sail `d27cd8b7…`.
- **M7:** `8d5be94` (`l7b-engine-focus`, including fixes 1–4). sail `4a369644…`
  with the locally built engine variant `…-b108babd3bd211dd`.
- **Results:** `bench/results/he1.json` and `vest1.json`. They are not
  committed; they are in the evidence archive.

| Catalogue | M6 loop | M7 loop | M6/M7 | M7 drivers | Samples |
|---|---:|---:|---:|---|---|
| HumanEval | 83.6 s | 82.7 s | 1.01× | 1 | all 10 valid |
| Vest | 288.5 s | 374.0 s | 0.77× | 1 | M6 5 valid. M7 5 fail `SAIL7102`, 4 also `SAIL7106` (read-time Unknown, so not accepted) |

Per-cycle times and first actionable evidence:

- **HumanEval.**
  - M6 cycles took 14.5–19.3 s, with first actionable evidence at 6.5–7.7 s.
  - M7 cycles took 14.3–19.3 s, with first actionable evidence at 5.3–5.4 s.
- **Vest.**
  - M6 cycles took 49.7–60.1 s, with first actionable evidence at 19.7–25.1 s.
  - M7 cycles took 70.0–76.8 s, with first actionable evidence at 16.7–17.0 s.
  - M7's first repair warm-up per function took 127–131 s with 2 drivers.
  - The second took 72–77 s with 1 driver.
- **Gate.** Correctly failing for both catalogues (`SAIL7108` and `SAIL7109`).
- **SAIL7110 in practice.** One round never repeats focus content, so no
  `SAIL7110` could appear in either run. It is covered by the unit tests and by
  the native e2e reproduction.

Vest M7 is slower than M6 even with one driver. This is the preparation and
post-VIR overhead in L1's lever 1, which is L7a's lane.

## Gates

All gates ran natively on the same host, using `ci/local.sh`.

| Tree | Step | Result |
|---|---|---|
| `l7b-vest-and-baseline` `a12cf4f` | `gate` | pass: 723 tests passed, 41 skipped |
| | `solver` | pass |
| | `e2e` (legacy, managed and repair) | pass, including the repeated-focus check |
| | `conformance fast pr` | exit 1 |
| `rescue/m6-baseline` `26f124f` | `gate` | pass: 328 tests passed, 36 skipped |

The `conformance fast pr` failure needs explaining:

- **Stock arms.** Pass-through and cache-on matched on 33 of 33 units, three
  times.
- **Engine arm.** It failed on one unit, `examples/guide/traits.rs`: one oracle
  failure, with exit 0 from plain and exit 1 from Sail.
- **Cause.** `engine/verus/README.md` already documents this as the stock
  automatic-trigger instability (VI-0015). It fails "in most runs".
- **Not caused by this branch.** This branch leaves the engine and its patch
  unchanged.

The lead should decide whether this pre-existing engine-arm failure blocks the
merge.

A first `gate` attempt failed on `clippy::too_many_lines` in
`repair::judge::violations`. Commit `a12cf4f` fixed it by moving the M6 checks
into a helper. That aborted run's e2e step was killed and rerun from the
start.

## Evidence

Raw logs are kept outside the repository in `~/sail-raw-evidence/l7b/`.

`SHA256SUMS` covers every file; its own sha256 is
`2a82ab9fded51b9f7afa8bfa07f54e25ebd688ea187cf76880246eab01f7030c`.

| File | sha256 |
|---|---|
| `focus-repro.tgz`: managed acceptance with repeated focus, base vs fixed | `746de2f8c055f6557d76733148c08284bac247783e7b7cbfc918a16f8123821d` |
| `focus-base.log` | `30eb5e56b4443bc01c819807335abdfcad9a4349a8298c1bd600ea07d176ba0f` |
| `focus-fix.log` | `4a424fa5e2a0affd002187472921e4beaadfa2b485785361f807f5467d8d8328` |
| `vest-repro.tgz`: SAIL3122/SAIL3114 reproductions, instrumented runs, engine diagnostic | `8b0417b30070d3c466e2ad9e4133e1cff794e5faa00a4166f35bdbffc7edbee4` |
| `bench-he1.tgz` (raw run and compact result) | `0d5282c431081353d7e76d3d3aa283d09956c6e50c0206c0452296d540289a8d` |
| `bench-he1.log` | `9414361e7f98ef8ef76857eb8f511b8cb3c160390109c6fea7a877244cdeffff` |
| `bench-vest1.tgz` | `05229ef02a519601072c9d18748ad28c94d5f2391df0a76c012b18a4940fbd30` |
| `bench-vest1.log` | `cef08204489d856ab00480e2e28b1c20fbb80dbbc027dc07774bed73a0f26d7f` |
| `eng-engdiag-build.log` (diagnostic engine) | `c53e9dad64332b3c2e1caca901a5825905509950cbad849f4c2384f69a37e99b` |
| `eng-engfix-build.log` (fixed engine) | `ec751a15bdb8d592c6a8d613a06ed65139fd380e6f3ddbacde146cad1b11b12e` |
| `gate-l7b-gate.log` | `87c0f7b48d2b519a242b436a93aa699a878e9a3149005f923b7492f6baa81f81` |
| `gate-l7b-solver.log` | `97ce7a39506b156f923c7b19197c95e8c8aa93c9647644f897be17d57a0815ea` |
| `gate-l7b-e2e.log` | `2080693b45057aae5d1fe0b1f5c147c92eb5c1f2b8438a4ce7cdceaa4b575e0e` |
| `gate-l7b-conformance-fast-pr.log` | `5cd5e423c21418e10daa423a49885903a3e7476717f81badb40a6638eb8bc365` |
| `gate-m6base-gate.log` | `8546a75b6707cfaf7c62d930365e6cb1758b603341e4e6051776d1a9448a9281` |
| `image-build.log` | `74965022901339b4dc27fac41a2560a2a0c7c1e8b279d57b0fe9e12cb1329044` |

The instrumented binaries were built from temporary, uncommitted edits made
only on the host: `eprintln!` statements in `precise.rs` and `runner.rs`, and
one `l7b-focus-diag` event in the engine patch. The scripts that made those
edits are not part of the repository.

## Documentation changed

- `docs/bench/m7-repair-acceptance.md`: the M6 arm is now `rescue/m6-baseline`,
  with `SAIL7110` and the invalid-baseline rule.
- `docs/freshness.md`: selection-scoped guard and script coverage.
- This report.

## Documentation edits needed (not made, as instructed)

- **CHANGELOG.md**
  - *Fixed*: "A focused managed check (`sail verify --incremental -- --verify-function …`)
    on source identical to the last focused build no longer fails with `SAIL3107`.
    Sail now cleans `<target>/verus-partial`, where `cargo verus focus` builds."
  - *Fixed*: "On workspaces whose members resolve dependencies that the selected
    package never builds, such as Vest, managed runs are no longer always Unknown
    (`SAIL3114 native-observer-setup-unavailable`). Proof-time coverage now spans
    the selected targets' dependency closure. A unit outside it reports
    `native-observer-scope-exceeded`."
  - *Changed*: "`sail-bench repair` reports `SAIL7110` for an M6 check that ended
    in a Sail error. Such a sample is never counted as M6 time."
  - After the engine artifact lands: "The interactive engine's focused report
    no longer fails on repeated friendly names, such as trait impls, which
    refused every Vest repair with `SAIL3122`."
- **ADR 0036** ("Disabled metadata scripts"): an addendum for scripts outside an
  explicit selection's closure, and the post-run unit check.
- **ADR 0023/0029** (or a new ADR): the coherent guard's selection scope. Also a
  decision on the read-time revision scope (option 1 above).
- **docs/known-issues.md**
  - Vest read-time Unknown remains until that decision.
  - The engine fix awaits a new artifact.
  - `cargo test -p sail-verus --lib`, run in-process, is flaky on macOS for
    `compiler_family_lock_is_process_exclusive…` and
    `shared_compiler_target_has_exclusive…`. The failure is "would block". It
    was reproduced on the unchanged base `5fb311f` in 2 of 6 runs.
- **ROADMAP**: Vest M7 samples exist now, but they are not valid until the
  read-time decision is made.
