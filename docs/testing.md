# Testing Sail

The complete gate and testing tiers are defined in [AGENTS.md](../AGENTS.md).
Run Linux checks on a native EC2 host with `ci/local.sh`; use `ci/run.sh host`
for the macOS platform check. Performance measurements require a quiet native
host, separate from builds and gates.

The e2e shell scripts need no GNU tools on macOS: fixture edits use portable
`sed`, and bounded steps use coreutils' `timeout` where it is installed or
`ci/timeout.py` (the same options and exit statuses, checked against GNU by
`ci/test-timeout.py`) where it is not.

## End-to-end phases

CI builds the release binaries once, then runs nine independent jobs. Each
job retains the existing ten-minute hard limit. The required `End to end`
aggregate accepts only nine successful results: failure, cancellation and a
skipped job all fail the aggregate.

| `ci/local.sh e2e PHASE` | Scope | Image |
|---|---|---|
| `legacy` | New projects, cold/warm output parity, JSON, MCP and portfolio | core |
| `output` | Edit-loop output, compile errors and partial verification | core |
| `flags` | Cargo/Verus invocation flags | core |
| `loader` | Loader-variable fallback | core |
| `managed` | Failing-first vstd reuse, complete receipts, jobs/MCP, missing evidence, edits, prune and Cargo metadata | mcp |
| `managed-copies` | Own passing donor, two fresh workspace-member proofs, Current adoption and each project's own receipt reuse | core |
| `managed-native` | Own passing donor, fresh plain summaries, unchanged silence and same-PATH compiler replacement against Sail and stock | core |
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
  --rounds 3 [--per-class N] [--filter SUBSTR] --out ~/mut/vest --machine "EC2 c7i.8xlarge, us-east-1"
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
speedups with 95% bootstrap intervals, the share of replies the solver cache
served and the share of Sail runs restored without a solver session.

Tiers: `ci/local.sh conformance standard` ends with two mutants per class of
`human-eval` for one round; `ci/local.sh conformance deep` runs every
catalogue for one round. Both are local-only. Timings come only from a native
host (`ci/run.sh bench mutants NAME ROUNDS`, three rounds for a release).

For release-product mutation measurements, pass `--vstd-lock
/path/to/release/deps/<bundle>/Cargo.lock` to both `mutants oracle` and
`mutants run`. Both arms then use the release's registry Verus crate versions;
workspace adjustments and the selected release lock are retained with the run.
Without that option, the suite keeps source-patched preparation for corpus
conformance. The two setups are reported separately. Regenerate the stock
oracle for the selected setup before measuring it.

A round fails if its daemon disappears or changes. Retained-proof share uses
newly published retained receipts; a run with no solver sessions alone is not
evidence of instant revert. Timeouts retain partial logs and stop descendant
processes, including when their original parent has already exited.
