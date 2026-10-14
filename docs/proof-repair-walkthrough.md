# Repair workflow walkthrough

This walkthrough takes one library through the experimental repair workflow:
install the engine, check a repair, follow its events, detect a regression the
focused phase cannot see, and do the same through MCP. Proof repair is
experimental and stock verification stays the default. Its qualification
status (gates, measured runs, open items) is in
[M7 validation](validation/m7.md); limitations are in
[known issues](known-issues.md).

A first managed run can still leave proof evidence Unknown while dependencies
are regenerated. [Managed runs](managed-cold-start.md) explains when that
happens and what the optional ordinary-build recovery changes.

## Install and select the engine

Start with Sail installed, the pinned Verus on `PATH` (see the
[README](../README.md#install)) and a project whose ordinary
`cargo verus verify` works. A release archive (Linux or macOS) bundles the
engine beside `sail`, and nothing else is needed:

```sh
sail engine status --output json   # "build" is under the unpacked engine/ directory
```

From a Sail source checkout, download the prebuilt runtime from the public
fork's releases (`artifact-macos.json` on macOS):

```sh
python3 ci/engine-artifact.py fetch --pin engine/verus/artifact-linux.json \
  --cache ~/.cache/sail      # or your SAIL_CACHE_DIR
sail engine status --output json
```

`fetch` downloads without credentials and checks the archive digest and every
runtime file against the committed pin. Or build it yourself:

```sh
cargo xtask build-verus-engine
sail engine status --output json
```

`build-verus-engine` builds the commit of Sail's public Verus fork
([Chelis-Lang/verus](https://github.com/Chelis-Lang/verus)) pinned in
`engine/verus/fork.toml`; it needs no GitHub credentials. It has been run on
x86-64 Linux. `engine status` reports that commit as `fork_commit`. If the
engine is missing, `sail engine status` and `sail repair-check` stop with
`SAIL3120`, name the file they expected, and say how to obtain the engine; no
job is submitted.

`fetch` and `build-verus-engine` install the engine in the Sail cache, which
`engine status` inspects first; a bundled engine is used only when the cache
has none.
Repairs must use that cache too; relative `SAIL_CACHE_DIR` settings
follow the invoking project directory. The engine runs stock lowering with a
focused phase in the same compiler callback, and prepares no lowering for
reuse. See the [engine description](../engine/verus/README.md) and the
[repair guide](proof-repair.md).

Keep `sail` and `sail-z3` side by side, and keep your ordinary
`cargo-verus` on `PATH` (or name it with `SAIL_CARGO_VERUS`): repairs fall back
to it. Linux freshness checks also need Python 3; see the
[freshness contract](freshness.md) for what is checked on each platform.

To import an existing build instead of compiling it:

```sh
sail engine install --from path/to/engine-build --output json
```

Import validates the pinned fork commit, source and toolchains, the manifest
and every artifact's bytes. It keeps a monitored link to that directory, so
keep the build available. Unrecognized or conflicting builds produce
`SAIL3120`; a stock Verus build is not an interactive engine.
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
Inspect `batch.batched`, `batch.focus`, `batch.selection_error` and the full result
(`batch.full` in `repair-check` output, or `result` in a job result).
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
