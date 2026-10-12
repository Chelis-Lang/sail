# Repair workflow walkthrough

This walkthrough covers the implemented experimental workflow. Current
`387ba80` passes the [complete local gate, original solver suite, product
installations and unchanged CLI/MCP repair fixture](validation/m7.md).
The repair fixture completes in 90.588 seconds with 299 operations under the
original 120/30/5 limits. Named-baseline feature smoke passed separately on
`57a319d`; it supplies no transferred current-product reader or performance credit.
The [b895 reader/retained pass](validation/m7.md)
remains historical: both unchanged collectors preserve full Current graphs and
p95 below 200 ms. The original 387 reader workflow fails Python Result p95 at
436.008 ms against the unchanged 200 ms bound; all 200 samples retain Current
evidence and the same continuing verifier. Rust and retained CLI/MCP checks
are unattempted, so current reader acceptance remains unfinished. Real-project M7 speed and complete standard/cache
coverage remain unfinished.
See [the roadmap](ROADMAP.md),
[known issues](known-issues.md) and [acceptance requirements](bench/m7-repair-acceptance.md).
The commands and response fields were checked against the CLI/MCP implementation;
this documentation change does not constitute a new proof run.

Initial dependency regeneration can still leave proof evidence Unknown.
Optional [ordinary-build recovery](managed-cold-start.md) passes its local
controls and changes only dependency-contract retention. It supplies
no proof acceptance or guaranteed first-run success; unsupported inputs retain
original cleaning and full validation.

## Install and select the engine

Start with the pinned Verus/Z3 toolchain and a project whose ordinary
`cargo verus verify` command works. From the Sail checkout:

```sh
cargo xtask fetch-verus
cargo xtask build-verus-engine
cargo build --release -p sail-cli -p sail-shim
export PATH="$PWD/target/release:$PATH"
sail engine status --output json
```

The source-build command installs the selected engine in the same Sail cache
that `engine status` inspects. Import and execution must use that cache too;
relative `SAIL_CACHE_DIR` settings follow the invoking project directory.
The selected runtime is experimental. It runs stock lowering with a focused
phase in the same compiler callback and prepares no lowering for reuse. The [b895 installed repair and reader checks](validation/m7.md)
pass; the [730 reader workflow](validation/m7.md) remains a separate historical pass.
M7 real-project performance remains unqualified. See
[engine qualification](../engine/verus/README.md) and the
[repair guide](proof-repair.md).

`fetch-verus` installs artifacts but does not put `cargo-verus` on `PATH`:
configure your ordinary stock launcher on `PATH` or through `SAIL_CARGO_VERUS`.
Keep `sail` and `sail-z3` side by side. Linux freshness validation also needs Python 3; see the
[freshness contract](freshness.md) for kernel observation, read leases and
unsupported filesystem coverage. macOS uses a conservative fallback.

To import an existing build instead of compiling it:

```sh
sail engine install --from path/to/engine-build --output json
```

Import validates the exact selected patch, pinned source/toolchains, manifest and
artifact bytes. It retains a monitored link to that directory, so keep the build
available. Unrecognized or conflicting builds produce `SAIL3120`; an arbitrary
stock build is not an interactive engine, and only the selected patch is
recognized.
A successful install/status check establishes runtime identity, not the unfinished
M7 acceptance claims. Keep the ordinary stock launcher available for fallback.
Before the first project submission, run `sail engine status --output json` from
that project's directory with the same cache environment as the MCP launcher.
This catches a different cache selection before a repair request.

## Establish a baseline with complete full validation

For an existing library, substitute its package and exact crate-qualified
function below. This small example uses the pinned published `vstd` dependency:

```sh
mkdir -p ../repair-demo/src
cd ../repair-demo
cat > rust-toolchain.toml <<'TOML'
[toolchain]
channel = "1.98.1"
TOML
cat > Cargo.toml <<'TOML'
[package]
name = "repair_demo"
version = "0.1.0"
edition = "2021"

[workspace]

[package.metadata.verus]
verify = true

[dependencies]
vstd = "=0.0.0-2026-10-04-0306"
TOML
cat > src/lib.rs <<'RUST'
use vstd::prelude::*;
verus! {
    pub proof fn selected(x: bool) ensures x || !x { }
    pub proof fn other(x: bool) ensures x == x { }
}
RUST
cargo verus verify --package repair_demo --lib
```

The demo pins Verus's Rust toolchain from `corpus/toolchain.toml`, separately
from the Rust toolchain used to build Sail itself.

Keep capture files outside the project package roots and declared inputs. Sail
observes those roots recursively: writing an event log or result file there
while verification is running is an input change and can correctly leave the
proof Unknown. This demo uses a fresh sibling directory for captures; keep it
available for the later commands.

Submit one exact repair scope and retain its ID:

```sh
EVIDENCE_DIR=$(mktemp -d "$PWD/../repair-evidence.XXXXXX")
repair_check() {
    sail repair-check --package repair_demo --lib \
        --function repair_demo::selected --engine interactive \
        --background --output json
}
repair_check > "$EVIDENCE_DIR/before.submit.json"
BEFORE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$EVIDENCE_DIR/before.submit.json")
sail jobs events "$BEFORE" --follow --output jsonl > "$EVIDENCE_DIR/before.events.jsonl"
sail jobs result "$BEFORE" --output json > "$EVIDENCE_DIR/before.result.json"
python3 - "$EVIDENCE_DIR/before.result.json" <<'PY'
import json, sys
value = json.load(open(sys.argv[1]))["value"]
assert value["job"]["state"] == "completed" and value["job"]["error"] is None
assert value["repair"]["accepted"] is True
assert value["repair"]["batch"]["full"]["currentness"] == "current"
print("Complete Current full validation accepted this baseline")
PY
sail baseline save before-repair --job "$BEFORE"
sail baseline list --output json
sail baseline get before-repair --output json
```

Continue only after the checks succeed. Background submission returns a **Status**
with `id`; its successful exit only acknowledges submission. `jobs result` returns
an envelope whose repair bundle is `value.repair`. A successful fetch is not a
successful proof. Without `--background`, `repair-check` returns the repair bundle
directly and exits 0 for acceptance, 1 for rejection or 2 for a command/service error.

Focus evidence remains provisional. Acceptance requires independent complete
Current full-target success, with no selection error. Cold dependency verification
can add a `vstd` compiler invocation: `compiler_invocations` is an observation,
not a promise of one driver. Unknown/Historical, incomplete coverage or a failed
full proof cannot authorize a repair. Inspect the retained notes, including
`SAIL3114`, before deciding what needs fixing; do not retry blindly until accepted.
A named baseline is immutable historical reference evidence; use a new name for
another snapshot. `baseline list` returns the saved names. To compare two explicit
completed jobs without a named baseline, use
`sail compare BEFORE_JOB_ID AFTER_JOB_ID --output json`; the earlier job comes
first. Neither form establishes current proof evidence for an edited workspace.

## Resume events and cancel deliberately

Fetch a page and use its returned cursor:

```sh
sail jobs events "$BEFORE" --output json > "$EVIDENCE_DIR/page.json"
STREAM=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["next"]["stream_id"])' "$EVIDENCE_DIR/page.json")
AFTER=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["next"]["after"])' "$EVIDENCE_DIR/page.json")
sail jobs events "$BEFORE" --stream "$STREAM" --after "$AFTER" --follow --output jsonl
```

JSONL contains observation and cursor/control records. Preserve the cursor and
inspect `gap`; a truncated or pruned journal cannot be treated as complete.
A terminal stream is not proof acceptance: fetch the final result. Journal limits
and exact identities are described in [the repair guide](proof-repair.md).

Ctrl-C on an events follower only stops observation. To cancel an owned running
job, use `sail jobs cancel JOB_ID --output json`. Foreground `repair-check` cancels
its owned job on Ctrl-C. Background jobs survive client disconnects; unfinished
jobs after a daemon restart become infrastructure failures. Identical established
in-flight requests can share a job ID, so cancellation applies to that shared job.

## Detect a focus pass with a full regression

Introduce a regression outside the selected function:

```sh
cat > src/lib.rs <<'RUST'
use vstd::prelude::*;
verus! {
    pub proof fn selected(x: bool) ensures x == x { }
    pub proof fn other(x: bool) ensures x { }
}
RUST
repair_check > "$EVIDENCE_DIR/after.submit.json"
AFTER_JOB=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' "$EVIDENCE_DIR/after.submit.json")
sail jobs events "$AFTER_JOB" --follow --output jsonl > "$EVIDENCE_DIR/after.events.jsonl"
sail jobs result "$AFTER_JOB" --output json > "$EVIDENCE_DIR/after.result.json"
PACKAGE_ID=$(python3 -c 'import json,sys; p=json.load(open(sys.argv[1]))["value"]["repair"]["batch"]["full"]; print(next(t["package_id"] for t in p["targets"] if t["requested"]))' "$EVIDENCE_DIR/after.result.json")
sail explain "$AFTER_JOB" --package-id "$PACKAGE_ID" --target repair_demo \
    --kind lib --function repair_demo::other --output json
sail compare "$AFTER_JOB" --baseline before-repair --output json
```

The selected tautology can pass while `other` fails for `false`; the full result
must therefore reject this repair. Inspect `value.repair.accepted`,
`batch.selection_error`, full diagnostics and exact attributed obligations.
`explain` preserves counterexamples when the verifier supplies them; it does not
invent models or edits. Obtain `package_id` from the report, not a guessed Cargo
identifier. For a binary, select `--bin NAME` and use its reported target/kind.

Comparisons expose `value.compatible`, `after_currentness` and exact-ID transitions.
Different modes, runtime contexts or requested scopes are incomparable. Restore
`other`'s original `ensures x == x`, submit again and inspect the new full result;
a historical baseline cannot establish today's proof.

## Use the same workflow through MCP

Run `sail mcp` in the project directory, or pass `project_dir` to submission.
Job reads, cancellation, events, explanations and comparisons do not accept
`project_dir`; they resolve the service from the MCP server's working directory.
Use a server started in this project, or configure the same absolute
`SAIL_CACHE_DIR` (and `SAIL_DAEMON_SOCKET`, if overridden) for submission, CLI and
MCP. A relative cache selected from another directory can make a valid job ID
unavailable to those reads. Baseline operations use that same service.
After normal MCP initialization, these are `tools/call` parameter objects:

```json
{"name":"start_repair_validation","arguments":{"package":"repair_demo","kind":"lib","function":"repair_demo::selected","engine":"interactive"}}
```

Use the returned `structuredContent.id` for subsequent calls:

```json
{"name":"verification_status","arguments":{"job_id":"JOB_ID"}}
{"name":"verification_events","arguments":{"job_id":"JOB_ID"}}
{"name":"verification_result","arguments":{"job_id":"JOB_ID"}}
{"name":"save_baseline","arguments":{"job_id":"JOB_ID","name":"before-repair"}}
{"name":"list_baselines","arguments":{}}
{"name":"get_baseline","arguments":{"name":"before-repair"}}
```

The events response is a page; resume with its `next.stream_id` and `next.after`.
In MCP those fields are `structuredContent.next.stream_id` and
`structuredContent.next.after`; return both as `stream_id` and unsigned `after`
on the next `verification_events` call. Inspect `gap` before treating the journal
as complete. Wrong-stream/future cursors fail explicitly rather than becoming a
gap page.
MCP does not expose CLI `--follow`. Result evidence has the same envelope:
inspect `structuredContent.value.repair.accepted` and the full Current report.
`isError: false` only means the tool call succeeded.

```json
{"name":"explain_failure","arguments":{"job_id":"JOB_ID","package_id":"PACKAGE_ID","target":"repair_demo","kind":"lib","function":"repair_demo::other"}}
{"name":"compare_verification","arguments":{"after_job":"AFTER_JOB_ID","baseline":"before-repair"}}
{"name":"cancel_verification","arguments":{"job_id":"OWNED_RUNNING_JOB_ID"}}
```

Save only after the job completes. The snapshot in `structuredContent.value`
retains the exact recorded reports, revision and context with `historical`
currentness. Provisional verdicts and incomplete targets stay unchanged. The
name is immutable and survives job pruning; it cannot establish today's proof.
Comparison takes exactly one `baseline` or `before_job`. Library submission omits
`target`; a binary uses `kind: "bin"` plus `target: "NAME"`. See the
[MCP schemas](mcp.md) for optional submission arguments and response details.
Once submission returns a durable ID, use `cancel_verification` to cancel that
job. Cancelling an unrelated lookup or disconnecting MCP does not cancel it.

## Return to stock verification

An explicitly unavailable or invalid engine returns a setup error. A runtime
internal failure uses the documented stock fallback and preserves failed engine
evidence; it cannot fabricate focused evidence or claim a successful batch.
Inspect `batch.batched`, `batch.focus`, `batch.selection_error` and the full result.
For a setup error, inspect it and cancel any owned unfinished job before
starting a separate stock run:

```sh
sail verify --incremental --package repair_demo --lib --output json
cargo verus verify --package repair_demo --lib
```

The first command selects stock managed verification because it has no engine
flag; the second invokes the configured ordinary stock launcher directly.
Neither promises an interactive focus/full bundle. Default solver service/cache
failure falls back to the pinned Z3. See [workspace verification](workspace-verification.md)
and [ADR 0027](decisions/0027-interactive-proof-engine-and-repair-workflow.md) for the contracts.
