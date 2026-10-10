# Repair workflow walkthrough

This walkthrough covers the implemented experimental workflow. It does not
establish the unfinished M7 speed, concurrent-reader latency or physical
preparation-memory claims. See [the roadmap](ROADMAP.md),
[known issues](known-issues.md) and [acceptance requirements](bench/m7-repair-acceptance.md).
The commands and response fields were checked against the CLI/MCP implementation;
this documentation change does not constitute a new proof run.

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
The selected runtime is experimental and has unresolved physical preparation
accounting; corrective variants are unselected until their complete gates pass.

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
stock build is not an interactive engine. Builds under
`engine/verus/candidates/` remain unselected unless explicitly activated in Sail.
A successful install/status check establishes runtime identity, not the unfinished
M7 acceptance claims. Keep the ordinary stock launcher available for fallback.

## Establish a baseline with complete full validation

For an existing library, substitute its package and exact crate-qualified
function below. This small example uses the pinned published `vstd` dependency:

```sh
mkdir -p ../repair-demo/src
cd ../repair-demo
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

Submit one exact repair scope and retain its ID:

```sh
repair_check() {
    sail repair-check --package repair_demo --lib \
        --function repair_demo::selected --engine interactive \
        --background --output json
}
repair_check > before.submit.json
BEFORE=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' before.submit.json)
sail jobs events "$BEFORE" --follow --output jsonl > before.events.jsonl
sail jobs result "$BEFORE" --output json > before.result.json
python3 - <<'PY'
import json
value = json.load(open("before.result.json"))["value"]
assert value["job"]["state"] == "completed" and value["job"]["error"] is None
assert value["repair"]["accepted"] is True
assert value["repair"]["batch"]["full"]["currentness"] == "current"
print("Complete Current full validation accepted this baseline")
PY
sail baseline save before-repair --job "$BEFORE"
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
another snapshot.

## Resume events and cancel deliberately

Fetch a page and use its returned cursor:

```sh
sail jobs events "$BEFORE" --output json > page.json
STREAM=$(python3 -c 'import json; print(json.load(open("page.json"))["next"]["stream_id"])')
AFTER=$(python3 -c 'import json; print(json.load(open("page.json"))["next"]["after"])')
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
repair_check > after.submit.json
AFTER_JOB=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["id"])' after.submit.json)
sail jobs events "$AFTER_JOB" --follow --output jsonl > after.events.jsonl
sail jobs result "$AFTER_JOB" --output json > after.result.json
PACKAGE_ID=$(python3 -c 'import json; p=json.load(open("after.result.json"))["value"]["repair"]["batch"]["full"]; print(next(t["package_id"] for t in p["targets"] if t["requested"]))')
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
After normal MCP initialization, these are `tools/call` parameter objects:

```json
{"name":"start_repair_validation","arguments":{"package":"repair_demo","kind":"lib","function":"repair_demo::selected","engine":"interactive"}}
```

Use the returned `structuredContent.id` for subsequent calls:

```json
{"name":"verification_status","arguments":{"job_id":"JOB_ID"}}
{"name":"verification_events","arguments":{"job_id":"JOB_ID"}}
{"name":"verification_result","arguments":{"job_id":"JOB_ID"}}
```

The events response is a page; resume with its `next.stream_id` and `next.after`.
MCP does not expose CLI `--follow`. Result evidence has the same envelope:
inspect `structuredContent.value.repair.accepted` and the full Current report.
`isError: false` only means the tool call succeeded.

```json
{"name":"explain_failure","arguments":{"job_id":"JOB_ID","package_id":"PACKAGE_ID","target":"repair_demo","kind":"lib","function":"repair_demo::other"}}
{"name":"compare_verification","arguments":{"after_job":"AFTER_JOB_ID","baseline":"before-repair"}}
{"name":"cancel_verification","arguments":{"job_id":"OWNED_RUNNING_JOB_ID"}}
```

Save named baselines through the CLI; there is no MCP baseline-save tool.
Comparison takes exactly one `baseline` or `before_job`. Library submission omits
`target`; a binary uses `kind: "bin"` plus `target: "NAME"`. See the
[MCP schemas](mcp.md) for optional submission arguments and response details.

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
