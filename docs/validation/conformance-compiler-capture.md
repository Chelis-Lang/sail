# Cargo conformance compiler capture

| Field | Current state |
|---|---|
| Tracking | #632, #716; parallel scaling remains #629 |
| Design | [ADR 0113](../decisions/0113-conformance-invocation-capture.md) |
| Runtime | Existing primary stock and fork pins; no runtime/solver changes |
| Implementation | Per-invocation capture in plain, shim/cache and engine arms; explicit project and nonempty selection guards |
| Status | Implemented, qualification pending; no parallel conformance or scaling acceptance claim |

Focused native Mac development checks pass 50 library tests, five harness
command tests and three actual delegate-process controls, plus clippy for
`sail-conformance` and `sail-verus` with all targets. The process controls
check original stdout/stderr/status, a killed delegate's incomplete marker,
corrupt recorded lengths and a closed forwarding pipe. The exact candidate
identity, complete Linux/Mac gates, raw evidence hashes and real-project /
parallel-workspace results will be recorded before merge. An actual Vest
harness run is in progress; no result is claimed here.

The original strict VI-0039 failures remain in the
[reproduction](../../corpus/repros/verus-parallel-json/README.md). This
implementation does not repair Cargo's public stdout framing. A line torn
within a write, a missing/extra invocation, any unmatched probe, incomplete
capture or exhausted interleaving search remains a failure. Original
aggregate streams and every compiler record are retained. This gate is
separate from broad-corpus and scaling performance qualification.

The consumer verification skill has no command or default-verification change;
this is contributor tooling. Documentation changes are ADR 0113, the error
reference, testing guide, changelog and this qualification record.

## Failed implementation attempts

| Date | Source | Result | Raw log SHA-256 |
|---|---|---|---|
| 2026-11-11 | `72aa59ac` | Parallel process tests: two pass, one refuses scratch creation with `AlreadyExists`; fixture now adds a process-local atomic sequence. No host gate credited. | `1110b6c06f4007a4a13612514efab722136c0257c4ff60286c795816ecf16fee` |
| 2026-11-11 | `72aa59ac` | Independent native review rejects named build-script probe identity: two adversarial controls demonstrate source-specific statuses can swap. Complete arguments now bind every invocation; replacement qualification pending. | `da5ad53f887d132225e847c10ca998338bbfaa36c053f79a5df5b761cc3888c4` |

| 2026-11-11 | `e2d210ed` | Independent live staged controls reject omitted physical packed-option channels / outcome reassociation and a failed reservation accepted after a prior probe. Format-2 requested/effective channels and a separate capture health record are implemented; replacement qualification pending. | `7c5bb9b7ccf66881117908c7f5e1220d034373870bae1d10f405eab8fe5b9a58` |
| 2026-11-11 | `e2d210ed` | Real Mac parallel stock-versus-stock/shim managed arm fails on shared harness log directory (`multiset.smt_transcript` removal); original complete report/captures retained. No parallel pass. | Archive `d629038eb946a106138fd8ffeb2c8161865ee113d66e490261dcc609fefdf048` |

The log collision is a harness setup defect: `run_unit` formerly added one
unit-wide `--log-dir` to Cargo-Verus. The actual product driver in `adapter.rs`
binds `runtime.sidecars.join(identity.key())` and each target's own `logs`
directory. The correction uses separate compiler-owned directories through
that same packed-option helper; it does not disable logging, serialize Cargo,
ignore diagnostics or alter solver input. The failed run remains failed.
