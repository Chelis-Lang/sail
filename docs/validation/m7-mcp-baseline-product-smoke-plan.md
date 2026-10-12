# Named-baseline product smoke plan

Status: executable addon prepared; seven pure controls pass, native smoke not
executed. The baseline feature's current integration has
not yet passed its native gate or received a fresh installed-product identity.
This plan supplies separate feature validation; it does not claim another pass
for the unchanged repair fixture or establish benchmark performance.

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
text/structured equality. The actual native smoke remains unexecuted. Only the
fresh product's eventual run can qualify this feature.
