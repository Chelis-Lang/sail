# Observed-input transaction proof component

The [input component](../../proofs/acceptance/input_binding.rs) verifies an
inductive observation/reuse transaction model and executable exact-byte
comparators for the [acceptance core](evidence-acceptance-core.md). Its shared
[context](../../proofs/acceptance/context.rs) contains the actual modeled input
map, exact request, selection, runtime, explicit solver identity and mode.
The [executable input core](../../crates/sail-acceptance/src/inputs.rs) now
runs in managed capture, projection, mutation-guard checks and retained-read
checks. Its [refinement adapter](../../proofs/refinement/inputs.rs) relates the
same checked executable bodies to the model's exact input-map representation.
Physical observation, compiler-closure discovery and the remaining native
caller/context mapping still have the boundaries listed below.

## Reads through the actual opened object

Fresh `managed/files.rs::file_digest` calls now bracket their byte stream with
structured pathname and opened-handle observations. The native adapter owns
one `File`: it obtains that handle's initial metadata, reads through the same
handle, obtains its final metadata, and rechecks the requested pathname. It
never closes and reopens a path to obtain the final handle stamp. On Unix the
adapter copies device, inode, byte length, mode, modification seconds/nanoseconds
and change seconds/nanoseconds without normalization. Length conversion is
checked; other platforms refuse this observation protocol conservatively.

The source-shared `OpenedRead::begin` requires a regular file and exact equality
of the original pathname and opened-handle stamps. Each successful nonempty
read prefix passes to `OpenedRead::chunk`. The returned private `ReadChunk`
retains those exact bytes and the original object identity; the native hasher
consumes only `ReadChunk::bytes()`. The counter admits partial chunks but cannot
exceed the original reported length. EOF, read failure and later chunks are
distinct transitions. Failure is sticky, and EOF before the exact byte length
cannot produce a `ReadSeal`. Final handle and pathname stamps must equal the
original stamp, which the transaction never replaces.

Phase-local digest entries retain the hash and original `ReadSeal` together.
Before admitting a parallel result under a proposed canonical key, and before
returning a cached value, `ReadSeal::matches_binding` compares both the key's
and the actual requested pathname's object observations with the original read.
An alias retarget between canonical key discovery and opening cannot populate
the old key with the newly opened object's digest. A changed cache-key observation
discards the old entry and rereads instead of attaching a fresh stamp to old bytes. As with the other
endpoint checks, inode reuse and repeated metadata remain OS observation limits;
this is not a proof of global atomicity or a retained live-descriptor lease.
The cache association defect is tracked in [#381](https://github.com/Chelis-Lang/sail/issues/381).

`open_read_digest` and `read_opened_digest` are now the actual executable
open/read/hash loops used by native digest production. The checked body owns the
file and hash state, passes only the actual returned read prefix through
`ReadChunk::bytes()`, and requires explicit EOF, exact length and original
endpoint observations before returning `CompletedRead`. Its logical
`FileObservation` trace binds the actual path/open/handle/read/EOF operations in
order. Hash operations preserve that filesystem trace; omitting an observation
or substituting an earlier stamp cannot establish the final trace contract.
The owned descriptor is dropped when native code extracts the digest/seal,
within each file read.

The kernel's unconditional safety contract permits real I/O failure. A separate
conditional productivity clause uses structured `ScriptedReadFile` primitive
outcomes, derived by a checked concrete provider from its owned byte vector.
Every concrete primitive body verifies the exact bytes and observations it
produces. Nonempty and empty successful kernel calls are proved without assuming
that hardware succeeds. The private native provider explicitly exposes no
scripted file or handle; these constructive outcomes cannot authorize native
filesystem or compiler-domain evidence. A deliberately colliding length-based
forward hash accepts two distinct three-byte streams with the same digest,
showing that the kernel does not need a universal inverse/injectivity law.
Native SHA identities require their separately named finite-domain crypto
condition or direct byte comparison when proving byte equality from a digest.

`checked_opened_read_trace` also verifies finite factory transitions. Its ghost
byte stream concatenates the exact supplied chunk slices in order. Constructive
controls exercise partial/overlong/post-EOF chunks, read errors and mismatched
original/final observations. Native controls hash a real multi-chunk file and an
empty file, refuse partially consumed or write-only handles (including empty
write-only files), and refuse an opened handle retained across a same-content
symlink retarget. A fresh open after retargeting remains usable. Successful
SHA-256 output and requested fold ordering are unchanged for stable files.

This shrinks the pathname-label/opened-object and short-read boundaries. The
named low-level OS contract is that `File::open` yields an owned object at its
initial offset, `File::metadata` describes that same object, successful reads
return the next observed bytes of that handle, and pathname metadata describes
the object resolved at that call. Private native ownership supplies the same
handle to these operations. The generic kernel proves its theorem for providers
satisfying these operation contracts. The private native forwarders and their
standard-library/OS/SHA semantics remain explicit source-bound assumptions; the
stock check does not prove those native bodies or the OS. Production Host
implementations and call sites require a closed source inventory. A generic
`CompletedRead` alone cannot mint a complete compiler-domain/service authority.
The pure factories themselves assume no filesystem or hash law.

This **does not** prove that equal endpoint metadata implies one atomic content
version. Transient changes, exact repeated metadata, timestamp granularity,
filesystem-specific identity behavior and path ABA remain separate observation
limits. It does not prove that a later pathname-based executable launch runs
this opened object, nor that SHA-256 was implemented correctly or is injective.
Native phase-cache map operations, retained-read caller mapping in `read_check.rs`, compiler
closure discovery, configuration/environment collection and launch identity
retain their separately listed refinement obligations. Native macOS/Linux host
gates and cost qualification apply to the integrated implementation.

### A guard over the complete recorded readback batch

`read_check::check` now begins an `OriginalCapture` before any content or alias
read and finishes it after all those reads. The independently collected guard
domain is the original record's file paths, binding keys and binding destinations;
its native stamp also observes resolved objects and linked ancestors. Parallel
content observations still retain their actual returned keys and pass the same
exact-domain comparison. Present, absent and failed observations remain distinct.
Present files use the checked read/hash kernel; a path-metadata failure observes
absence only for actual `NotFound`, while subsequent kernel failures remain
unreadable. No extra content read or change to successful revision bytes is needed.

The source-shared `guard_readback` factory retains the exact `Readback` and
optional `OriginalGuard` in private `GuardedReadback` evidence. It invokes the
actual comparison and capture factories itself, with no caller-supplied success
predicate. Its iff contract requires a nonempty exact complete typed domain and
present equal original/final stamp bytes before both changed/unknown flags can
be false. `compare_readback` also proves its equality flag is exactly the
complement of those two flags. The native adapter preserves the original
capture across the full batch, then consumes the checked getters. Changed stamps
yield a different revision; unavailable stamps cannot yield the old complete
revision. The actual-map refinement and constructive controls cover source,
alias, absence, missing stamps, changed stamps, partial and duplicate domains.
The run-local stamp encodes presence explicitly
and validates the returned metadata-key inventory before folding it in sorted
path order. It is separate from the logical content/revision identity.

The real-file [#387](https://github.com/Chelis-Lang/sail/issues/387) control
records A0/B0, begins checking A0/B1, reads A0, writes A1 then B0 and finally
reads B0. No filesystem state during that check had both old values. The original
helper nevertheless returned the old complete revision. The batch guard now
refuses it, and removing that guard reproduces the failure. The exact-revert,
unchanged, absence, linked-ancestor and tagged read-domain controls stay intact.

This guards the original recorded domain, not an independently proved complete
compiler-input closure. It does not prove a global linearization point from
metadata equality. The native metadata traversal, observation ordering,
unsupported mutation classes and cryptographic stamp assumptions remain named
boundaries. In particular, no axiom equates a repeated ctime with an immutable
file version, and a loaded record is still only a candidate under the original
protected evidence/domain contract.

### Recovery of uncovered source inventories

`previous_covered` remains required. A later empty or partial declaration journal
cannot reestablish an inventory that previously lost a required observation.
Deleting only that source record would forget the missing domain while allowing
Cargo to reuse the same compiled target; it is not a proved recovery protocol.

A justified replacement protocol needs an independently identified fresh
compiler/unit execution domain. One concrete candidate is a fresh owned compiler
target plus the current pre-Cargo unit graph, with each required unit matched to
actual wrapper starts, normal completion/artifact disposition and usable dep-info
reads. It must preserve exact package/target/features/platform identity, distinguish
fresh/up-to-date and unexecuted units on failure, include generated inputs and
declared environment/configuration, and retain original before/after guards.
Only a complete reestablishment could replace old uncovered hints; interrupted
or partial runs cannot promote them. The current package-only unit-plan check
cannot establish this replacement. This recovery protocol is not implemented
by the opened-read transaction and remains tracked with compiler closure.

## Declared environment names and provider failures

The executable `environment_name_matches` checks a proposed dep-info name
against its exact rustc spelling. Its iff contract preserves ordinary name bytes
including whitespace and checks the exact `\n`, `\r` and `\\` escape relation.
`encoded_environment_prefix` proves the loop's prefix relation. The native
parser's proposed decoded UTF-8 string cannot become a different sampled key:
the shared checker must admit its byte mapping first. A nonempty stock witness
accepts exact whitespace and newline names and refuses trimmed, escaped,
empty and unrelated keys. A real rustc child emits these same dep-info cases;
a separate real Cargo fingerprint witness establishes instruction-line framing
and preservation of leading payload whitespace, including indented prefixes and
Unicode trailing whitespace. The Rust string/line framing and Cargo directive semantics
remain named native provider boundaries, not a proved complete compiler parser.

Declaration traversal distinguishes a successfully enumerated empty tree from
missing directories, failed entries, unreadable/non-UTF-8 required files,
nonregular required files and unsupported symlinks. Failure preserves the fresh
verifier report but withholds new pending receipts and global Current. A retained
receipt also needs the exact currently observed declared-name/value map. The
build-script producer separately retains actual output-read failure rather than
converting it into an empty successful contract. This closes concrete provider
loss/name-association defects; it does not discover undeclared process I/O or
prove that the enumerated target tree contains every actual compiler request.

The provider-loss and wrong-name defects are tracked separately in
[#382](https://github.com/Chelis-Lang/sail/issues/382) and
[#383](https://github.com/Chelis-Lang/sail/issues/383).

Receipt policy 6, read-time record policy 3 and proof-context policy 2 refuse
legacy proof authority established with partial scans or incorrectly mapped
names. Compiler-family artifact identity is unchanged; it cannot mint proof
receipts by itself. Dependency-store admission checks the receipt version.
Baselines retain Historical evidence. Shipped/native tracked-environment JSON
records use exact original keys independently of this declaration parser and
are not reclassified by an unrelated schema bump.
Verification-phase environment completeness and original compiler-environment
preservation remain independently tracked defects in
[#386](https://github.com/Chelis-Lang/sail/issues/386) and
[#385](https://github.com/Chelis-Lang/sail/issues/385). The read kernel does not
close either gap; their exact projects are in `corpus/repros/`.

## Completed compiler-declaration providers

The default managed Cargo adapter records a private pre-spawn intent in the
run's artifact journal, then the PID from the actual `Child`. Each retained
`compiler-artifact` or `build-script-executed` message carries that same producer
and child identity. The terminal record is written only after the stdout reader
reaches EOF and `wait` returns a normal exit status. A normal nonzero Cargo exit
can complete observation accounting; it establishes no successful proof. Spawn,
read, forwarding, journal-write and wait errors leave an unsealed intent, and the
owned child is killed and reaped on an early return.

The source-shared `seal_provider` checks the concrete event sequence: one intent,
one nonzero started child, only frames from that producer/child, and one terminal
with the exact frame count. Zero frames is a valid completed empty observation;
a missing journal is not. Interleaved and appended children retain separate
intent inventories. An unfinished child cannot disappear merely because another
child completed. Duplicate producer intents, malformed JSON, records outside an
intent, unknown record kinds and unterminated final records refuse coverage.
PID reuse inside the same run namespace also refuses coverage conservatively.
The build-script scheduling and unit-plan consumers skip only schema-validated
private control records. Their original unit package/scope checks remain in
force; malformed controls and unknown records do not gain a metadata exemption.

Before sealing, the native Cargo observer reuses the complete-document parser
for mixed stdout. It retains artifact/build-script documents, permits known
Cargo diagnostic/build-finished documents and Verus verification documents, and
leaves ordinary verifier summary/timing text unchanged. JSON-looking malformed
or truncated remainder and unsupported JSON documents withhold the terminal
record even after a normal exit; they cannot become an observed empty child.
Capture accounting retains at most 64 MiB, then discards its buffer and withholds
the terminal while forwarding and returning the actual child status as before.
This parser is reviewed native code, not a complete raw-output proof. Corrupted
bytes that no longer look like JSON can be indistinguishable from ordinary
verifier text. That interpretation/provider boundary remains open.

`seal_compiler_provider` additionally scans actual artifact-frame positions.
Every ordinary compiler artifact requires exactly one tagged successful dep-info
read. Build-script and custom-build frames are counted but do not require a
source dep-info read. Each `ArtifactRead` carries its original producer, child,
frame position and actual dep-info bytes. The native read returns these together;
a sorter only proposes their order. The checked scan rejects missing, duplicate,
foreign-child, extra and unreadable outcomes. It proves both directions of the
exact read-domain relation, strict uniqueness and successful raw-byte presence.
The refinement adapter constructs an exact `Map<usize, Seq<u8>>` from those
returned outcomes and proves that its domain equals the observed required
artifact positions. It does not substitute an independently supplied complete
inventory for the actual frame positions.

The inductive `artifact_prefix_step` theorem covers each concrete scan step;
`artifact_next` proves that complete typed outcomes determine the next required
read. The production constructor's acceptance predicate is proved in both
directions, so the nonempty successful witness establishes an actual usable
seal. Controls cover a missing read, wrong child, incomplete execution, failed
wait and an observed empty child. Native controls also exercise reordered,
duplicate and outside outcomes, multiple child intents, malformed/truncated
journals and a real spawned child with a normal nonzero exit. The existing
package-attribution fixture retains its original assertions under a synthetic
completed-provider envelope.

`managed/source_inputs.rs` preserves the old successful dependency parsing,
lexical aliases, canonical destinations and output ordering. Empty or unusable
dep-info rules withhold coverage. Previously uncovered source hints remain
uncovered rather than acquiring coverage from a later empty stream. Establishing
a replacement complete source inventory after such a refusal remains a separate
recovery policy; this slice does not invent one.

The standalone [undeclared build-script input fixture](../../corpus/repros/undeclared-build-input/README.md)
records an ordinary Cargo control in which an undeclared external read generates
an included Rust constant. Cargo keeps the previous constant until an explicit
declaration or a clean build makes the generator run again. Its Sail sequence is
an unqualified qualification plan, not observed proof-reuse acceptance. The
fixture specifies the opaque generator I/O boundary that declaration journals
alone cannot close.

These are **declaration-provider** proofs. The following boundaries remain:

- The native JSON/make parsers, package attribution, path encoding and conversion
  from the retained raw messages to typed events are source-bound reviewed
  adapters, not stock-Verus-proved parsers. The journal retains recognized Cargo
  artifact/build-script messages; it is not a full raw stdout transcript.
- The adapter's actual `spawn`/PID/stdout EOF/`wait` correspondence and protected
  endpoint append/read semantics are exercised by real-child controls and
  structurally owned Rust code. The Rust standard library and OS primitives are
  not proved by this component. Removing an entire invocation before its
  pre-spawn intent is recorded is outside this endpoint's observation domain.
- A completed journal establishes accounting for observed declarations, not that
  Cargo/rustc emitted every physical compiler input, environment observation,
  generated file or dynamic-library access. Compiler dep-info completeness and
  independently expected invocation coverage remain explicit provider contracts.
- Reading a dep-info file after compilation does not prove an atomic relation
  between a previous executable hash and the subsequently launched process, or
  that the dependency file could not be replaced. Compiler-launch identity,
  actual consumed source bytes and filesystem observation semantics remain open
  refinement obligations. No digest injectivity or complete-filesystem axiom is
  introduced.

The [issue #377](https://github.com/Chelis-Lang/sail/issues/377) regression
against the original updater body demonstrates that a missing
artifact journal could set declaration coverage to true. This is an accounting
defect witness, not an end-to-end false-proof demonstration. Final macOS/native
Linux gates and the managed failing-first workflow qualify the integrated PR;
focused controls alone do not establish those broader results.

## Source-shared production refinement

The dependency-free `sail-acceptance` leaf compiles the same Rust function
bodies for Cargo and direct stock Verus. The syntax macros erase contracts and
proof statements for Cargo and expose them to Verus; they do not replace the
executable comparisons, branches, loops or guard transitions. No adapter is
allowed to pass an `unchanged` Boolean in place of actual observations.

`InputFact` contains the namespace, exact encoded lexical name and a typed
`InputValue`: present identity bytes, observed absence or an unreadable outcome.
Native content rows use namespace zero and content-digest bytes; lexical alias
and linked-ancestor rows use namespace one and resolved path bytes. Paths and
alias destinations are compared exactly. Inventory sorting here orders metadata
keys for a checked join; it does not normalize solver query bytes.

The native sorter supplies forward and inverse index vectors. `permutation`
checks both directions, every bound and the complete length in linear time.
`input_order` additionally checks strict adjacent key order, with a proof that
all raw keys are unique. A missing, repeated or out-of-range position cannot
hide behind a plausible selected count. `byte_eq` and `byte_lt` prove exact
sequence equality and lexicographic order without digest axioms.

| Production call | Source-shared executable obligation | Model refinement |
|---|---|---|
| `fold_contents` in `managed/input.rs` | `reads_complete` checks the independent required-name ledger against every successful read outcome; missing, failed, extra and duplicate outcomes cannot establish capture completion | Every declared required key has a present readable observation in `input_map` |
| `bindings_current` in `managed/input.rs` | `compare_readback` checks actual canonicalization outcomes against all original aliases and linked ancestors | Exact lexical key and resolved destination identity |
| `stamped_snapshot` in `managed/input.rs` | `OriginalCapture::begin` precedes content reads; `finish` accepts only present, equal before/after stamp bytes and binds the produced revision | A sealed guard retains the original stamp and revision bytes |
| `unchanged_since` in `managed/input.rs` | `OriginalGuard::retained` checks the bound revision and both supplied/current stamp bytes against its privately held original stamp | A fresh supplied stamp cannot relabel older guarded bytes |
| `project_guard` in `managed/input.rs` | `selection_covered` checks every selected key/value against the original exact source inventory; `OriginalGuard::project` preserves the old stamp and binds the projected revision | The selected map is an exact submap of the source map |
| `check` in `managed/read_check.rs` | `compare_readback` receives separately built expected and actual observed ledgers, checks their full unique domains and every readable value | Accepted equality implies equality of complete interpreted maps and readable outcomes |

Capture no longer folds `files.zip(reads)` without accounting for unmatched
positions. Each required file gets an explicit outcome; a missing result is
unreadable, and extra results prevent completion. The original digest encoding
for complete successful reads is preserved. Projection retains the original
observation's entire path namespace through an immutable shared native path
set. It does not attach a stamp sampled after projection to old content bytes.
A later guard check still checks that original namespace, including paths that
were needed by the full observation but do not appear in the selected revision.

The core guard has no ordinary `Clone` implementation. Its checked `duplicate`
factory proves preservation of the original stamp and bound revision. Native
snapshot cloning calls that factory and shares the original path set; every
privileged core constructor remains inside an explicit verified envelope.

`Readback` has private fields and no deserializer. Its constructor proves both
that accepted equality matches the exact checked sorted rows and that every raw
row appears on the other side with the same key and readable value. The proof
adapter constructs `input_map` from the entire actual row sequence, derives key
uniqueness from the checked order, and proves exact map equality. It does not
assume that a caller-selected domain was complete. The context theorem retains
the actual request, scope, runtime, mode and solver fields while replacing only
the modeled input map; it does not manufacture those fields from an opaque ID.

Read-time absence uses a successful not-found primitive result. A file-open,
content-read or canonicalization failure remains `Unreadable`. A directory
appearing at a formerly absent configuration path is a change. The existing
reporting precedence is preserved: a concrete difference gives a changed
revision even if another observation is unavailable; unavailability without a
known difference gives Unknown. Currentness remains independent of successful
verification, so a fresh failed verification can remain Current.

The source-shared refinement has nonempty accepting controls with source,
alias and absent-configuration facts; reordering, incomplete permutation,
unreadable, stale-restamp, ABA and fresh-capture-after-exact-revert controls.
Negative mutations remove actual value comparison, current-stamp validation,
read-presence validation and permutation inverse checks. Each must fail a
required executable contract or refinement theorem under the same complete
stock verification scope. Native tests separately exercise missing/extra read
outcomes, duplicate retained records, real unreadable paths, ancestor retargets,
restored timestamps and supplying a fresh stamp for an older snapshot.

### Tagged native read results

Native content, alias, metadata-stamp, retained-read and receipt-artifact workers
return their actual input path with the raw outcome in one owned result.
Borrowed path descriptors avoid cloning paths or rereading files. The observed
ledger is built from these returned keys; the expected ledger is built
independently from requested paths. No caller reconstructs a result's identity
by zipping the expected path list with an untagged result list.

The generic executor can reorder or omit whole results without silently
assigning one path's outcome to another. Complete capture, retained-read,
metadata-stamp and artifact checks validate the full independent key domains;
dropped, extra and duplicate keys cannot establish completeness. Content and
stamp folds then look up each outcome by its exact returned name while retaining
the original sorted path/revision byte encoding. Core key/value validation and
safe Rust ownership provide this structural defense; this is not a machine
proof of `parallel::in_order`, standard-library I/O or path encoding.

`FileDigests` prefetch keeps the canonical and lexical descriptor with each
hash. It checks successful returned descriptors against the requested set;
missing or failed prefetches stay uncached for the existing later read.
`PhaseDigests` validates the full ledger of returned outcome records before
memoizing their own keys, retaining errors as errors. Its descriptor-presence
markers certify that an outcome record exists, not that the underlying file
was read successfully. Actual receipt matching still checks each raw result
and exact digest. Normal successful and failed-read retry behavior performs
the same physical reads as before this tagging change.

Native controls reverse real read-result vectors and check unchanged output,
then swap physical file contents or alias destinations and reverse results
again. These changes cannot cancel through positional relabeling. Additional
controls cover missing, extra, duplicate and descriptor-mismatched outcomes,
including metadata absence and phase-local artifact memoization.

The remaining six positional readers also use tagged result ledgers:

| Native reader | Independent descriptor and failure behavior |
|---|---|
| `source_build::artifact_hashes` | Each result owns the portable runtime name, actual file path and digest/error. Both names must match the complete requested inventory; keyed folding preserves the original first-error priority. |
| `adapter::driver_dependencies` | Exact extern paths travel with digest/error. Repeated identical path requests are coalesced before reads; keyed folding preserves the first requested path's error priority and existing successful map bytes. |
| `managed/absent.rs` | Existence results retain their original paths. Missing, extra or duplicate results leave the native observer unready; they cannot become a complete empty watch set. The existing `Path::exists` primitive behavior remains a platform boundary. |
| `managed/dependency_snapshot.rs` | Each content/size result retains its original artifact path. Complete domain validation precedes keyed folding and the original per-file/total byte limits. |
| `managed/shipped.rs` | Each result owns the portable sysroot name, actual file path, complete digest and size. Both names bind; the original sorted portable map encoding is preserved. |
| `managed/runner.rs` | Target revisions retain their exact package identities. A complete keyed map replaces positional vectors at every receipt caller; unavailable identities mark receipts dirty or prevent completed-receipt publication. |

The shared native `parallel::exact_descriptors` adapter checks these actual
identity/binding rows through the executable core. These rows describe native
record identity, not successful physical I/O. The actual raw outcome still
controls success, error or refusal. Repeated exact package identities across
receipts are captured once and looked up for each receipt; duplicate returned
outcomes remain invalid. Literal request coalescing changes neither solver
query bytes nor successful revision/portable-map encodings and adds no physical
reads. Invalid projected revision batches are recomputed from fresh tagged
reads; invalid fresh batches remain unavailable.

The controls reverse real runtime, dependency, sysroot, artifact-body, absence
and target-revision results, reject incomplete/duplicate/foreign descriptors,
and check changed physical inputs cannot cancel against reversed worker
completion order. Dependency-body limits and runtime first-error priority have
separate checks. Linux watcher readiness still needs native Linux validation;
the descriptor-mapping control also runs on macOS without claiming Linux OS
behavior. Other native predicate-only aggregation and provider construction
remain separate refinement obligations; this does not claim every production
provider or the generic executor itself has been proved.

### Primitive and provider boundaries

The completed map theorems concern the identity bytes actually supplied by the
native adapter. They do not prove any of the following by renaming an external
premise as an inventory-completeness or currentness predicate:

- **Content digest equality:** native file rows contain SHA-256 identities,
  rather than full file bytes. Relating an equal pair of those identities to
  physical byte equality needs collision exclusion for the actual compared
  content pair, plus correct hashing/read completion. The proof assumes no
  globally injective digest function. Aggregate revision hashes and mutation
  stamp hashes have their own encoding/collision obligations.
- **Observation primitives and ordering:** the native adapter must return the
  outcome of the named open/read, canonicalization and metadata operations,
  preserving exact encoded paths and distinguishing not-found from failure.
  `stamped_snapshot`'s actual before/read/after ordering is visible in the
  production code; the proof of a byte-equal guard alone does not establish
  that the OS prevents concurrent changes or that every relevant mutation
  changes a metadata stamp. The model's monotone epoch is not a hash value.
- **Discovery and selection:** the compiler/Cargo/build-script providers still
  must enumerate actual consumed files, generated inputs, configuration,
  absence and aliases with correct package attribution. Checked projection
  proves every proposed selected row is an original row; it does not prove
  that the provider requested every input of the selected compilation.
- **Read-point atomicity and authority:** parallel reads happen over an
  interval. The native observation protocol must justify a consistent result
  at the API observation point and bind that observation to the executed
  request/runtime/solver. This component proves no claim of linearizable OS
  observation or launch integrity. Fresh retained reads compare actual bytes
  again; their physical observation completeness remains an adapter boundary.

These distinctions preserve the exclusions in [freshness](../freshness.md)
and the [trust contract](../trust.md). In particular, the identity-level
simulation does not discharge Linux/macOS filesystem correctness or the whole
compiler-input closure. The lifecycle model below states the stronger logical
transaction contract that those providers must implement.

## Transaction lifecycle

`InputWorld` records a structured `ModelContext`, a mutation epoch and an
observation point. Each input key preserves a role and its lexical identity.
`ReadOutcome::Present` records resolved identity and bytes; `Absent` and
`Unreadable` are distinct variants. Source, linked-ancestor, generated-file,
configuration, environment and runtime roles have separate keys even when
names or bytes agree. The epoch and point are mathematical model values, not
an assertion that a filesystem timestamp or hash implements a monotone clock.

`RawCapture` is an unfinished transaction with private fields. It records the
original context and epoch, beginning point, required key sequence, actual read
witnesses and an invalidation flag. It is not usable input evidence.

| Transition | Checked behavior |
|---|---|
| Begin | Retains the original epoch and starts with no read witnesses |
| Read | Appends the modeled outcome for the exact key at the read's point and epoch; off-plan, repeated, out-of-order or unreadable observations invalidate the transaction |
| Change | Changes the explicit input/execution context and advances both epoch and point; old observations keep their old epochs |
| Tick | Advances the point while preserving every input fact and execution-context field |
| Invalidate | Makes sealing unavailable, including after cancellation or an observation failure represented by this event |
| Learn requirement | Expands the required inventory and invalidates the old capture; newly discovered generated or declared inputs cannot inherit old completion |
| Reobserve | Begins a new empty transaction; it cannot attach a fresh stamp to the old read witnesses |
| Project | Copies only existing reads with exact requested keys and unique in-range positions; validates the complete projected domain and preserves the original epoch |
| Seal | Validates the whole raw transaction and constructs private usable input evidence at the actual final model point |

`capture_valid` relates every captured read to the world: points and epochs are
within the transaction's observed interval, and a read still at the current
epoch has the actual current modeled outcome. Equal original/current epochs
also preserve the complete original context. Begin establishes this invariant;
every transition preserves it; `all_input_executions_preserve` proves it for
arbitrary finite sequences, including projection, mutation and reobservation.
No premise asserting that an arbitrary platform stamp detects every mutation
is used in this lifecycle induction. A modeled Change advances the epoch by
construction; proving that actual platform changes produce the corresponding
event remains a separate obligation.

`seal_capture` requires that established invariant and checks all of:

- The transaction is not invalidated, the original epoch still matches, and
  the exact request, scope, runtime, solver, mode and input map remain the captured ones.
- The required domain is nonempty, has no duplicate keys and equals the entire
  explicit requested input-map domain.
- There is exactly one matching ordered read witness for every required key;
  every role is supported and every outcome is readable.
- The shared context is well formed. An unreadable entry cannot be represented
  by a successful absence observation.

`InputEvidence` has private construction and no deserializer. Its `view()`
returns the exact structured context; `point()` returns the validation point;
`valid_at(world)` relates its complete read witnesses to that world's actual
map. `input_evidence_facts` exports these properties for composition without
exposing construction. The required-domain check concerns explicit modeled
keys, not a Boolean whose name assumes the compiler's closure complete.

`input_execution_seals_only_current` checks the final transaction of any finite
execution. It returns evidence exactly when sealing is admissible and proves
that every returned context and point are the final modeled context and point.
The closed finite-trace control includes a real nonempty accepting execution
and rejection of the same reads followed by an input mutation.

## Projection, retained reads and exact reversion

A projected context must preserve request, runtime, solver and mode and be an exact
restriction of the current input map. The declared required keys must cover
that projected map exactly, without duplicates. Existing read positions must
be unique and match each requested key. A projection with the same scope, or
one claiming whole scope, cannot drop any of the full map's keys. A valid
partial projection retains its exact partial selection; it does not become
whole-target evidence.

Projection preserves the original capture epoch and read witnesses. It remains
a raw candidate until final validation after the batch. A mismatch invalidates
the projected state, and subsequent reobservation starts empty. Actual
compiler closure and dependency selection must still justify which map a
requested partial selection denotes.

Retained input evidence describes its original observation point, not every
future point. `refresh_retained` requires a newly completed transaction begun
strictly after the retained evidence's point, checks the exact old context
against the newly observed context and returns newly sealed evidence at the
read point. Its postcondition also proves that every returned read witness is
later than the old evidence. `capture_unchanged` additionally proves that all
its read witnesses occur at the supplied world point and are after every
strictly earlier cutoff; composition can consume that guarantee without
opening the private token. Merely moving the old raw capture to a later
point does not pass this factory.

Mutation followed by exact restoration advances the modeled epoch twice.
The old capture cannot seal even when the input map has returned byte-for-byte
to its original form. A fresh complete capture after the revert can match the
old context and authorize input reuse at that later point. This distinguishes
safe content reversion from reusing an old guard through an ABA change.

The earlier `Atom`/vector comparators remain checked independently. They prove
executable exact-byte identity and selected-position comparison, including
lexical/resolved bindings and presence. Reordering conservatively misses;
no path normalization or digest injectivity is used. The earlier
`guarded_observation_unchanged` lemma is conditional on an explicitly covering
counter; it does not independently establish a concrete stamp's coverage.

## Environment and authentication selection

The current adapter excludes `SAIL_DEPENDENCY_READ_TOKEN` and
`SAIL_PRIVATE_GIT_READY` from implicit proof-context selection. Fetched source
bytes remain source inputs. Explicit `env!`/tracked compiler reads and build
scripts' declared reads still bind each variable's exact value or absence.

`environment_is_bound` verifies the corresponding decision shape: a declared
read is always bound, while authentication-only plumbing is excluded from
implicit selection. The credential control checks both cases. A modeled Tick
may represent a plumbing change only when the entire effective input context
is unchanged. A declared credential read changes the environment input fact
and must instead be modeled as a Change. Relating actual variable names,
compiler declarations and tool behavior to this classification remains part
of the environment-adapter refinement.

## Proof checking and controls

The [owning checker](acceptance-proof-checker.md) captures this module and the
shared context from Git, generates the complete driver and invokes the validated
stock Verus and pinned Z3 directly with `--no-cheating`. It checks exact
whole-driver counts and the complete function inventory. Sail's driver, fork,
shim, caches and managed receipts are not proof authorities.

```sh
python3.12 scripts/check-acceptance-proofs.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_PROOF_OUT"
```

The output directory must be outside the checkout. The manifest and owning
[validation record](../validation/evidence-acceptance-models.md) identify the
actual checked source, runtime, scope, counts and platform qualification. The
expanded module has passed the captured-driver macOS compatibility check; that
is checking an inductive model proof with stock Verus, not production acceptance or a proof
that an OS adapter refines this model.

The source adds no project `assume`, `admit`, `external_body` or external
specification. Stock Verus, Z3, pinned vstd, the compiler and the validated
runtime remain trusted. The code uses exact bytes and structured maps; it does
not model a hash as injective.

Closed controls check nonempty successful captures with all six roles,
read-time reuse, rejection of old restamped reads, mutation/ABA/exact revert,
partial reads, unreadable outcomes, valid partial projection, changed-solver
projection refusal, duplicate or
incomplete projection, false whole-scope claims and credential selection.

Deliberately broken source controls cover the actual obligations:

| Broken check | Required verification failure |
|---|---|
| Omit alias destination comparison | Executable atom comparison no longer implements exact input identity |
| Omit selected-position bounds | Selected indexing is no longer justified for every request |
| Omit the original epoch in sealing | Old or ABA observations could claim new-point evidence |
| Omit equality of required and context-map domains | Observing a plausible subset could claim complete input evidence |
| Omit solver equality during projection | A different selected solver could inherit a projection without the required context-change boundary |
| Omit the later transaction boundary for retained reads | Old read witnesses could satisfy a required fresh-read projection |
| Omit an observed frame's child identity | A foreign child frame could satisfy provider completion |
| Omit the final retained-frame count | A truncated or augmented provider trace could seal |
| Omit the actual dep-info outcome's frame identity | A read from another artifact could satisfy a required position |
| Omit successful raw-read presence | An unreadable artifact could enter the observed input map |

The checker mutates temporary captured copies using unique manifest needles,
retains the same verification scope and requires actual verification errors.
Parse errors, dropped proof functions and resource-limit failures do not count
as successful negative controls.

## Remaining concrete refinement

The completed lifecycle and source-shared identity obligations above leave these native/provider interfaces open:

| Production boundary | Remaining relation |
|---|---|
| `managed/input.rs`: `Bindings::observe`, `collect`, `stamped_snapshot` | Preserve every consumed lexical source and linked ancestor, canonical destination, bytes and absence; distinguish errors; justify the observation point |
| `managed/source_inputs.rs`, `managed/build_inputs_sources.rs` | Relate the explicit required map to actual compiler-consumed files and generated inputs, including facts learned after compilation and consuming-package attribution |
| `managed/input.rs`: `closure_digests`, `closure_bindings`, `Scope` | Derive the complete requested map and correct partial restriction, preserving declared external aliases and Cargo manifests/configuration |
| `managed/input.rs`: shared projections, reobservations, `unchanged_since` | Relate actual original stamps and mutation observations to the modeled Change/Tick distinction; preserve the original guard across the entire batch and reobserve unsupported cases |
| `managed/runner.rs`: context/environment/runtime identities; `managed/environment.rs`; receipts | Bind the actual executed runtime, options, effective environment, declared reads and source map to the structured context |
| `managed/read_check.rs`: `entries`, `record`, `check` | Reconstruct fresh read witnesses at the API observation point and preserve explicit absence/bindings; deserialize only candidates and validate before usable authority |

In particular, these interfaces are not discharged by:

- An inventory marked complete, a count of observed rows, or an assertion that
  a function returns all effective inputs. The real compiler-input closure is
  still unproved.
- Filesystem metadata that can miss mutation classes. The exclusions in
  [freshness](../freshness.md) remain exclusions on Linux and macOS. Atomicity,
  counter coverage, encoded path bytes, read failures and point ordering across
  processes or recovery need concrete justification.
- Digest equality without an explicit cryptographic assumption or byte check.
  Runtime immutability/launch binding also remains part of the
  [trust contract](../trust.md).
- Loading a durable successful record, scheduling hints or mutation notices.
  Recovery must establish valid fresh observations or refuse currentness;
  private model tokens do not verify the production serialization path.
- Calling these proof functions from an unverified adapter. Production does
  not compile or call this module, and source sharing plus every acceptance
  caller still need implementation refinement, conformance and native cost
  qualification.

The model proves complete observation of its explicit requested map and safe
reuse within its transition system. The executable refinement proves the
actual input core preserves exact declared identity and guard bytes. It does
not prove that every real compiler/filesystem observation produces that map
or that every production Current response follows the complete modeled
authority transaction.
