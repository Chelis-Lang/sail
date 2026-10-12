# Independent ordinary-script bootstrap candidate

The corrected implementation passes the [complete c2a4b23 local gate and solver](../m7-core-c2a4b23-validation.md)
and separate [emission sensitivity](../m7-bootstrap-c2a4b23-sensitivity.md).
Exact [release and byte installation](../m7-product-c2a4b23-installation.md) also
pass, followed by the [unchanged complete original repair fixture](../m7-repair-c2a4b23-terminal.md).
Original real-project benchmark recovery and performance remain pending. The
first two root integrations failed strict compilation before tests; both failures
remain retained. The third frozen integration (`602ec67`) passed strict checks,
but its eight bootstrap controls had five passes and three failures: the first
real-Cargo positive, empty-generation recovery and the reused-binary refusal
control's initial admissible baseline. Their original assertions are unchanged.
That whole gate ended FAIL with 646/649 workspace tests passing; the solver was
unattempted. Neither the passing negative controls nor the later format
captures turn those positive failures into acceptance.
The [portable 602 failure and three Cargo format captures](../m7-bootstrap-602ec67-gate-failure.md)
preserve those outcomes separately from corrected validation.

Three owned tiny offline Cargo format diagnostics establish the rejected path:
the single-package prerequisite is `build.rs`; a workspace member emits
`member/build.rs` whether Cargo is invoked from the root or the member. The old
certificate resolved both against Sail's own current directory. Exact installed
Cargo source explains the rule: `prepare_rustc` calls `build_base_args`, which
calls `add_path_args`; `path_args` sets workspace cwd for a local source under
that workspace. Other source kinds use package cwd and unstable `root-dir` can
change the base. See the [pinned official path implementation](https://github.com/rust-lang/cargo/blob/5f94df4789f005f9a352888e8355ffc645b7ed0e/src/util/workspace.rs#L128)
and [source/capture seals](cargo-cwd-source.json).

The correction resolves relative prerequisites one at a time against the exact
absolute canonical metadata workspace root only for that supported local-path
role. Absolute prerequisites, the exact dep-info target header, matched script
source and raw-path equality/native membership checks remain unchanged.
Unstable/inline command overrides, unstable environment selectors and parsed
unstable/include Cargo configuration refuse optional acquisition. This matters
because pinned `RUSTC_BOOTSTRAP=1` also enables Cargo's dev-channel settings.
The optional owner now covers the exact virtual workspace manifest and lock
in addition to its member package roots; it does not walk the whole workspace.
The new real-member control retains the original positive oracle and requires
wrong-root, foreign-source, relative-root and override refusal plus manifest
mutation refusal. All ten corrected controls pass in the later c2a4b23 gate.

The candidate models the cleanup/regeneration hypothesis from the
[730 benchmark failure](../m7-benchmark-730a321-bootstrap-failure.md): cleaning a
script regenerates a declared Rust include after the original proof snapshot.
The proof guard must still reject that changed binding. The exact failed
historical binding was not captured; this mechanism is not a historical cause
claim.

The optional owner is acquired before cleaning only when a nonempty, entirely
eligible family generation is unavailable and metadata has a supported build
script. It watches all package inputs, preknown declarations, Cargo configuration,
and exact runtime files and identity sidecars. It hashes that inventory after
independent namespace and read-lease acquisition. Workloads without a supported
custom-build target skip this additional owner and runtime hashing.

Only a normal completed Cargo exit zero can use the new path. Each ordinary
contract must match current package metadata, a single execution unit and an
actual `custom-build` artifact with `fresh=false`: rustc compiled that script
binary during this guarded invocation. `fresh=true` cannot bootstrap. The exact
binary aliases, dep-info header, script source and all compilation prerequisites
must agree, and all prerequisites and declarations must be pre-acquired regular
files. Newly learned raw aliases are refused even if they later resolve to a
guarded canonical file. Directory/default contracts and build dependencies are
unsupported. Every declared compilation/execution environment value must match
the guarded effective environment; per-run or Cargo-injected declarations refuse
sharing except the pinned explicit `RUSTC_BOOTSTRAP=1`. The shared family/bootstrap
policy also refuses Cargo dynamic-library variables, `RUSTDOC`, `RUSTC_LINKER`,
removed `RUSTFLAGS` and dependency metadata. Cargo documents its child environment
in the [official environment reference](https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-sets-for-build-scripts);
a parent value is not a certificate for an injected child value. An environment
with a non-UTF-8 key or value refuses bootstrap before native acquisition;
a declared key whose current value is non-UTF-8 refuses either publication
route. Such a value is never encoded as absence or normalized.

At publication, bounded hashes are compared again, exact sealed output contracts
are reread, the effective environment is checked again, and both independent
native barriers must pass. The emitted records are precisely that checked set;
there is no subsequent input read. Emission affects ordinary script observations
only. It does not change proof Currentness, promote an Unknown result, create a
proof receipt, or allow the selected verified crate to skip verification. An
eligible ordinary contract may therefore be published after an exit-zero proof
run that remains Unknown, independently of the failed proof owner.

The existing ADR0036 trusted private Cargo-artifact boundary applies. A checksum
and a post-run `.d` file do not authenticate dep-info completeness against an
actor that can fabricate private Cargo/rustc outputs. The candidate refuses
missing, malformed, uncovered and ambiguous lineage within that boundary; it
makes no malicious-omission claim.

Each hash pass admits at most 4 GiB aggregate and 256 MiB per file, using a fixed
64 KiB heap buffer. A refused read can consume one further buffer before its byte
limit is detected. Each pass has a cooperative 30-second clock and cancellation
checks between reads. Native acquisition uses the existing bounded APIs. These
checks cannot interrupt a blocked filesystem operation, and existing helper
teardown retains its original behavior. The managed API has no caller deadline
or cancellation token to pass through: job cancellation still terminates its
original owned worker; the optional owner has no detached task. The original
120/600-second outer policies are unchanged.

The observed runtime alone has 418 files totaling 1,852,005,612 bytes, largest
199,557,488 bytes. The 4 GiB byte admission fits that runtime by size, unlike a
1 GiB cap; two actual hash passes have not been timed. A slow or oversized owner
refuses the optional certificate. The 32 MiB observer/lease sublimit and 40 MiB
whole-owner limit account requested retained capacities after construction,
including roots, exclusions, hashes, environment and inline fields. They are
not construction-allocation, allocator-RSS, kernel or child-process limits.
Bounded JSON can allocate expanded parsed graphs transiently; no whole-peak
quota is claimed. [Source identities and limits](source.json) bind the draft.

All ten Linux controls pass in the corrected c2a4b23 freeze. The older
eight-control scope retains its five passes/three failures above. The controls
use actual tiny offline
Cargo compilation and original native owners: independent publication while the
proof guard still refuses generated input; fresh-binary/dep-info/environment
lineage refusal; restored bytes/mtime and cancellation; external aliases,
directories, build dependencies and duplicate units; helper loss and invalid
export recovery; empty exports and scriptless fast refusal; joint runtime
identity/sidecar binding; and actual Cargo dynamic-library environment declaration refusal in both publication routes; and isolated same-test child processes receiving
non-UTF-8 keys/values through `Command` without process environment mutation. Their Cargo supervisor has a 60-second limit plus a
two-second termination policy, file-backed captures capped when read at 1 MiB,
and bounded error cleanup. Exceptional supervisor teardown does not certify
complete descendant closure. No control fabricates a Current proof.

The [execution procedure](sensitivity-procedure.md) reuses the existing bounded
owned phase runner and preserves three separate tested images. The
[sensitivity patch](disable-publication.patch) removes only new independent
ordinary emission. The unchanged real-Cargo positive must then fail its export
assertion; restoring exact source must pass. All three outcomes are verified in
the separate sensitivity record. Complete affected tests, all-target strict
checks and the fresh combined gate/solver pass. The unchanged whole repair fixture
also passes separately. Original two-warmup real-project benchmark remains
outstanding. No third warmup, oracle change, proof retry or performance credit is
included.

The observed runtime size requires at least 61,733,520.4 bytes/second
(about 58.87 MiB/second) to finish a single hash pass within its cooperative
30-second budget, before package inputs or native acquisition work. Two passes
read at least 3,704,011,224 runtime bytes. The 4 GiB aggregate cap leaves
2,442,961,684 bytes for other guarded inputs; the largest observed runtime file
is 68,877,968 bytes below the per-file cap. These size calculations establish
byte admission only. They do not establish emulated storage throughput,
30-second viability, or original 120/600-second performance. Blocking filesystem
calls can exceed the cooperative budget; supported-script acquisition may
refuse. The scriptless workload avoids this extra acquisition entirely. The
original whole workload must decide viability after native controls pass.
### c2 warmup failure and source-only recovery

The fresh original c2 benchmark failed during warmup after 889.987 seconds,
before measured samples. Both repair attempts exited zero but remained Unknown
with build-script declarations established during the run; neither was accepted.
Current metadata confirms that serde is reachable only through indexmap's exact
`cfg(any())` edge and never executes, while required observation coverage still
includes it. The final repair retry journal contains five reused script binaries
and one newly compiled vstd binary; independent bootstrap correctly refuses
fresh compilation authority. The first attempt has six newly compiled scripts
but no export, so these facts do not prove a complete failure explanation.

The source-only follow-up excludes only that provably disabled role from
unobserved script coverage and rebuilds eligible scripts during the existing
missing-target retry under the same independent owner. All source/runtime roots,
proof guards, Unknown handling, receipts and final `fresh=false` checks remain.
Selecting the disabled package explicitly, or an ambiguous package selector,
conservatively retains original cleaning. A unique explicit workspace-root
selection keeps the false-edge exemption. Unsupported
scripts still prevent publication; extra cleaning can increase first-run cost.

The new real Cargo fixture has an active scripted dependency and a disabled
script that fails if executed. It requires first-pass fresh compilation, refusal
of the replaced reused journal, actual retry cleaning/recompilation, independent
export admission and subsequent input-barrier readiness. It exercises shared
production retry selection, not actual source-engine verification. Original
controls remain unchanged. Native tests, strict checks, complete product gate
and unchanged real-project warmups/benchmark are pending. The
[source record](scheduling-recovery.json) binds sources and private metadata
digests; no latency, speedup or M7 acceptance is claimed.
