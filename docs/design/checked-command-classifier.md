# Shared command classification and response expectation

Status: implemented in the candidate based on main `bdd5e050`. Conditional
Stock refinements pass at `67d1c84c`; fresh Linux full runtime/conformance and
bounded component-cost checks pass at `e63ac6aa`. The full Mac host gate passes
at `f4d686fc`; lead review accepts this bounded adoption and its measured cost
tradeoff. The [qualification record](../validation/checked-command-classifier.md) separates these
receipts from native host effects and the full service theorem.

`sail_smtlib::command::classify` delegates to the actual shared
`query_command::classify` body. Public `Command`, `CommandKind` and `Output`
declarations are reexported with the same fields, variants and methods.
`Session::expects_output` delegates through the same facade with its original
`:print-success` readback. No crate dependency edge changes.

The classifier reads the original borrowed command head and only the arguments
needed by the existing shallow classifier. It preserves default push/pop
counts, decimal overflow refusal, keyword admission, nested option-value spans
and lexical/error offsets. Deliberate shallow behavior remains: `(assert`
classifies as Assert and a scalar value need not consume a closing list. This
does not validate SMT arity, the whole command or the solver's interpretation.

The lexer extension reads exactly two original tokens for the head. Its
framing-conditional head-span formula moves unchanged into `parser_grammar`.
Existing splitter/lexer operation bodies remain intact. The complete independent
`command` root compiles inputs, grammar, lexer, policy and command modules with
their nonempty constructive witnesses.

The extraction preserves the `c6b46500` command/policy bodies and all 29 related
executable controls: 22 command, three policy and four lexical-head controls.
Namespace relocation changes no contract formula or fault effect. Later
pre-apply and output-category equality additions are excluded.

The native facade, error conversion and complete Session source are hash-bound,
not Stock-compiled. Session storage/reset semantics, solver interpretation,
key/prefix association, output ownership and physical process/origin facts
remain separate obligations under [the full design](evidence-acceptance-core.md).

The head extension changes the complete parser root too. Its old 133-verified,
208-function inventory is retained as a prior baseline. Parser and command
roots now have actual whole-root discoveries: parser 135 verified / 213
functions and command 108 / 163. The manifest locks these observed values;
all 54 parser and 29 command failure controls now fail ordinarily at that scope.
These calls were freshly executed; separately reused historical receipts retain
their actual source/runtime attribution. No counts are guessed.

Selection requires complete direct-stock proofs/ordinary controls, native
Linux/Mac compatibility, solver faults, full E2E, scoped fast/standard
conformance and measured added cost. Source/proof checks alone establish no
diagnostic-parity or performance result.
