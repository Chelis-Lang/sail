# Testing Sail

The complete gate and testing tiers are defined in [AGENTS.md](../AGENTS.md).
Run Linux checks on a native EC2 host with `ci/local.sh`; use `ci/run.sh host`
for the macOS platform check. Performance measurements require a quiet native
host, separate from builds and gates.

Local containers use a Cargo target volume per physical checkout (#475).
Distinct worktrees cannot reuse one another's binaries merely because both are
mounted at `/work`; normal edits and symlink aliases retain the same checkout's
warm target. `ci/local.sh target-volume` prints its volume; the optional
`ci/local-singular.sh target-volume` prints its separate Singular volume.
Shared non-target cache volumes are unchanged. Custom Docker qualification
runners must also isolate prepared snapshots or rebuild the changed packages;
a clean Git checkout does not establish which binary Cargo executed.

Real-project benchmarks keep their timings and logs and delete each completed
arm's temporary source, build artifacts and cache before starting the next arm.

`scripts/perf-real.sh` wraps each measured command with `python3 -I
scripts/perf-mem.py`. Each `rN/MODE.PHASE.memory.json` records process-tree
peak RSS, largest accounted child high-water RSS and separate daemon peak RSS,
in KiB. Tree and daemon peaks sample every 50 ms; shared pages are counted per
process and shorter peaks may be missed. The sampler's own process and its
`ps` children are excluded. Observed detached daemons remain tracked by PID
and start identity across phases; the cache daemon reported by `sail status`
is also seeded before each run. A daemon that detaches before its first
observation is discovered from fresh status at completion and sampled again
outside the wall interval. This can still miss an earlier short peak;
Linux's final per-process `VmHWM` supplies a separate lifetime cross-check.
At arm end, `MODE.daemon-memory.json` retains
that cross-check before the harness stops the daemon. Cache byte usage from
`sail status` is not RSS.

The last three `results.tsv` columns are `tree_peak_kb`, `largest_kb` and
`daemon_peak_kb`. The report accepts both original 19-column rows and new
22-column rows; old memory is `NA`. Malformed row widths refuse reporting
instead of silently losing samples. `SAIL_PERF_MEMORY=0` disables sampling
for paired overhead controls, leaving memory unavailable. The wrapper changes
no child arguments or environment variables. Use the same setting throughout
an arm, and never treat disabled or failed sampling as memory acceptance.
The wrapper signals readiness before the wall clock starts and waits on an
owned FIFO; the harness then releases the actual command. Python startup and
the initial process inventory are outside the timed interval. Disabled
sampling uses the same readiness path, with no process scans or sampler
thread, so the overhead control retains the same command-launch boundary.
Native sampler overhead and the four-project Linux/macOS memory matrix must
be measured before claiming memory acceptance for a release.

Both native host helpers support primary Git checkouts and linked worktrees:

```sh
scripts/perf-host.sh sync LANE
scripts/perf-host.sh run LANE git status --short
```

For an already approved Mac host, use `scripts/perf-host-mac.sh sync LANE`;
its `ec2-user` transport and host allocation policy are unchanged.

Sync freezes the actual tracked/untracked working files, committed HEAD and
staged entries, then installs standalone Git metadata on the host. Dirty inputs
stay dirty; staged-new files modified again retain both versions. It copies no
local Git config, hooks, linked-worktree pointers or index optimization flags.
Content checks and current timestamps remain in force; native Cargo outputs
remain on the host. The existing target, agent, bytecode, conformance and release
output exclusions still apply.
The release factory's `release-out-proof-evidence/` is also protected from
ordinary source transfer and deletion, including container-root-owned records.
Tracked source under that exclusion still transfers through the same explicit
tracked-file rules as other output directories.

This is not a clean-source certificate: proof
gates must retain their normal clean-source and canonical-byte checks.

HEAD/index-tracked paths override those exclusions: tracked project settings
under `.claude` still transfer, including dirty contents and staged additions.
Tracked deletions stay deleted; files are not restored from committed blobs.
Untracked private siblings and agent worktrees remain excluded. A host-side
`sail-sync-owned-paths.json` ledger records prior tracked names so committed
branch deletions also retire previously transferred files inside exclusions.
It is replaced atomically under the sync lock and retains the union of owned
names after failure, allowing a retry to remove stale source without deleting
unknown private siblings. Initial migration reads only standalone Git metadata,
never a linked-worktree pointer or redirected/shared Git metadata. A legacy
pointer or nonempty checkout without known ownership refuses with `SAIL9003`;
use a fresh lane, or explicitly clean the destination under owner control while
preserving any needed build/private files. This is not an in-place migration of
unknown legacy source. Malformed ledger state refuses before source
mutation; do not discard the ledger while reusing a native checkout.

Sparse/skip-worktree, intent-to-add and submodule index entries refuse before
remote mutation. Stage or unstage intent entries and use a full ordinary
worktree. Source, index or HEAD changes during sync also refuse. A failed
transfer leaves no admitted Git metadata; an interrupted connection can retain
the owned `sail-sync.lock` on the host. Check that no sync is running before
removing that lock and retrying. Full local Git merge-operation state is not
exported; unmerged index stages and actual conflict files are preserved.

`python3 scripts/test-perf-host-sync.py` exercises real Git and rsync with local
SSH transport. It is included in the workspace gate. See the
[source-sync record](validation/host-source-sync.md) for qualification scope.

The e2e shell scripts need no GNU tools on macOS: fixture edits use portable
`sed`, and bounded steps use coreutils' `timeout` where it is installed or
`ci/timeout.py` (the same options and exit statuses, checked against GNU by
`ci/test-timeout.py`) where it is not.

Fixture waits must tolerate slow process starts. macOS assesses a freshly
written executable (a mock worker script, a copied binary) on its first exec,
and during a parallel `cargo nextest run --workspace` on a loaded Mac that has
taken over ten seconds, while a known executable or `sh SCRIPT` starts in
milliseconds. A wait that only guards against a hang therefore uses a bound of
minutes, and a bound that is itself under test must not include the start of a
new executable. A test that needs another package's binary supplies a stand-in
(Cargo builds `sail-bench`, for example, only when that package is selected)
and leaves checks of the real binary to that package's tests.

## Release performance matrix

`scripts/perf-matrix.py run` measures passing verification workflows. Both arms
must exit zero and retain the existing strict identity checks. A matching pair
of failed verifications keeps its observed exit codes and `same` identity,
but its performance row is invalid; failures are never dropped.

The matrix runs Vest with `--rlimit 100` on both stock and Sail, including
the untimed baseline and timed new checkout in the worktree scenario. An
unsuccessful baseline invalidates that scenario. Its default
limit reproduces VI-0032 and cannot qualify a passing-workload speed claim.
The ordinary conformance corpus retains that default-limit regression. The
matrix header records additional project flags in `extra_verus_args_by_project`.

## End-to-end phases

CI builds the release binaries once, then runs ten independent jobs. Each
job retains the existing ten-minute hard limit. The required `End to end`
aggregate accepts only ten successful results: failure, cancellation and a
skipped job all fail the aggregate.

| `ci/local.sh e2e PHASE` | Scope | Image |
|---|---|---|
| `legacy` | New projects, cold/warm output parity, JSON, MCP and portfolio | core |
| `output` | Edit-loop output, compile errors, partial verification, a seeded-mutation smoke, then (same warm targets) unchanged-call replay against stock and misses for edits, restored mtimes, touches, files, binaries, declared variables, dep-info inputs, the lockfile, fingerprints, JSON and partial scope | core |
| `flags` | Cargo/Verus invocation flags | core |
| `loader` | Loader-variable fallback | core |
| `managed` | Failing-first vstd reuse, complete receipts, jobs/MCP, missing evidence, edits, source-alias retargeting, prune and Cargo metadata | mcp |
| `managed-copies` | Own passing donor, two fresh workspace-member proofs, Current adoption and each project's own receipt reuse | core |
| `managed-native` | Own passing donor, fresh plain summaries, unchanged silence and same-PATH compiler replacement against Sail and stock | core |
| `contracts` | Installed source-only Sail service, proof/unknown retention, compiled runtime witness, restart and baselines after job pruning | core |
| `scripts` | Generated includes, build scripts and declared environment inputs | core |
| `repair` | Repair CLI/MCP against the pinned prebuilt engine | core |

The two fresh-project phases each own their workspace, target directories,
daemon, private compiler wrapper and passing donor. They share no receipts or
compiled artifacts with other jobs. Their requested-target and vstd verdict,
cardinality, Current and publication-refusal assertions remain required.
Only `managed` invokes the real external Claude Code tool-listing check;
the ordinary MCP checks remain there too.

Every stock oracle runs `RUSTUP_TOOLCHAIN=<effective toolchain> cargo verus
verify`, the toolchain Sail selects for its own Cargo
([ADR 0058](decisions/0058-managed-builds-use-the-pinned-cargo.md)); Sail runs
without the variable, so its pinned selection is exercised. `ci/stock-toolchain.py`
prints it (the caller's `RUSTUP_TOOLCHAIN`, else the engine's pin) and fails
when the pin lacks `rust-src`, which the CI image installs.

`ci/local.sh e2e` runs the complete union, including both extracted fresh-project
groups once using the already established managed donor, then the local-only
`extra` flag/workspace cases. Validate standalone `managed-copies` and
`managed-native` as well: their independent donor setup is an additional entry
path. No phase or assertion can be omitted to authorize a merge.

## Output failure evidence

`ci/test-output.sh` retains selected failure observations outside its temporary
fixture. Set `SAIL_E2E_OUTPUT_OUT` to choose the destination; the default is
`target/e2e-output-evidence`. The index records the original test exit,
executable identity, retained byte hashes, refusals and incomplete captures.

Selection prioritizes the last command's streams and metadata, then newer
producer records, current source, other streams and older command snapshots.
At most 1,024 files, 8 MiB per file and 64 MiB total are retained, under the
existing 60-second collector deadline. Discovery retains only a bounded heap
of 1,025 candidate paths, including an overflow witness. Hitting a limit still
marks the capture incomplete; collector failure preserves the test's exit.
Modification times prioritize diagnostics only: they do not associate a
producer with a command or reconstruct stdout/stderr order.

`python3 ci/test-output-evidence.py` checks the real shell trap with controlled
children and filesystem overflow/refusal fixtures. It does not run Verus.
The [validation record](validation/e2e-output-evidence.md#priority-selection)
distinguishes these checks from real failed-run qualification.

## Managed timeout evidence

The managed fixture writes an operation record and a flushed progress line
before each bounded verification starts, then records its exit and elapsed
time. Verifications retain their 240-second child limit; the independent failing
stock oracle retains 300 seconds. These limits are unchanged by sharding.
Partial producer output is drained after owned process groups are killed.

Managed jobs request bounded fixture evidence on failure or cancellation.
GitHub's hard job timeout can bypass that upload step, so a missing artifact
does not locate the blocked command or establish a product failure. The original
containment cancellation and the revised layout's completed native scope are in
the [containment record](validation/dependency-store-fail-closed.md).

## Mutation suite

Both stock and managed commands explicitly use the pinned runtime's Rust
Cargo toolchain, including projects with a checked-in nightly override.
The run header records `cargo_toolchain`; attaching it to a release matrix
requires the same pin. This is a controlled comparison, not a claim that Sail
ignores a user's project toolchain in ordinary use.

The seeded mutation suite ([ADR 0063](decisions/0063-seeded-mutation-suite.md))
is both a soundness probe and the release benchmark's edit loop. Catalogues
live in `corpus/mutants/` (`<name>.toml` in the `corpus/edits.toml` format,
plus a `<name>.meta.json` sidecar); results are in the
[validation record](validation/mutation-suite.md).

| Catalogue | Unit | Generated with |
|---|---|---|
| `human-eval` | `human-eval-verus/tasks/lib.rs` (wrapped as a Cargo library) | `--project human-eval-verus --name human-eval --per-class 6` |
| `ironkv` | `verified-ironkv/ironsht` | `--project verified-ironkv --name ironkv --per-class 6` |
| `vest` | `vest/vest_lib` | `--project vest --name vest --per-class 5` |
| `capybarakv` | `verified-storage/capybaraKV/capybarakv` | `--project verified-storage --unit verified-storage/capybaraKV/capybarakv --name capybarakv --per-class 5` |

All use `--seed 20261020`. To make or extend a catalogue:

```sh
sail-conformance fetch --tier standard --project vest       # the pinned source
cargo xtask gen-mutants --project vest --name vest --seed 20261020 --per-class 5
# On native Linux x86-64 (the reference platform), with stock Verus installed
# (`cargo xtask fetch-verus`): three stock runs per mutant settle `expected`
# and drop mutants that vary, never compile or do not finish.
cargo build --release -p sail-cli -p sail-shim -p sail-bench
./target/release/sail-bench mutants oracle --catalogue corpus/mutants/vest.toml --jobs 4
```

Generation reads only the source text and is byte-identical for the same
seed, generator version and pin on macOS and Linux; the sidecar records the
generated catalogue's sha256. The oracle rewrites the catalogue in place,
keeping only settled mutants, and records every run in the sidecar.

To run a catalogue (Sail's Verus from `SAIL_VERUS_BUILD`, or the CI image's
`SAIL_ENGINE_BUILD`; otherwise Sail's own lookup):

```sh
./target/release/sail-bench mutants run --catalogue corpus/mutants/vest.toml \
  --vstd-lock scripts/shipped-deps/Cargo.lock --rounds 3 [--per-class N] [--filter SUBSTR] --out ~/mut/vest --machine "EC2 c7i.8xlarge, us-east-1"
./target/release/sail-bench mutants report ~/mut/vest      # the tables again
```

Per round it keeps one working copy and one Sail cache, so one daemon serves
every mutant; stock `cargo verus verify` and `sail verify` run in the same
tree with separate target directories, alternating order. After the last
mutant the original is restored and verified ordinarily and with
`SAIL_MANAGED_FORCE=1`; Sail must match stock and its own cold result. The
command exits non-zero on any difference, false verified (a function stock
rejects that Sail reports verified), failed revert check or step that could
not run, and copies those steps' logs to `<out>/failing/`. Every sample is in
`<out>/samples.jsonl`; `report.md` has per-class medians, total and geomean
speedups with 95% bootstrap intervals, the cache hit share of solver lookups and the share of Sail runs with a
newly published retained-proof receipt.

Tiers: the e2e `output` phase (CI) ends with a smoke on a tiny local crate,
`ci/fixtures/mutation-smoke/lib.rs`, with four mutants of its catalogue
`smoke.toml` (`cargo xtask gen-mutants --file ci/fixtures/mutation-smoke/lib.rs
--name smoke --seed 1 --per-class 1`; `expected` settled from stock runs and
checked on every run). It runs the original, an unchanged re-run, each mutant
followed by a revert and a forced re-run, comparing each step's exit code,
stdout and stderr with stock's as the rest of `ci/test-output.sh` does and
each step's per-function verdicts (`check_output`); reverts and the forced
run must get the original's answers. `ci/local.sh conformance standard` ends
with two mutants per class of `human-eval` for one round;
`ci/local.sh conformance deep` runs every catalogue for one round. Both are
local-only. Timings come only from a native
host (`ci/run.sh bench mutants NAME ROUNDS`, three rounds for a release).

For release-product mutation measurements, pass `--vstd-lock release-out/deps/<bundle>/Cargo.lock` to both `mutants oracle` and
`mutants run`. Both arms then use the release's registry Verus crate versions;
workspace adjustments and the selected release lock are retained with the run.
Without that option, the suite keeps source-patched preparation for explicit
diagnostic runs. The standard and deep mutation probes use the registry lock,
matching the checked-in catalogues. The runner refuses an oracle from another
platform, stock release or preparation, or one with fewer than three runs.
Regenerate the stock oracle for the selected setup before measuring it.

Root-only JSON/time logging leaves vstd's product path intact. Both arms ask
Verus to suppress wall-time notices; the reader preserves all stderr. The
oracle must match protocol `roots-json-strict-2`, not merely the generator
version. Historical all-crate logging runs do not qualify this protocol.

A round fails if its daemon disappears or changes. Retained-proof share uses
newly published retained receipts; a run with no solver sessions alone is not
evidence of instant revert. Timeouts retain partial logs and stop descendant
processes, including when their original parent has already exited.

For changes that overlap managed input checks and receipt reads, also run
`python3 scripts/check-managed-retry.py SAIL_INSTALL NEW_OUTPUT_DIRECTORY` on a
native host. It removes only the selected receipt, mutates the proof during the
actual retry's Cargo cleanup, and requires a real failed proof with no Current
result. This is a local acceptance control, outside CI and timed benchmarks.
