# Private control through the normal Verus Cargo wrapper

Status: proposal, not implemented or selected in this integration. Main's
[#440](https://github.com/Chelis-Lang/sail/pull/440) fixes the original
#385/#393 witnesses with file-backed alias settings and packed Cargo-Verus
options. [#436](https://github.com/Chelis-Lang/sail/pull/436) fixes the original
#386 ghost-environment witness, and
[#445](https://github.com/Chelis-Lang/sail/pull/445) adds ghost-only file
declarations. This integration retains those corrections. Remaining
compiler-visible channels are tracked in
[#435](https://github.com/Chelis-Lang/sail/issues/435); the broader original
launch/provider relation remains open. This proposal authorizes no runtime pin
and claims no complete compiler-input observation.

The earlier baseline selected a Cargo alias and a different Rust compiler
wrapper through environment variables. Cargo, build scripts, procedural macros
and the verifier can observe that environment. Removing those variables only
when spawning the final verifier leaves earlier programs affected. Recording a
variable as operational, or excluding it from a cache key, cannot make it
unobservable. Original absence must remain absence; an empty string differs.

The proposed interception point is the existing fork's **normal Verus Cargo
wrapper**. Plain Cargo-Verus continues to launch ordinary Cargo with its normal
`CARGO`, `RUSTC_WRAPPER`, toolchain and per-unit environment. Sail supplies no
extra control environment to that tree. A reviewed early wrapper hook can join
an independently established managed session before launching or executing the
actual verifier. Without that join, it executes the normal plain-wrapper path.

## Routing is not authority

For real compiler units, the normal wrapper receives Cargo's original
`--out-dir`. It can use that argument to locate a possible registration under an
owned target directory. Metadata/version probes often have no output directory;
they must keep their ordinary behavior and cannot obtain managed proof authority
through a guessed working directory.

A local macOS probe uses the selected `36d65c3f` fork in an isolated hardlinked
runtime. The control runs normal Cargo-Verus; the observation arm changes only
its owned wrapper entry to an argv logger that executes the same wrapper binary
in the same runtime directory. Cargo, requested source, environment and target
are unchanged. Both runs exit zero and emit byte-identical structured stdout.
All 21 real units, including build scripts, procedural macros, vstd and the
requested target, carry `--out-dir`; probes without real units do not. The units
share six output directories, so directory equality cannot identify a unit.
All 25 stable-path `.rlib`/`.rmeta` files have identical hashes. An additional
incremental metadata file has identical bytes under a different ordinary
incremental session directory; the original whole-path comparison remains false.
This is routing feasibility evidence, not acceptance of the proposed transport.
The external probe record's SHA256 is
`e73cc5a0e267b5551b6598d98a8755f867d598bea5555ce59ff33356c61bc050`.

The wrapper must parse one exact output-directory argument, preserve its raw
bytes and original option boundaries, and refuse ambiguous or unsupported
spellings for managed admission. Native execution continues without admission.
A bounded search may discover a candidate registration only beneath a separately
recognized owned target root. Lexical ancestry, resolved ancestry and symlink
handling must have explicit contracts; a path prefix test is insufficient.

A file at a familiar name supplies **no authority**. A registration is usable
only after joining all of these independently produced facts:

- The controller owns the target root and holds its existing workspace/compiler
  family exclusion for this active run.
- A protected registration carries the full original run/request, target root,
  reserved launcher identity, expected immutable runtime and semantic mode.
- The controller retains the actual Cargo-Verus child and its actual Cargo
  launch lineage. PID text alone is insufficient: PID reuse and restart epochs
  require kernel process identity or a source-bound child/exec bridge.
- The private endpoint authenticates both peers and binds the connecting wrapper
  to that active launch lineage, actual immutable wrapper image and observed
  invocation. Unix file ownership or possession of the registration alone does
  not establish this process join.
- The request/reply challenge binds that process, run, registration generation
  and a fresh attempt allocation. A delayed reply cannot attach to a replacement
  process or a subsequent run with the same target directory.

Peer credentials, retained process handles, child/exec attribution, protected
registration publication and exclusive-create behavior are native interfaces to
implement and qualify on Linux and macOS. UID equality or unqualified ancestry
is weaker than the required join. The present code does not implement this
protocol, and this document does not substitute a Boolean assertion for it.

A Cargo-launched build script can exec the normal wrapper in place. Its immediate
parent is then the actual Cargo process and its current image is the approved
wrapper, although Cargo did not launch that compiler invocation. A parent/image
conjunction therefore cannot supply original launch intent or arguments. The
protocol needs an independently complete expected compiler invocation or an
attributed spawn/exec history, and must reject this control. An extra descendant,
a delayed connection after exit, an inherited connection and PID reuse require
separate controls; a live credential sample cannot replace their history.

The current dependency pin is Tokio 1.53.3. Its safe Unix peer-credential helper
returns a PID on Linux and macOS, but the macOS implementation requests
`LOCAL_PEEREPID` and asserts the successful reply length. Apple's published
socket implementation distinguishes a delegated socket's effective PID from its
last PID. Neither value certifies Cargo's launch intent. This is source review
of a possible primitive, not qualification of a deployed kernel or approval of
a new dependency edge. The native adapter must account for the dependency's
assertion path, exact socket/process lifetime, fork/exec behavior and both
platforms before this primitive can contribute to admission.

## Proposed execution sequence

1. Sail retains the submitted request and resolves its independently checked
   Cargo selection. It creates an exclusive active-run registration and starts
   **normal** Cargo-Verus, with the environment that plain Cargo-Verus receives.
   The controller records the actual launcher/child identity. Source-bound
   launcher bridges or kernel observations must extend that identity through
   Cargo to the real wrapper.
2. The normal wrapper reads its original compiler arguments and uses `--out-dir`
   only to discover a possible registration. It authenticates the controller;
   the controller authenticates the actual wrapper process and joins the exact
   original run, unit and effective options before allocating an attempt.
3. A fresh native allocation is obtained before preparing observation paths or
   sealing final arguments. Separate invocations in one run use distinct slots;
   old reservations and observations remain intact. The complete raw
   run/unit/slot/argv tuple enters the checked request codec.
4. The authenticated response supplies a private in-memory launch configuration.
   It includes the validated solver/shim choice, namespace/mode, observation
   destinations and explicitly selected engine policy. The wrapper preserves
   the original compiler environment and the complete original OS argument
   vector. This includes the currently injected output, timing, logging and record
   options; moving only a new observation envelope is insufficient. Private
   compiler control cannot travel through extra argv tokens:
   stripping a local parsed vector does not change `std::env::args_os` or
   kernel-visible command-line bytes observed by procedural macros.
5. The wrapper starts or execs the actual verifier with its ordinary arguments
   under the retained identity bridge. The verifier independently joins the
   same active registration using its own original `--out-dir` and authenticated
   process/exec lineage. No additional compiler argv, environment variable or
   inherited descriptor carries the configuration. It remains in trusted fork
   memory after the private handshake, whose descriptors close before user
   compiler plugins execute. The controller and verifier observation channels
   retain exact argv,
   process status, output, compiler inputs and solver-child histories. A normal
   exit is not completion authority. Existing checked document, requested-scope,
   current-input and query-origin checks remain necessary.
6. Publication consumes the same admitted process/request history. Replayed
   reports do not recreate live starts. Stale, missing, partial, unowned or
   mismatched registrations withhold managed authority and reuse; they never
   select another run's metadata or change plain verification into success.

The authenticated response cannot resolve targets by copying final report
labels. Package/target/kind/source and effective options must be joined to the
independently resolved request and actual compiler unit. Generated inputs must
not rebind that original request after verification to make it match.

## Private solver selection and environment fidelity

The current `VERUS_Z3_PATH=shim` override is itself observable to `env!` and
`option_env!`. The reviewed fork needs a private solver-launch configuration
separate from the compiler environment. It chooses the validated Sail shim and
its independent pinned-Z3 fallback in memory, without rewriting
`VERUS_Z3_PATH`, `SAIL_Z3_PATH`, daemon/provenance variables or other ambient
compiler inputs. Existing user-supplied values retain their plain meanings.

The shim receives its control through a separately authenticated solver-process
join or an explicitly qualified solver-only channel, rather than environment
inherited by compilation. Even a solver-only argument envelope needs an explicit
observability contract; this proposal does not declare it invisible to arbitrary
process/filesystem inspection.
Mode and solver identity remain explicit, and its unconditional forwarded byte
stream and direct pinned-Z3 fallback retain their existing safety contract.
Operational capabilities and observation paths must not enter emitted SMT
bytes. That separation requires source review, roundtrip checks, whole-corpus
conformance and determinism qualification; it is not justified merely by naming
a field operational.

Build scripts need no environment restoration if Cargo and its actual children
never receive the new controls. Existing Cargo-generated and Cargo-Verus-owned
per-unit values must remain plain; blindly restoring all submitting-parent
values would also be wrong. A real Cargo probe of forced `[env] RUSTC_WRAPPER`
restoration confirms the distinction: the compiled `env!` and wrapper process
see the forced value, but build-script execution still sees Cargo's selected
wrapper. That configuration alone does not address the historical #385 witness;
main fixes it through the separate file-backed transport. The external probe is
exploratory and is not a selected workaround.

Optional unit-graph planning and other metadata probes must not leak their
special environment into actual compilation. Observer failures must not abort
or replace fresh verifier output. On unsupported transport, the reviewed normal
wrapper follows the original plain path and withholds managed authority; it
must not guess an active registration or fabricate a successful durable seal.

## Proof and rollout obligations

The shared acceptance core can prove unchanged argument-vector correspondence,
descriptor binding, slot uniqueness within its supplied namespace, protocol state changes
and refusal of mismatched authority. Completing the service theorem also needs
concrete producers for the native process/ownership observations above and for
the full compiler-input domain, including verification-only environment reads.
The final erased dep-info file is not that domain: the historical #386 witness
omits a verification-only environment dependency there. Main now records those
reads before erasure; that correction does not certify opaque procedural-macro
or build-script reads or every physical compiler input.

Before selection, implement and qualify the authenticated process bridge,
original-request normalization, actual per-unit environment relation, private
solver selection, complete verification-phase input capture and retained-domain
transfer. Run malformed/stale/replaced-process and registration controls, real
build-script/procedural-macro/environment fixtures, both platform gates,
default-mode conformance, deterministic query checks and native cost tests.
No capability is minted from an unqualified build's self-described features.
The current runtime and argv remain selected until the owner qualifies and
selects a new pin.
