# Shared stream parser

The source candidate moves actual streaming and lexical operations into
`sail-acceptance`, using the same Cargo/Stock syntax envelopes as the existing
acceptance core. Its [decision](../decisions/0086-checked-stream-framing.md) is
proposed. The [qualification record](../validation/checked-stream-parser.md)
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
code generation are not identical to main. Narrow native cost measurements
and scoped standard conformance are required before accepting this adoption.

The native API reexports the original data declarations. Existing methods,
byte fields, tokenizer and error-code/offset conversion are preserved. Native `command` and `sexpr`
consumers stay at their main source. Their bodies and the thin adapters are
reviewed/hash-bound source, not Stock-compiled code.

The independent `parser` production root compiles the complete selected
modules and their actual nonempty witnesses. Its 54 planned ordinary controls
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

Complete pinned Stock discovery observes 129 verified / 204 function entries,
including the checked history/emission lemmas used by the actual chunk loop.
The manifest locks this observed inventory; unknown pending inventories still
earn no credit. The original-byte mutation fails on its intended assertions
without exhausting resources. All 54 parser controls and 168 current-main
controls remain mandatory at unchanged resources, alongside native
compatibility, conformance and cost qualification.

The native splitter adapters do not discharge all representable-offset,
output-capacity, allocation or process-lifetime premises. Physical reads/EOF,
external original-byte ownership and queue association, complete SMT/Session
and pinned-solver interpretation, and truthful origins remain separate work.
The caller/hash binding is not a proof of those relations. Narrow cost and scoped standard
conformance remain required before owner acceptance; no speed claim is made.
