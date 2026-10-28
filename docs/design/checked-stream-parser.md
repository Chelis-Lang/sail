# Shared stream parser

The implementation shares actual streaming and lexical operations in
`sail-acceptance`, using the same Cargo/Stock syntax envelopes as the existing
acceptance core. Its [decision](../decisions/0086-checked-stream-framing.md) is
accepted. The [qualification record](../validation/checked-stream-parser.md)
separates source assembly from proof, native behavior and runtime selection.

`query_split` owns the actual pending list/atom bytes, input/output side,
absolute cursor and emitted-item vector. Its model relates each real byte/chunk
transition to exact ordered `(kind, offset, bytes)` emissions. Input atoms end
on the original delimiter set, including CR; output atoms end on LF and trim a
preceding CR only on termination. Explicit finish preserves incomplete atoms
and reports incomplete lists as Stray. It does not assert physical EOF.

`query_lex` owns the actual borrowed source, cursor and sticky failure state.
The checked operations cover trivia, ordinary/quoted/string endpoints,
doubled quotes, numeric/keyword token categories, exact token spans, copy and
error offsets. Public `Token::text` keeps its original must-use annotation
(#533) and returns an empty slice for invalid publicly supplied spans.

`parser_grammar` contains only structural/lexical byte rules and the finite
flat-list lemmas needed by nonempty witnesses. There is no command classifier,
Session option store, key/hash operation, raw transcript owner, output queue,
replay decision, cache publication or presentation authority in this slice.

The candidate replaces the existing default native implementation: both
`CommandSplitter` and `ResponseSplitter` delegate construction, push and finish
to the shared `StreamSplitter`, and `Lexer::next` delegates to `next_token`.
These are selected public paths when this candidate is built, not unused extra
APIs. The splitter now owns disjoint atom/list states, transfers its item vector
through the checked chunk loop, and uses the first-close list chunk path; the
lexer uses checked indexed endpoint/category loops. Intended public bytes,
spans, errors and finish behavior remain the same, but representation and
code generation differ from the previous implementation. Native microcost and
scoped standard conformance are qualified; ADR 0086 accepts this bounded
adoption with the documented fragmentation and lexer regressions.

The native API reexports the original data declarations. Existing methods,
byte fields, tokenizer and error-code/offset conversion are preserved. The
stacked [command classifier](checked-command-classifier.md) uses a separate
shared root. The `sexpr` consumer and thin native adapters remain hash-bound,
not Stock-compiled. The added lexer-head operations extend the complete parser
inventory to 135 verified / 213 function entries. All 55 parser cases were
freshly checked in the command/cache checkpoint; earlier parser-only and
Holding/filename receipts retain their historical inventories and scope.

The independent `parser` production root compiles the complete selected
modules and their actual nonempty witnesses. Its 54 ordinary controls
retain the 49 transferred fault effects and direct delimiter/whitespace
faults. Every selected original contract, loop annotation and proof assertion
is retained, including the corrected full-transition/chunk fixtures (#537).
The parser feature leaves the three current main roots and all their 168
controls unchanged. Source comparison preserves each of those 171 complete
positive/control programs. Existing main proof receipts do not qualify the
new parser root.

Three original combined mutations cross erased proof boundaries and are
refused by the strict interval guard. Each is decomposed into two independent
one-interval mutations: first-atom versus second-Stray loss, List versus outer
item loss, and List versus outer zero consumption. Both parts retain the full
original signature, loops and proof groups. The control-source reference in
`proofs/refinement/parser-control-decompositions.json` preserves the original
combined descriptors; a source test verifies that applying each pair in either
order reconstructs its original `after` body byte for byte. The refused combined
descriptors earn no canonical credit. Both independent parts require ordinary
whole-root Stock failure at the unchanged resources before qualification.

The current union's complete pinned Stock parser root checks 133 verified
checks and all 208 function entries. This retains the checked
history/emission lemmas used by the actual chunk loop and adds the four shared
filename functions. All 54 parser mutations fail ordinarily at whole-root scope
without exhausting resources. The manifest locks the actual discovered inventory.
The combined matrix has 238 cases: 183 query/input/authority calls execute at
`58edf644`, and all 55 parser calls execute freshly at `838706f0`; exact compiled
program hashes bind those calls to the combined source. The parent parser-only
checkpoint's earlier 129/204 and 55 fresh + 171 reused receipts retain their
historical scope. See the [combined qualification](../validation/compiler-filename-refinement.md)
for native compatibility, conformance, current-main transfer and cost status.

The native splitter adapters do not discharge all representable-offset,
output-capacity, allocation or process-lifetime premises. Physical reads/EOF,
external original-byte ownership and queue association, complete SMT/Session
and pinned-solver interpretation, and truthful origins remain separate work.
The caller/hash binding is not a proof of those relations. ADR 0086 accepts the
measured parser/lexer microcost tradeoff and qualified default-runtime adoption;
no service-latency, general performance or completed-service theorem is claimed.
