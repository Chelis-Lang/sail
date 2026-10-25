# Query association and local origin model

**Status: checked local-origin model plus source-shared production queue,
replay, publication and disk-envelope operations. The bounded simulations do
not establish a whole-service origin or I/O theorem.** This component of the
[evidence-acceptance design](evidence-acceptance-core.md) is tracked by
[#346](https://github.com/Chelis-Lang/sail/issues/346). The source-sharing
boundary and dependency edges are recorded in [ADR 0079](../decisions/0079-source-shared-acceptance-refinements.md).
Solver input and corpus sources are unchanged.

## The proved boundary

[`query_association.rs`](../../proofs/acceptance/query_association.rs) no longer
starts a session with an arbitrary sequence of supposedly admitted replies.
The session's cache and origin ledger are computed by replaying local-store
actions from an empty authority state. The initial disk image is arbitrary
untrusted data. A recovered record must be connected to a prior publication
from an observed trusted live run; a self-consistent history embedded in the
record is insufficient.

The principal statement, `executed_reply_has_admitted_origin`, follows from
induction over every finite session action history. Each reused reply has
an origin admitted by the derived store, and its bytes and binding equal
that origin's entry. Admission means all of the following:

- The origin belongs to the publication ledger built by the store actions.
- Its run identity, context, solver and event history match a prefix of the
  corresponding observed live-run history. Untrusted disk replacement cannot
  edit this ledger or manufacture a modeled solver observation.
- The selected entry comes from the ordered command/reply pairing of that
  run. The run closes input, stdout and stderr, terminates successfully,
  remains quiet, consumes the complete output and pairs every command/reply.
- The origin entry equals the stored/recovered payload exactly.

`executed_reply_has_exact_binding` separately proves that every delivered
entry matches its requested position, solver, mode, complete raw input prefix
and command endpoint. Combining these results excludes serving one query's
admitted answer for another query. Mathematical soundness of the solver
remains trusted; the model does not infer it from response shape or hashes.

Fresh replies are tied to the current solver's ordered `live_observed`
stream. Cached replies require the stronger completed clean-run origin.
Forwarding a fresh response does not itself establish successful verification;
the verifier/report acceptance component must establish completion and success.

## Production source sharing and bounded simulation

[`sail-acceptance::query`](../../crates/sail-acceptance/src/query.rs) contains
ordinary executable Rust compiled into Sail. Its source envelopes erase only
specifications, loop invariants and proof statements for Cargo. Stock Verus
checks the same executable signatures, control flow and function bodies,
using the captured [syntax definitions](../../crates/sail-acceptance/src/syntax.rs).
It does not trust an external specification of an independently implemented
queue.

The production adapters now use these operations:

| Production path | Checked executable operation | Established boundary |
|---|---|---|
| Shim `cached::Pairer` | `CorrelatedQueue<Expect, Vec<u8>>` owns both queues; `take_pair` returns a private `BoundPair` | Both oldest actual payloads are removed together; a missing side or permanent stop changes neither queue. Every unmatched suffix is preserved exactly. |
| Shim replay output | `ReplayCursor::advance` | Exactly the already-served prefix is suppressed. The countdown cannot overflow while forwarding an arbitrarily long fresh suffix. |
| Shim `spawn_storer` | `PublicationBatch<Entry>` | Checked byte accounting bounds the actual pending payloads; release returns exactly their original values in order, or no values. |
| Shim publication adapter | `PublicationGate::allows` | Every supplied closure observation is required. The adapter requires input completion, active pairing, empty queues, successful exit, quiet stderr EOF and successful forwarding before opening the storer's atomic gate. |
| Daemon write-behind | `Fifo<Job>` | The actual immutable key/value/metadata job leaves in insertion order. Mutex behavior, memory insertion and persistence are outside this FIFO proof. |
| `parse_entry` disk admission | `RecoveryRecord<Meta>::recover` | The original metadata and exact reply bytes survive only if decoded version, requested key and computed payload digest all match. Errors preserve the existing version/key/hash priority. |

The two-stack FIFO is proved directly over its complete ordered payload
sequence. Its stack reversal has a checked loop invariant and decreasing
measure. The generic correlator holds the production expectation itself,
including its key and command metadata, rather than a separately supplied
index or success tag. Each shim response entry retains the returned private
`BoundPair` through held-check effort annotation and publication buffering;
only the wire conversion consumes it into its original parts. A silence
certificate is a separate entry variant and is not presented as a solver
response pair. Because this ownership retains exact command text until batch
release, the storer charges that text alongside reply and command-name bytes
to its existing 64 MiB pending-byte limit. An oversized batch is discarded
without changing forwarded output.

[`proofs/refinement/query.rs`](../../proofs/refinement/query.rs) calls these
actual executable operations and proves simulation into the existing model.
`queue_refines` relates every outstanding production expectation/reply to the
model's exact unpaired suffix. Arrival, paired removal and permanent stopping
preserve that relation. `replay_frame_refines` relates the production countdown
to the model's `seen`/`skip` suppression condition. `publication_refines` proves
that a complete gate releases only the model's ordered entries, without
changing the actual buffered payloads. The simulation is generic over maps
from owned expectation/reply values to model bindings/bytes; establishing
those maps for the shim's parsers, hasher and process observations remains a
separate adapter obligation.

Publication now refuses an otherwise successful, quiet producer session if
its input is incomplete, correlation stopped or expectations/replies remain
unmatched. This narrows cache writes; it preserves forwarded bytes, exit status
and diagnostics. Supported complete sessions continue to populate and reuse
the default cache. A real producer-fault regression runs a solver wrapper that
exits successfully after its first response: direct and cached output match,
and neither repeated cached run publishes the earlier response. This is a
bounded publication regression, not a demonstrated false proof.

Disk integrity is deliberately a smaller theorem than solver provenance.
`RecoveryRecord` does not turn the header's `provenance` string into an origin.
JSON extraction and digest computation must pass the actual decoded fields and
hash of the actual owned reply. The production disk format does not persist
this model's prior-publication ledger. Therefore the `Store::Recover`
clean-origin premise still needs protected prior writer history; this source
linkage does not prove arbitrary preexisting, shared or bundled entries true.

## Construction and lifecycle

| Boundary | Modeled construction or transition | Preserved fact |
|---|---|---|
| Trusted run | `StartRun`, then `ObserveRun` events | Solver identity, shared execution context, raw input/output fragments, command endpoints, framing, pairing and termination are captured in one append-only history. |
| Publication | `Publish(run_index, reply_index)` | Only a complete clean recorded run can mint an origin ledger entry, available claim and pending local write. It accepts indices into observed history, not a caller-supplied origin. |
| Pending memory | Available claims while writes are queued | Every claim already has a completed origin; durability is not needed for mathematical answer provenance. |
| Write | Start temporary file, finish payload, then commit final name | A temporary or partial file is not a recovered entry. Commit validates payload/origin correspondence. |
| Crash | Drop available, pending and temporary state | Previously committed records and ghost prior-publication history remain; pending writes cannot become durable through a crash. |
| Recovery | Validate a committed complete record, exact payload and prior ledger membership | Corrupt, partial, temporary or unpublished records cannot mint serveable evidence. |
| Session reuse | Match a derived claim's complete SMT binding | Every reused reply retains its admitted origin through serving and fallback; an entry for another solver, mode or raw prefix cannot serve this request. |
| External imports | Shared and bundled import actions are refused | Neither a signature nor an arbitrary external provenance field enters this first local-origin theorem. |

The preserved ledger is **ghost history of earlier trusted publications**, not
an implemented disk certificate format. It spans process generations in the
model so recovery can be related to the writes that preceded a crash. A
legitimate preexisting local cache needs that prior protected-writer history
in the refinement; the model never presumes an arbitrary startup cache true.
Without an origin in that history, even a structurally valid record with an
internally consistent purported run is refused.

Shared and bundled origins are explicitly excluded from this model. Production
Sail supports them, so a whole-service theorem must either include a reviewed
writer/refinement contract for those paths or restrict its admitted origins.
This model does not silently change production's backend or cache policy.

## Exact current context and transcript composition

The component uses `ModelContext`, `Mode` and `SolverId` from the captured
[shared context](../../proofs/acceptance/context.rs). A current session retains
the exact request, scope, runtime, solver, mode and effective input map throughout
execution. Session/live-run validity requires the observed solver to equal
`ModelContext.solver`; mismatched run starts cannot publish. An origin may
come from different caller inputs when its complete
SMT query binding is identical; requiring the old caller input map to equal
the current map would incorrectly forbid legitimate solver-query reuse.

`session_transcript` projects the solver identity, complete raw input,
ordered command endpoints and delivered reply bytes into `SolverTranscript`.
`executed_transcript_projects_observations` proves that projection from a
checked execution. `session_complete` means the delivered and requested
command counts agree. It is **not** a process-success or verified-result
predicate. Composition must match the transcript inventory of every completed
verifier invocation to complete query executions with the same current
context; an unrelated or empty session cannot substitute for observed work.

The specification proves arbitrary finite histories, not a bounded
enumeration. Reply/input fragmentation, ordered pairing, exact replay,
suppression of already served responses and permanent departure from cache
serving remain covered. Unsupported or inapplicable actions cannot mint
additional evidence.

## Positive and broken controls

The nonempty witnesses exercise actual modeled admission paths:

- A readable nonempty input map produces an observed one-query run, clean
  publication, pending write, complete atomic commit, crash and successful
  recovery from the committed record.
- An unpublished startup record with a self-consistent clean-run payload is
  refused because it has no prior publication in the ledger.
- Partial records and changed reply payloads are refused after disk
  replacement; a crash before rename leaves nothing recoverable.
- A run that emits stderr cannot publish, even if it otherwise completes.
- A two-query session reuses one recovered answer, falls back with exact
  input, suppresses precisely its replayed first answer and delivers a fresh
  second answer. Its current effective input map differs from the origin's.

Negative controls alter real guards or transitions in temporary captured
copies and rerun the complete proof scope:

| Control | Unique source needle | Replacement | Required failure |
|---|---|---|---|
| Shift reply pairing | `binding: s.commands[s.paired as int],` | `binding: s.commands[0],` | Ordered pairing proof fails. |
| Suppress an extra reply | `delivered: if s.seen < s.skip {` | `delivered: if s.seen <= s.skip {` | Exact suppression proof fails. |
| Publish noisy runs | `&& run.quiet` | `&& true` | Completed-origin/noisy-run obligations fail. |
| Admit a changed payload | `&& frame.payload == frame.claim.entry` | `&& true` | Store preservation/corrupt recovery obligations fail. |
| Recover an unrecorded origin | `&& store.recorded.contains(store.durable[index as int].claim.origin)` | `&& true` | Origin-ledger preservation/unpublished-record obligations fail. |

The source-shared closure additionally checks a nonempty executable witness
with two distinct expectations and replies arriving asynchronously, a noisy
nonempty batch refusal, and exact-payload recovery plus corrupt-digest refusal.
Six controls mutate actual production executable statements, while leaving
their contracts, simulations and every existing model proof in scope:

| Executable mutation | Required ordinary verification failure |
|---|---|
| Reverse the FIFO by popping its front stack instead of its back stack | FIFO ordering/termination obligation fails. |
| Pop a pair without first checking the reply queue | A missing side consumes an expectation; unchanged-suffix obligation fails. |
| Treat one remaining replay response as already fresh | Exact served-prefix suppression obligation fails. |
| Remove quiet diagnostics from the publication conjunction | Publication completeness obligation fails. |
| Skip the recovery payload-digest check | Integrity admission obligation fails. |
| Release a batch regardless of its gate | Rejected batches can escape; exact release condition fails. |

Each source mutation fails with 93 verified obligations and one ordinary
verification error against the 94-obligation positive closure. These are
checked statement changes, not injected `assert(false)` controls.

A timeout or resource-limit failure does not qualify as a successful negative
control. Omitted modules, changed runtimes and narrowed scope have separate
checker refusal controls. No inconsistent assumptions are inserted.

## Direct checking

The component imports the shared context, so it must be checked with its
captured dependency closure rather than passed alone as a Rust crate root.
The coordinated runner snapshots those exact sources, generates a fixed
module driver, invokes stock Verus directly and checks the complete function
inventory. No Sail executable, fork, shim, daemon or managed receipt
participates in proof checking.

```sh
python3.12 scripts/check-acceptance-proofs.py \
  --stock-archive "$stock_archive" --stock-dir "$stock_runtime" \
  --out "$external_output"
```

For source-sharing checks, the captured driver additionally loads
`crates/sail-acceptance/src/syntax.rs`, the exact production `query.rs` as
`query_core`, and `proofs/refinement/query.rs`, alongside the existing
`context` and `query_association` modules. It selects the stock macro branches
with `--cfg sail_verus`; Cargo selects the erasure branches in the same
captured syntax file. A direct invocation is:

```sh
RUSTUP_TOOLCHAIN=1.99.1 VERUS_Z3_PATH="$stock_runtime/z3" \
  "$stock_runtime/verus" "$captured_driver" \
  --cfg sail_verus --no-cheating --output-json --num-threads 1
```

The source-sharing closure passes 94 obligations: 59 existing model/context,
24 executable operations and 11 simulation/witness obligations. Focused Rust
clippy checks cover acceptance, shim, cache and daemon; their 158 nonignored
Rust tests pass. The seven real
pinned-Z3 `cached_session` tests pass, including the unmatched clean-exit
regression and supported warm-cache reuse. These local component checks do
not replace native Linux gates or whole-corpus conformance.

The upstream pin remains Verus `0.2026.10.18.7d82994`, revision
`7d829946231d6413a398f6e4160cdfe951261dad`, Rust `1.99.1` and pinned Z3
`4.16.0`. The runner validates archive/runtime identities, clears inherited
injected settings, sets the explicit Z3 path, and requires
`--no-cheating --output-json` with whole-crate scope. The local query/shared
context driver passes 59 obligations, zero errors and normal termination;
all named witnesses remain in scope. The owning
[validation record](../validation/evidence-acceptance-models.md) binds the
combined source, runtime, environment and native qualification. This component
record does not replace the full repository gate or integrated acceptance.

## Remaining production refinement and trust

There are no assumed lemmas, `assume`, `admit`, `external_body` or unchecked
external specifications in this component. Opaque specification definitions
are proved through their explicit definitions and helper lemmas; opacity
controls solver expansion and does not introduce an axiom. Origin extraction, live-run/store replay
and observation-history construction expose their checked facts through local
lemmas; the lifecycle witness does not repeatedly unfold the full solver and
pairer history. Pairing checks the appended entry directly before proving its
quantified invariant. These proof hints preserve the same statements, complete
verification scope and ordinary solver limits. The nonempty live-run witness
constructs each observed event with checked after-push facts instead of
unfolding the complete replay history at once.

The model-level local-origin invariant is closed under its transitions. The
following physical interfaces remain obligations, not established guarantees:

| Current code | Refinement still required |
|---|---|
| `sail-key::PrefixHasher` | Every actual solver-input byte, endpoint, solver and mode enters the key. Logical byte equality must be related to concrete digests under an explicit cryptographic assumption, not hash injectivity. |
| Shim splitters, `observe`, session classification | Actual output-producing commands and whole response frames appear exactly once in order. Byte accounting does not prove SMT-LIB parsing. |
| Shim adapters around the checked correlator/replay core | Channels, exact frame offsets, expectation construction, held-check release and effort annotation preserve the proved payload interpretation. The owned queue removal and countdown bodies themselves are now checked. |
| Process launch, stdout/stderr monitor, publication adapters | Supplied closure observations belong to the identified immutable process. The source-shared gate requires all supplied observations and the batch preserves its actual owned values; OS observations and atomic flag/channel handoff remain unproved. |
| Daemon `HELLO`/`PUT`, write-behind and memory stamps | Every admitted local writer refines the observed-run publication path. A handshake alone does not prove its arbitrary `PUT` truthful. Actual write jobs use the checked FIFO, but locking, insertion/settlement and I/O correspondence remain obligations. |
| `DiskCache::put_scoped/get_stamped` | Protected prior publication, unique temporary files, atomic rename, digest computation, concrete stamps and crash behavior refine the ghost history. The actual decoded-envelope admission body now checks version/key/digest and preserves the exact payload. The implementation does not store or verify this model's ghost histories. |
| Shared/bundled admission | Excluded here; needs a reviewed writer-origin contract or independent certificate checking before inclusion. |

The source-shared changes start from production base `53d30e87`, which
includes main `237c7d8d`. The executable core and bounded simulations pass 94
whole-crate obligations with zero errors through the direct stock driver
(including 59 existing query/context obligations); the existing model scope
and controls remain intact. Full native qualification and integrated checker
evidence belong to the owning validation record. Stock Verus/vstd, pinned solver soundness,
compiler/runtime identity and the operating-system/process interfaces remain
trusted where stated. No claim of verified production I/O, default diagnostic
parity, termination, performance or an end-to-end verified Sail follows from
this model alone.
