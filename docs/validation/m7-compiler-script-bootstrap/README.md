# Independent ordinary-script bootstrap candidate

The corrected candidate is uncompiled and unexecuted. Its first root integration
failed strict compilation on a moved context hash and a similar local name;
workspace tests and the solver were unattempted. The correction borrows the
unchanged full context hash from its owning report and uses unambiguous names.
A separately identified injected-environment fixture mistake is also corrected:
an empty export file is not authority; the original route must have no exported
records, refuse admission and force cleaning. The first failed scope remains
failed. This source candidate remains unselected. It addresses the concrete
missing-generation mechanism in the [730 benchmark failure](../m7-benchmark-730a321-bootstrap-failure.md): cleaning a script regenerates a declared Rust include after the original proof snapshot. The proof guard must still reject that changed binding. The exact failed historical binding was not captured; this mechanism is not a historical cause claim.

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
a parent value is not a certificate for an injected child value.

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
64 KiB buffer. A refused read can consume one further buffer before its byte
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

Eight Linux controls are drafted, all unexecuted. They use actual tiny offline
Cargo compilation and original native owners: independent publication while the
proof guard still refuses generated input; fresh-binary/dep-info/environment
lineage refusal; restored bytes/mtime and cancellation; external aliases,
directories, build dependencies and duplicate units; helper loss and invalid
export recovery; empty exports and scriptless fast refusal; joint runtime
identity/sidecar binding; and actual Cargo dynamic-library environment declaration refusal in both publication routes. Their Cargo supervisor has a 60-second limit plus a
two-second termination policy, file-backed captures capped when read at 1 MiB,
and bounded error cleanup. Exceptional supervisor teardown does not certify
complete descendant closure. No control fabricates a Current proof.

The [sensitivity patch](disable-publication.patch) removes only new independent
ordinary emission. The unchanged real-Cargo positive must then fail its export
assertion; restoring exact source must pass. Neither outcome has run. Required
native checks include the complete affected tests, all-target strict checks,
fresh combined gate/solver, original two-warmup real-project benchmark and the
unchanged whole repair fixture. No third warmup, oracle change, proof retry or
performance credit is included.
