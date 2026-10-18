# Focused module work observation

This developer example is an untimed, explicitly partial fixture reader for
`vdeployment_controller::proof::liveness::resource_match` in Anvil. It does not
run Verus, establish whole-target Current, authorize proof reuse, or change the
existing `actual_work` complete-run guard. No spinoff mode is selected.

```sh
cargo run -p sail-conformance --example focused_work -- \
  --fixture FIXTURE --authority-sha256 FROZEN_AUTHORITY_SHA256
```

The controlled caller must independently freeze `authority.json`, source/provider/
lock/runtime/executable inventories, actual invocation PID/argv/exit and complete
stdout/stderr, and an exhaustive session census. This reader cannot prove those
process or compiler-consumption facts from a self-authored JSON document. Its
input authority is therefore a prerequisite, never a replacement for native
caller qualification. All paths are literal owned regular files below FIXTURE;
body SHA-256 values must match. Individual files are limited to128MiB, and the
input/session lists to16384 entries each; the eventual native caller must also
bound total capture bytes and time. No native queue is authorized by this example.

Both the observed invocation and genuine pinned stock oracle must use exactly one
literal `--verify-only-module MODULE` pair. Alternative selectors, equals forms,
duplicate selectors and every recognized `--no-verify` form refuse. A nonempty full function/module/mode multiset,
every verdict, exit, and complete diagnostics must agree through the existing
conformance comparator. Failed `fail` cases remain failed compiler runs; matching
failure is never a verified result. `fail` requires both nonzero exits, explicit
failed verification with positive error counts, and an actual failed function.
The other four cases require zero exits, successful verification with positive
verified counts, no errors or internal-error flags, and no failed functions.
Agreement on the wrong case outcome refuses. The canonical catalogue is A, body-early,
body-late, fail, fix. The caller must supply the real frozen edits and stock
expectations separately for every case; the local test fixtures are doubles.

Each distinct session is joined by its literal input digest, preserving repeated
inputs and PID multiplicity. Cached sessions need their actual strict provenance
record. Direct captures need complete existing trace start/end records bound to
PID, pinned solver body and exact raw input; incomplete or ambiguous capture
refuses. Every solver-started session counts every actual check, including cached
prefix checks replayed in that child. Newly answered checks and cached answers
are separate counters. The result always says `partial:true`, identifies its
fixture hash, and never emits Current or a proof receipt. Refusal is `SAIL0931`.

The pure transcript/session/count join is shared with `actual_work`; its complete
Current/run/invocation gates remain unchanged. Raw SMT, outputs, models,
statistics, paths and diagnostic bytes are not normalized.

The proposed next native scope is correctness first: five frozen cases across
stock, normal fork and spinoff fork, each fork both uncached and cached, using the
same genuine e536 runtime/body guards. Exact capture/command authority still needs
review before execution. No native Anvil result, gain, public-runtime selection,
full local gate or default adoption is claimed. The old real Iron finite-rlimit
verdict disagreement remains a default-adoption blocker even if Anvil later wins.

The initial a5 source checks passed2 pure count tests,2 complete-observer guards,
4 partial catalogue/refusal tests, and exact-example lint; their original record
remains `spinoff-anvil-focused-source-2026-10-18/final-source-checks.json`, SHA-256
`734863c217a9c78b089b2ebcfd5a8145692d79eabdcde1fc637b4bf440757451`.
Independent review then found equals-selector and wrong-catalogue-outcome gaps.
The correction passed the same2 pure/2 complete-observer tests,6 focused tests
including both negative families, and lint for both exact examples. Its record is
`spinoff-anvil-focused-source-2026-10-18/selector-outcome/final-source-checks.json`,
SHA-256 `f3a6a7144f0c6c533708321e9e02c4a93af652c2501614489d50b757a9dc8ef4`.
These macOS source checks executed no real Verus/Z3 and qualify no native result.
Earlier lint failures remain preserved.
