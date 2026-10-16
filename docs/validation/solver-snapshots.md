# Solver-continuation investigation (#158)

**Status: blocked and unselected.** Reports and a minimal reproduction only;
the complete prototype is archived. The strict raw gate fails on actual
real-trace fresh controls, so both live samples are skipped. No product
integration, changed-check gain or issue completion is qualified.

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

The report-only source at `519cb773` also passes the full native workspace
gate: 638 tests passed, 35 skipped, plus formatting, clippy, doctests,
rustdoc, layering, dependency policy and generated benchmark checks. This
ran on c7i.4xlarge, us-east-1, image context `3c02e445e25df766`.
The subsequent rebases onto `555b8467` and `1060a8ec` changes only this report's parent;
its own delta remains documentation and the standalone SMT reproduction.
All previously gated Rust sources and Cargo inputs are byte-identical.
The parent additionally introduces the independently qualified `actual_work`
example and the independently qualified CI image split. No same-head workspace gate is claimed after the rebase.
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
  exit. Both live measurements skip; this is the current failed gate.

Future default integration still needs a complete eligibility/output boundary,
actual runtime and both-platform qualification, full-corpus verdict/diagnostic
equivalence and a useful native gain including preparation CPU and resources.
The [report](../reports/2026-10-16-solver-snapshots.md) and proposed
[ADR 0051](../decisions/0051-exact-solver-snapshots.md) do not waive those gates.
