# Original repair benchmark freeze

The source candidate has passed the complete original 13-stage qualification,
all 150 complete-source controls and both strict checks. Its unchanged runtime
has passed local packaging and fresh installation. Product repair speed and
reader responsiveness remain unmeasured by those results.

This bundle prepares the existing `scripts/m7-repair-bench.py` workload.
It changes neither that runner nor `corpus/repair-edits.json`. The catalogue
contains HumanEval and Vest body, specification, refactor, failing and repair
cycles. Each project must complete three rounds and independently reach at
least 2x median improvement over the frozen M6 binary. All paired full verdicts
and diagnostics must match. M6 time includes both focused and full invocations;
M7 time includes the complete focused/full repair batch. Warm-up and installation
remain separately recorded under the original runner's rules.

The original `prepare` command clones pinned committed source objects into new
native Linux scratch projects, without hardlinks, worktree edits or proof
receipts. Both modes use the same paths, exact edit versions and dependencies.
The original `run` command owns private caches and daemons. It preserves genuine
failure-to-repair content, alternating mode order, the original 600-second
invocation limit, Current full evidence, one M7 compiler driver and enabled
preparation telemetry at or below 512 MiB. It alone determines benchmark
acceptance; incomplete samples or setup failures do not pass.

`run-original.py` adds input identity and lifecycle evidence around only those
two original commands. It checks the qualified c77 runtime's 13 exact artifacts,
modes, source files and lock before and after execution; it records product and
baseline binary identities, phase actors, raw logs and the primary result before
independent seal closure. An interruption first signals the original runner so
its existing owned CLI/job/daemon cleanup can run. A bounded fallback records
cleanup as unavailable rather than claiming actor closure. The final launch
must also retain an independent process census after the runner exits.

The M7 product commit and both product/shim SHA-256 values are deliberately
pending the root's rebuilt-product grant. The launcher checks the explicitly
granted bytes; it does not infer binary provenance from an unrelated repository
HEAD. Execution has not started. There are no inherited `SAIL_*` selectors, no
trace selectors and no source-engine rebuild in this benchmark scope. The
existing runner installs the qualified runtime into its own fresh M7 cache.

The exact commands are retained in the launcher: original `prepare`, followed
by original `run --rounds 3 --timeout 600` with the actual emulated Linux/shared
load environment label. Native scratch, caches, output and binaries are required;
Docker host bind mounts cannot establish kernel freshness coverage.

Validation of this freeze: Python compilation and all seven original acceptance
oracle controls plus four original lifecycle controls pass. These tests establish
harness behavior, not HumanEval/Vest verification or speed. The separate 200 ms
p95 established-reader goal remains required after this repair workload.

See `docs/bench/m7-repair-acceptance.md`, `qualification-result.json` and
`packaging/result.json` in the parent bundle for their distinct result scopes.
