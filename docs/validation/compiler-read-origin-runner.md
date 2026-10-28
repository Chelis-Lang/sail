# Compiler-read origin regression runner

Status: the local runner is implemented and has syntax/argument checks only.
No paired-runtime execution is claimed here. The owning
[qualification record](compiler-read-origin.md) records accepted source/runtime
and actual native results for [#551](https://github.com/Chelis-Lang/sail/issues/551).

Run `scripts/qualify-compiler-read-origin.py` with Python 3.11 or later on an
owned native host. It refuses CI and performs no builds. Linux and macOS use the
same public Sail CLI, durable-job and stdio MCP paths. A platform name alone
does not establish native execution: the owning host controller must supply the
host record and retain its orchestration evidence.

```sh
python3 scripts/qualify-compiler-read-origin.py \
  --sail "$SAIL_CANDIDATE_BIN/sail" \
  --build "$SAIL_CANDIDATE_BUILD" \
  --build-receipt "$SAIL_BUILD_RECEIPT" \
  --host-record "$SAIL_HOST_RECORD" \
  --out "$SAIL_ORIGIN_EVIDENCE"
```

The evidence directory must be new, outside Git, and short enough for its owned
Unix socket. The build must be a complete immutable installed variant with `sail-build.json`
and runtime members under `verus/source/target-verus/release`, without directory
symlinks. The executable directory must also contain `sail-z3`. The candidate
is selected through normal validated `SAIL_VERUS_BUILD`; inherited
`SAIL_CARGO_VERUS` is removed. The variant manifest, staged platform artifact
manifest, fork/source-lock/features, solver and full installed inventory must
match. Public `sail engine status` must confirm the selected variant.
The source checkout must be clean. Each normal verification has a 240-second
wall deadline; solver flags and resource limits are unchanged.

The host controller supplies JSON containing `native: true`, `system`,
`machine` and a nonempty `orchestration` description. Its `system` and `machine`
must match the runner's observed platform. Include the EC2 instance ID, instance
type and region when applicable; the runner preserves the entire record and
its digest. This is controller-supplied native-host evidence, not an emulation
detector.

The build controller supplies JSON containing:

- `sail_source_commit` and `verus_source_commit`: complete source commit IDs;
- `sail_sha256` and `shim_sha256`: the actual built executable digests;
- `runtime_sha256`: every file's path relative to the complete installed build root
  mapped to its SHA-256 digest, including symlinked files' actual bytes.

Keep this receipt outside the installed build. The runner compares executable and
complete runtime inventories and independently checks that all tracked crate,
xtask, engine and Cargo/toolchain inputs equal canonical Git bytes at the
recorded Sail build source. Script/documentation-only delivery commits can
therefore use the unchanged build inputs. Binary/source correspondence itself
comes from the owning build controller's retained build evidence; this is not a
reproducible-build theorem. Final guards repeat the source, runtime and
executable checks. Actual `verus --version --output-json` must report the full
fork commit from the owning build receipt, both before and after execution.
The fixture explicitly selects the pinned solver and Verus
Rust toolchain; the solver digest and observed compiler commit/version are
checked, and the compiler identity is repeated at the end.

## Implemented checks

The runner creates benign and semantic-control projects and a private
cache/daemon. It requires:

- A fresh complete Current result for a no-sensitive-read fixture, then unchanged
  JSON, background-job and real MCP verification with zero compiler invocations,
  the original establishing run and the unchanged complete report. At most two
  initial noncurrent discovery attempts are retained without qualification
  credit; they never substitute for the required fresh Current result.
- Ghost-only and exec-visible reads of COMMON (` __VERUS_DRIVER_ARGS__`),
  `SAIL_PROVENANCE_DIR`, `SAIL_COMPILER_INPUTS` and
  `SAIL_COMPILER_INPUTS_BINDING`. For each, human-first, JSON, repeated JSON
  and human-again calls must genuinely compile. Raw schema2 observations must
  retain the actual source values in the expected phases; request endpoints,
  bindings and provenance paths must match the actual request. The request
  binding is independently recomputed from the run ID and compiler-unit key. Consecutive
  JSON calls must expose distinct raw run values. The target must publish no
  eligible receipt and must retain its non-authorizing origin marker.
- Removing each read at the same package/unit identity must establish a fresh
  Current result, clear that unit's marker and recover unchanged warm reuse.
  This does not clear conservative markers for older argument variants.
- A separate semantic fixture asserts that `option_env!("SAIL_PROVENANCE_DIR")`
  is absent. The same source must freshly prove in human mode with a complete
  Current result, then genuinely compile and fail its named obligation under
  JSON attribution. Both semantic assertions use `by(bit_vector)` so their failed goals
  produce a SAT counterexample with the pinned solver, independently checked
  against stock and the candidate without changing resources. Expected public Cargo CLI exit 101 (nested Verus exit 1), a new same-unit attempt, a confirmed
  failed function and settled SAT obligation, and the retained pending marker
  are mandatory. Missing successful-phase `inputs.json` is permitted only for
  this failed run. Discovery attempts retain no qualification credit; when
  needed, the supported `SAIL_MANAGED_FORCE=1` flag forces a fresh human
  baseline and is removed before the JSON request. This tests truthful reuse
  across ADR 0088's source-visible setting difference; it is not an executed
  old-main wrong-verdict reproduction.
- A separate ghost-only ` __VERUS_DRIVER_SHADOW__` proof first establishes a
  complete fresh Current JSON result with its actual absent source value. At
  most three warm calls, separated by the real stamp-settling interval, must
  reach an actual unchanged-call replay `hit`; zero compiler invocations alone
  do not qualify. The runner copies that version-4 record and requires its
  environment map to bind the shadow name even though all captured dep-info
  omits it. Changing only the caller's shadow value to `Some` must reach the
  recorded-key environment-mismatch guard and genuinely compile the same
  source/unit/context into the named confirmed failed SAT obligation, with
  expected public Cargo CLI exit 101 (nested Verus exit 1). Raw per-call trace deltas, the baseline observation and
  replay record, and the new failed attempt remain in the evidence. This is
  an ordinary inherited source variable, so successful baseline reuse is
  required; no source-sensitive ban or successful-run marker is expected.
- Missing, legacy and malformed family-origin metadata must cause genuine
  compilation before fresh proof authority, restore the valid `CACHEDIR.TAG`,
  and allow the next unchanged call to reuse the newly established report.

The counters fixtures assert only benign truths; the separate semantic controls
expect their stated verification failures. The runner performs no private
provider/transport experiments and adds no CI job. It does not exercise the
remaining seven explicit child-overlay names as real compiler fixtures; their
source guards and unit controls remain separately qualified.

## Evidence and remaining gates

`report.json` records the exact scope, source/runtime/build/host digests,
commands, result identities, counters and failures. Raw command streams,
project results, actual compiler documents/invocations/source fixtures,
receipt inventories and selected receipts, actual pending-marker bytes,
before/mutated/after origin-state inventories, and MCP frames remain beside it.
The `work` subtree holds the disposable project/cache; exclude that subtree
when archiving the concise raw packet, preserving the separately copied raw
receipts and source fixtures. No logs or JSON dumps belong in Git.

Exit zero means only `implemented_scope_complete: true` for the selected scope;
it does **not** mean complete #551 acceptance. The default `--scope basic`
records cancellation/restart and retained restoration as unrun. The separate
`--scope lifecycle` runs the bounded controls below and records the basic scope
as unrun in that invocation. The owning acceptance record joins these actual
packets alongside full native gates, conformance, determinism and both-platform
runtime selection. The runner always reports
`overall_qualification_complete: false`.

## Lifecycle scope

On the same idle native Linux host, after the basic scope completes, use the
same installed runtime/build/host receipts and a new evidence directory:

```sh
python3 scripts/qualify-compiler-read-origin.py \
  --sail "$sail" --build "$build" \
  --build-receipt "$build_receipt" --host-record "$host_record" \
  --out "$lifecycle_out" --scope lifecycle \
  --retained-project "$prepared_vest"
```

Cancellation requires Linux pidfds in the normal Python runtime. A private
fixture establishes a fresh proof, then adds a benign tracked provenance read
and submits a normal public background verification. Before stopping anything,
the runner binds the actual started compiler's command, installed executable,
working directory, run/request binding and pending marker, and the owning
public-job process-group leader. It retains opened pidfds, stops only that
compiler, invokes public `jobs cancel`, then requires all opened processes to
exit and no live members of that owned process group to remain before restart.
Each enumerated group member is admitted only after checking its identity and
group membership while its pidfd remains live; unverified descriptors are
closed without signaling. The compiler and leader must also remain live after
their binding/command checks and immediately before the first control signal.
A bounded read-only wait can follow the observed normal `rustup` exec on the
same held pidfd; the launcher itself never authorizes a control signal. The
selected installed compiler must still pass every original identity check.
No numeric-PID signal can race PID reuse. The marker must survive cancellation;
same-source restart and repetition must really compile. Removing the read,
forcing one fresh supported verification, then rerunning must clear its marker
and recover useful ordinary reuse. Raw process identities, start record,
marker bytes, public responses and compiler observations are preserved.

The retained fixture is a separate prepared checkout of the corpus's exact
Vest commit `11af01ddaef76cceded03517cd48764adb9d35b1`. Its vstd requirement
and lock must use the existing public crates.io
`vstd = 0.0.0-2026-10-18-0147` alloc/std profile. The source-patched
conformance fixture is a different profile and does not qualify this gate.
The runner compares every nonmanifest/lock source against actual canonical Git
blobs at the pinned commit, independently of working-tree status or index
flags. It copies the checked bytes to its own workspace and preserves both the
actual canonical-to-prepared manifest diff and complete source/Git-blob digest
inventory. It never edits the supplied checkout.
Every call uses the ordinary public CLI with
`-p vest_lib -- --rlimit 100` (VI-0032), without research or force overrides.
Noncurrent discovery attempts are uncredited; a fresh Current A1 with a genuine
two-phase source document is mandatory. Appending a comment establishes fresh
B, and restoring the exact original source must produce A2 and unchanged A3
with zero verifier invocations and A1's original establishing run.
For A2 the runner checks exact equality with A1 except for the explicitly
reconstructed attribution fields: all obligations must be null, every function
must have default/unrecorded provenance and zero solved/served/unrecorded
checks, the sole attribution note must count every missing function, and the
solver version must be null. All other fields, including solver SHA, function
identity/verdict/establishment, diagnostics, timing and command, remain exactly
equal. A3 must retain A2's report exactly while still naming A1. This follows
the existing [compiler-restoration contract](../instant-revert.md); it does not
copy a cached report into production authority or change report generation.
The actual A2 invocation and committed receipt must preserve that original
lineage; A2 must have no compiler-start record. The runner preserves raw
compiler documents, reports, receipts and source bytes. The existing measured
profitability guard remains active: a refusal or recompilation fails this
gate rather than manufacturing costs or changing its threshold.

The first native lifecycle attempt to reach A2 (`7a1c420b`, 2026-10-28) remains
failed: its newly added whole-report equality expected the original obligation
detail to survive compiler restoration. The raw A1/B/A2 reports and full diff
show unchanged verdicts, diagnostics, timing, command, solver SHA and original
establishing run with zero A2 verifiers, alongside the documented missing-log
shape. This test expectation was stronger than ADR 0050's contract. Its actual
cancellation controls and final source/runtime guards passed; A3 was unrun.
The corrected comparator and fresh whole lifecycle run require independent
review and native execution; no pass is inferred from reinterpreting that
failed packet.

- `87ebe062`, 2026-10-28: lifecycle refused before any control signal while the
  bound live child was still in the normal `rustup run ... -- rust_verify`
  exec stage. Raw command, request binding and per-clause diagnostics are
  retained; final source/runtime guards passed. This attempt supplies no
  cancellation or retained-restoration pass. The bounded exec wait needs its
  own reviewed fresh lifecycle run.

Interruption here means the tested public cancellation of an owned job and its
compiler group. This does not establish arbitrary parent-crash/orphan recovery.
Neither this runner nor the origin marker's existing
atomic JSON/rename operation proves recovery from machine power loss or a
filesystem crash. Broader filesystem/process assumptions remain in the design
tracker. Missing required evidence, an unknown response shape, timeout,
incomplete source observation or changed source/runtime makes the implemented
scope fail; an unrun scenario is never reported as passed.
