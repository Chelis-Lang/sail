# Evaluating general performance

The local evaluation covers complete invocations and how their work grows.
The four-project release matrix remains a regression baseline; it does not
stand in for the broader corpus. The [algorithm review](design/general-performance.md)
records the implementation boundaries and the [qualification record](validation/general-performance.md)
distinguishes implementation from accepted results.

Run builds, gates and timings on native EC2 hosts as required by
[AGENTS.md](../AGENTS.md). Keep builds and correctness jobs separate from timed
work. macOS qualification uses a separately approved Mac session. These are
local checks, not additional long-running CI jobs.

## Workloads and compatibility intake

`corpus/performance-suite.json` selects 18 workloads: IronKV, Vest, Anvil,
CapybaraKV, multilog, pmemlog, VeriSMo, generated Vest, node replication,
page tables, HumanEval, vstd, Verus-TLA, three semi-persistent packages, the
TLSF allocator and hekate-math. The entries record structural coverage rather
than assigning special optimization behavior to a repository name.

```sh
python3 scripts/perf-corpus.py list
python3 scripts/perf-corpus.py run --phase intake \
  --project verus-tla --project semi-containers --project hekate-math \
  --out "$HOME/sail-perf/general-intake"
```

The runner needs `sail-conformance`, `sail-bench`, `sail`, `sail-z3`, stock
Cargo-Verus and the pinned Rust toolchain on `PATH`. Set `SAIL_VERUS_HOME` when
stock is installed outside the normal cache. Each output directory must be
new and outside the checkout. It retains both manifests, the suite selection,
every attempted command, logs and the original Rust conformance report.

The four new repositories are pinned in
`corpus/performance-candidates.toml`, independently of the ordinary CI corpus.
Their initial `unqualified` status is intentional. Intake compares stock
against stock with the bare shim, using the existing comparator. A matching
failed stock run is recorded as conformance evidence and **does not** establish
a passing project baseline. Missing requirements, unsupported inputs,
instability, different output and infrastructure failures remain visible.
Scratch preparation discovers Cargo's complete workspace membership before
resolution and updates Verus pins in every member, including non-selected
packages. It respects Cargo's globs/exclusions and refuses paths escaping
scratch; it does not modify proof source or the fetched checkout.
Do not port proof source, change a runtime or omit failing units to get a
positive intake result. The TLSF project's own script supplies Singular; an
ordinary-runtime attempt does not qualify that script's complete workflow.

Semi-persistent and hekate-math are held out from optimization tuning. Intake
may establish compatibility; timing them requires a frozen candidate. At least
two independently maintained new repositories need usable baselines before
calling the expansion qualified. Incompatible candidates do not count.

## Paired timing

Set `SAIL_PERF_VERUS_BUILD` to the validated engine build directory in the
candidate release, not to its `verus` executable. The runner supplies that
runtime to the Sail arm; stock remains the oracle.

```sh
python3 scripts/perf-corpus.py run --phase screen \
  --project multilog --project pmemlog --project node-replication \
  --machine 'c7i.8xlarge, us-east-1' \
  --sail-verus-build "$SAIL_PERF_VERUS_BUILD" \
  --out "$HOME/sail-perf/general-screen"
python3 scripts/perf-corpus.py run --phase accept \
  --project semi-containers --project hekate-math \
  --machine 'c7i.8xlarge, us-east-1' \
  --frozen-candidate "$(git rev-parse HEAD)" \
  --sail-verus-build "$SAIL_PERF_VERUS_BUILD" \
  --out "$HOME/sail-perf/general-held-out"
```

`screen` records two rounds; `accept` records six. The latter names a capture
recipe, **not release acceptance**. A frozen candidate must equal the full
clean checkout HEAD. The runner checks completeness and retains refusals. It records source state
and input digests, then refuses a capture if the source state, executable,
manifest or runner changes during the run. Git warnings or failed source
inventory also refuse capture. An exit of zero says the requested
comparisons and captures completed.

Managed workloads use `perf-real.sh`: cold, first and second unchanged calls,
an unchanged call with replay disabled and a comment edit. Direct units use
`sail-bench cold` and `warm`, each with the existing per-run comparator and a
validated fork on the Sail side. Direct warm results measure query-cache
reuse, not managed receipts or unchanged-call replay.

Managed timings explicitly select the pinned Cargo/Rust toolchain for both
arms, except Anvil's existing project-toolchain profile. The index records
that choice. Install required Rust targets before cross-target cases and run
them after ordinary profiles: an extra target changes the complete sysroot
identity and can invalidate a shipped vstd profile. Keep those environments
and their cold results separate.

The existing targeted body/failure/fix/revert recipes remain available in
`perf-real.sh`. The new managed roots currently have no qualified targeted
edit catalogue; their comment-only captures do not establish real-edit,
specification-edit or failure/fix acceptance. Feature variants likewise need
explicit same-argument stock controls. Keep those coverage gaps visible.

Timing runs disable diagnostic tracing and the RSS observer. Memory and
observer-overhead qualification remain separate required gates; disabling an
observer does not qualify its overhead or replace release memory measurements.
Report each workload's samples, median and worst regressions. An aggregate
speedup cannot hide a slower workload. Compare the previous Sail build as well
as stock before selecting an optimization.

The underlying CLIs accept explicit manifests without changing their defaults:

```sh
sail-conformance run --corpus corpus/performance-candidates.toml \
  --tier deep --project verus-tla --out "$HOME/sail-perf/tla-conformance"
sail-bench warm --corpus corpus/performance-candidates.toml \
  --tier deep --project hekate-math --rounds 2 \
  --sail-verus-build "$SAIL_PERF_VERUS_BUILD" \
  --out "$HOME/sail-perf/hekate-warm.json"
```

Without `--sail-verus-build`, `sail-bench` preserves its historical
stock-with-shim diagnostic control. New result metadata explicitly labels that
control or records the selected fork manifest digest. `--out` applies to cold,
warm, edit, agent-loop and history-replay results; its default remains
`bench/results`. Fragility retains its existing output and runtime interface.

## Real proof edits

The additional local catalogues edit original functions at their pinned source:
`corpus/performance-edits.toml` covers a bit-vector proof body, contract,
false assertion and an imported specification body in hekate-math.
`corpus/performance-loops.toml` changes a Verus-TLA proof body and contract,
introduces a false assertion, fixes it and refines the fixed body. Expected
outcomes remain hypotheses until the actual stock run confirms them.

```sh
sail-bench edit --corpus corpus/performance-candidates.toml \
  --edits corpus/performance-edits.toml --rounds 1 \
  --sail-verus-build "$SAIL_PERF_VERUS_BUILD" \
  --out "$HOME/sail-perf/proof-edits.json"
sail-bench agent-loop --corpus corpus/performance-candidates.toml \
  --loops corpus/performance-loops.toml --rounds 1 \
  --sail-verus-build "$SAIL_PERF_VERUS_BUILD" \
  --out "$HOME/sail-perf/proof-loop.json"
```

Run six independently retained one-round captures. The existing result schema
stores the last observed outcome, so one-round captures allow checking every
stock outcome against its expectation. Require four edits with two `same`
comparisons each, and six loop states with one `same` comparison each; require
zero differences, instability or infrastructure errors and complete samples.
The initial loop state must verify. The benchmark command alone does not
reject a wrong expected outcome: audit the result fields before claiming a
passing edit qualification. These runs use the selected fork and exact query
cache; they do not establish managed receipt or replay performance for edits.
Cargo projects use the existing cumulative loop harness; the single-edit
harness remains limited to direct Verus units.

## Scaling and work accounting

`--default-all-members` makes every generated package a default Cargo target;
without it, the aggregate root remains the sole default member and reaches
all packages through dependencies. This lets the same generated graph cover
multiple requested packages as well as dependency-only verification.

`perf-scaling.py` generates reproducible verified Cargo workspaces. Vary one
dimension at a time: packages, files, shared dependencies, root binary targets
or bytes per file. Chains, repeated diamonds and shared dependencies expose
different traversal costs. Every root binary imports the Verus prelude and
checks a call into its generated root library. Supply the pinned vstd source
explicitly. A generation/control-test pass does not establish that the
resulting programs compile: run the complete workspace/all-targets comparison
and require a passing plain baseline before recording scaling performance.

```sh
python3 scripts/perf-scaling.py --shape diamond --packages 16 --files 16 \
  --targets 4 --vstd "$SAIL_PINNED_VSTD" \
  --out "$HOME/sail-perf/scaling-diamond-16"
python3 scripts/perf-work.py --out "$HOME/sail-perf/scaling-work" -- \
  sail verify --manifest-path "$HOME/sail-perf/scaling-diamond-16/Cargo.toml"
```

The work counter uses native Linux `strace` summary mode. It records syscall
counts and errors without capturing arguments or source contents. Tracing
changes timing: `work.json` is diagnostic only, never a benchmark sample.
Pair it with the existing `SAIL_JOB_EVENTS` setup phase timings when diagnosing
a stage, then rerun without either observer for performance qualification.

The initial index/JSON component diagnostics belong to the unselected
`32f7bba6` experiment, not to the current runtime or CI gate. Their recorded
component results did not predict a material whole-invocation improvement;
see the [qualification record](validation/general-performance.md).

Generated programs and component timings establish a bounded diagnostic
scope, not real-project speedups. Keep raw logs outside git and cite their
hashes in the qualification record.

## Diagnostic order in benchmark identity

Benchmark comparisons use ADR 0042's existing complete-message rule. Build
its Rust adapter before using the release matrix:

```sh
cargo build --release -p sail-conformance --example check_diagnostics
```

The default adapter is `target/release/examples/check_diagnostics`;
`perf-matrix.py run --diagnostics-helper PATH` selects another built copy.
Complete spanned diagnostic bytes and counts are compared as a multiset;
normalized summaries and remaining output stay ordered. Missing, duplicated
or changed message bodies still fail. Adapter refusal keeps the sample and
marks it failed. Legacy captures retain their original strict comparison
unless their retained raw logs are separately reprocessed with this adapter;
no message data is inferred from headlines alone.
