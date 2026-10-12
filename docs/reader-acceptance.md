# Reader acceptance

The M7 reader gate requires established Status and complete Result reads below
200 ms p95 on Linux while a selected-crate verification is running. Four
concurrent clients read the same complete, Current repair job. The client RPC
deadline is 5 s. Two collectors run in turn, the Python probe
(`scripts/m7-job-read-bench.py`) and the Rust probe
(`crates/sail-cli/examples/job_read_bench`). Each retains 200 samples, 100 Status
and 100 Result. Every sample must be Current for the job's input revision, with
the same live verifier process before and after the RPC. Retained CLI/MCP
scenarios then run against the same service, with 120 s workflow, 30 s command
and 5 s RPC limits.

## Current state

Native Linux, `c7i.4xlarge` in `us-east-1` (ADR 0038). Every run passed both
collectors and the retained CLI/MCP scenarios. Full Results were about 4.05 MB,
with complete graphs of all-verified functions for vstd (1,871) and HumanEval (331).

| Product | Runs | Python Status p50/p95/p99 | Python Result p50/p95/p99 | Rust Status p50/p95/p99 | Rust Result p50/p95/p99 |
|---|---:|---|---|---|---|
| L5 readers branch (`be2246c`) | 3 | 46.4 / 68.6 / 84.5 | 72.1 / 101.9 / 115.7 | 25.5 / 34.0 / 38.8 | 27.3 / 43.6 / 71.4 |
| `73a3b46` (`rescue/m7`) | 3 | 39.9 / 80.0 / 94.6 | 70.5 / 100.5 / 110.1 | 32.6 / 39.7 / 41.5 | 42.9 / 69.9 / 82.7 |
| `387ba80` | 1 | 32.3 / 63.3 / 69.3 | 72.2 / 93.3 / 97.1 | 34.0 / 45.2 / 46.7 | 43.3 / 67.4 / 91.3 |
| `b89546c` | 1 | 36.5 / 61.5 / 67.4 | 75.4 / 96.6 / 102.2 | 32.9 / 44.7 / 49.9 | 43.4 / 67.2 / 85.7 |

All values are milliseconds, pooled over the listed runs, using the collectors'
own quantile rule. See [the L5 report](reports/rescue/l5-readers.md) for the
fixes, the attribution and raw evidence hashes.

The Python collector's timed window shares the interpreter lock with three
other reader threads. Those threads scan `/proc` and parse 4 MB Results, so
its spread is larger than the server-side change. The Rust collector tracks
server-side cost more closely.

## Method

1. `scripts/perf-host.sh up LANE`, `sync LANE`, then build the CI image on the
   host (`ci/local.sh` builds it on first use; `run` supplies the engine token).
2. In a container from that image, build the product and probe:
   `cargo build --release --locked -p sail-cli -p sail-shim --bins --example job_read_bench`.
   Give each source tree its own target directory, or copy it with fresh
   mtimes. Cargo's workspace-relative dep-info can otherwise treat an older
   synced tree as fresh against another tree's artifacts.
3. Prepare the source-only HumanEval workspace as `m7-repair-bench.py prepare`
   does. Use `human-eval-verus` at the `corpus/repair-edits.json` commit, with
   the `cargo_wrapper` `Cargo.toml` whose `vstd` path names `source/vstd` of a
   Verus checkout at the catalogue's `verus_commit`. Then run
   `cargo generate-lockfile` with the engine's Rust toolchain.
4. Run:

   ```sh
   python3 scripts/reader-acceptance-run.py --install-from "$CARGO_TARGET_DIR" \
     --bin-dir PRODUCT --probe-bin-dir PROBE --source-workspace WORKSPACE \
     --source-runtime /opt/sail-engine --stock-bin-dir /opt/verus/verus-x86-linux \
     --out NEW_DIR --environment-label "native c7i.4xlarge us-east-1"
   ```

   `--install-from` creates new product and probe directories with their
   `freeze.json` records. The script drives the unchanged
   `scripts/m7-reader-acceptance.py` controller with `--prepare-contexts`, a
   600 s setup budget and a retained service, then runs
   `scripts/m7-retained-jobs-acceptance.py`. It prints p50/p95/p99 per collector.
   A run takes about seven minutes on a `c7i.4xlarge`.
5. `scripts/perf-host.sh down LANE`.

The controller copies the source into independent baseline and active
workspaces without prior receipts or Cargo output. It records an unmeasured full
preparation of each, then the original 600 s ordinary and exact repair-context
checks. It then starts the measured active proof, waits for its live
`human_eval_verus` verifier, and runs both collectors. It admits each census only
if all 200 samples are Current, with the same input revision, crate and
continuing verifier. Owned jobs are archived from exactly one of
`projects/jobs/<id>` or `results/projects/jobs/<id>`, through descriptors that
refuse symlinks, FIFOs and substituted directories, and are then cancelled.
Raw output never admits a proof.

## Diagnostics

`--timings` sets `SAIL_JOB_RPC_TIMINGS=1` and `SAIL_FRESHNESS_TRACE=1` on the
owned daemon ([RPC diagnostics](result-rpc-diagnostics.md)). Such a run is a
diagnostic, not an acceptance sample. On the native host, a Status or Result
spends about 18-24 ms p50 in freshness validation. Most of that is the kernel
read-lease barrier: about 5 ms of binding lookups for ~1,180 leased files and
two `/proc/locks` sweeps of ~225 KB at about 3-7 ms each. The output seal costs
about 1.6 ms and socket output of a 4 MB frame about 1.7 ms. Concurrent readers
already share an in-flight freshness validation when it started after they
arrived; about half of all reads did so.

## Earlier attempts

Under Linux amd64 emulation on arm64 macOS (superseded by ADR 0038). The
per-run records are condensed in [M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads)
and preserved at the tag `archive/m7-contracts-f7103bf`:

- `4472279`, 2026-10-11: failed, Python Result p95 394.7 ms; cause unknown.
- `0f31c1d`, 2026-10-11, stage diagnostic: failed, Python Result p95 403.3 ms.
- `8e09eaa` and the native-binding product: setup timed out at the 600 s
  ordinary baseline check.
- `730a321`, 2026-10-12: passed, Python Result/Status p95 94.2/59.9 ms, Rust
  67.5/50.3 ms.
- `b89546c`, 2026-10-12: passed, Python 91.2/57.0 ms, Rust 83.9/44.4 ms.
- `387ba80`, 2026-10-12: failed, Python Result p95 436.0 ms, Status 137.1 ms;
  Rust and retained checks not attempted. The same commit passes natively
  (93.3 ms). The read path is unchanged since `b89546c`, so the emulated
  failure is not a code regression. Its cause is unknown.

## Developer checks

`python3 scripts/test-m7-reader-acceptance.py` (part of `ci/local.sh release`,
not CI) runs the fast controller
regressions: both job layouts, ambiguity, missing output, record, scope,
revision and seal mismatches, substituted directories, symlinks, FIFOs, duplicate
JSON fields and unchanged public-proof admission. These fixtures do not replace
a real four-reader run. See [the proof-repair guide](proof-repair.md) for the
public commands these workloads exercise, and
[native product setup](native-product-setup.md) for installed-product lease
ownership.
