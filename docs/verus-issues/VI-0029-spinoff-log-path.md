# VI-0029: spinoff logging exceeds the filename limit

Status: reproduced with the selected `e5365c05` fork runtime. No workaround or
runtime replacement is selected; nothing was filed upstream.

The [minimal reproduction](../../corpus/repros/verus-spinoff-log-path/) has one
trivial proof in a nested module, with Anvil's long function name and the
`spinoff_prover` attribute. Four actual macOS correctness calls yield:

| Source / flags | Result |
|---|---|
| Long identifier, normal solver mode, AIR/transcript logs | Pass |
| Long identifier, spinoff-all, no logs | Pass |
| Short identifier, spinoff-all, AIR/transcript logs | Pass |
| Long identifier, spinoff-all, AIR/transcript logs | Fail: `PanicOnDropVec` panic at verifier line 451 |

The selected runtime's complete manifest file map remains identical before and
after all four calls. Raw result SHA-256:
`7ad7b9cb26fd14e404f0f87d664c2998e35ebff7d6bd6ddddba19c9b3da4c507`.
These are correctness controls, not native performance or Anvil acceptance.

The normal log basename contains the module and function once. Spinoff appends
the complete function path again. The actual short spinoff `.air` basename is
151 bytes; substituting the original long name in both positions produces the
source-derived 311-byte component. An actual file-create control rejects that
component with `ENAMETOOLONG` (macOS errno 63); record SHA-256:
`75d6486dad2b81ad095be2a48ae60afa41bb912c7590881d446abecdc73d714d`.
Its first prerequisite stat also encountered that error before the explicit
file-create control; both attempts remain recorded.

`create_log_file` returns a verifier error on the failed open. Its caller
propagates that error while an unconsumed `PanicOnDropVec` is live, whose
destructor replaces the I/O error with a panic. The fix needs bounded unique log
names and an executable regression; dropping requested logs is not acceptable.

The original native Anvil cohort also has this panic and long source-derived
log names, but concurrent worker output did not retain the first failed filename
or errno. The minimal controls demonstrate this failure mechanism without
claiming which native worker failed first. That 25-call cohort stays failed.
Spinoff adoption still requires the separate verdict/resource-limit and real
workflow gates in [the investigation](../reports/2026-10-16-solver-snapshots.md).
