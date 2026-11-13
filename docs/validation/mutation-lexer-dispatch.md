# Exact mutation-source punctuation dispatch

Status: native lexical component acceptance, independent review and the
current-main workspace gate pass on this integration branch.
This changes the source mutator's operator lookup. No consumer command,
released availability, solver input, proof verdict or cache-key behavior changes.
The consumer skill and user guides therefore remain unchanged.

## Component acceptance

The exact candidate lexer SHA-256 is
`1a6d6eb0b5e2de197c79c50653ebd6d653b214014927cbf701bacad2350412a5`.
It is byte-identical to qualified development source `8529259d`. Its original
lexer is byte-identical to the integration base `e5f513f7`, SHA-256
`db0a820efb016937723ea4be81a973c9614842e6cbad60b94a7ad961086e481f`.
No dependency or selected runtime is changed.

Native EC2 c7i.4xlarge/us-east-2, Rust 1.99.0: five lexer tests, focused
mutation tests, affected all-target lint and release build pass. The explicit
copied-source lexical corpus has 698 Rust files from HumanEval, IronKV,
CapybaraKV and Vest; all token kinds and byte offsets agree exactly. The
old ordered punctuation table stays as a test oracle. The full lexer loop and
all outside-edit preservation checks remain.

Fresh independent native review compiles the complete old and new lexers from
frozen Git source and compares 17,204,481 punctuation inputs and 2,291,193
complete lexical inputs, including bracket vectors. The 698 original files
contain 2,439,479 tokens; copied inputs and their archive/index hashes are
verified. Additional coverage includes 4,886 corpus transformations, 100,000
fuzz cases, all ASCII strings through length three and malformed/trivia/Unicode
cases. Five intentionally corrupted controls fail. Missing and empty corpus
inventories fail explicitly. This is lexical equivalence, not a solver or
service-wide proof.

All 751 material review entries are independently rehashed by the lead. Archive:
`2e60d4b86ef4215ac35e69895e9621f09e007d22e20a13c9ebf52867a1656538`;
manifest `57131ed24132d6ff84ed64b36c8512b232585a69e79d0daff4ff3c4894febdd2`;
report `5f0510e568fa8dae4ac2fb694a2aadf8bd604838b234598d7452c860905978a5`.
Raw logs, binaries and source cases remain outside the repository.

## Integration and measured lexer throughput

Exact integration `44db8799` on the current-main `e5f513f7` base passes the full
native workspace gate: 1,399 tests pass, 54 explicit skips; formatting, clippy,
doctests, rustdoc, layering, dependency policy and generated benchmark page pass.
The copied lexer remains byte-identical to the reviewed component.

Three paired in-memory lexer rounds on native c7i.4xlarge/us-east-2 use Rust
1.99.0 `-O`, black-boxed complete token vectors and the same 698 preloaded files.
Each sample runs the corpus twenty times (48,789,580 tokens); order alternates
old/new and new/old. Old seconds: 3.073459, 3.052826, 3.056672. New seconds:
0.721403, 0.720993, 0.721503. Median throughput improves 4.24 times
(3.056672 / 0.721403). This excludes file loading and measures lexical work only;
it does not establish an end-to-end verification or campaign gain. Execution is
inside a native Linux container, not emulation, with no competing build/test.

The integration/benchmark archive and all material entries are independently
rehashed by the lead; archive SHA-256
`84835071bccb4b4f6ac70c979a62f0e753c805c8979c4d5ee1516832400a8c26`; index
`f2330aa4c45765276a4c9e07f4a65c72736cf7df29f8f79bdb905b81d395e921`. Book source, render and
link checks are required before pushing the final documentation amendment.

## Scope and attempts

ADR 0087 classification: internal source-mutation tooling. Selected verifier,
solver, reply/parser and proof-reuse behavior are unaffected. No ordinary solver
corpus entry changes. Required PR conformance still applies; no unrun standard,
deep or Mac result is claimed. No end-to-end speed gain is claimed: both the
original and dispatch-only Vest planners exceed their 600-second diagnostic
bounds. Those failures remain retained separately from lexical acceptance.

| Source/date | Actual result |
|---|---|
| `2ab61712`, 2026-11-13 | Focused and 698-file lexical comparisons pass, then clippy refuses three unnested operator alternatives; release build not reached. Log `dbc5b0af52d78bd5d4bbf1f4ad0d47e1c4fe1aff370c72fd0c72b5155b581346` |
| Review provenance check, 2026-11-13 | Historical producer paths are absent on the review host; the supplemental reopen attempt fails before another test. Copied corpus/archive identities and the completed lexical comparisons remain independently checked |
| Initial throughput launcher, 2026-11-13 | systemd rejects an unbraced variable in its command argument before the benchmark starts. A script wrapper runs the unchanged binary in the native compiler container; all six samples and exit zero are retained |
| Initial review archive, 2026-11-13 | Tar warns that its containing directory changed. Original retained; explicit-entry replacement archive and all 751 entries verified |

Documentation changed: this validation record and changelog. Workspace/PR
integration results are recorded above; required PR checks remain separate.
