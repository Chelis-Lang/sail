# Spike: per-invocation capture for concurrent Cargo-Verus output

Tractability spike for the capture gate of
[VI-0039](../../../../docs/verus-issues/VI-0039-concurrent-cargo-json-framing.md),
run on 2026-11-09/10. It is a prototype and a measurement, not an acceptance
record: nothing here is wired into `sail-conformance`'s corpus runs.

## Environment

- c7i.4xlarge, us-east-1, Ubuntu 24.04.5, kernel 7.0.0-1014-aws, native.
- Stock Verus `0.2026.10.18.7d82994` release binaries; fork runtime variant
  `7d829946231d6413a398f6e4160cdfe951261dad-9a6672c6115468e5` (fork commit
  `a0e11cff`), both run as plain `cargo-verus` with Rust 1.99.1 and the
  pinned Z3. The fork arm is the fork's own `cargo-verus`, not `sail verify`.
- Projects from `scripts/perf-scaling.py` at `b0c6b3a5`: `proj2` is
  `--shape shared --packages 2 --files 1 --targets 1` run with
  `CARGO_BUILD_JOBS=4`; `proj8` is `--shape shared --packages 8 --files 4
  --targets 2` with Cargo's default jobs (16 CPUs).
- Command: `cargo verus verify --workspace --all-targets -- --output-json
  --time`, a fresh target directory for every run.
- Tee binary SHA-256 `5b8001cb56e41825bbac6949378f19d30457647f39b8de359888b54d41d68a20`.
- Raw runs, captures, traces and analysis JSON: one external archive,
  SHA-256 `29ce7e5d211adbea470a793b2a0e1312c97a858db168c56bc1fb220cb4806f36`.

Load note: the stock and fork plain series ran side by side, and partly
beside the tee series. That varies scheduling, which is the point; the wall
times in the logs are not performance measurements.

## Where the tears fall

Every tear is on a line boundary. No mid-line tear was observed.

| Project | Arm | Plain runs | Exit 0 | Stream parses | Lines failing pretty-JSON line grammar | Digit-masked line multiset equals the captured documents |
|---|---|---|---|---|---|---|
| proj2 | stock | 20 | 20 | 16 | 0 | 20 |
| proj2 | fork | 20 | 20 | 10 | 0 | 20 |
| proj8 | stock | 20 | 20 | 5 | 0 | 20 |
| proj8 | fork | 20 | 20 | 5 | 0 | 20 |

The cause was traced with `strace` (`strace.sh`, `writes.py`), one run per
arm and project. The Verus driver writes a whole document in one `write`
(up to 1,651,580 bytes) to its own pipe; Cargo is the only writer of the
combined stdout and issues exactly one `write` per line:

| Run | Combined stdout | Cargo writes | Writes that are exactly one whole line | Stream parses |
|---|---|---|---|---|
| stock proj2 | 1,523,973 bytes | 49,566 | 49,566 | yes |
| fork proj2 | 1,674,056 bytes | 51,448 | 51,448 | yes |
| stock proj8 | 1,630,332 bytes | 53,840 | 53,840 | no (5 documents) |
| fork proj8 | 1,788,383 bytes | 55,850 | 55,850 | no (5 documents) |

So the documents are not torn by concurrent writers sharing a descriptor.
Cargo reads each compiler's stdout separately and forwards it line by line,
interleaving the lines of jobs that run at the same time.

## Prototype

- `sail-conformance-tee` (`crates/sail-conformance/src/bin/`): placed as
  `verus` beside an unchanged copy of `cargo-verus`, it starts the real
  driver with the same arguments, environment and stdin, forwards stdout
  and stderr unchanged, then records the exact bytes with the package,
  crate, crate types, test flag, target and a digest of the arguments and
  `__VERUS_DRIVER_*` environment.
- `sail_conformance::invocations`: `explain` accepts a combined stdout only
  if it is exactly an order-preserving line interleaving of the records
  (all candidate assignments are kept; an abandoned search is never a
  pass); `compare_arms` pairs the two arms by identity and applies the
  existing `sail_conformance::compare` to each pair unchanged.
- `sail-conformance-invocations check|compare` runs both over run
  directories.

## Results with the tee

| Project | Arm | Runs | Exit 0 | Whole-stream parse succeeds | Combined stdout explained by the records | Documents recovered per run | Incomplete records |
|---|---|---|---|---|---|---|---|
| proj2 | stock | 20 | 20 | 14 | 20 | 9 of 9 | 0 |
| proj2 | fork | 20 | 20 | 11 | 20 | 9 of 9 | 0 |
| proj8 | stock | 20 | 20 | 2 | 20 | 23 of 23 | 0 |
| proj8 | fork | 20 | 20 | 3 | 20 | 23 of 23 | 0 |
| proj2 with one failing function | stock | 5 | 0 (101) | 5 | 5 | 3 of 3 | 0 |
| proj2 with one failing function | fork | 5 | 0 (101) | 5 | 5 | 3 of 3 | 0 |

The explanation check takes 0.07 s per proj8 run (53,840 lines, 23
documents, at most 462 simultaneous candidates).

Pairing, same round of each arm, both at one fixed target path:

| Project | Comparisons | Same | Pairs per comparison | Pairs with documents | Functions | Equal argument digests | Stderr byte-equal | Stdout byte-equal |
|---|---|---|---|---|---|---|---|---|
| proj2 stock vs fork | 20 | 20 | 28 | 9 | 1,882 | 28 | 28 | 19 |
| proj8 stock vs fork | 20 | 20 | 42 | 23 | 2,010 | 42 | 42 | 19 |
| failing proj2 stock vs fork | 5 | 5 | 22 | 3 | 1,872 | 22 | 22 | 19 |
| proj2 stock vs next stock | 19 | 19 | 28 | 9 | 1,882 | 28 | 28 | 19 |
| proj8 stock vs next stock | 19 | 19 | 42 | 23 | 2,010 | 42 | 42 | 19 |

## Limits found

- **Stdout is never byte-equal for a pair that has a document**, even stock
  against stock: `times-ms` differs every run, stock's `func-details` key
  order differs between runs, and stock against fork also differs in
  `verus` (version). The 19 byte-equal pairs are the ones with empty
  stdout. Per-invocation byte equality holds for stderr only; stdout needs
  the existing structured comparison per pair.
- The existing comparison reports 437 functions with rlimit drift between
  stock and fork and none between runs of one arm. It records drift without
  failing; the spike did not change or investigate that.
- Argument digests are equal only because both arms used the same target
  path one after the other. Different target paths change the arguments.
- Three recorded invocations per run are compiler probes run by build
  scripts (`verus_syn`, `proc-macro2`). They carry a package name but no
  `--crate-name NAME` pair, so they are counted, not paired or compared.
  Their streams go to the build script, not to Cargo's output.
- Combined stderr is Cargo's rendering of the drivers' JSON diagnostics plus
  Cargo's own status lines, so the interleaving rule does not apply to it
  as written. Only per-invocation stderr equality was checked.
- A failing library stops its dependents, so the failing project produced
  three documents and no interleaving. Failure output under an actual tear
  was not exercised.
- The tee adds a process and a pipe per compiler invocation. Its cost was
  not measured.

## Files

- `runs.sh`, `tee-series.sh`: the run loops used on the host.
- `strace.sh`, `writes.py`: the write trace and its classification.
- `analyze.py`: aggregation (diagnostic only, not a conformance rule).
