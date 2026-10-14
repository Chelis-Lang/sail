# Reader acceptance

The M7 reader gate requires Status and complete Result reads below 200 ms p95
on Linux while a verification is running. Four concurrent clients make 200
reads (100 Status, 100 Result) of the same completed, Current repair job,
whose Result is about 4 MB. Every sample must be Current for the job's input
revision, and the second, active job must be running before and after every
read. Status and results are in
[M7 validation, gate 5](validation/m7.md#gate-5-responsive-reads).

## Running it

On a native x86-64 Linux host (ADR 0038), inside the CI image:

1. `scripts/perf-host.sh up LANE`, `sync LANE`; build the image on the host
   (`ci/local.sh` builds it on first use, and `run` supplies the engine token).
2. In a container from the image, build the product and the probe:
   `cargo build --release --locked -p sail-cli -p sail-shim --bins --example job_read_bench`.
3. Prepare the source-only HumanEval workspace: `human-eval-verus` at the
   `corpus/repair-edits.json` commit, with the catalogue's `cargo_wrapper`
   `Cargo.toml` whose `vstd` path names `source/vstd` of a Verus checkout at
   the catalogue's `verus_commit` (`sail-bench repair prepare` does the same),
   then `cargo generate-lockfile` with Rust 1.98.1.
4. Run:

   ```sh
   scripts/reader-gate.sh target/release target/release/examples/job_read_bench WORKSPACE OUT
   ```

   The script starts a private daemon, installs the engine from
   `SAIL_ENGINE_BUILD` (default `/opt/sail-engine`), and copies the workspace
   twice. In the first copy it runs an ordinary full verification, then a
   repair of `compute_gcd` until one is accepted and Current (a first run may
   only establish build-script declarations). It then submits a cold repair
   in the second copy, waits for that job's `human_eval_verus` verifier, and
   runs the probe. The probe prints p50/p95/p99 per operation, writes every
   sample to `OUT/samples.json`, and exits nonzero unless the gate passes. The
   script also fails if the active verifier finished during the reads.
5. `scripts/perf-host.sh down LANE`.

A run takes a few minutes on a `c7i.4xlarge`; most of it is setup.

## Where the time goes

On the native host a Status or Result spends about 18-24 ms p50 in freshness
validation, mostly the kernel read-lease barrier. The output seal costs about
1.6 ms and socket output of a 4 MB frame about 1.7 ms
(L5 rescue lane report, tag `archive/rescue-m7-98f6292`).
