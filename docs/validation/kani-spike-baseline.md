# Kani spike S1: honest baseline

Status: **passes**. One shape is within 5% of direct `cargo kani -j8` on a
warm target on both projects: a single original `kani-driver` run over every
selected harness with `-j 8`, started the way Sail's executor starts it (staged
tool bundle, cleared environment, `--export-json`), on a target directory kept
between runs. Its median wall is 1.8% below direct on `arrayvec` (5.44 s
against 5.54 s) and 0.6% below on `s2n-quic-core` (290.2 s against 292.1 s).
Sail's executor as it is takes 85.8 s for the nine `arrayvec` harnesses, 15.5
times direct; 7.4 s of every invocation is Sail's own fixed cost. A batch
executor must make that cost once per batch and far smaller (below 0.28 s to
stay inside 5% on `arrayvec`). This is spike S1 of the Kani phase design
(`docs/design/kani-phase.md`, ADR 0180); nothing here is product code.

## Environment

| | |
|---|---|
| Host | one `c7i.8xlarge` (32 vCPU Xeon Platinum 8488C, 61 GiB), us-east-2c, Ubuntu 24.04.5, kernel 7.0.0-1018-aws, lane `kani-s1`, 2026-11-27, 3.9 hours |
| Kani | 0.68.0 (`cargo install --locked kani-verifier --version 0.68.0`, `cargo kani setup`); source at tag `kani-0.68.0`, `0d2328a93f0e0ff66132d6bfa1a7d884877cf862`, read only |
| CBMC | 6.11.0 from the bundle, Kissat from the bundle where harnesses select it |
| Rust | `nightly-2026-08-21` from the bundle setup |
| Sail | executor from `feat/kani-executor` (PR #810) at `9fbf4f84`, release build, `sail engine install --backend kani` |
| `arrayvec` | 0.7.8, nine pilot harnesses (`spikes/kani-pilot/pilot` on `spike/v030-kani`): 6 succeed, 3 fail by design |
| `s2n-quic-core` | aws/s2n-quic at `e11382fda8a52979a2f952a789efc616db4ff4e4`, S3's `Cargo.lock`, 40 harnesses, all succeed |

`RUSTC_WRAPPER` was empty. Every timed command ran under GNU `time` in its own
cgroup; RSS below is the largest single process, "tree peak" the cgroup's
`memory.peak`.

## Arms

- **a** `cargo kani -j8 --output-format terse` over all harnesses, on a removed
  target (cold), then again (warm).
- **b** one `cargo kani --harness X --exact` per harness on the shared target,
  sequential or 8 at a time (`xargs -P8`). "First" is the first time each
  harness filter runs on that target; "repeat" runs the same filters again.
  Cargo keys the build by the harness filter, so each new filter recompiles the
  crate once (seen in a fingerprint log: same filter again, 0 compiles; a new
  filter, 1; an earlier filter again, 0).
- **c** `sail verify --backend kani --harness X --unwind N --output json`, nine
  in sequence, with the bounds the harnesses state (6, and 2 for `e1`).
- **d2b** batch prototype outside Kani: one `cargo kani --only-codegen -j 8`
  builds and links every harness, then each harness runs only the
  `goto-cc --function`, `goto-instrument` and `cbmc` commands Kani would run,
  8 at a time. The argv came from one recording run with wrappers installed as
  the bundle's `goto-cc`, `goto-instrument` and `cbmc` (the S3 method) and is
  replayed against the bundle binaries directly. No cargo, rustc or
  `kani-driver` runs per harness. Kani has no supported interface for the
  post-codegen stage of one harness; `--only-codegen` is supported, the replay
  is not.
- **d3** one `kani-driver kani -Z unstable-options --export-json … -j 8` with
  every harness as `--harness … --exact`, started from Sail's staged bundle
  with the executor's environment (`env -i`, private `HOME` and `TMPDIR`,
  offline Cargo, staged toolchain on `PATH`) and a persistent target directory.
  This is what a batch executor built on Kani's own multi-harness run would do,
  without Sail's per-invocation inspection.

Three rounds of each arm unless stated.

## Results

Median wall in seconds (range), median user and system CPU seconds, RSS and
tree peak in MiB.

`arrayvec` (9 harnesses):

| Arm | Wall | User / sys | RSS / tree peak | Against a warm |
|---|---|---|---|---|
| a cold (6 rounds) | 6.29 (6.16–6.39) | 18.1 / 1.5 | 623 / 507 | |
| a warm (6 rounds) | 5.54 (5.36–5.63) | 16.6 / 1.1 | 222 / 540 | |
| b sequential, first | 19.01 (18.74–19.07) | 17.4 / 2.1 | 319 / 304 | 3.43× |
| b sequential, repeat | 17.05 (16.59–17.30) | 15.9 / 1.6 | 219 / 293 | 3.08× |
| b 8-way, first | 6.05 (5.85–6.21) | 18.4 / 2.1 | 320 / 454 | +9.2% |
| b 8-way, repeat (6 rounds) | 5.46 (5.41–5.61) | 16.9 / 1.6 | 219 / 416 | −1.4% |
| c Sail executor, sequential | 85.78 (85.56–85.84) | 69.6 / 16.5 | 296 / 315 | 15.5× |
| d2b cold: codegen + replay | 1.23 + 5.42 = 6.65 | 18.0 / 1.8 | 624 / 483 | |
| d2b warm: codegen + replay | 0.47 + 5.36 = 5.83 | 16.6 / 1.3 | 214 / 410 | +5.2% |
| d2b warm, longest first | 0.49 + 5.42 = 5.91 | 16.8 / 1.4 | 214 / 410 | +6.7% |
| d3 cold | 6.29 (6.25–6.37) | 17.9 / 1.4 | 600 / 532 | |
| d3 warm | 5.44 (5.41–5.57) | 16.5 / 1.1 | 233 / 485 | **−1.8%** |

`s2n-quic-core` (40 harnesses):

| Arm | Wall | User / sys | RSS / tree peak | Against a warm |
|---|---|---|---|---|
| a cold | 292.35 (286.32–319.20) | 1198 / 36 | 8412 / 28683 | |
| a warm | 292.07 (280.88–304.10) | 1140 / 30 | 8413 / 28037 | |
| b sequential, repeat | 1138.38 (1134.78–1172.72) | 1115 / 35 | 8415 / 17843 | 3.90× |
| b 8-way, first (4 rounds) | 509.87 (507.14–513.25) | 1448 / 51 | 8414 / 24928 | 1.75× |
| b 8-way, repeat | 273.46 (273.08–275.76) | 1160 / 34 | 8414 / 24071 | −6.4% |
| d2b cold (1 round): codegen + replay | 21.04 + 316.51 = 337.6 | 1304 / 55 | 8387 / 26244 | |
| d2b warm: codegen + replay | 2.04 + 317.14 (287.04–319.02) = 319.2 | 1252 / 49 | 8387 / 26467 | +9.3% |
| d2b warm, longest first | 2.08 + 284.88 (283.36–287.56) = 286.96 | 1261 / 51 | 8387 / 26640 | −1.7% |
| d3 cold (1 round) | 303.27 | 1125 / 34 | 8408 / 28566 | |
| d3 warm | 290.20 (260.93–295.13) | 1075 / 29 | 8408 / 28465 | **−0.6%** |

Verdicts agree in every arm: `arrayvec` 6 successes and 3 failures (Sail: 6
`bounded_success`, 2 `counterexample`, 1 `bound_insufficient`), `s2n-quic-core`
40 successes. For d2b every replayed harness's multiset of (property, status)
in the raw `cbmc --json-ui` output and its exit code equal the recording run's
(9 of 9 with 2,127 results in all nine d2b and twelve d2 runs; 40 of 40 with
31,365 results in all seven d2b runs).

On `s2n-quic-core` the wall is set by two harnesses of about 280 s each; the
rest finish inside that time on the other seven slots. Arms differ mainly in
when those two start, which is why 8-way repeat with no shared build is faster
than `-j8` and why d2b in file order loses up to 35 s that longest-first
recovers. The longest-first order came from d2b round 4's per-harness times.

## Where Sail's per-harness time goes

From `strace -f -ttt -e trace=execve` of three `arrayvec` harnesses, three
rounds each, against direct `cargo kani --harness X --exact --unwind 6` on a
fresh and on a warm target. Milliseconds, ranges over the three rounds:

| Phase | Sail `p6` | Sail `p3` | Direct, fresh target | Direct, warm target |
|---|---|---|---|---|
| Before `kani-driver` starts | 5424–5439 | 5424–5443 | 12–13 | 12–14 |
| Cargo and `kani-compiler` | 500–546 | 1263–1354 | `p6` 491–505, `p3` 1271–1312 | 131–146 |
| `goto-cc` and `goto-instrument` | 251–270 | 1204–1279 | same as Sail | same |
| CBMC | 240–242 | 4088–4150 | same as Sail | same |
| Driver after CBMC | 6–9 | 71–91 | same as Sail | same |
| After `kani-driver` exits | 1947–1954 | 1947–1954 | 0–1 | 0–1 |
| Total | 8388–8441 | 14034–14226 | 976–987, 6524–6640 | 608–637, 5378–5513 |

Sail minus direct on a fresh target is 7.39–7.46 s for every harness. It is:

- **Staging and inspection before the driver, 5.4 s.** A file-open trace of
  one run shows Sail reading the original bundle (485 MB), the original Rust
  payload (1.5 GB, 4,224 opens) and the private stage (485 MB) before it starts
  the driver. `strace -c` of the Sail process: 1,256,753 `read` calls, 69,618
  `statx`, 28,912 `open`.
- **Inspection after the driver, 1.95 s.** The same trees are opened again
  (1,408 Rust payload files and the stage). Decoding the export is a few
  milliseconds of this; it could not be separated further.
- **Fresh target, 0.35–1.15 s** (direct fresh minus direct warm), from
  recompiling the project and its dependencies in every invocation.
- CBMC and Kani's own steps take the same time under Sail as directly.

Over nine harnesses that is 85.8 s against 19.0 s for direct sequential
invocations and 5.5 s for one `-j8` run.

## Against the pass rule

The rule: a batch shape whose wall is within 5% of direct `cargo kani -jN`
warm on both projects. **d3 passes** (−1.8% and −0.6%). d2b passes only on
`s2n-quic-core`, and only when the longest harnesses start first (−1.7%;
+9.3% in file order); on `arrayvec` it is +5.2% in file order and +6.7%
longest first, the shared codegen step and per-harness process starts being a
larger share of a 5.5 s run.

For Sail's executor to reach d3 it must:

1. run one original `kani-driver` per batch with every selected harness and
   `-j N`, and decode each harness from that run's single export, instead of
   one driver per harness;
2. keep a per-project target directory between runs instead of a fresh one;
3. inspect the tool stage once per batch and cheaply. Today's 7.4 s fixed cost
   alone is 134% of the warm `arrayvec` wall; the 5% budget there is 0.28 s,
   so the per-run check has to rest on identities established at install time
   rather than rereading 2.5 GB;
4. leave scheduling to Kani or start the longest harnesses first; on
   `s2n-quic-core` the start time of two harnesses moves wall by about 10%.

A per-harness CBMC stage after one shared codegen (d2b) keeps verdicts
unchanged but uses 10% more user CPU than `-j8` on `s2n-quic-core` and depends
on Kani's internal command lines, which Kani does not offer as an interface.

## Limits

- One host, two projects, 49 harnesses; no contracts or stubbing harnesses.
- d3 omits Sail's inspection, so it measures the batch shape, not a built
  executor. d2b replays recorded argv; a Kani release that changes those
  commands breaks it.
- The d2b cold arm and d3 cold arm on `s2n-quic-core` ran one round; the other
  rounds of those cells were warm-only.
- The Sail executor (c) and the phase trace ran on `arrayvec` only.
- `-j 8` only; no other `-j` was measured.

## Attempts

| # | Arm | Result | Cause |
|---|---|---|---|
| 1 | `arrayvec` phase trace without `strace --seccomp-bpf` | inflated phases (Sail `p3` 29.0 s) | tracing every syscall; rerun with seccomp filtering |
| 2 | `arrayvec` d2 (replay includes the link of each `.symtab.out`) | codegen 1.53 + replay 5.84 s, +33% | links twice; superseded by d2b |
| 3 | `s2n-quic-core` round 1 a cold | stopped after 60 s | restarted to add d2b |
| 4 | `s2n-quic-core` round 1 d2b warm without the bundle on `PATH` | 30 of 40 harnesses exit 10 | CBMC could not find Kissat |

## Evidence

`2026-11-27-kani-spike-baseline.tar.gz`, 12,160,138 bytes, sha256
`f499f485f345d023fc04e8e67f58a47b233264f3e210f52026755f0a032d9c35`, kept
outside the repository. It holds every script, the time, cgroup and log files
of every run, the recorded argv and replay scripts, the strace timelines and
file-open trace, the fingerprint logs, the Sail JSON results, setup and build
logs, and the failed attempts. Raw CBMC JSON (3.3 GB per replay round) and
Kani export files are not kept; their sha256 and per-harness status counts
are (`large-files.sha256`, `cbmc-compact.tsv`).
