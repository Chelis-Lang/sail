# Managed verification overhead

Validation of cheaper managed checks described in
[ADR 0047](../decisions/0047-attribution-logs-on-request.md#note-2026-10-16-cheaper-checks-156).

| Field | Evidence |
|---|---|
| Status | Corrected source qualified; post-#183 combined native gate, solver, full end-to-end and PR conformance pass |
| Source | Original corpus/performance evidence: `6563a170`, based on `0a7c1d0d`; initial checks also cover `6f73ab9e`. Joined-source checks: `a6629ee4`, based on #183 `ba2220b9` |
| Runtime | Sail fork `73dea021`, pinned Z3 4.16.0; stock oracle `0.2026.10.04.426d8b0` |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, CI image; macOS host correctness checks |
| Scope | Managed receipt/input/store lifecycle via full end-to-end suite; stock-versus-shim/fork/engine differential corpus |
| Limits | No corpus-wide managed-workflow claim; residual overhead keeps #156 open; no final-head cold or Mac performance claim |

## Correctness

The integration retains #183's complete foreign dependency closure binding:
the target-revision foreign-binding block and dependency-store binding
function are byte-identical to `ba2220b9`. Snapshot projection still includes
every closure package's files, and foreign binding uses the full unprojected
resolution graph. The only rebase conflict was in the changelog; both entries
were retained. On joined source `a6629ee4`, the native workspace gate passes
628 tests, solver passes 35 tests, every end-to-end phase plus local `extra`
passes, and PR conformance is 33/33 Same (311 functions) in all four arms,
with zero differences, instability, infrastructure errors, skips or rlimit
drift. All 502 tracked-file hashes were checked before and after the run;
only deliberately excluded local agent settings differ. Changed internal
packages were cleaned and rebuilt. Independent integration review confirmed
the complete foreign binding above is unchanged.

Joined-source archive: `managed-integration-native-evidence.tgz`, SHA-256
`c8d8862be77219b4e8c35ea44d50069489fa4a16dd1ad9f533cdd2c94b45e091`.
The retained native binaries archive has SHA-256
`37cc8f7fab6a896be49f2f864d258d777a0dd6af334919b40febab7a3eed167e`.
Full fast/standard and timing evidence below qualify their stated earlier
sources; those tiers and measurements were not repeated on the join.

Corrected source passes the native gate (625 tests), solver, all seven
end-to-end phases and local `extra`, and PR conformance (33/33 Same in all
four arms). Full standard passes: 159/159 units and 12,726 functions in each
arm, with zero differences, unstable results, infrastructure errors or skips.
Cache-warm and engine each record one rlimit-only drift (51948 to 51945)
for `log_entry_alive_value_wrap_around`; verdicts and diagnostics match.
The preceding source also passed full fast (147/147 Same in all four arms,
zero differences, instability or infrastructure errors). Corrected standard
includes the fast corpus. The corrected Mac host run passes clippy and 614
tests, with 35 solver tests skipped.

All 501 tracked-file hashes were compared with the native source before and
after checks; only deliberately excluded local agent settings differ. Native
logs show changed crates recompiling. Independent review caught unstable
sorting retaining a different raw spelling of equal paths. Stable sorting now
preserves the first inserted spelling. A raw-byte regression with repeated
aliases in both directions fails against the old implementation and passes
the corrected code.

Corrected native evidence: `corrected-native-evidence.tgz`, SHA-256
`82b3ff3053c6df0e0d2a77b66d8a3c4bc745ff2d0cf920074e19b85798493434`.
Original fast/partial standard evidence: `original-extended-evidence.tgz`,
SHA-256 `099b1f49b2c9e009aed64c597d71354618f7cddff9ce9fec448f49a3f9e0faab`.
Raw logs remain outside the repository.

## Performance

Three alternating, interleaved rounds on isolated native c7i.8xlarge compare
corrected `6563a170` against released rc.5 `c3687990`, using the same fork
and solver. Each project and mode has an independent warmed compiler target
and automatic Sail daemon/cache. All 24 measured commands exit 0. Candidate
binaries are stripped by the release procedure without a source rebuild.

| Case | rc.5 median | Candidate median |
|---|---:|---:|
| Demo unchanged | 0.388 s | 0.207 s |
| Demo body edit | 1.154 s | 0.957 s |
| Vest unchanged | 1.013 s | 0.539 s |
| Vest body edit | 24.078 s | 23.817 s |

Vest edit ranges overlap (23.86–24.23 s versus 23.70–24.08 s); the edit gain
is inconclusive. These measurements do not qualify cold, plain, failing/fixing,
MCP or Mac performance. Wall time uses blocking waitpid and a separate owned
timeout timer, avoiding timeout-poll quantization.

Final benchmark: `accurate-final-head-perf.tgz`, SHA-256
`aaa0af59c08d28590ef482df426d3bc0efb75174f812bdde1083487f69385e88`.
The binary/source/runtime identities, every sample, matching edited-source
hashes and command outputs are included.

Earlier c7i.4xlarge measurements preceded the rebase and final spelling fix:
vest unchanged 1.04 to 0.59 s, edit 29.1 to 28.6 s; demo unchanged 0.35 to
0.22 s, edit 1.19 to 1.08 s. Cold vest/demo 80.7/36.5 to 80.5/36.6 s.
They remain historical evidence, not current-head acceptance.
Archive `m114-linux-perf.tgz`, SHA-256
`2b94e9bf9be67ac3410576a9eacc61b634750b73e25e3b73f43ad90e92b5f8af`.

## Small-project overhead (#156)

| Field | Evidence |
|---|---|
| Status | Measured; edit and revert still above the 0.1 s sub-second bar |
| Source | Baseline `main` `4f373727`; candidate branch `perf/small-overhead` at `ac8422a2` (its crates are byte-identical to the branch rebased onto `5f79459d`, which added only release notices) |
| Runtime | Sail fork `9c0ca7e7` (7d82994, manifest `f4568e29…`), pinned Z3 4.16.0; stock oracle `0.2026.10.18.7d82994`, Cargo 1.99.1 both |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, CI `core` image, one idle lane |
| Scope | `sail new --lib` demo with one function; per round: add one function (edit), rerun unchanged, return to the original (revert); 7 rotated rounds after two warm-ups, own project copy, cache and target per arm; Sail binaries copied onto the cache filesystem |
| Limits | Linux only; no macOS claim; demo and current-pin Vest only; plain `sail verify` output |

Medians of seven paired rounds (ranges in the archive). All 63 timed runs
exit 0 and print the same `verification results` lines as stock.

| Case | Stock | `main` | Candidate |
|---|---:|---:|---:|
| Small edit | 0.792 s | 2.551 s | 1.042 s |
| Unchanged rerun | 0.117 s | 0.223 s | 0.215 s |
| Revert to verified content | 0.794 s | 1.715 s | 1.011 s |

Attribution on `main` (perf call-stack timelines, `SAIL_JOB_EVENTS` setup
phases and diagnostic builds): on the qualified Linux runtime the demo
project takes the ordinary instant-revert route, and retention cost 1.49 s
per edit and 0.68 s per revert. Of that, about 140 ms re-reads the engine
inventory to admit the plan, 0.9 s copies and checks the dependency graph
(vstd's 43 MB `.vir` and the macro libraries, read five to ten times) before
the compiler starts and 0.45 s checks it again afterwards: about 1.6 GB read
and hashed per edit, against a 0.8 s compile. The candidate skips retention
when the target's last compile cost less than four times its retention (see
[instant revert](../instant-revert.md)). Vest keeps instant revert. Five rotated paired rounds of the
requalification cycle (current-pin Vest with crates.io vstd
`0.0.0-2026-10-18-0147`, `-p vest_lib -- --rlimit 100`, plain output, fresh
copy and cache per arm and round) give these medians, with ranges:

| Step | Stock | `main` | Candidate |
|---|---:|---:|---:|
| A cold | 76.10 s (75.72–76.68) | 85.42 s (84.75–85.56) | 85.14 s (84.19–85.46) |
| B edit | 42.47 s (42.27–42.81) | 28.58 s (28.33–29.17) | 28.80 s (28.44–28.89) |
| A returned | 42.32 s (42.14–42.91) | 2.99 s (2.96–3.01) | 2.99 s (2.99–3.00) |
| A unchanged | 0.22 s (0.22–0.25) | 0.57 s (0.56–0.58) | 0.58 s (0.57–0.58) |
| Cycle | 161.38 s | 117.27 s | 117.44 s |

All 60 runs exit 0. Each of the 40 Sail runs prints the same lines as its
stock pair once the arm's root path and Cargo's `Finished … in` time are
normalized; lines are compared as a set. A one-round JSON check of the
candidate's returned A shows zero verifier invocations and both targets
reused, and its cost records were 26–29 s of compile against 1.4–1.7 s of
retention.

What remains of the candidate's gap, from the setup phases:

| Item | Edit | Unchanged |
|---|---:|---:|
| Runtime context and Cargo metadata (sysroot and verifier probes overlap the first metadata) | 38 ms | 37 ms |
| Input snapshot and checks | 17 ms | 16 ms |
| Targeted `cargo clean` of the edited package, dirty-receipt revisions, solver digest | 51 ms | 11 ms |
| Driver after the compiler: dependency digests (vstd `.vir` dominates), report | about 45 ms | — |
| One `sail-z3` session: handshake before Z3 starts | about 20 ms | — |
| Receipt checks with the post-run Cargo metadata (critical path) | 46 ms | 45 ms |
| Post-run snapshot and publication | 26 ms | 18 ms |

Each is a complete content read or a Cargo process the checks require.
Stock pays one Cargo metadata, Sail three (one inside Cargo-Verus, as stock).
The two small cuts are concurrent per-target revision snapshots (about 10 ms
per run) and comparing copied driver aliases byte for byte: with sail on
another filesystem than its cache the aliases are copies, and the unchanged
run fell from 0.474 s to 0.240 s (edit 1.304 to 1.075 s, five rounds).
cold-gap's daemon and shim commits (`7c91c4f5`, `fc78624c`) applied on top
measured 10 ms on the edit (six rounds); they are not part of this branch.

Raw evidence: Vest cycles `vest-evidence.tgz` sha256
`1f1677714cbe51a4d9f7f32a802968a315d42620e135a717e81cf14186c344b2`;
demo cohorts `final-evidence.tgz` sha256
`6cc9ed2239d65bf9ce49d8f91ee637c57660180188df96dd590aefd51e9fe295` (every
sample, outputs and setup-phase files of the final, retention-off and alias
cohorts, and the Vest cycle's JSON results and cost records).

## Earlier attempts

- `6f73ab9e`, 2026-10-16: required gates and full fast pass; independent review found the raw-path representative defect, corrected in `6563a170`.
- `6f73ab9e`, 2026-10-16: original standard run interrupted and superseded by the corrected-source standard run; partial logs preserved in the original extended archive.
- `6563a170`, 2026-10-16: first short benchmark had timeout-poll wall-time quantization; retained as `coarse-final-head-perf.tgz`, SHA-256 `3c5513f519edbf6fe3a68380051ab2373d25d9652d173043706977fc6a510d77`, replaced by the blocking-wait run above.

## Existing feature-alias compatibility

The actual metadata constructor's feature-alias forwarding (#78, landed in
#83) is byte-identical in the managed-overhead code and tested main-derived
`e2a37897`. Native constructor tests cover three short `-F` spellings, both
long forms, repeated selectors and the first `--` cutoff. A correctness-only
Mac no-vstd feature-gated target selects a complete Current target for all
five forms, reuses unchanged results, retains five distinct argument contexts,
and recompiles an enabled type-error control (exit101/E0308). This is target
selection evidence, with no SMT, performance or native-product claim.
Archive `feature-alias-2026-10-16.tgz`, SHA-256
`63b8d70de973695411972f6f434c45d9f1062cfec4ef859b4951a4ed62131374`.

## Snapshot-hint order (#156)

| Field | Evidence |
|---|---|
| Status | Measured |
| Source | Baseline `main` `356238cd`; candidate branch `fix/snapshot-hint-order` |
| Runtime | Sail fork `9c0ca7e7` (7d82994), pinned Z3 4.16.0; stock `0.2026.10.18.7d82994`, Cargo 1.99.1 |
| Environment | Native EC2 c7i.8xlarge, us-east-1, Ubuntu 24.04, CI `core` image, one lane |
| Scope | Current-pin Vest (`-p vest_lib -- --rlimit 100`): a cold run, then unchanged reruns, fresh copy and cache per arm and round, rotated; demo edit/unchanged/revert as a control |
| Limits | Linux only; benefits runs that follow a change in build-script execution order |

An unchanged run reuses the snapshot it takes while Cargo reads the metadata
only when its selection equals the previous run's hint. The extras in that
selection (build-script `rerun-if-changed` paths) kept Cargo's emission order,
which varies between runs. A selection equal as a set but in another order
discarded the snapshot and took a second one: 105–110 ms on Vest, measured
in-process. The extras are now sorted. The snapshot never depended on their
order, except that a symbolic link reached after its target was collected
was not recorded as a root; links are now always roots, which only adds
watched paths.

Medians over eleven paired rounds (five with a stock arm, six without), with ranges:

| Run after cold | `main` | Candidate |
|---|---:|---:|
| First unchanged (hint from the cold run's selection) | 0.683–0.696 s | 0.688–0.751 s |
| Second unchanged | 0.693–0.728 s (0.613–0.762) | 0.573–0.631 s (0.510–0.636) |
| Third and fourth unchanged | 0.618–0.621 s | 0.626–0.629 s |
| Cold | 84.4–87.2 s | 86.8–92.7 s |

Cold differs only by noise: stock's own cold ranged 71–89 s in the same
cohort, and the paired cold-only rounds gave 87.2 s against 86.8 s. The
demo control (five rounds) is unchanged: edit 2.352 against 2.363 s,
unchanged 0.191 against 0.187 s, revert 1.598 against 1.597 s. All runs
exit 0.

In-process timings on this path also show that Sail's two `cargo metadata`
calls (105–110 ms each) run alongside an input snapshot of the same length.
A metadata memo alone therefore saves 0–10 ms; it was not built.

Raw evidence: `hint-evidence.tgz` sha256
`573e1b25295a0174ccf60be255d617a4b824e27974bc17e7a0edc4d1253ca5b3`.
