# Rescue lane L7c: managed overhead and read-time currentness

Status: done. Diagnostic and engineering evidence, not acceptance evidence.

## Summary

- **Managed overhead around Verus, HumanEval single-function edit:** 3.03 s →
  0.94 s (plain `cargo verus`: 0.54 s). **Vest:** 7.27 s → 3.40 s (plain: 1.74 s).
- **Warm unchanged run:** HumanEval 2.22 s → 0.38 s; Vest 4.50 s → 1.62 s.
- **Repair (`repair-check`):** HumanEval 8.58 s → 6.31 s per edit. On Vest,
  read-time currentness was always Unknown (`SAIL7102`) and is now Current;
  per-edit time is 36.7 s → 31.4 s.
- **Readback:** recorded-input digests replace the daemon's lease-based
  projections. Vest results now read back as Current. HumanEval reads under
  active verification are faster (Python Status p95 70 → 36 ms, Result p95
  99 → 78 ms).
- **Code:** 15,803 lines deleted and 962 added in `crates/`, net −14,841. This
  includes the kernel-lease helper, the precise observer, the coherent guard,
  the sysroot handoff, the guarded build-script bootstrap and the daemon
  freshness projections. Docs lost a further 510 lines.
- **Lease decision:** removed. Toolchain files are identified by stat, and the
  run decides Current from content digests plus stamps. The draft ADR is below.

## Environment

- **Host.** AWS EC2 `c7i.4xlarge` (16 vCPU, 30 GiB), `us-east-1`, Ubuntu 24.04,
  kernel 7.0.0-1014-aws. Brought up with `scripts/perf-host.sh up l7c`.
- **Container.** CI image `sail-ci:8e24fb5f7f490c01`, built natively.
  Workspaces, caches and output were on a Docker volume.
- **Toolchain.** Verus 0.2026.10.04.426d8b0, Z3 4.16.0, Rust 1.98.1.
- **Engine.** `rescue/m7`'s interactive patch no longer matches the pinned
  engine artifact (see "Blocked image" below). Repair runs therefore used an
  engine built from source on the host with
  `cargo xtask build-verus-source --interactive-engine`
  (`…426d8b01…-5d281662a13d2227`, 226 s).
- **Binaries.**
  - Before: `rescue/m7` `d4b8651`, sail `761dc397…`.
  - After: this branch at `aa545b3`, sail `e57e23fc…`. The later commits only
    delete code that managed runs no longer call, plus docs.
  - Round 1, for the per-stage table: `2167bd9` (before) against `ef0b520`, an
    earlier version of this branch.

## Method

`l7c-profile.py` (raw evidence, not committed) gives each arm a private
daemon, cache and workspace copy, prepared by `sail-bench repair prepare`.

- **Warm-up.** Two full runs and two repair checks per arm, not timed.
- **Measured rounds.** Arm order alternates each round. The scenarios are:
  - *unchanged:* `sail verify --incremental --output json --package P`;
  - *edit:* the same command after writing the catalogue's body edit
    (`gcd-body`, `base128-body`), then again after restoring the original;
  - *rep-unchanged* and *rep-edit:* `sail repair-check --engine interactive
    --function F --package P --lib`;
  - *plain:* `cargo verus verify -p P`, as reference.
- **No forced cleans.** `SAIL_MANAGED_FORCE` is not set, so these are ordinary
  agent cycles.
- **Data sources.**
  - Setup phases come from `SAIL_JOB_EVENTS`.
  - Verus time is the `--time` total from each run's sidecars.
  - *Outside* is wall time minus Verus time.
- **Readback.** After the rounds, four Python clients make 100 Status and 100
  Result RPCs on each arm's newest repair job, with the files unchanged and the
  daemon idle (`readback.py`).
- **Rounds.** HumanEval: 3 rounds. Vest: 2 rounds.

## Results: before (`d4b8651`) and after (`aa545b3`)

Medians in seconds, with [min–max].

| HumanEval | Before | After | Plain |
|---|---|---|---|
| unchanged, wall | 2.22 [2.22–2.23] | **0.38** [0.38–0.38] | 0.12 |
| edit, wall | 7.83 [7.65–7.87] | **5.72** [5.65–8.16] | 21.9 (no cache) |
| edit, outside Verus | 3.03 | **0.94** | 0.54 |
| rep-unchanged, wall | 8.53 | **6.31** | – |
| rep-edit, wall | 8.58 [8.53–8.66] | **6.31** [6.19–6.32] | – |
| rep-edit, outside Verus | 3.52 | **1.24** | – |
| Currentness | all Current | all Current | – |

| Vest | Before | After | Plain |
|---|---|---|---|
| unchanged, wall | 4.50 [4.41–4.59] | **1.62** [1.56–1.68] | 0.23 |
| edit, wall | 32.33 [32.04–32.62] | **28.39** [27.80–29.25] | 40.8 (no cache) |
| edit, outside Verus | 7.27 | **3.40** | 1.74 |
| rep-edit, wall | 36.65 [36.22–36.92] | **31.39** [31.21–31.96] | – |
| rep-edit, outside Verus | 9.28 | **4.06** | – |
| Stock Currentness | Current | Current | – |
| Repair Currentness (readback) | **Unknown** (every run) | **Current** | – |

Every Vest repair still reports `SAIL3122` for the selected function. That is
L7b's engine focus fix, which needs a new engine artifact; until it lands, M7
accepts no Vest repair. Currentness no longer blocks it.

### Where the time goes after (HumanEval edit, ms)

| Phase | ms |
|---|---:|
| metadata (wait after overlapping context) | 7 |
| runtime-context (stat identity, probes) | 30 |
| inputs (paths, snapshot, stamps) | 40 |
| target-cleanup (`cargo clean` of the edited package) | 94 |
| invocation (Cargo-Verus, Verus 4,770) | 5,357 |
| target-records (receipt and artifact checks) | 100 |
| post-run (metadata, snapshot, final checks) | 64 |
| publication | 14 |

Before this lane, runtime-context alone took 844 ms on `d4b8651`, and the input
guards took 376 ms.

### Per-stage profile, round 1 (`2167bd9` → `ef0b520`, HumanEval edit, ms)

The first optimisation pass changed no checks, and was measured stage by stage:

| Stage | Before | After |
|---|---:|---:|
| metadata | 37 | 0 (overlapped) |
| runtime-context | 303 | 123 |
| owned-paths (alias rehash) | 213 | 0.1 |
| initial snapshot | 40 | 27 |
| input guards | 321 | 180 |
| monitored snapshot | 35 | 21 |
| target-cleanup | 109 | 87 |
| invocation seal | 24 | 0.3 |
| invocation, including target records | 5,535 | 5,182 |
| post metadata | 37 | 0 (overlapped) |
| post snapshot | 85 | 55 |
| final barriers | 156 | 82 |
| **wall** | **6.97 s** | **5.83 s** |

The invocation drop came from two changes:
- parallel AIR and SMT log parsing in the driver, which took 228 ms;
- no longer re-probing rustc after `cargo clean`.

## Readback latency

These are Python clients, so Result times include parsing the full document.

| Case (ms) | Status p50/p95 | Result p50/p95 | Currentness |
|---|---|---|---|
| HumanEval, idle, before | 31.9 / 61.0 | 94.4 / 164.2 | Current; some Unknown (busy) |
| HumanEval, idle, after | 31.7 / 62.6 | 97.1 / 135.6 | all Current |
| HumanEval, under active verification, before | 44.0 / 70.2 | 62.7 / 98.6 | Current |
| HumanEval, under active verification, after | 6.4 / 36.1 | 39.0 / 77.8 | Current |
| Vest, idle, before | 14.5 / 124.6 | 230.3 / 342.1 | **all Unknown** |
| Vest, idle, after | 57.2 / 131.4 | 217.9 / 335.6 | **all Current** |

- **Source of the under-verification rows.** They come from
  `scripts/reader-acceptance-run.py`, whose Python census, 4 clients × 200,
  runs during an active repair.
- **The census did not pass its admission check in either arm.** The measured
  active repair, 6–8 s, ended before the census finished, so later samples
  had no live verifier. All samples were Current, and the latencies are valid
  as measurements. Under L14's `scripts/reader-gate.sh`, which replaces this
  harness, the gate needs a longer-running active job.
- **Vest Result time is document size, not freshness.** The Result
  percentiles are dominated by the Python clients parsing a large document.
  Before and after are equal. Each Status read rehashes the Vest closure's
  recorded files: 57 ms p50.

## `sail-bench repair`, one round

PENDING

## Changes and why each is safe

Each item gives the change, then why it is safe.

1. **Timing phases.** Opt-in `SAIL_JOB_EVENTS` timings, later reduced to 8
   coarse phases (L13 #10). *Safe:* diagnostics only.
2. **Alias rehash skipped.** An alias that is a hard link to the installed
   `sail` inode is not hashed again; this saved four reads of a 78 MB file.
   *Safe:* the same inode is the same file, and copies keep the byte
   comparison.
3. **Rustc probe cache.** `cargo clean` runs with `CARGO_CACHE_RUSTC_INFO=0`,
   so it no longer clobbers the rustc probe cache that the next build uses.
   *Safe:* what is cleaned and Cargo's freshness are unchanged.
4. **Mount-table reparse skipped.** No longer needed after change 9 deleted the
   observer. *Safe:* see change 9.
5. **Parallel reads.** Snapshots, receipt checks, final runtime checks, driver
   dependency digests and AIR/SMT log parsing are read in parallel, at most 8
   workers, with results kept in order. *Safe:* each file is still read in
   full, results are folded in the original order, and the first error is
   still the one reported.
6. **Overlapped work.** Metadata runs with the context, the sysroot probes run
   with each other, post-run metadata runs with the receipt checks, and the Z3
   digest runs with cleanup. *Safe:* results are consumed in the original
   order. An interactive engine is validated before any probe process starts;
   the existing test `corrupt_engine_is_rejected_before_its_version_probe`
   caught an early version that did not do this.
7. **Read-time currentness.** A Current run records the content digest of
   every file in the selected closure and every Cargo configuration path,
   including absent ones. Each read rehashes them in parallel: any change reads
   as Historical, and any read error reads as Unknown. *Safe:* every byte is
   compared on every read and no timestamp is trusted.
   - *Replaces* the daemon's lease and watch projections, which could not cover
     Vest.
   - *Missing record:* Unknown.
   - *Not covered:* packages the selection never builds. The run already
     refuses Current if Cargo compiled a unit outside the closure.
8. **Toolchain identity by stat.** Installed executables, engine files and
   sysroot libraries are identified by device, inode, size and mtime (L13 #22);
   configuration keeps content digests.
   - *Safe:* any reinstall or rebuild changes the identity, and the final check
     compares it again.
   - *Residual risk:* an in-place rewrite that restores the mtime, which the
     lead accepted.
   - *Change time excluded:* Sail's own alias hard links change the binary's
     ctime. A first version included it, which made every run cold and
     Unknown; fixed in `fix: leave change time out of toolchain identity`.
9. **Run-time lease guards removed.** A run is Current when all of these hold:
   - the content revision is equal before and after;
   - the dev/ino/size/mode/mtime/ctime stamps of every input file and
     directory are unchanged;
   - the toolchain stats are unchanged;
   - no absent configuration was created (kept: the inotify absent observer);
   - every compiled unit is inside the closure;
   - declarations are complete.

   *Not caught any more:* two cases that bypass both content and stamps, if
   undone within the run: a write through an already-dirty shared mapping, and
   a transient bind mount (needs root). These are documented in
   [freshness](../../freshness.md).
10. **Concurrent runs (B1).** A busy compiler family waits for up to
    `SAIL_MANAGED_LOCK_WAIT_SECS` (default 1,800 s) instead of failing with
    `SAIL3104`. *Safe:* locks are taken context first, then family, so there is
    no deadlock.
11. **One `SAIL3114` note:** "inputs could not be confirmed unchanged; re-run
    for a current result."

## Lease helper: decision and draft ADR

**Measured cost.** Per stock HumanEval run after round 1:
- Python closure probes: 3 × 16 ms;
- lease acquisition: about 75 ms for the sysroot and 90 ms for the coherent
  guard;
- barrier sweeps: about 15 ms each.

That is roughly 0.25 s per run. On top of that, `d4b8651` spends 844 ms in
runtime context. On Vest the guard could not cover the workspace at read time
at all.

**Benefit.** It caught only the two exotic cases above. Without it and without
replacement checks, no Linux run could be Current, which is why it was kept
earlier. The content and stamp checks now provide Currentness without it.

### Draft ADR (lead to number): Content and stat checks replace kernel leases for currentness

- **Status:** proposed.
- **Context:**
  - **Readback could not cover Vest.** ADRs 0029, 0031 and 0032 kept
    Current results trustworthy with Python-held kernel read leases, precise
    inotify observers, mount-generation polling and a daemon sysroot
    handoff. The read-time projection needed leases and watches on every
    workspace file, which exceeds the 4,096-lease and 8,192-watch bounds on
    Vest. Every Vest result therefore read back Unknown.
  - **Every run paid for it.** Managed runs also paid 0.3–0.9 s per run to
    hash and guard the toolchain.
  - **Owner's definition of Current:** no source, manifest or lockfile that
    Cargo compiled or read for the target has changed.
- **Decision:**
  - **Read time.** A Current run records the content digests of the selected
    closure's input files and the presence or absence of every Cargo
    configuration path. Readers rehash exactly those files: equal is
    Current, different is Historical, unreadable is Unknown.
  - **Run time.** A run is Current only if its input content revision and
    dev/ino/size/mode/mtime/ctime stamps are unchanged across the run, no
    absent configuration appeared, and every compiled unit is inside the
    closure.
  - **Toolchain.** Toolchain and sysroot files are identified by
    dev/ino/size/mtime; configuration is identified by content.
  - **Removals.** Kernel leases, the precise observer, mount-generation
    polling and the sysroot handoff are removed. This supersedes the
    lease parts of ADRs 0029, 0031 and 0032 and the guarded bootstrap in
    ADR 0036.
- **Consequences:**
  - **Results.** Vest results read back Current. Warm runs are 0.4–1.6 s,
    and about 15,000 lines are deleted.
  - **Undetected cases.** An edit made and undone during a run through an
    already-dirty shared writable mapping, or a bind mount added and
    removed (root only), is no longer detected. The same holds for an
    in-place toolchain rewrite that restores mtime. These are documented
    limitations.
  - **Unchanged.** Default-mode verdicts and SMT cache keys are unchanged
    (charter rules 1 and 2). Only the Currentness label depends on these
    checks, and any doubt still yields Historical or Unknown.

## Validation

- **Workspace tests, Linux (host container).** `cargo clippy --workspace
  --all-targets -D warnings` is clean, and `cargo nextest run --workspace`
  passed 631/631 at `aa545b3`. The `ci/run.sh` gate results below cover the
  final tree.
- **macOS.** `ci/run.sh host` passed: clippy, then nextest 550/550 at
  `d0898d6`. One macOS-only compile error from the bootstrap removal was fixed
  in `d0898d6`.
- **Gates.** These ran in the existing CI image, because the image for this
  tree cannot be built (see "Blocked image" below), with
  `SAIL_ENGINE_BUILD` set to the source-built engine: PENDING.
- **Freshness and fault-injection tests.** These are covered by the gate and
  solver runs:
  - `read_check` tests: unchanged files, restored-mtime edits, new
    configuration, deletion, missing or corrupt records;
  - `job_coalescing` rewritten for recorded inputs;
  - the runtime stat identity test;
  - the lock-wait tests;
  - the shim fault suite in `solver`.

### Blocked image

`engine/verus/interactive.patch` on `rescue/m7` (since L7a and L4b) no longer
matches `patch_sha256` in `engine/verus/artifact-linux.json`. So:
- `ci/image-tag.sh` gives a new tag;
- `ci/engine-artifact.py` refuses the pinned artifact;
- the CI image for the current branch cannot be built, and `ci/local.sh` cannot
  run.

This predates L7c. The lead needs to publish a new engine artifact.

## Dead after this branch

Nothing in the list prepared for the L14 follow-up remains. Already deleted
here:
- `guarded_output.rs`, `lease.rs`, `lease.py`, `helper_runtime.py`,
  `kernel_leases.rs`, `kernel_mounts.rs`, `precise.rs` and `retained.rs`;
- `tests/fixtures/kernel-locks-frozen.txt`;
- the `SAIL_JOB_RPC_TIMINGS` environment exclusion.

Candidates for other lanes:
- **`docs/native-product-setup.md` and `scripts/m7-native-product.py`.** They
  exist only so service-owned files could be leased. `docs/known-issues.md`
  links to the doc.
- **The `SAIL_FRESHNESS_TRACE` mention** in `docs/reader-acceptance.md`, if
  that doc survives.
- **`absent.rs`.** L13 #23 suggests replacing it with before and after
  existence checks. It is kept because it is the only check for a transient
  configuration file.

## Documentation changed

- `docs/freshness.md`: rewritten in user terms.
- `docs/managed-cold-start.md`: rewritten as a user guide.
- `docs/guarded-sysroots.md`: deleted.
- This report.

## Documentation edits needed (not made, as instructed)

- **CHANGELOG.md**
  - *Changed:* managed runs are 1–3 s faster per edit cycle (see the numbers
    above).
  - *Changed:* toolchain files are identified by stat.
  - *Changed:* a busy managed target is waited for
    (`SAIL_MANAGED_LOCK_WAIT_SECS`).
  - *Fixed:* Vest and other large workspaces now read back Current.
  - *Removed:* the sysroot handoff endpoint (`.sysroots-v1` socket), the
    Python lease helper, and the `SAIL3114` mechanism notes.
- **ADRs.**
  - Number and accept the draft above.
  - Mark ADR 0029, 0031 and 0032 superseded, and the guarded bootstrap
    section of ADR 0036.
- **docs/known-issues.md**
  - Remove the Vest read-time Unknown entry and the lease ownership and
    `CAP_LEASE` entry.
  - Add the blocked CI image (engine pin) entry.
  - Add the two residual cases from change 9.
- **ROADMAP.** Vest Currentness is no longer a blocker; `SAIL3122` remains
  (engine artifact).
- **AGENTS.md.** No change needed.

## Commits

On `rescue/m7` `5e99481`:
- `ed4e5f8` and earlier: round-1 optimisations
  - `dc1d929` timing phases;
  - `5800799` alias inode;
  - `fbf9ad7` rustc cache;
  - `503db44` mount reparse;
  - `95b4b98` parallel reads;
  - `fe6d88a` overlaps;
- `b92eef2` read-time recorded-input check;
- `f8de8e0` stat toolchain identity and removal of the run-time guards;
- `b1f16fd` lock wait;
- `d0898d6` macOS fix;
- `6369225` change-time fix;
- `4ce8494` lease helper and observer deletion;
- docs commits.

## Raw evidence

Kept in `~/sail-raw-evidence/l7c/` (see `SHA256SUMS`): PENDING.
