# Determinism of the fork's Verus

The owner's direction (2026-10-15): Sail runs its Verus fork by default, and
the fork's output must be free of run-to-run randomness. This record measures
the fork on identical input, lists the fixes, and states what remains.

| Field | Value |
|---|---|
| Status | **Pass on the measured scope** with fork branch `l42/determinism` (`4285ae11`): one solver stream per bucket, one stdout (times blanked), one stderr and one exit code per unit. Both fixes are on the fork's `sail` branch as `9e32601e` and `bc9cda09` and pinned from `73dea021` ([re-pin check](#pinned-runtime-73dea021)) |
| Source | Sail `l42/determinism` (lane L42), 2026-10-15. Fork `Chelis-Lang/verus`: before = runtime commit `13a1176c`, pinned at the time (`engine/verus/fork.toml`); after = `l42/determinism` `4285ae11` (`13a1176c` + `54640eed` VI-0017 + `4285ae11` VI-0007), upstream `426d8b0`. The fork's `sail` branch carries the same two changes as `9e32601e` and `bc9cda09` |
| Runtime | Both built on the host with `cargo xtask build-verus-engine` (vstd rebuilt with `cargo-verus`, as the release does), run as plain `verus` / `cargo verus` (no `SAIL_ENGINE`); pinned Z3 4.16.0; stock release `0.2026.10.04.426d8b0` for the stock comparisons |
| Environment | Native EC2 c7i.8xlarge (32 vCPU), us-east-1, Ubuntu 24.04; standard-tier units ran 5 at a time, so scheduling was contended |
| Scope | Fast tier (147 units: `verus-examples`, `vstd`, `human-eval-verus`, direct `verus`); standard tier `vest`, `vest-generated`, `verismo` (2 units) and `verismo-main` (`cargo verus focus`, Verus dependencies patched to the pinned upstream source, as the harness does), plus `vest` patched to the fork's own source (fork macros) after the fixes; before only, 10 runs: `verified-node-replication`, `verified-ironkv`, `verified-storage` `multilog` and `pmemlog` |
| Limits | 3 runs per unit after the fixes (10 before for the fast tier); `page-table`, `anvil` and `capybarakv` not measured. One host and OS; macOS not measured. The flags are the conformance harness's (`--output-json --time --no-report-long-running -Z write-long-types-to-disk=no`) |

## Method

Each run captured the exact bytes Verus sent to every solver process (a
`tee` wrapper as `VERUS_Z3_PATH`), stdout, stderr and the exit code. Solver
processes were matched across runs by the sorted set of names their stream
declares, so any byte change inside a bucket shows, whatever order processes
started in. stdout was compared with every wall-clock number inside `times-ms`
replaced by 0 (rlimit counts, key order and array order kept); stderr was
compared byte for byte. The lane's harness was a temporary `sail-conformance`
subcommand (source in the raw tarball); the committed equivalent is
`scripts/verus-determinism.py` (below), which reproduced the after result on
the fast tier's direct units.

## Results

| Scope | Runs | Buckets | Before (`13a1176c`) | After (`4285ae11`) |
|---|---|---|---|---|
| Fast tier, 147 units | 10 before, 3 after | 577 | streams: 0 buckets vary; stderr: 0 units vary; stdout: **136 units vary** | 147 of 147 identical in everything |
| Fast tier, `scripts/verus-determinism.py` (examples and HumanEval, 153 inputs) | 3 | — | — | 153 of 153 identical |
| `vest` | 3 | 206 | streams 0 vary; stdout varies | identical |
| `vest`, Verus dependencies patched to the fork's source | 3 | 206 | — | identical |
| `vest-generated` | 3 | 6 | stdout varies | identical |
| `verismo` `verismo_tspec` | 3 | 283 | stdout varies | identical |
| `verismo` `bitflags_verus` | 3 | 3 | stdout varies | identical |
| `verismo-main` | 3 | 121 | stdout varies | identical |
| `verified-node-replication`, `ironsht`, `multilog`, `pmemlog` | 10 | 22, 25, 17, 7 | streams 0 vary; stderr 0 vary; stdout varies in all 4 | not rerun |

Every before stdout difference was the order of the `times-ms` lists
(VI-0017). No solver stream varied anywhere, before or after: the earlier fork
fixes (VI-0001, VI-0015) hold on this scope, including `cargo verus` with
either macro source.

The fast tier showed no stderr variation in 10 runs, but stock's error order
across modules is a race (VI-0007) that these inputs rarely lose: the
pinned runtime printed two orders of `examples/debug_expand.rs` in 5 runs.

## Fixes

| Issue | Fork commit | Test (fails on `13a1176c`, passes after) | Rule |
|---|---|---|---|
| [VI-0017](../verus-issues/VI-0017-timing-json-order.md): `--output-json --time` lists modules by time and functions in hash order | `9e32601e` (lane branch `54640eed`) | `sail_vi0017_timing_order` (modules came out `"", m1, m3, m2`) | structured output: lists sorted by name, values unchanged |
| [VI-0007](../verus-issues/VI-0007-nondeterministic-error-order.md): errors from several buckets print in report order | `bc9cda09` (lane branch `4285ae11`; the dropped `f98b87c5`, reinstated) | `sail_vi0007_error_order` (`[39, 38, 37, 40, 35]` instead of `[35, 37, 38, 39, 40]`) | order stock produces: stock printed the fork's exact stderr for `debug_expand.rs` in 1 of 40 runs, and for the VI-0007 repro in 8 of 20 |

On `4285ae11` the full `rust_verify_test` suite (`vargo test --release -p
rust_verify_test`, vargo-built vstd, every `sail_vi*` test included) gave
4,649 passed, 0 failed, 58 ignored. A first attempt failed 1,913 tests with
`can't find crate for vstd` because the tree's vstd had been rebuilt the
release way (`cargo-verus build`) for the corpus runs; rebuilding vstd with
vargo fixed it (test harness setup, not a fork defect). Fork commits were checked with
`scripts/check-ai-markers.sh range 13a1176c 4285ae11` (clean).

## Pinned runtime 73dea021

The re-pin to fork `73dea021` (lane L43, 2026-10-15) repeated the committed
check on the published Linux runtime (`engine-runtime-73dea021-linux`, in the
CI image on an EC2 c7i.8xlarge): `scripts/verus-determinism.py --runs 3` over the
fast tier's 153 direct inputs (examples and HumanEval, the conformance flags)
found 153 of 153 identical in solver input (373 buckets), blanked stdout,
stderr and exit code (`determinism-fast.log` `2d0708f8…2bf6`;
[fork-default record](fork-default.md)).

## Residuals

- **Wall-clock numbers** in `--output-json --time` (and Cargo's `Finished … in
  Ns`) differ between runs by nature. They are measurements, not results; any
  byte comparison must blank them, as the check does.
- **Immediate notes** (`note: verifying module …` under `--verify-module` or
  `--verify-function`, VI-0012) are printed when a bucket starts, so where
  they fall among another bucket's diagnostics still depends on timing. None
  of the measured units printed both.
- **Not measured:** `page-table`, `anvil`, `capybarakv`; macOS.

## Check

```sh
VERUS_Z3_PATH=<pinned z3> scripts/verus-determinism.py --runs 3 \
  --inputs <corpus>/examples/*.rs ... -- <fork>/verus --crate-type=lib {} \
  --output-json --time --no-report-long-running -Z write-long-types-to-disk=no
```

It runs each input K times and exits 1 on any difference in solver input,
blanked stdout, stderr or exit code, printing the first differing line. A
`cargo verus` project works the same way with the command run in its
directory. It is local only: 3 runs of the fast tier's 153 direct inputs took about
8 minutes on a c7i.8xlarge, beyond the CI budget.

The committed runner retains each completed run's raw stdout, stderr, command
and exit status as `run-N.stdout`, `run-N.stderr`, `run-N.command.json` and
`run-N.result.json`, beside its `run-N` solver-capture directory. These sidecars
do not enter solver-bucket matching. Timing normalization applies to the
comparison; the retained streams keep their original bytes.

## Evidence

Everything below is in `l42-determinism-evidence.tar.gz`, kept outside the repository
(sha256
`026d3c35b632f2e406008d3ff7c23d44bec9bffb75d3d6ce911fdb624f0c875f`): the host
tarball `l42-determinism-raw.tar.gz` (sha256 `8a9a0273…fbeb4`; per-run
stdout and stderr, reports, divergence excerpts, stock variant outputs,
helper scripts), the partial 10-run standard log and the lane harness source.

| Log | sha256 |
|---|---|
| fast tier before, 10 runs | `ceaee923…e0e6` |
| fast tier after, 3 runs | `376c898f…7581` |
| standard before, 3 runs | `15a09863…f23c` |
| standard after, 3 runs (its `vest` row is void: a concurrent run shared its scratch tree) | `dc0b2481…06bf` |
| `vest` after, upstream source / fork source | `984cccf2…5b3a` / `ad3b90c5…cf79` |
| standard before, 10 runs, partial (`before-std-partial-10runs.tgz`) | `cfb9e91c…7406` |
| `scripts/verus-determinism.py`, fast tier after | `25f2fc49…f3ae` |
| `rust_verify_test` on `4285ae11` | `22092ec1…bc0b` |
| VI-0007 stock runs, `debug_expand.rs` / repro | `cbe5941d…19bb` / `00dc2cc0…c8b7` |

Failed or interrupted attempts: 2026-10-15, standard tier with 10 runs,
stopped after 4 units when the scope was reduced to 3 runs (owner); the
first after-fix fast run captured no solver streams (rustup missing from the
host shell's `PATH`, every run failed identically) and was rerun.
