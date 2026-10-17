# Solver-continuation investigation (#158)

**Status: blocked and unselected.** The standalone one-use Python research
prototype is available under `native/sail-live-prefix`; no product path uses it.
The original exact-byte experiment fails. A separately authorized numeric-time
experiment has one historical IronKV success, then strict workload refusals;
Anvil never reaches live serving. No application gain or issue completion is
qualified. The earlier C/Rust snapshot integration stays archived.

## Best evidence and actual scope

Source branch `perf/solver-snapshots` ends at `b9345c2a116693f8b6a7b6c23ee3d812ef04dec5`.
Its complete Git history is retained in `solver-prototype-b9345c2a.bundle`
under the external `live-prefix-review-2026-10-16` evidence set. Restoring
that bundle reproduces the actual C/Rust/Python research source; none of the
shim/API/benchmark integration is part of this report's source change.

Native real controls: c7i.8xlarge, us-east-1, Xeon Platinum 8488C, 32 logical
CPUs, Linux `7.0.0-1014-aws`. CI image context `3c02e445e25df766`, actual image
ID `03d6865e6602ae702f480b4396543c5c1c51fe07f88fd80faa210880a66abef6`.
Pinned Linux Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
Core SHA-256 `a9704a14845e69c61fca6190b4fb986500612165613d68df99f3ea56918a7cf0`;
driver `cadb166f602bc697b60e8c4c55202b4ff7f02ab6f58fc3b0c13ec061b7e99e67`;
real controller `606910691e32f53957c1f58bbbf0dd58860ab08d6fcc65217608a1004f3cc157`.

- All 28 bounded native controls pass: exact tiny continuations, actual
  foreign-image refusal, retained timer-worker process, one-use claim,
  timeout/refusal/fallback and input/resource limits. This is core scope.
- Six complete fresh runs: three IronKV and three Anvil. Every solver exits
  0, stderr is empty, all check replies match, and all actual images verify.
  Full stdout differs in `:time`; both fresh gates fail. One IronKV sample
  additionally retains the memory-observer `ESRCH` exit race. Both live
  samples are explicitly skipped; parent exits 1.
- Real limits: 300 s preparation/serving/fallback/oracle, 600 s TTL, one slot,
  16MiB input, 1GiB sampled RSS and a separate 2GiB container cap. Every raw
  stream/status and refusal remains. No masking or normalization is applied.

| Artifact in `live-prefix-review-2026-10-16` | SHA-256 |
|---|---|
| `solver-prototype-b9345c2a.bundle` (complete source history) | `c6fc3d1c80e793776aed3d9e43319e5ff61d61ed6f7c4f1cf417d8a95c57f65b` |
| `native-bounded-28-result/native-results/manifest.json` | `e42238f526ce2e1c8d695bfc00fdfa36ea971bb8ce9e4b5591112ab0df406627` |
| `real-trace-result.tgz` (source, inputs, every raw sample/manifest) | `4b43d3a60365266f3f9be19d54e350a9654f07fce4e0395fe619c430f48ac657` |
| `real-trace-result-summary.json` (original-byte difference locations) | `b05dbaf9eca11fb942db7e591aba4a844a9dc062b04db8abc872b94a6b19b405` |

Historical report-only source at `519cb773` passes the full native workspace
gate: 638 tests passed, 35 skipped, plus formatting, clippy, doctests,
rustdoc, layering, dependency policy and generated benchmark checks. This
ran on c7i.4xlarge, us-east-1, image context `3c02e445e25df766`.
The historical rebases onto `555b8467` and `1060a8ec` change only that report's parent;
its own delta remains documentation and the standalone SMT reproduction.
All previously gated Rust sources and Cargo inputs are byte-identical.
The parent additionally introduces the independently qualified `actual_work`
example and the independently qualified CI image split. No same-head workspace gate is claimed for those historical rebases.
The gate log/source bundle is retained in the external frontend evidence
`raw-input-probe/m8-native-gates-complete.tgz`, SHA-256
`de98a5f61e703ce3c1811742cdff0eff3a807bd96895cc1a129cbf5fc17b10ac`.

Historical native evidence is separate under `takeover-2026-10-16`:
`m112a-results.tgz` SHA-256
`bcccdcc149d31cb6bef770e5b760897c085d2ebbf7e917d83552c385ffcef573`;
actual binaries/preload archive SHA-256
`68294281141178e832e17a15a8323e613fb16c068ca90e6572afaa1456b0c309`.
Masked historical statistics never qualify complete raw equality.

## Indexed failed or interrupted attempts

- `88076b62`, 2026-10-16: genuine NUL framing changes real input; original
  parked-count admission exceeds its cap. Deterministic controls fail.
- Earlier preload hardening, 2026-10-16: a missing provenance directory
  failed to establish a solver start; incompatible preload stderr also
  escaped before admission. Corrected archived controls remain separate.
- Inherited native replay, 2026-10-16: raw statistics disagree, including
  `num-allocs`; masked successes and discarded stderr are not acceptance.
- Inherited Anvil managed runs, 2026-10-16: partial selectors under `verify`
  refuse before verification; no performance claim from those samples.
- `9051aa78` source digest, 2026-10-16: actual executable-path ABA runs a
  foreign image and returns `sat` instead of pinned `unsat`; source rejected.
- First native live-control setup, 2026-10-16: wrong solver path gives
  `FileNotFound`; retained separately from the corrected 23/28 controls.
- `60691069` controller digest, 2026-10-16: all six real fresh traces agree
  on replies but fail literal clock equality; one observer also refuses on
  exit. Both live measurements skip; this is the original exact-byte failed gate.

Future default integration still needs a complete eligibility/output boundary,
actual runtime and both-platform qualification, full-corpus verdict/diagnostic
equivalence and a useful native gain including preparation CPU and resources.
The [report](../reports/2026-10-16-solver-snapshots.md) and proposed
[ADR 0051](../decisions/0051-exact-solver-snapshots.md) do not waive those gates.


## Named numeric-time-only research experiment

The owner authorized a separate experimental policy allowing only bare numeric
`:time` values to vary in complete top-level flat numeric statistics replies.
Every other response byte, including whitespace, other statistics, models,
errors, stderr and status, remains exact. Original queries and all response
bytes stay retained; raw gate fields stay strict. The policy has a separate
pool binding and is opt-in in this standalone research driver. It does not
change Sail's default keys, oracle, daemon, shim or verdict model.

The fixed archived edited traces are IronKV (2,464,885 bytes, 23 prior/1 remaining
check, SHA-256 `c6744afeba46d2277efeadf69af43f8a5ebbeca21a3266d26632403985f6cf87`)
and Anvil (6,285,771 bytes, 7 prior/2 remaining checks, SHA-256
`148bdb444d0b31f998ad989b3919ef86ad90444c40d6febed845760db531e7cf`).
Each attempt requires three fresh runs before one eligible live continuation;
failed samples are retained, never replaced. These selected solver traces do
not establish an application hit rate or a plain-Verus benchmark.

All attempts below use native c7i.4xlarge, us-east-1, Linux
`7.0.0-1014-aws` and pinned Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
Source/runtime identities are checked before and after. The final source is
`6c89a9e92d5cc749006b4ddae03ed67836c5b191`; importing its standalone Python
files into this PR changes no compiler, solver, input or production code.

| Attempt | Actual result | Measured cost and limits |
|---|---|---|
| `133b4721`, first named-policy pair | IronKV: three fresh runs and one continuation pass, 24 `unsat` each; same prepared/serving PID and no fallback. Anvil: fresh RSS observer refuses; no live sample | One IronKV sample: preparation 2.132 s, foreground 0.322 s versus guarded fresh 2.196 s (6.83× foreground ratio); total CPU 2.171 s versus fresh 2.167 s. This spends preparation work and saves no CPU |
| `8b7c9922`, lifecycle preflight | 8 of 9 native controls pass; the mandatory gate fails: bounded attempts do not observe the required pre-waitable window. Neither dataset runs | Functional controls actually pass; the later archive checkpoint is not reached. No timing claim |
| `6c89a9e9`, final terminal-only policy | Native lifecycle controls pass 9/9. IronKV fresh gate passes, but the live monitor refuses `ESRCH` and falls back. Anvil's third fresh run (`fresh2`) refuses while the held pidfd is nonterminal; no live attempt | IronKV preparation 2.287 s; foreground including fallback 2.673 s versus guarded fresh 2.352 s. Total solver CPU 4.658 s versus fresh 2.322 s, including 2.315 s fallback CPU |

The final Anvil fresh walls are 59.181/57.367/57.120 s and CPU times
59.134/57.322/57.078 s. All three actual solvers exit 0 and return 9 `unsat`;
only numeric time tokens differ. The third fresh run (`fresh2`) still fails because its observer records
`ESRCH`, after 5,179 valid samples, while the continuously held owned pidfd is
not terminal. A successful eventual exit cannot override this refusal. IronKV's
three fresh walls are 2.379/2.359/2.338 s; every run returns 24 `unsat`. Its live
monitor is unchanged and remains strict. The final aggregate exits 1 with both
projects failed/refused and `qualified=false`, `selected=false`. Genuine native
control success does not override these actual workload failures.

The fresh-process classifier supports only an already-terminal continuously
held owned pidfd, original PID/starttime binding and prior valid RSS; it retains
the original read error and subsequently requires bounded exit 0 and complete
I/O. Nonterminal pidfds always refuse; no `PF_EXITING` or eventual-exit exception
exists. The native positives prove genuine smaps failures after reaping and
while unreaped but already terminal, with seven live/foreign/status negatives.
No pre-waitable-window claim follows. Local parser/FD (23), owned-child lifecycle
(15) and stream-retention controls use explicit kernel/image doubles; actual
native 9/9 and original core 28 controls are separate evidence. The same native
attempt also runs the 31/8/9/12/2 policy/driver/parent/readiness controls.

External evidence is under `live-prefix-time-only-native-2026-10-16`:

| Artifact | SHA-256 |
|---|---|
| First pair final archive, aggregate failed | `59fff5bb03ae54fda0a4b0b60aeb5e8ff1767a837d132f6c66b199137dff6938` |
| First pair actual outcome/cost summary | `7a00d113e3eb8073d2d75c1f47d4181853a2c9417504333843bbdea16e28ccf4` |
| Pre-waitable pretrace final archive, failed | `b651f57f4d6d0bec55eab54ef611967da63bf35c1439e88a24ee61ec10c5e913` |
| Final terminal-only controls archive | `2d6e0685ea59c76cce614a114ed9433fa91f57f9e1f436f20273613a9fe29f98` |
| Final IronKV archive, refused/fallback | `dc3b4698cdd77d42286901221f12b3033add6a65e403d67c513697265d6c80f9` |
| Final Anvil archive, refused fresh gate | `a07eb7d8e23a681bd73b668fb05de16fa9f234ebde2c1f8ad9548bf9f37a755f` |
| Final aggregate archive | `0bc3fb9889194ac09ddd15e522a12885d667686e78dc28241dec495542ceea80` |
| Final observed outcome/cost summary | `22f72ea410cfcf55c4362b08faac8e7e6fae40c38a1e31557522489565715122` |

Additional failed/rejected attempts stay indexed: `6001346c` local readiness
latched across additional incomplete output (fixed by `ebbf09a5`); `c3c99584`
parent trusted a true manifest despite actual exit7 (rejected before native,
then fixed); `362ee196` packet lost native-control failure streams (unrun,
fixed by `8b7c9922`); first terminal-only timeout fixture waited before testing
its timeout (local setup corrected). The native failures in the table remain
failures. No further lifecycle sampling or classifier relaxation is selected.
The useful result is a narrow one-sample foreground feasibility observation,
with a demonstrated resource-observation reliability limit and real fallback
cost. Default integration and full-corpus acceptance remain unimplemented.

The extracted main-based research source `fee8abff` passes the native Linux
workspace gate (655 tests, 35 skips, two doctests) and the macOS host gate
(644 tests, 35 skips, clippy). Source inventories match before and after both
runs. These workspace checks qualify the standalone research packaging, not
the failed solver-continuation experiment or default-mode integration. The
following records are separate from the workload evidence above:

| Artifact | SHA-256 |
|---|---|
| Native research workspace gate, `store-containment-integration-joined-2026-10-16/native-results/research-gate.tgz` | `339db7b82064f408cbaf9ae643da91757764d972a56ace0f233504720f1bbf39` |
| Joined final source/runtime guards, same evidence set `final.tgz` | `85a913c6a411fd2f06570b83004c0085cfe7743075c196280abde6c995bd7314` |
| macOS research host log, `live-prefix-research-macos-2026-10-16/host.log` | `7a397619c25714f91010f632fe49651658427543f8b98bb34e70b13548f34156` |
