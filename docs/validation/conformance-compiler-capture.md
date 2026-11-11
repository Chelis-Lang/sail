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
parallel-workspace results will be recorded before merge. Earlier real-project runs are indexed below with their exact source and limits;
none substitutes for replacement qualification.

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
| 2026-11-11 | `5f888071` | Linux standard first arm: 156 Same, one Different (Anvil's randomized autocfg instance labels), two infrastructure refusals (binary metadata probes). Not a standard pass; later stages did not run. Full records retained; replacement source has strict stdin binding and known native probe handling. | `b5c0151a0941d5e385af714c7531448ef59eae6955c4a7024722ed783bb3292b` |
| 2026-11-11 | `1dc11f5d` | Independent live review rejects unbound stdin: different piped sources can exchange 0/7 outcomes. It also reproduces the binary-probe refusal. Complete stdin/format-3 replacement and bounded recheck pending. | Stdin archive `78325be8c36964e6ea3903e80663853b9d9a4a3fe940bd46d099816c74235f13` |
| 2026-11-11 | `5f888071` | Mac generated parallel stock/shim passes; fork arm differs on a selected-package `-vV` structured JSON version field. Adjacent Rust verbose text and actual verification fields match; the initial probe rule was stricter than the requested existing structured-stdout oracle. | Archive `701193925dbfb9fc31434d3df54031f7d2004a49d80e0f94acb31f49fffc1b77` |

| 2026-11-11 | `928e1725` | Independent staged control: an earlier valid job followed by an inaccessible health record can hide a failed probe (tee exit 125) and classify Same after health restoration. Preflight now reserves its incomplete marker before checking either driver or health; replacement qualification pending. | `e7440f3dde034e80c44f9600db630cad9d9f59160096037783dd768ba3d8c66d` |

The log collision is a harness setup defect: `run_unit` formerly added one
unit-wide `--log-dir` to Cargo-Verus. The actual product driver in `adapter.rs`
binds `runtime.sidecars.join(identity.key())` and each target's own `logs`
directory. The correction uses separate compiler-owned directories through
that same packed-option helper; it does not disable logging, serialize Cargo,
ignore diagnostics or alter solver input. The failed run remains failed.

Independent raw review of the subsequent `1dc11f5d` parallel runs confirms
nine complete compiler documents in every selected run, with 1,882 functions
and zero comparison differences in both stock/shim and fork arms. Aggregate
stdout actually tears after seven documents in stock plain/Sail and fork
plain/plain-2; the other two selected streams parse all nine. Every stream
still has a complete byte/newline interleaving witness. Stale confirmation
folders from earlier failed attempts are not selected by these reports'
`plain_runs = 2` and earn no credit. This framing evidence does not qualify
the later stdin/binary changes or the full standard corpus.

The actual rustix metadata probe retained from the failed Linux arm has argv
`--crate-type=rlib --emit=metadata --target x86_64-unknown-linux-gnu -o - -`,
2,262 stdout bytes, exit zero and SHA-256
`3e7c95b09300d42c1fe9aa722143f764dc009acbcef1b4bb388fdc0c9fcdf5a0`.
Raw record archive SHA-256 is
`1172bfd28c27c549eaf87b95cb67703d540e27923cf5d73a2b8f7f11c5be868a`.
