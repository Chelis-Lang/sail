# Query association and local origin model

**Status: checked local-origin model plus source-shared production queue,
replay, exact raw-prefix feeds, publication and disk-envelope operations. The bounded simulations do
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
| Shim input capture and `observe` | `RawTranscript` and borrowed `PrefixFeed` | The append body retains exact received bytes. A candidate must match its raw interval, follow the last endpoint, and leave only whitespace or terminated line comments in the gap. Each feed contains the entire contiguous raw prefix increment, including separators. |
| Shim `cached::Pairer` | `CorrelatedQueue<Expect, Vec<u8>>` owns both queues; `take_pair` returns a private `BoundPair` | Both oldest actual payloads are removed together; a missing side or permanent stop changes neither queue. Every unmatched suffix is preserved exactly. |
| Shim replay output | `ReplayCursor::advance` | Exactly the already-served prefix is suppressed. The countdown cannot overflow while forwarding an arbitrarily long fresh suffix. |
| Shim `spawn_storer` | `PublicationBatch<Entry>` | Checked byte accounting bounds the actual pending payloads; release returns exactly their original values in order, or no values. |
| Shim publication adapter | `PublicationGate::allows` | Every supplied closure observation is required. The adapter requires input completion, active pairing, empty queues, successful exit, quiet stderr EOF and successful forwarding before opening the storer's atomic gate. |
| Daemon write-behind | `Fifo<Job>` | The actual immutable key/value/metadata job leaves in insertion order. Mutex behavior, memory insertion and persistence are outside this FIFO proof. |
| Local disk write/read framing | `encode_record`, `split_record` | The actual writer emits exactly serialized header bytes, one newline and the unchanged reply. The actual reader splits at that first newline; the checked roundtrip preserves arbitrary multiline/binary payloads. |
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
to the model's `seen`/`skip` suppression condition. `publication_refines`
retains the stricter ordered-batch relation.
`permutation_publication_refines` additionally checks a complete forward/inverse
bijection and proves that every released immutable pair equals its mapped
canonical command/reply and that every canonical entry appears. The native
held-check path can release keys `[1,3,2,4]`; a nonempty executable witness with
four distinct pairs checks exactly that order. Silence cannot enter this
pair-origin theorem because its batch element type is `BoundPair`, not the
adapter's mixed response/silence `Entry` enum. Relating the actual held-check
transitions and pair-only projection of that mixed batch to the checked index
map remains an explicit adapter obligation; ordered batch equality is not
presumed for native held-check release. The simulation is generic over maps
from owned expectation/reply values to model bindings/bytes; establishing
those maps for the shim's parsers, hasher and process observations remains a
separate adapter obligation.

`RawTranscript` also relates the actual received vector to each model binding.
`transcript_append_refines` preserves the received/fed relation across arbitrary
fragmentation. `command_bytes_refine_binding` constructs the model prefix from
the actual borrowed feed, proves it equals the received bytes through the
endpoint, and retains the supplied context's exact solver and mode.
`tail_bytes_complete_replay` proves that concatenating all feeds and the final
suffix gives the complete original input used by first-miss solver replay.
No normalized or reserialized command enters this relation. The shim checks
its hasher byte count against each checked endpoint; refusal disables cache
interpretation and preserves the original bytes forwarded to the solver.

Production command admission now additionally calls `framed_command`. Its
`complete_list` body is proved equivalent to a byte grammar: the form begins
with `(`, ends with `)`, every proper nonempty prefix has positive nesting,
and the final state has zero nesting outside a literal or comment. Doubled
quotes escape string quotes, bar-quoted symbols hide parentheses, and input
semicolon comments end at CR or LF. The first root close must be the last byte;
two complete forms cannot masquerade as one item. This is structural framing,
not validation of SMT arity, types, the complete simple-symbol alphabet or
solver meaning. Bare command-head tokens match the native lexer's lenient byte
rules, including non-ASCII bytes.

`command_head` extracts the first actual bare token only after that complete
framing check. The shim compares this checked head to its native classifier's
head before using the native command kind. Quoted heads, unmatched literals,
coalesced candidates or an unsupported native classification permanently refuse
cache interpretation and preserve the original replay input. `framed_prefix_refines_binding`
connects the returned head and borrowed bytes to the exact model binding.
`scan_reset_join` and `trivia_framing_state` establish the inductive full-prefix
boundary: completed prior forms plus actual inter-item trivia leave zero nesting
outside a literal/comment before the next framed command. This removes the
assumption that proposed offsets alone identify a complete form.

`response_frame` is likewise proved equivalent to its supported byte grammar.
A reply is one balanced output list, or one non-list line terminated by LF,
with only the four SMT whitespace bytes around it. Output has no semicolon
comments. Trimming establishes validation bounds only; it never changes stored
or forwarded bytes. `plausible` requires this checked shape both before serving
a cache candidate and before recording a paired live reply. Two complete lists,
a partial list, multiple non-list lines or an unterminated atom cannot pass as
one response. Echo response meaning remains command-dependent and the existing
exact echo-text check remains required.

The native splitters still propose top-level items. Their complete emission inventory,
command-kind/output-count semantics, option extraction, replay-frame counting,
concrete solver/mode encoding and process correspondence remain obligations
under [#371](https://github.com/Chelis-Lang/sail/issues/371). In particular,
these admission validators do not prove that every actual solver stream or
command is observed. An independent launch inventory and a complete concrete
stream history are required for that service-level refinement.

### Actual owned streaming-list kernel candidate

`query_split::ListFrame` now owns the actual pending list used by both native
splitters. Its checked constructor begins with `(`; the native opener arm calls
it for the byte it just consumed. The native
splitter passes each actual contiguous chunk to `push_chunk`, which calls the existing
checked `frame_step` for each actual byte and stops at the first root close.
Completion returns the same owned original byte vector; a pending result retains
exactly the old bytes followed by the entire consumed chunk. The returned count
identifies the exact chunk prefix consumed, leaving the next form untouched.
Input comments, doubled quotes, quoted symbols and output non-comment behavior
use the existing byte grammar. Pending finish returns unchanged bytes for the
native stray item, and does not claim completion or physical EOF. The native
lexer also calls the existing checked `delimiter` operation directly.

These actual production operations enter the complete query Stock assembly,
rather than remaining native hash bindings. Nonempty constructive checks cover
real completion and stopping before a second form; ten ordinary controls cover
original bytes, comment domain, completion, chunk cursor/order, finish, checked
copy and full-body always-pending/overconsuming substitutions. The native public
splitter `Clone` API delegates its list state to the checked exact-copy operation.
All older functions and controls remain required. Native original tests remain,
with additional unexecuted chunk-width cases for literals/comments/CRLF and
incomplete finish. No passing verifier, native or conformance claim is made for
the latest annotated source. Whole query discovery at `a182c1e4` passes 482/837
with all old functions retained. Native SMT tests pass 22 cases; strict package
Clippy then refuses three new-kernel style lints (#470). Narrow annotations
preserve the actual statements; the `e6e0e67a` native rerun passes all 22 tests
and strict package Clippy. All ordinary controls and broader parser/solver joins
remain pending. The manifest locks only the observed 482/837 query inventory;
the ten new runtime edits and corrected comparison control 151 enter bounded
whole-root diagnostics before the later complete 171-control bridge matrix.

The theorem is conditional on representable buffer append; it adds no trusted
Boolean or grammar premise on supplied bytes. The outer native `u64` offset
accounting, copying into `Item`, top-level atom/trivia emission inventory and
all callers' capacity obligations are still unproved. The native atom/CRLF and
stray behavior is preserved but remains outside this checked kernel. Full lexer
spans/classification, SMT/Session/solver semantics, launch completeness and cache
origins remain separate. The proposed dependency edge is recorded by
[ADR 0086](../decisions/0086-checked-stream-framing.md), with no third-party
dependency, reverse edge or runtime selection. Actual runtime cost remains
unmeasured; no extra full-frame validation scan is introduced by the kernel.

### Actual outer splitter and paired input-tape candidate

The next source candidate moves the actual outer machine into
`query_split::StreamSplitter`, including idle/comment dispatch, owned top-level
atoms, original absolute `StreamPosition`, list-chunk fast path, ordered item
emission, copy and explicit finish. `sail-smtlib` keeps its existing public
splitter paths and reexports the unchanged `Item`/`ItemKind` fields. Its wrappers
move the existing output vector through the shared operation; no payload copy or
second retained raw tape is added. Input delimiters are reconsidered at the same
absolute offset before the cursor advances once. Output atoms still end only at
LF; delimiter termination removes a trailing CR, while unfinished-atom finish
keeps it. Input comments end at either CR or LF, and incomplete lists remain
strays. These are the original mechanical rules, without SMT meaning.

The actual one-byte transition has an exact state/emission-shape contract, not
only a one-way claim about whatever items happen to be emitted. The chunk loop
relates every pending buffer and every newly emitted item to the same original
history and preserves all preexisting output items. The chunk loop also has an exact ordered-inventory contract against repeated
scalar transitions (`model_scan`), with inductive pending-list/first-close lemmas
for its list-chunk optimization. These specifications refine the same actual
operations; they introduce no assumed parser result or second runtime parser.
This wider theorem is implemented as a proof candidate and awaits whole-root
qualification; a raw-byte theorem alone would not exclude an omitted item. Nonempty checked candidates exercise atom-plus-stray ordering, absolute
offsets, delimiter CR trim versus unfinished finish, comments/CRLF and list
completion across chunk boundaries. Sixteen appended ordinary controls include
whole-transition always-idle and whole-chunk dropped-atom substitutions; every
one of the 171 inherited query control objects remains unchanged.

`QueryPrefix` now owns that actual input splitter alongside its original raw
transcript and hash prefix. `append_items` appends the actual raw bytes and
returns the actual splitter items in one shared operation, preserving raw bytes,
feed position and hash material. Coherence relates the pending splitter state
to that same owned transcript. The shim consumes these returned items and uses
the same parser's read-only pending-item test at input end, replacing its two
separate native append/split calls. Existing `append` still returns unit and
retains all original bytes; its discarded items grant no command authority.
The raw Vec's actual post-append length supplies representability for this input
join, rather than an assumed history or all-inputs Boolean. The standalone
splitter's representable cursor/output-vector premises and the output-side
physical tape/endpoint join remain explicit. Allocation success is not proved.

This wider candidate has no native or whole-root verifier result yet. The
manifest retains only the historical observed query 482/837 lock until actual
whole-positive discovery; new positive/function counts must not be inferred.
There are 187 query, 115 input and 121 authority controls: 426 planned unique
cases, or 435 calls with four partitions per component. The closed assembly
remains 21/9/14 files. The 16 new controls and native paired-owner fixture are
unexecuted. All prior assertions, functions and controls remain required.
Remaining qualification of the ordered-inventory theorem, lexical
spans/classification, solver interpretation, output-tape
composition, launch/EOF and cache-origin obligations stay open under #371/#372.
Runtime cost is unmeasured; no new full-frame scan, normalization, trust axiom,
wire token, verdict or cache policy is introduced. ADR 0086 remains proposed.

### Scalar policy-option readback candidate

`query_options.rs` adds a source-shared candidate for the raw `:print-success`
and `:rlimit` state used by `cached::observe`. A native lexer supplies candidate
head, keyword and value intervals. The shared operation validates them against
the complete original command: exact `set-option` head, a complete colon-led
keyword, ordered intervals, complete whitespace/comment gaps, one nonempty
bare scalar value and only complete trivia before the final close. It copies
the original value bytes, including numeral spelling, without normalization.
Strings, quoted symbols, compound values, omitted values and trailing
arguments for these two keys refuse cache interpretation. Unrelated options
retain the two tracked values; their compound values are still supported by
the existing native Session. Forwarded input, raw keys and live fallback use
the original bytes in every case.

Private `OptionState` retains those raw values through every accepted command.
The full original bare head determines whether a transition updates one key,
clears both at `reset`, or preserves both, including `reset-assertions`.
`ObservedSession::apply` passes the actual post-apply
`Session::option` readbacks to that transition. A mismatch permanently refuses
the state. `observe` also compares the actual map to the retained state before
using `expects_output` or reading the effort limit. Ordinary-command
preservation is checked against real readback each time; it is not an assumed
property of native Session. The constructor's absent values describe the native
map; the first use also checks that actual map.

| Source | Concrete responsibility and remaining boundary |
|---|---|
| `sail-smtlib` `command.rs`, `lex.rs`, `split.rs` | Propose head/kind/value intervals and top-level items. Actual list buffer/byte/chunk operations and delimiter calls delegate to the checked leaf; outer stream-emission inventory, command arity/type semantics and classification remain native obligations. |
| `sail-session/src/lib.rs` | Applies the native option map, resets and output policy. This candidate checks its two actual readbacks against raw transitions; the rest of Session remains unproved. |
| `query.rs`, `query_policy.rs`, `query_options.rs` | Check whole command framing, bare head, default head policy and the two raw scalar-option transitions. The new option component still needs direct-stock qualification. |
| `sail-shim/src/cached.rs` | Supplies original command bytes and actual Session readback, preserves forwarding, and crosschecks native head/output policy. This caller is hash-bound native source, not a stock-compiled body. |
| `query_output`, `query_output_owner`, `query_checkpoints` | Check response framing/coverage, owner/ordinal and original acknowledged-input checkpoints. They do not establish SMT command or option semantics. |

The theorem concerns raw bytes and native map readback. It does not establish
that pinned Z3 interprets these two map keys as the policy assumes. Initial
solver defaults, process arguments, aliases, other options and reset behavior
can affect solver output without changing these two map keys. Successful
command semantics, the existing native `u64` rlimit conversion and its relation
to actual solver effort also remain separate obligations under #371. No
all-command or all-option completeness premise is introduced.

The capture adds the option source plus eight native Session/parser/assembly
bindings; those native files are explicitly excluded from checked bodies.
All 115 inherited query controls remain, followed by 19 ordinary option
controls, including whole-transition refusal, swapped/omitted fields, trailing
data, readback mismatch, reset and persistence. A concrete nonempty history
exercises both updates, ordinary input, reset-assertions and reset. Four native
tests exercise the real lexer/Session bridge and refusal paths. These tests and
the new positive/negative proofs are source candidates, not executed evidence.
The old 431-obligation/759-function query inventory is intentionally retained
until whole-positive discovery supplies the actual expanded inventory; it
cannot qualify the changed driver. Inputs/authority bodies and their
308/498 inventories and 115/107 controls are unchanged. Native compilation,
solver tests, full differential checks and cost qualification remain pending.

Trailing EOF input must be only whitespace or line comments, or caching is
refused. A valid explicit exit retains its existing logical completion and
exact ignored-suffix behavior. The tests compare actual prefix and silence keys
against feeding the original unmodified input directly, and compare native
splitter items with the checked grammar at every chunk width of quoted,
commented and CR/LF examples.

### Original served-byte replay comparison candidate

`query_replay` owns each actual served cache-hit byte vector and a checked copy
of its complete original command. A private statistics tag is minted only from
validated raw `get-info`/`:all-statistics` intervals, complete framing and trivia,
and exactly one keyword. Native classification/lexer output only proposes the
intervals. Unsupported statistics-command argument forms refuse cache serving
in the plausibility check; the original input then follows live fallback.
The option-state candidate is unchanged.

The actual native `ReplayCheck` delegates to the shared `Replay` controller.
For ordinary positions the decision compares the retained served bytes to the
actual stdout slice exactly. For a minted statistics position it applies the
existing broad complete `(:...)` list-shape policy. The independent native
`is_single_response` refusal check remains in front of that statistics
comparison: checked `response_frame` alone permits leading transport trivia,
whereas the native predicate also requires the first emitted item at offset
zero. Neither check is removed or replaced by a supplied success Boolean.

The controller advances exactly one original served position after an accepted
comparison, exposes the remaining count, latches divergence, and preserves the
existing clean-short-replay decision. `comparison_step` calls that same body
and inductively relates successful steps to their actual byte inputs in original
order. Native framing refusal calls its checked permanent-refusal operation.
`CompletedStream::with_replay` consumes the actual completed stdout observation
only when comparisons are complete, its original drained suppression count
equals the original served count, and the original opaque owners match through
the existing `OwnerIdentity` contract. Equal counts from another child cannot
substitute. The join additionally visits every original suppressed ordinal in
the completed output, borrows its exact retained endpoint interval and rechecks
that interval against the original served receipt's mechanical reproduction
predicate. Its postcondition states that relation for every suppressed position.
Completed comparisons against unrelated bytes cannot substitute even with the
same owner and count ([#457](https://github.com/Chelis-Lang/sail/issues/457)).
The native process/read correspondence still must establish that this retained
output came from the actual endpoint; that physical origin remains unproved.

This theorem is correspondence to the existing mechanical comparison policy.
It does not assert semantic equivalence of arbitrary `(:...)` lists. Charter
rule 3 and ADR 0011 retain actual stored bytes, including statistics. ADR 0011
permits stored time/memory figures; ADR 0017 permits publisher resource counts;
ADR 0009 treats rlimit drift as output. Their shape guards do not establish
truthful origins or statistics-field meaning. Pinned-solver interpretation,
complete protected cache origins (#371/#372), rlimit-count extraction, reason
conversion, counter reset/monotonicity and `unknown_servable` remain separate.
No continuation-oracle normalization is authorized by this refinement
(ADR 0051 and open question 31).

The serving budget charges actual retained command and payload vector
capacities, receipt-vector capacity, and fixed receipt/controller carriers.
Those charges transfer into the solver recording budget when the prefix starts
replaying. Exhaustion retains mandatory replay comparisons and live forwarding
while withholding recording through existing bounds. Allocation capacities are
native accounting facts, not a proved RSS bound; allocator overhead and runtime
cost remain unmeasured. Retaining commands and checking their framing adds work
and can reach the existing recording/overflow bounds earlier.

The candidate appends 27 ordinary controls to the 134 inherited query controls,
including complete mint/comparison/join always-refuse controls, original byte
and order substitutions, sticky divergence and wrong-owner/count joins.
Constructive paths exercise a nonempty exact/statistics history and the actual
stdout completion join. A nonempty refusal witness completes an unsat comparison
but retains sat stdout with the same owner/count, then requires refusal.
Four native tests cover minting, owner/count/comparison joins, different actual
stdout and retained allocation accounting. They are not executed evidence.
The full-factory always-refuse mutation preserves the new prefix loop and its
obligations. Qualification of the #457 relation, witness and all controls is
pending. The recheck adds another bounded prefix scan without storing duplicate
response payloads; its runtime cost remains unmeasured.
The query driver now has twenty actual files; its old verified count and full
function list remain intentionally unchanged pending actual whole-positive
discovery. Inputs/authority controls and inventories remain unchanged, including
the lead's inherited full-scope mutant corrections (#454). Native compilation,
all ordinary controls, differential behavior and cost qualification are pending.

Publication now refuses an otherwise successful, quiet producer session if
its input is incomplete, correlation stopped or expectations/replies remain
unmatched. It also latches every solver-stdin write/flush failure and carries
the final parser/input-read completion result into publication. Finishing the
input thread alone is not evidence that all input was delivered. Incomplete
or failed delivery queues a permanent stop instead of a silence certificate.
Every expectation-channel send checks its result and permanently poisons the
same validity latch on failure. The terminal send completes before the input
side announces `done`, so lost metadata cannot race an otherwise open gate.
A closed-receiver regression exercises that real channel error and checks that
neither another failed send nor terminal metadata restores validity.
These conditions narrow cache writes while preserving forwarded bytes, exit
status and diagnostics. Supported complete sessions continue to populate and reuse
the default cache. A real producer-fault regression runs a solver wrapper that
exits successfully after its first response: direct and cached output match,
and neither repeated cached run publishes the earlier response. This is a
bounded publication regression, not a demonstrated false proof. A second
real-path fixture closes solver stdin after the first complete command or
returns successfully on a trailing partial command. Both preserve direct
output/status and leave the daemon's PUT count at zero. Removing the sticky
write-failure latch or the parser-completion latch causes the fixture to
observe two PUTs (response and silence), so it exercises each actual defect.

The stderr monitor closes its actual socket reader immediately on forwarding
or read failure and on observed EOF. It does not retain a failed reader until
solver shutdown: that ownership can leave a producer waiting on its stderr
write while the shim waits for stdout EOF. This is shutdown availability
hardening; neither the sampled hang nor its fix demonstrates a false proof.
The existing closed-stderr regression retains its five-second deadline.

`DiskCache::put_scoped` passes its actual serialized metadata vector and
borrowed reply to `encode_record`. A newline-bearing header is refused before
writing. Successful encoding returns exactly those header bytes, one newline
and that complete reply, with no additional allocation or payload copy over
the previous format. `parse_entry` uses `split_record` over the actual read
buffer; both returned slices are proved to partition that buffer at its first
newline. `record_partition_unique` and `record_bytes_roundtrip` prove that
reading a successfully encoded record restores its exact original header and
reply, even with additional newlines, NUL or non-UTF-8 bytes in the payload.
Missing separators retain `BadHeader`; JSON decode and version/key/digest
checks retain their existing error priority.

The framing theorem interprets metadata as arbitrary bytes. Serde's mapping
between `Meta` fields and those bytes, the standard slice copy into the owned
reply, hashing, successful OS write/read correspondence, rename/crash behavior
and prior truthful solver publication remain explicit obligations. In
particular, a valid record containing `unsat` acquires no solver authority from
this roundtrip. Shared and bundled codecs and their writer contracts remain
outside this local framing theorem.

The byte checks add linear comparison over proposed command bytes, a constant-space
linear structural scan of each candidate command/reply, and linear scanning of
inter-item trivia, without another raw transcript copy. Head extraction scans
only the initial trivia and first token after framing. The disk
writer adds one linear scan of its serialized header; the decoder retains a
linear first-newline search. These are correctness guards, with no measured
performance claim. Native timing and whole-corpus qualification belong to the
combined change's owning gate.

Disk integrity is deliberately a smaller theorem than solver provenance.
`RecoveryRecord` does not turn the header's `provenance` string into an origin.
JSON extraction and digest computation must pass the actual decoded fields and
hash of the actual owned reply. The production disk format does not persist
this model's prior-publication ledger. The residual protected-writer and
persistence obligations are tracked by [#372](https://github.com/Chelis-Lang/sail/issues/372). Therefore the `Store::Recover`
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

### Typed local publication observation (source candidate)

`query_publication::observe_publication` consumes the actual private
`query_delivery::Delivered` token, the actual private
`query_output::CompleteOutput` owner and the existing supplied
`PublicationGate`. Missing input/output tokens or any failed closure conjunct
refuse the operation. Its private-field `PublicationObservation` retains those
exact owners and gate; it has no unchecked or deserializing constructor.
`release_observed` consumes that observation and returns the original
`PublicationBatch` entries in their original order, or no entries when the
typed observation is missing. A caller-supplied release Boolean is not this
new transition's interface.

This is a supplied-observation/data-preservation slice of #372. It does not
assert that each generic batch entry belongs to these streams, that the
streams came from one actual trusted process, or that its gate describes that
process truthfully. The closure gate still records the actual native adapter's
wait, stderr, pairing, forwarding and input-completion observations; it is not
a new trusted-host success predicate. The complete-output invariant is supplied
by its original private EOF/coverage constructor, and frozen input still comes
from its original full-write controller. Their constructors and checks remain
part of the source inventory and required controls.

The constructive source witness uses nonempty supplied typed input/output and
a nonempty original payload batch. Missing delivery, missing complete stdout,
missing joint observation and any incomplete gate have explicit refusal
controls. These additions are unrun and unqualified. They do not refine
`StoreAction::Publish` into a full truthful-origin transition or supply its
prior-publication ledger membership.

The remaining exact joins are entry/frame/command-key association, actual
child/image/pipe and diagnostic/wait attribution, protected protocol writers,
memory insertion/settlement and physical persistence/recovery. Startup disk
data still lacks the modeled ghost prior-publication history; a self-consistent
header/hash/provenance string cannot establish it. Shared and bundled writers
retain separate open contracts. The [source trace on #372](https://github.com/Chelis-Lang/sail/issues/372#issuecomment-6426627954)
records these actual paths without claiming a false-proof reproduction or
changing production's admitted cache policy.

The native candidate preserves `DeliveredInput` during
`Pairer::finish_observed` beside the existing silence entry, rather than keeping
only its key. The ordinary pairer test wrapper delegates to the same operation;
existing assertions remain unchanged. After actual stdout EOF/coverage sealing,
the existing wait, stderr, input, pairing and forwarding observations feed the
same closure gate. `DeliveredInput::observe_publication` forwards those owned
values to the shared constructor and wraps its result in private-field
`ObservedPublication`. It supplies no additional truth or physical-origin
assertion.

`Storing::close` sends that owned optional observation through a dedicated
completion channel. The storer drains its original entry channel, receives the
observation, then calls the checked typed release before the unchanged PUT
conversion. Missing completion or any disconnected channel withholds stores;
there is no Boolean publication flag for that consumer to trust. Neither
channel reception nor the token authorizes arbitrary daemon PUT or disk
recovery. The shared generic batch still needs its exact entry/frame/key
association join before a solver-origin theorem applies.

Forwarding, replay comparison, direct pinned-solver fallback, gate fields,
buffer bounds, effort annotation and daemon/store policies stay on their
existing paths. Original typed stream/gate/batch controllers and their required
controls remain in scope. New source tests exercise actual SHA-backed input
accounting, complete output, every missing closure condition, exact pair/batch
preservation and one-time terminal transfer; their pipe/closure observations
are scripted, not a trusted solver or native I/O qualification. The shared and
native additions, normal controls, macOS/Linux gates, unchanged conformance and
validation-cost measurement are unrun/unqualified.

### Original output ownership and frame transfer (source candidate)

The next source slice distinguishes value preservation from stream identity.
`query_output::FramePayload` preserves copied bytes, ordinal and start/end from
the actual checked borrowed frame; its membership check against `CompleteOutput`
is only a value relation. Equal-valued streams can satisfy that relation.
`query_output_owner::StreamOutput` therefore owns the original controller and
an opaque ownership lease. Its `StreamFrame` can produce private-field
`OwnedFrame` only after a retained lease compares equal to that original borrow.
`CompletedStream` preserves the same original lease at EOF. Membership checks
require both lease identity and exact original frame values/position. There is
no public raw-Vec or deserializing constructor for an owned frame.

The identity primitive is the unchanged shared `inputs::OwnerIdentity`
contract: native comparison uses only `Arc::ptr_eq` on strongly retained
allocations. Its logical identity projection is not a numeric runtime label,
PID or successful-result flag. Moving that shared trait from `phase_inputs`
without changing its contract is a prerequisite captured separately by the
integration owner. These additions remain source-only until that dependency
and their complete module/type/import/function inventories qualify together.

Native `start_solver` allocates one lease beside the actual owned spawn and
extraction of that child's stdin/stdout. Private `SolverProcess` retains the
actual child and the lease for the output worker; private `Solving` retains its
other strong reference beside the original input-delivery controller. Its
terminal metadata carries the actual frozen `DeliveredInput` and the same
lease together. `observe_owned_publication` requires that input lease to match
the completed output lease through the narrow identity primitive. This is the
actual common-child lease flow, rather than an equality premise on supplied
PIDs. Actual descriptor extraction, OS I/O, executable identity and truthful
native attribution of that private constructor remain provider obligations.
The core delivered-input token alone does not prove those native facts.

The real pairer now owns `BoundPair<Expect, OwnedFrame>` through held checks,
effort annotation and the pending batch. Fresh output copies originate at the
same successful checked `frame` call; replay comparison still consumes the
original byte slices and uses its existing semantics. Before releasing any
batch, the actual storer checks each paired entry against that retained output
owner and checks the silence key against the original frozen input. Final PUT
conversion consumes only the original owned response bytes. No forwarding,
fallback, gate, diagnostic or exit path depends on this interpretation.

Swapped equal-byte owners, wrong retained leases and different input/output
child leases have explicit refusal source controls. Native script tests keep
equal byte/ordinal/range values while allocating distinct live Arc leases and
check refusal; they do not qualify a real trusted solver or OS launch. Existing
pairing, replay, held-order and effort assertions remain unchanged, with test
fixtures routed through the original checked frame controller. All new
proofs/tests/mutations, native gates, default conformance and cost measurements
are unrun/unqualified.

This does not finish Entry-to-origin refinement. The original command/prefix/key
checkpoint and complete accepted-command inventory still need their actual
production joins. The held-operation bridge is described below. Neither frame membership
nor the common lease provides the modeled prior-publication ledger, protected
protocol writer, restart initialization or shared/bundled origin contract.

### Actual deferred payload and annotation transitions (source candidate)

`query_holding::Holding<Payload, Metadata>` replaces the real native held-check
storage. Its private fields own the actual original `BoundPair<Expect,
OwnedFrame>` separately from `HeldMetadata` (the original prior resource count
and effort fields). It is not an additional history ledger beside a separate
native held pair. `replace` returns the exact displaced original payload;
`take` releases it once. `take_metadata` and `put_metadata` expose only the
annotation, preserving the private payload. An orphan annotation cannot create
a held value. Native reason/statistics parsing can therefore update effort
without obtaining mutable access to its original key, command or frame owner.

The actual statistics branch calls shared `release_before(current_pair)`.
That checked operation emits the original held pair and its current annotation
before the exact current statistics pair, then clears the held storage.
Reason-unknown entries keep their existing immediate position. The existing
`[1, 3, 2, 4]` publication-order assertion and all original effort/reason/end
release/error assertions remain unchanged. New constructive source controls
exercise actual correlated pairs, metadata detach/restore, replacement,
one-time release and that precise permitted permutation.

The native metadata update preserves the original `saturating_sub`, reason
bytes/conversion, prior-count update and effort fields. An impossible failed
metadata restoration stops recording; it does not change forwarding or process
status. The old native `Entry::effort_mut` helper is replaced by the metadata-only
interface; no existing checked queue, delivery, output, publication or model
proof function is removed. The holding controller, native mapping and controls
remain unrun/unqualified, with native gates/conformance/cost still required.

The shared operations prove exact payload retention and the actual local
release permutation. Complete native command-kind classification, the full
accepted-command/frame inventory and its model transition trace remain separate
joins. A correct holding operation cannot establish solver truth, reconstruct
missing producer observations or authenticate a restart cache.

### Frozen original command and acknowledged-prefix validation (source candidate)

`FramedCommand` now retains the actual original command slice and offset beside
its head, endpoint and borrowed prefix. `freeze` copies those exact command/head
bytes, original key and native hash checkpoint into private-field
`FrozenCommand`. The production SHA state has constant size; this does not copy
the full input prefix for every command. Later parser advancement cannot change
that checkpoint. Its key uses the original unchanged solver/mode header, raw
bytes and endpoint encoding. The shim's existing fixed `-smt2 -in` cache-eligibility
gate remains in place; the snapshot does not widen that supported argument set.

Native `OwnedCommand` also retains its parser-session Arc lease. The same
private lineage flows through that parser's actual delivery/frozen input. It
never enters a cache key, SMT byte stream or wire record. Same-session and
endpoint checks are only preliminary admission: they cannot by themselves prove
that arbitrary legacy `write`/`seal` calls delivered the original raw prefix.

The actual shared `Delivery::write` therefore optionally retains its exact
acknowledged bytes in one bounded tape, after the same real pipe write succeeds.
Failed/partial writes clear that tape and retain the original failure latch.
Capacity refusal clears recording while preserving original writes, flushes and
closure; recording cannot resume midway through a session. The original seal
transfers that owned tape and header-only seed unchanged into `Delivered` and
proves their correspondence to the actual acknowledged stream. No raw-Vec or
deserializing constructor can supply this evidence. Legacy hash-only APIs remain
available but cannot satisfy the new actual-prefix validation.

`query_checkpoints::validate_checkpoints` checks a complete forward/inverse
permutation over the original candidate checkpoints. In certified endpoint
order it checks nonoverlapping original command spans against the acknowledged
bytes and advances one hash state through their actual full prefixes. Each
original key must equal the digest computed from those actual bytes. It never
infers byte equality from hash equality. Evidence visit order leaves SMT and
publication order unchanged, including held-check release order. This is one
linear input pass plus constant-size digest work per checkpoint, rather than a
full-prefix replay/hash from the beginning for every entry.

The native pending `Store` payload carries one real `OwnedCommand` with its
limit, rather than independent mutable key/name/text copies. The existing
symbolic Pairer tests retain their exact assertions through test-only metadata
inside that same Store payload. They exercise the same pairing, holding,
annotation and release transitions; symbolic metadata is always refused by live
publication admission and cannot mint a private checkpoint. Separate positives
use one actual parser/delivery session and SHA keys; equal-key checkpoints from
another live parser session must refuse.

Snapshot objects, command/head payloads, frame metadata, the one acknowledged
tape and validation-index working storage are conservatively charged against
the existing 64 MiB recording/publication bound. The bound is not raised.
Exhaustion permanently withholds recording and can reduce reuse; original
delivery/forwarding and independently selected fallback remain unchanged.
Allocator bookkeeping is outside this existing logical payload budget.
The native charge includes the complete inline `Entry` carrier (including the
largest expectation variant, effort and enum padding), rather than adding the
command and frame sizes separately. Silence entries also pay their carrier
cost. An ordinary native regression for #456 fails with the earlier formula
(356 charged versus 476 required for one actual `check-sat` checkpoint on
x86-64) and passes with the corrected charge. This is object and logical-payload
accounting, not a hard bound on allocator capacity or total process memory.
Capture/key controls, native tests, complete-source proofs, default conformance
and the added validation/retention cost remain unrun/unqualified. Actual parser emission/semantic coverage,
physical I/O/image contracts, protected writers and restart still need their
separate concrete joins; this is not full #371/#372 closure.

### Original command and fresh-response association (source candidate)

`StreamOutput` owns the actual `ReplayCursor` alongside its original stdout
controller and lease. The production loop creates them together from the same
original served-reply count and advances that cursor at the same actual parser
events. `CompletedStream` retains the cursor's completed prefix count through
`OwnedPublication`; an independently constructed count token cannot be supplied
to the publication constructor. Every preexisting replay byte comparison,
forwarding decision and divergence failure remains required. The count records
suppression, not solver truth or proof of semantic parsing.

`OwnedPublication::validate_bindings` checks both the acknowledged-prefix
relation and the complete fresh response domain. Every candidate keeps its
original command/frame pair. In certified command-endpoint order, each frame
must belong to the original output lease and exact output partition, with
ordinal equal to the completed replay prefix plus that command's rank. Pair
count must equal the entire fresh suffix length. Missing, duplicate or swapped
original frames therefore refuse even when their bytes are identical. Sorting
only validation references preserves the existing held-check publication order,
including the statistics/reason/check sequence.

Native `validate_pairs` constructs that certificate over the same original
paired entries inspected by the private storer gate; publication consumes the
unchanged pending batch. Symbolic fixture metadata cannot pass this gate. The
working arrays are charged at six machine-word slots per candidate plus five
vector headers within the existing bound. Fixed retained delivery/publication
and incremental hash state also participate. The same atomic counter charges
actual fresh reply copies and the one input tape, so these cannot each consume
an independent recording allowance. Ordinary UTF-8 command metadata, key
bytes, solver arguments, effort rules, forwarding and independent fallback stay
unchanged. Capacity refusal can reduce caching and needs native cost measurement.

The source includes a constructive nonempty parser/delivery/output/binding
witness and missing/wrong-original controls. Native source regressions cover
the actual Pairer held-release permutation, equal-byte ordinal swaps, missing
frames, equal-session/wrong acknowledged prefixes, absent capture and original
complete/incomplete replay cursors, and unchanged original wire metadata after
release. They are unrun and unqualified. This is a
checked relation over supplied original parser/frame observations, not full
semantic event coverage or a proved native storer/OS refinement. Actual delimiter
and mode coverage, pinned executed image, protected writers, arbitrary startup
content and crash/restart/recovery remain separate open requirements.

The qualification candidate extends the canonical query closure with
`query_output_owner`, `query_holding` and `query_checkpoints`, retaining all 78
earlier query mutations and adding 37 ordinary-Verus controls. The input and
authority inventories retain their 115 and 107 controls. Exact module/import,
private-constructor, host/script, native key/shim and declaration projections
are bound before complete positive-root discovery; expected function/count
inventories change only from successful stock output with final source/runtime
checks. These selected controls are not yet qualified, and native Rust mutation
results never count as ordinary-Verus negative results.

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
Seven controls mutate actual production executable statements, while leaving
their contracts, simulations and every existing model proof in scope:

| Executable mutation | Required ordinary verification failure |
|---|---|
| Reverse the FIFO by popping its front stack instead of its back stack | FIFO ordering/termination obligation fails. |
| Pop a pair without first checking the reply queue | A missing side consumes an expectation; unchanged-suffix obligation fails. |
| Treat one remaining replay response as already fresh | Exact served-prefix suppression obligation fails. |
| Remove quiet diagnostics from the publication conjunction | Publication completeness obligation fails. |
| Skip the recovery payload-digest check | Integrity admission obligation fails. |
| Release a batch regardless of its gate | Rejected batches can escape; exact release condition fails. |
| Skip the permutation inverse check | A non-bijection can omit canonical entries; the checked certificate contract fails. |

The raw-prefix extension adds three controls against the actual executable
`RawTranscript::command` body: omit exact command-byte comparison, omit the
inter-item trivia check, and start a feed at the proposed command offset
instead of the previous endpoint. Their contracts, simulations and nonempty
witnesses remain in scope. The disk-framing extension additionally checks
removing the header-newline guard, changing the encoded separator to carriage
return, and retaining the separator inside the decoded payload. Each of these
six mutations remain in the complete source-sharing scope. The lexical
extension adds six more actual-statement controls: accept an early root close,
ignore a doubled quote, ignore an input comment, skip command framing, skip an
atom's line terminator, or skip response-list framing. Each new control fails
with 199 verified obligations and one ordinary verification error in the
200-obligation closure. These are checked statement changes, not injected
`assert(false)` controls.

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
`query_core`, the source-shared `inputs.rs` as `inputs_core`, and
`proofs/refinement/query.rs`, alongside the existing `context` and `query_association` modules. It selects the stock macro branches
with `--cfg sail_verus`; Cargo selects the erasure branches in the same
captured syntax file. A direct invocation is:

```sh
RUSTUP_TOOLCHAIN=1.99.1 VERUS_Z3_PATH="$stock_runtime/z3" \
  "$stock_runtime/verus" "$captured_driver" \
  --cfg sail_verus --no-cheating --output-json --num-threads 1
```

The local source-sharing closure passes 200 obligations: 59 existing
model/context, 62 query-core obligations, 47 reused input-core obligations and
32 query simulation/witness obligations. The new nonempty byte witness includes CR/LF,
line comments, quoted parentheses and a command split across input chunks;
refusal witnesses cover skipped nontrivia, changed bytes and offset overflow.
Focused Rust clippy checks cover acceptance and shim; all thirteen shim unit
tests pass. The randomized queue test retains its arrival/drain coverage and
checks complete payload equality using structurally valid response fixtures. The ten real pinned-Z3 `cached_session` tests pass, including the unmatched
clean-exit regressions, supported warm-cache reuse and two late-refusal paths.
The first late-refusal test changes a second disk reply to two concatenated
lists while preserving its envelope key and digest, then starts a fresh daemon.
It proves the first reply was served from cache (`replies_served = 1`), the
second caused live replay, and complete stdout, stderr and status equal the
direct pinned solver. Removing only the actual response-framing guard makes
this test fail with the extra list in stdout. The second compares quoted-head
and unterminated-string input refusal after the same confirmed cache hit.
These controls establish behavior at those concrete paths, not a false-proof
exploit or a theorem of arbitrary process/transport failures. These local component checks do
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
| Shim splitters, `observe`, session classification | Actual admission checks structural lexical framing, first-head bytes and raw interval/gap/feed preservation. Complete native emission inventory, command-kind/output-count semantics and option extraction remain open. Actual output-producing commands and whole response frames appear exactly once in order. Byte accounting does not prove SMT-LIB parsing. |
| Shim adapters around the checked correlator/replay core | Channels, exact frame offsets, expectation construction, held-check release, the complete pair-only permutation/projection and effort annotation preserve the proved payload interpretation. The owned queue removal and countdown bodies themselves are now checked. |
| Process launch, stdout/stderr monitor, publication adapters | Supplied closure observations belong to the identified immutable process. The source-shared gate requires all supplied observations and the batch preserves its actual owned values; OS observations and atomic flag/channel handoff remain unproved. |
| Daemon `HELLO`/`PUT`, write-behind and memory stamps | Every admitted local writer refines the observed-run publication path. A handshake alone does not prove its arbitrary `PUT` truthful. Actual write jobs use the checked FIFO, but locking, insertion/settlement and I/O correspondence remain obligations. |
| `DiskCache::put_scoped/get_stamped` | Protected prior publication, unique temporary files, atomic rename, digest computation, concrete stamps and crash behavior refine the ghost history. The actual byte encoder/first-newline decoder now preserve header and payload; the decoded-envelope admission body checks version/key/digest and preserves the exact payload. The implementation does not store or verify this model's ghost histories. |
| Shared/bundled admission | Excluded here; needs a reviewed writer-origin contract or independent certificate checking before inclusion. |

The source-shared changes start from production base `53d30e87`, which
includes main `237c7d8d`; the raw-prefix extension starts from `c6270f75`.
The executable core and bounded simulations pass 200
whole-crate obligations with zero errors through the direct stock driver
(including 59 existing query/context obligations); the existing model scope
and controls remain intact. Full native qualification and integrated checker
evidence belong to the owning validation record. Stock Verus/vstd, pinned solver soundness,
compiler/runtime identity and the operating-system/process interfaces remain
trusted where stated. No claim of verified production I/O, default diagnostic
parity, termination, performance or an end-to-end verified Sail follows from
this model alone.

## Exact production v2 key material

[`key_prefix`](../../crates/sail-acceptance/src/key_prefix.rs) now owns the
incremental framing executed by `sail-key::PrefixHasher`. The source-shared
[`prefix_factory`](../../crates/sail-key/src/prefix_factory.rs) reads the real
`SolverIdentity` fields and `Mode` variant. The stock driver captures their full
original declarations, including the opt-in `String` payload and binary array,
rather than substituting an unrelated proof-only identity. Neither type moves
out of its public `sail-key` namespace. [ADR 0085](../decisions/0085-checked-query-key-framing.md)
proposes the explicit key-to-leaf edge; there is no cross-crate source-path bypass.

The controller proves these exact effects:

- Initialization feeds the unchanged v2 domain, mode tag/name and every solver
  identity field as an eight-byte little-endian length followed by its bytes.
- Every update appends its entire original slice exactly once. Cloning preserves
  the incremental state, length and sticky validity flag.
- `feed` accepts a `RawTranscript`-owned `PrefixFeed` only at the current byte
  counter, with a matching interval length. Repeated or out-of-order feeds
  permanently refuse exact-length authority before another key can be used.
- Normal keys append the existing end marker and cumulative length to a copied
  state. Silence keys keep the existing separate domain and nested normal key.
- Equal fully framed hash-input bytes imply equality of all solver fields,
  default/opt-in variant and exact opt-in name, and the full raw input prefix.
  Field boundaries, comments, Unicode bytes and fragmentation are not normalized.

The last theorem is about the bytes supplied to hashing. It does not assert
that a finite hash is globally injective. The private production `Sha256Host`
remains the narrow reviewed primitive boundary: fresh SHA-256 state, exact
update, state copy and consuming finalization. Its source and caller bindings
must be captured alongside the checked controller. Neither a matching digest
nor an implementation of the public primitive trait establishes solver origin.
The production cache caller owns a `SessionPrefix`, which combines the raw
transcript and this controller in one object. Its checked command operation
borrows an immutable command/key view from that same object; no caller can
supply a feed from another transcript. Its before/after checks also require
the exact-length flag. Counter overflow is
sticky and disables interpretation/publication while preserving input forwarding.
The measurement trace omits a prefix key when that flag has been lost.

The local whole-source compatibility check covers the input/query cores, key
controller and real typed factory with directly invoked pinned stock Verus,
`--cfg sail_verus --no-cheating --output-json --num-threads 1`, whole-crate scope
and ordinary resource limits. It proves 163 obligations. All eleven controls
fail through ordinary verification: wrong solver field, mode tag, raw update,
end marker, counter overflow, feed order, silence domain, state copy,
little-endian length, typed platform mapping and typed opt-in mapping. Earlier
control attempts reached unrelated resource limits and were refused; the final
checked modular lemmas and opaque recursive encoding qualify all controls without
raising limits or omitting proof functions. Canonical integration must retain
these new source/type/import/function inventories and native primitive bindings.

`cargo test -p sail-key -p sail-acceptance` runs the executable controls and fixed
independent SHA-256 vectors for default, empty opt-in and named opt-in modes over
every fragment boundary. The pinned-solver `sail-shim` cached-session tests also
cover normal reuse, diagnostic preservation, unmatched/incomplete publication
refusal and late replay after a cached prefix. Native integration qualification
and added-cost measurements remain separate gates. The checked joint owner establishes transcript-to-hash ownership. Complete
invocation/stream refinement must still establish that this owner observed
every actual physical input and reply. Reported solver metadata must also be
bound to the executable actually launched, and persistent replies need a
truthful writer origin.

## Native observation-candidate refusal

The exact candidate fork `12459c9cc3a3022c91822643aa1cb8e7fac74871` builds on native
x86-64 Linux with the pinned source lock and Z3 and verifies all 2,059 vstd
obligations. Its complete corrected nonignored regression run passes 4,838 tests
across all 161 test binaries; 63 source-ignored tests remain ignored and none are
filtered. An earlier run used the wrong working directory and failed the Cargo
and examples binaries; that failed attempt remains indexed rather than counted
as acceptance.

The candidate is nevertheless **refused for selection**. For each of the actual
procedural-macro `args`, `args_os` and `/proc/self/cmdline` controls, stock,
selected fork and unwrapped candidate produce zero verified functions and one
error. Both active `off` and `on` envelopes change that to one verified function
and zero errors. Local argv restoration does not preserve physical compiler
inputs. This is an executable default-mode conformance defect in the unselected
transport, not a solver arithmetic counterexample. Runtime selection, pinning
and publication remain unchanged. Full corpus, determinism and cost qualification
are deferred until a transport preserving actual compiler inputs replaces it.

The native receipts retain exact commands, source/lock/toolchain identities,
runtime file inventory and every raw result. Their SHA-256 indexes are:

| Gate | Result | Receipt SHA-256 |
|---|---|---|
| Observation semantic control | Refused; six active-envelope proof changes | `f1da9ee02c3127f731dfbb47ff25c60ea0536bce0809955cc76c464c8680662d` |
| Full fork regression, first attempt | Failed; incorrect harness working directory | `7019b8387aa9cf1216a78220eee96ff7fc394cdee510ab62534d9e870ce27478` |
| Full fork regression, corrected attempt | Passed; 4,838/4,838 nonignored tests | `9a63547ea3f4f052ff5c57b5c7396c70b3366aff6673244fa1aa5ad6d6276316` |
| Built suite runtime inventory | Captured; unselected candidate | `ebc67cb3131b39d412bc8621e8f7ec0a7f965b6c167bac01b9ff7ca9a17b3577` |

The native machine is a `c7i.4xlarge` in `us-east-1`; these are correctness
qualification observations, not clean performance measurements. Raw logs remain
outside the repository. The failed semantic gate remains decisive despite the
successful build and ordinary regression run.


## Existing managed argument transport defect

[Sail #393](https://github.com/Chelis-Lang/sail/issues/393) also reproduces this
boundary on the existing selected fork: the current native managed wrapper's
physical `--time` argument changes an actual procedural macro's proof, and Sail
returns success for an original request rejected by plain Cargo-Verus. The
[corpus fixture](../../corpus/repros/sail-managed-physical-time-argv/README.md)
and [source/runtime record](../validation/managed-physical-argv.md) distinguish
the exact tested committed Sail checkpoint, same selected verifier/Z3, safe
unsupported-configuration fallback and explicit original-time controls. This
is not limited to the unselected observer envelope. Query framing and exact key
material correctly describe the SMT they receive; they cannot establish the
original program's verification when a preceding layer changes compiler inputs.
The full proof must move existing output/timing/logging metadata into an
input-preserving private channel too. This defect and the complete physical
invocation/stream and writer refinements remain open.


## Same-owner command and key transitions

The actual cache loop uses `sail_key::SessionPrefix`, whose private
`QueryPrefix<Sha256Host>` owns both the complete raw transcript and incremental
hash state. The source-shared constructor reads the original typed solver and
mode. Induction over append, command, refusal and tail transitions establishes
that the hash material is exactly the identity/mode header followed by the
owner's raw bytes through its current endpoint. Appending and refusing never
change replay bytes; refusal stays sticky. A command must match an exact
contiguous interval, with only trivia between the previous endpoint and its
start, and pass the supported structural frame/head checks.

An accepted `CommandFrame` immutably borrows that owner's state. It exposes the
exact checked head, endpoint and v2 digest at that transition; the borrow
prevents advancing the owner while the frame remains usable. No-output commands
need no hash copy/finalization. The actual GET uses the key already retained in
its output expectation. Finishing feeds the exact owned suffix and checks the
complete endpoint before publication. Buffer overflow consumes and drops the
owner after replay, then continues forwarding without retaining another input
copy. This preserves the bounded-buffer and failed-output safeguards.

The constructive stock/native control fragments a real `check-sat`, derives its
key before a trailing comment, completes the tail, and checks exact replay bytes.
Native tests compare the real SHA implementation with the existing independent
v2 vectors and detached reference hasher over every fragment boundary in all
three mode cases. The complete original query proof root plus the joint owner
checks 273 obligations at ordinary settings. The eleven original key controls
remain in scope; new ordinary controls remove the actual command feed, remove
the tail feed, replace the accepted endpoint, and remove sticky refusal.

This is structural framing and exact binding, not a proof of SMT command
arity/types/meaning or complete response cardinality. The native session policy,
input/output splitter extraction and physical process/I/O correspondence remain
separate obligations. Neither a successful hash transition nor the tail method
asserts EOF, a clean solver exit, or a truthful stored solver origin. The
publication gate and protected-store/writer obligations remain required.


## Supported head-extraction completeness

`command_head` proves both directions for its explicit `head_span` grammar:
every returned head is the exact first supported bare head in a complete list,
and every list satisfying that grammar returns its unique head, including
leading whitespace and completed comments. Its loop invariants bound the actual
scan against every candidate span; the proof does not assume a supplied parser
offset or a successful classification Boolean. `framed_command` and the joint
owner preserve that constructive acceptance condition after checking the exact
owned interval and preceding trivia. The executable stock/native comment witness
uses a nonempty head after a newline-terminated comment.

The full original query root checks 274 obligations with this witness. Removing
the actual leading-trivia scan or truncating the returned head causes ordinary
verification failures at unchanged scope and limits. These clauses specify the
supported lexical grammar, not the complete SMT simple-symbol alphabet or the
arity, types, semantic effects and response cardinality of every command.


## Tactic-check response accounting correction

The actual `CommandKind::output` dispatch now includes `CheckSatUsing` in the
one-response class. [Sail #397](https://github.com/Chelis-Lang/sail/issues/397)
retains the incorrect original rule, a focused failing-before/passing-after test,
and the [pinned-solver reproduction](../../corpus/repros/sail-check-sat-using-cardinality/README.md).
The actual shim cold/warm regression compares full stdout/stderr/status, then
requires two correctly associated cached replies and zero new warm solver.
Removing the production dispatch arm reproduces the missing warm replies.
The fix is a native production correction; the complete command/response
classification refinement remains open and is not established by this test.


## Response expectations from the accepted raw head

`FramedCommand::policy` now computes response metadata from the exact head
borrowed by the same command observation that supplies its endpoint and key.
The source-shared `query_policy` dispatch checks exact UTF-8 command words and
the literal `get-` prefix; it never strips, normalizes or reconstructs the raw
command. The actual shim uses that checked policy and requires agreement with
its native session policy. A disagreement or unknown command takes the existing
fallback; the print-success refusal remains in force. This avoids treating an
independently supplied enum or response-count Boolean as authority for pairing.

The checked rule describes successful commands with print-success disabled:
checks, echo and get queries produce one response; the explicit supported
mutation/option/declaration commands produce none. Failed or malformed commands
can produce errors, so this rule does not itself establish a clean live run.
Arity, types, complete solver protocol behavior and physical stream coverage
retain their separate obligations. The native error/unmatched/publication guards
are still required. The existing default cache gate accepts only the exact
`-smt2 -in` solver arguments; other argument vectors stay pass-through.

A concrete stock/native tactic-check witness proves the actual ASCII bytes
belong to the one-response class. Native adapter tests cover completed comments,
quoted string contents, doubled quotes, declarations, ordinary/tactic checks,
get commands and quoted/unknown heads, and retain each exact expectation key.
The full original query graph checks 280 obligations. Four ordinary controls
change the tactic name, remove the get prefix, replace whole-word equality with
prefix matching, and bind a frame's policy to different bytes. An initial
get-prefix control reached a resource limit and was refused. The final checked
word/prefix contracts isolate their exact UTF-8 definitions; all controls fail
ordinary verification at unchanged whole-crate scope and resource settings.

The added dispatch performs only bounded fixed-word comparisons and borrows
existing bytes; it adds no query copy or normalization. Its native performance
qualification remains part of the integration gate.

## Captured same-owner query assembly

The canonical query root includes the production `query_prefix` and
`query_policy` modules and the actual `new_query_prefix` factory. A private
`QueryPrefix` owns both the original transcript and its incremental key state;
a `FramedCommand` borrows that owner's transition to expose its complete head,
endpoint, response policy and key. The source capture binds the native
`SessionPrefix`/`CommandFrame` forwarding implementations separately as reviewed
code. It checks every source-shared factory and method, the real solver/mode
declarations, exact imports and the complete function inventory.

The whole positive root has 288 verified obligations and 503 function entries,
preserving all 462 earlier entries. Its 39 required negative controls retain
all 30 earlier controls and add complete-head, policy, owned append/endpoint and
tail-hash failures. The four required CI partitions each repeat this whole
positive root; only their complete union covers every negative control.
Frozen `cc8c24a1` passes this complete 40-case component; later additions
require their own source-bound qualification.
These checks do not establish actual solver delivery, clean termination,
truthful cache origin, physical persistence or the service-wide theorem.

The first expanded canonical development probe hit the unchanged solver
resource limit in `header_fields`. Splitting its existing proof by the actual
mode case proves the identical contract within the existing budget. No
runtime operation, verifier limit or proof obligation is removed.

## Actual record-write transaction

The disk cache's actual `put_scoped` path now invokes the checked
`cache_write::write_record` transaction through its private
`persist::NativeWriteHost`. The same bodies perform exclusive temporary-file
creation, metadata serialization, checked record encoding, full byte write,
metadata observation on that handle, close, rename and owned-file cleanup.
Successful call history contains those operations in that order, with the exact
serialized header/newline/reply image. Failure returns the original first
primitive error, without a completed-write receipt. Cleanup applies only after
temporary creation succeeded.

Exclusive creation is a concrete ownership correction: an existing regular
file, directory or symlink makes the write refuse without modifying or deleting
that name. Its create error now takes priority over later serialization/write
operations. Previously `File::create` could truncate an existing target and the
common failure cleanup attempted removal even when creation failed. An existing
name now remains for its owner or ordinary stale-temp maintenance. The normal
reply stream continues through the existing cache-write failure path.

The native provider is the private `NativeWriteHost`, whose methods forward
the actual filesystem/serde operations. These forwards remain reviewed primitive
boundaries; stock checks the transaction body, not the OS or serde implementation.
Canonical integration must bind the exact provider, trait, native call sites and
metadata mapping. Generic host implementations and the scripted control provider
are not physical I/O or solver-origin authority. The primitive ledger records exact
calls, returned errors and written bytes. It does not promise that an observed
file stamp uniquely identifies contents, that a later pathname still denotes
the closed descriptor, or that a successful rename is durable. In particular,
`rename_requests` associates a written image with the successful rename call;
it is not an assertion about the destination's later contents. No fsync is added.

The nonempty constructive control writes an exact record image. Further controls
exercise all five primitive failure stages and newline-bearing metadata. Native
tests retain existing and dangling symlinks, a preexisting regular file, rename
failure cleanup, and exact serialized record/stamp behavior. Six ordinary stock
controls omit the full write or rename, stamp before the write, clean an unowned
name, replace the original error, or omit cleanup of an owned failed write. The
whole original query root checks 295 obligations at unchanged scope and limits.

`WriteCommit` is data, not a solved-origin capability. The next required link is
the filesystem-wide protected object/name history across every local writer,
GC, rename, interruption and recovery transition. Only that history, tied to
complete actual solver-run publication and the original shared/bundle writer
contracts, can establish that deserialized bytes have an admitted solved origin.
Checksums, framing, stamps and a successful call sequence cannot establish it
alone. Canonical native Linux, differential and added-cost qualification remain
integration gates.


## Actual solver-input delivery

The cached shim transfers the `ChildStdin` taken from its actual owned solver
child into `query_delivery::Delivery`. The only native primitive writes the
supplied bytes with `write_all`, flushes that descriptor and returns the actual
outcome. Successful calls append those exact bytes to an independent incremental
v2 prefix. Any failed or partial write permanently withholds publication; a
later successful write cannot heal it. The primitive may have delivered part of
a failed call, which is deliberately not sufficient for a complete input claim.

The parser owner and delivery seed are jointly constructed by the checked
`new_session_seed` factory from the same typed solver identity and verification
mode. Before terminal expectations become eligible, the complete-silence key
computed from all successful pipe writes must equal the parser owner's terminal
key. Missing, extra or reordered bytes and different solver/mode headers withhold
publication under the existing cryptographic assumptions. Hash equality still
does not prove exact logical equality without those assumptions.

Terminal metadata and the existing completion signal precede closing this same
owned pipe. This ordering preserves the output thread's existing early-exit
behavior: closing first could let it observe a dead child before input
completion. Failed writes close the descriptor immediately, preserving the old
failure behavior. Original bytes, solver output, status and the quiet/normal-exit
publication guards retain their original paths. The check adds another streaming
hash of bytes actually delivered to a running solver; native cost qualification
is pending.

Stock Verus checks the generic delivery operations, nonempty two-write witness,
partial-write failure and permanent refusal. Script-only optional outcome hints
prove constructive controls; native I/O has no predicted outcome. The native
`SolverPipe` forwards remain reviewed OS/library boundaries, not a proof of the
operating system. Focused controls cover different physical chunking, omitted,
extra and reordered bytes, solver/mode mismatches and unhealable partial failure.
The initial successful-write compatibility assembly has 305 obligations.
Eight ordinary mutations
omit the actual write, hash different bytes, heal a failure, drop the terminal
comparison, accept a closed pipe, omit closure, or detach either factory result
from the requested mode. Canonical capture and full platform/differential gates
remain integration work.

The final complete-input handoff is an owned `DeliveredInput` token, not a
caller-supplied terminal key. Only the delivery controller's checked `seal` can
construct its private payload. Sealing freezes writes permanently, succeeds
once and preserves the exact acknowledged key. The output-side pairer rejects
any duplicate terminal or later expectation. A nonempty checked witness and
native controls exercise attempted writes and resealing after the freeze.
The local compatibility root checks 308 obligations and all thirteen delivery
mutations fail ordinary verification. Five SHA-backed delivery controls,
fifteen shim units and thirteen actual pinned-Z3 cached sessions pass.
Canonical assembly and broader qualification still remain pending.

This is a concrete successful-write correspondence step toward query-only
origin. It does not yet connect complete exact stdout framing, stderr and
observed normal termination through a query-only solved-origin type to every
writer. The executable's physical identity, filesystem-wide protected store
history, truthful shared/bundle writers, compiler closure and full service
composition remain open.


## WIP handoff

The current owner slice is committed for WIP publication, not accepted as the
complete design proof. It contains joint raw/hash ownership, complete supported
head extraction, exact head-derived response policy, the tactic-check accounting
fix ([#397](https://github.com/Chelis-Lang/sail/issues/397)), the actual
record-write transaction with exclusive temporary-file creation, and independent
owned solver-input delivery bound to the parser terminal key.

The owner compatibility graph checks 308 obligations. The six writer and
thirteen delivery mutations fail ordinary verification. Focused native cache tests, executable
write scripts and clippy pass. This is separate from the canonical integration
capture, its newer obligations, native Linux, full host/solver/differential
qualification and performance acceptance. The writer source/primitive inventory
and added controls still need incorporation into the lead's canonical checker.
The WIP push does not authorize merging or selecting a runtime.

The next required origin interface is query-only: exact solver identity, mode,
raw input and output, actual owned I/O and termination. It must not fill unknown
compiler/request facts with an empty context. An owned child PID and status do
not establish which pinned image executed; immutable-CAS/updater or actual image
binding needs its own source-bound evidence. That solved-stream publication must
then connect to every local writer and filesystem object/name transition,
including GC, crashes and recovery, plus the original shared/bundle writer
contracts. The complete verifier/request stream inventory remains a separate
service-composition requirement.

The owned-argv observer transport remains disproved and unselected. The normal
wrapper registration/peer bridge is a proposal, with no activated protocol or
new runtime pin. The managed compiler-visible argument defect
([#393](https://github.com/Chelis-Lang/sail/issues/393)) remains open. None of the
framing, write receipts, hashes or metadata can substitute for the remaining
solved-origin witness.

## Captured delivery and write adapters

The canonical query driver includes the actual `cache_write` and
`query_delivery` modules, the typed session/delivery seed factory and every
method of their private `WriteCommit`, `Delivery` and `Delivered` types. The
complete module/import/function/control inventory includes their script
implementations and exact primitive contracts. Trait or script drift refuses
capture before proof execution.

The surrounding native bodies are separately reviewed and pinned in full:
`DiskCache::put_scoped`, `persist::NativeWriteHost`, `SessionPrefix` and
`InputDelivery`, and the shim's actual `SolverPipe`/`Solving`/`Pairer` caller.
Their source hashes do not turn filesystem, serde, OS pipe or executable-image
semantics into proved bodies. Reports list these native adapters separately
from the source-shared transaction/controller checked by stock Verus.

A whole positive development probe checks 322 obligations and 584 function
entries, preserving every earlier query function. The required query inventory
adds six write and thirteen delivery/seal controls to the previous 39 controls.
All 32 selected shim/actual pinned-Z3 session tests and 43 acceptance/key/cache
component tests pass on the integrated native macOS source; strict clippy
passes. These focused checks do not qualify the full current control matrix,
Linux integration, solver faults, differential suites or added cost.

The same-owner seed and actual successful pipe writes establish delivery-key
agreement under the explicit cryptographic assumptions. A frozen input token
prevents later writes and cannot be reconstructed from a success Boolean or
terminal key. It still needs the actual output, termination and publication
refinement. No unknown compiler context is represented as an empty context,
and a write receipt never supplies solver truth. The broader service theorem
and physical provider obligations remain open.

## Owned stdout integration candidate

The separate output candidate moves the actual retained stdout buffer into
`query_output::Transcript`. Every parser interval must follow the previous
endpoint in that same buffer and pass the existing response grammar. Actual
zero-byte reads mark EOF; read/framing failures latch refusal. Publication
requires consuming that owner with an exact partition of all retained bytes.
The original served-reply comparisons, statistics policy, kill-on-divergence,
forwarding and overflow paths remain in place. Endpoint storage participates in
the native buffer budget alongside payload bytes.

`Frame` and `CompleteOutput` attest byte intervals and coverage, not solver
truth or normal termination. The existing process, input, quiet-stderr and
pairing gates remain necessary. This source candidate has constructive and
refusal source controls but no completed proof, native test, canonical capture,
differential or cost qualification. It does not close #371/#372 or the service
composition. No compiler context is supplied by an empty placeholder.
