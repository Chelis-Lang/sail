# Named-baseline product smoke

Status: PASS on installed source `57a319d6d11c8a3ed76120f9c74ea6a79cf0b694`.
The actual MCP addon used a genuine completed job in a fresh repair-fixture cache.
This is separate feature validation: both retained records explicitly set
`original_acceptance_credit: false`. The unchanged original repair fixture
passes in its [own scope](m7-compiler-script-bootstrap/source-inventory.json);
this derivative does not supply another original pass, reader timing,
real-project speedup or overall M7 acceptance.

## Executed scope

The host controller exited zero in 104.851773166 seconds; its native phase took
98.98996608599919 seconds. Inside that phase, the derived fixture took
95.22979050100002 seconds under the original 120-second clock, with 312 original
fixture operations. The addon took 0.5058105420011998 seconds and recorded eleven
operations: nine MCP exchanges and two CLI reads. The longest MCP exchange was
78.365125 milliseconds; this small functional smoke is not a latency census.
CLI operations retained their 30-second limit and addon MCP responses their
five-second complete-decoding limit, both capped by the original remaining clock.
The 300-second native supervisor and 360-second host envelope include setup and
retention; they did not replace the original proof/tool deadline.

Actual save/list/get replies and both CLI reads contained the same complete
Historical snapshot, matching original context, input revision, establishing
run and target reports. The literal `--json` name worked; the sorted list was
`["--json", "before-repair"]`. Saving again returned MCP `isError: true` and
`SAIL0905: baseline already exists and is immutable`. A normally restarted MCP
client read the identical snapshot. The baseline remained exactly 3,025 bytes,
SHA-256 `4f29f51e69ecef24938ea93f181854cd24e74075555ba4a5ef8a53f69d4adeb1`.
All successful tool replies matched their complete JSON text content.
This checks client restart, not daemon restart or pruning; the latter remain
separate daemon integration controls.

Source/helper/addon/CLI/shim pre/post seals match. Both fixture and addon
cleanup errors are empty, fixture progress failures are empty, uncertainty is
empty, and the container identity/birth is unchanged. The recorded known owned
groups have no survivors, unavailable observations or terminated-unreaped rows;
this is scoped closure, not a whole-system orphan census. The operator-retained
archive contains matching fixture/addon/derivative records and both CLI outputs.
Independently removing the single hook restores the exact original fixture hash.

| Retained file | Bytes | SHA-256 |
|---|---:|---|
| `controller.json` | 1,286 | `af9ab509673f710e7859555705c6facd402393c71e2a12d8415624b460cdf2a1` |
| `result.json` | 213,873 | `29c58649769958208ce8bd85638c5a8e87280bc23b652bd93a2a09bc8979b715` |
| `baseline-feature-smoke.log` | 117 | `c04243bf90f20daea45899b082177a73787ca202e169d6264908d59856c494b7` |
| `native-raw.tar.gz` | 106,561 | `c9f7891821672f56bb9d1b285373b5da2cd0ce674cc96806f51a1a8979871321` |
| Derived fixture | 21,854 | `4bce993fe00d4014c6479045cd64caf6d06287b204ddd995d56b5837fbdfadc2` |
| Addon script | 13,447 | `dd3653cc9fe456f79871a15d35cff6c85090493d279588dd91c7ee6d477e4fe3` |
| Installed `sail` | 77,283,160 | `08038d3042a4109b7e83fa6097ba8f351ad0211db0980ea1b965733a2c252d3f` |
| Installed `sail-z3` | 11,001,744 | `f15b4ef2aae8928dacb262cb7d09b32effe83c208e0c9332cde38a24880a7f92` |

The product identity record SHA-256 is
`bfa621ca2c19b219ad2ee15f17546eab8c00e50e653f0138fd5c9adfaf98d671`;
its source archive is `6f49612cba4b396187c32e66be7424dd311c8cb1e0cfe6814d1f66dcd0c93245`.
The original setup selected Verus source `426d8b01e7ffb910a36c15e1f870ff985bb61eef`,
Rust `1.98.1` and engine manifest
`54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d`.
Execution was Linux amd64 Docker emulation on arm64 macOS under development load;
no causal comparison with another product or machine is claimed.

The production argument extraction in `3a3657f` preserves every original
baseline command, literal-name separator and output option. The assertion
extraction in `e65f948` preserves the saved-baseline equality oracle;
`bdc20a6` fixes its boxed-value comparison with `loaded.as_ref()`. The failed
strict integration preceding that correction remains a failure, not test credit.

## Fixture and lifecycle

`scripts/m7-baseline-product-smoke.py` implements the derivative. Once an exact
fresh tested product and selected engine/source are bound by the existing product
records, execute it with the original fixture arguments:

```sh
python3 scripts/m7-baseline-product-smoke.py \
  --sail "$SAIL_PRODUCT/sail" --engine-build "$SAIL_ENGINE_BUILD" \
  --verus-source "$SAIL_VERUS_SOURCE" --out "$SAIL_EVIDENCE/baseline-feature" \
  --timeout 120 --environment-label "$SAIL_ENVIRONMENT_LABEL"
```

The output directory must be new. The addon retains its verified derived source,
pre/post fixture/helper/script/CLI/shim seals and `derivative-record.json`.
The original engine-install summary retains the selected source commit and
engine manifest hash; external product/runtime records still supply the complete
selected inventory binding. `baseline-addon/summary.json` retains full tool
requests/results and received JSON-RPC messages. This is a derivative feature
scope, never another unchanged-original acceptance pass.

Use a separately retained derivative of `ci/test-repair.py`, whose original
SHA-256 is `9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7`.
Leave the repository fixture unchanged. The derivative adds one explicit hook
after `assert not progress_failures, progress_failures` and before
`summary["passed"] = True`. Preserve every original assertion and operation,
including its declaration transition and single-driver prerequisite. No new
proof submission, warmup, retry or synthetic report is needed.

The hook receives the actual `good_job`, original admitted `good` result,
project, cache/socket environment and evidence directory. That completed job
must have passed the existing Current, accepted, complete-full and single-driver
assertions. Later source changes do not turn its saved evidence into a claim
about current code. The earlier repair run's temporary cache was removed;
this scope must create its own fresh cache and genuine job through the original
fixture. No old receipts or baseline files are copied.

Run an additional owned `sail mcp` process in the same project and environment,
with transcripts in a separate evidence subdirectory outside watched inputs.
Reuse the complete-frame deadline read and owned cleanup pattern from
`ci/test-mcp-live-service.py` (SHA-256
`e6d44009fa4485056fc53ab18ca58c38c2d46fd2d6de0da2cf70dbc9c3521ae5`).
Initialize and notify that process normally. Each added MCP exchange has an
absolute deadline of at most five seconds, including complete decoding;
each CLI operation retains its thirty-second limit. Both are further capped by
the original fixture's remaining 120-second clock. The hook must not reset or
extend that clock. Close and reap its MCP process in `finally`, then let the
original fixture perform its unchanged actor and evidence cleanup.
The added read refuses a diagnostic frame above 16 MiB (with at most one
64 KiB read before refusal) or more than 64 received messages per client.
Artifact sealing uses a fixed 1 MiB read buffer with 256 MiB per bound input and
32 MiB per baseline admission; baseline sealing also checks the original clock
between reads. These are fixture admissions, not new product limits, parsed-heap
or RSS guarantees. Existing pipe-write/filesystem blocking and original cleanup
limitations are retained.

## Required tool calls and assertions

1. Read `tools/list` and require the actual `save_baseline`, `list_baselines`
   and `get_baseline` definitions, including their name/job schemas and read-tool
   annotations. For each successful tool call below, require equality of its
   structured content and its complete JSON text content.
2. Call `save_baseline` with `{"name":"--json","job_id":good_job}`.
   Require `isError:false` and the version-1 `baseline` response, matching name
   and job ID, complete Historical project result, and the original context,
   input revision, requested targets, establishing runs and full target reports.
   Preserve provisional focus evidence as provisional; do not promote it.
3. Call `list_baselines` with `{}`. Require sorted unique names containing both
   the original fixture's `before-repair` and the new literal `--json` name.
   Call `get_baseline` for `--json`, then CLI
   `baseline get --output json -- --json`. Require entire parsed response
   equality with the save response, not only matching counts or verdicts.
4. Record the named baseline's bytes and SHA-256. Save the same name through MCP
   again; require `isError:true` with stable `SAIL0905` immutable-name refusal.
   Repeat get through MCP and CLI and require complete equality and unchanged
   stored bytes. A refusal, timeout or incomplete response stops the scope.
5. Normally close the added MCP client and open one fresh client against the
   same owned daemon/cache. Get the baseline again and require the same full
   Historical response and text/structured equality. This checks client restart
   persistence; it does not claim daemon restart or job-pruning coverage, which
   remains the separate daemon integration test's scope.

Freeze the derived source and exact insertion diff, addon/helper bytes, product
and shim bytes, selected engine/source identities and invocation before running.
Retain each full request/reply, CLI streams, baseline pre/post seal and cleanup
outcome even on failure. A pure derivation control must prove that removing the
one hook restores the exact original fixture bytes; response controls must
reject changed provenance, non-Historical results, incomplete text, overwrite
success and changed stored bytes. Seven pure controls in
`scripts/test-m7-baseline-product-smoke.py` pass, including exact hook restoration,
changed/missing/duplicate source refusal, identity/provenance checks and complete
text/structured equality. The bound native execution above passes the feature
checks without changing the original fixture bytes or supplying transferred
credit to other scopes.
