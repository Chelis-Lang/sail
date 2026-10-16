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
