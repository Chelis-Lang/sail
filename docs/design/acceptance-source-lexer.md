# Acceptance source lexer qualification plan

Status: the frozen implementation passes exact lexer equality and native
source-preflight measurement, tracked in [#432](https://github.com/Chelis-Lang/sail/issues/432).
At `714bf6e5`, all 419 Rust sources, 296 mutations and 40 source-control tests
(48,852 dual calls) agree. At `11f3be8c`, a quiet c7i.4xlarge in us-east-1
checks 727 frozen source/mutation cases over four alternating rounds, retaining
all 16 timing samples and final source checks. Median lexer time falls from
27.2620 s to 8.4631 s; unchanged source-preflight time falls from75.1638 s to
27.0651 s. This measures source tooling only. Full canonical/current-source
qualification and production runtime cost remain separate; see the
[owning record](../validation/acceptance-boundary-refinements.md).

`scripts/acceptance-source.py::tokens` formerly passed `source[at:]` to
`re.match` for raw prefixes and again for ordinary words, characters or
lifetimes. Repeated suffix slicing copies the remaining string at each such
position. A source containing many small tokens therefore has a quadratic
remaining-string copy pattern. This identifies an allocation mechanism, not
its measured share of native preflight or a cold image/build gate.

The candidate compiles the same four patterns once and calls
`pattern.match(source, at)`. Their flags, alternatives, character classes and
ordering are unchanged. They contain no anchors or lookbehind, so matching at
the original offset has the same inspected prefix as matching a suffix.
Match endpoints are now absolute: raw/character/lifetime/word branches assign
`.end()` instead of adding it to `at`. No literal, nested-comment, whitespace,
unknown-character or refusal branch changes. Token values still use the
original `source[start:at]`; positions remain Python character offsets.

The new `SourceLexerTests` in the existing source-control test module cover:

- All regex branches at nonzero offsets, with exact token triples.
- Unicode characters and whitespace with original character positions.
- Nested comments, escaped quotes and comment-looking literal contents.
- Every existing raw prefix and exact hash terminator.
- Character-before-lifetime precedence and the existing limited literal subset.
- Exact error messages for malformed strings, raw strings, comments and chars.

No existing source, mutation, golden or timing check is removed or relaxed.
This optimization does not widen the parser's supported Rust subset.

## Standalone correctness differential

Run on a quiet native Linux host with one validation worker and the candidate
integrated into the intended source revision. Apply only the lexer milestone
when comparing an independently frozen compiler/proof milestone; its earlier
compiler implementation commits are not dependencies of this optimization.

Set `SAIL_LEXER_DIFF_REPORT` to a fresh file outside the checkout, with an
existing parent directory. Freeze the full source commit, then run:

```sh
SAIL_LEXER_SOURCE_COMMIT="$(git rev-parse HEAD)"
python3.12 scripts/compare-acceptance-lexers.py \
  --source-commit "$SAIL_LEXER_SOURCE_COMMIT" \
  --original-blob 4716092075d2a3c0bba82f748da0a60c2577dcaa \
  --unit-controls \
  --out "$SAIL_LEXER_DIFF_REPORT"
```

`4716092075d2a3c0bba82f748da0a60c2577dcaa` is the original integrated lexer
blob inspected before this optimization. Its `tokens` body matches the original
suffix-based implementation. Retain its baseline commit or Git bundle on the
native host: a missing original object refuses the comparison, with no fallback
to working files or a reconstructed baseline. The helper reads that exact Git blob and the
candidate lexer from the frozen commit, verifies blob object identities, and
loads each into a separate module. It then compares every token value and
begin/end offset, or the exact `SourceRefusal` message, for:

1. Every tracked regular Rust source blob in the frozen commit.
2. Every individual negative mutation in both acceptance manifests, including
   inherited model source paths. Each original needle must occur exactly once.
3. Every tokenization call made by the complete existing source-control unittest
   module, with both lexers inspecting the exact same supplied string. The
   original candidate tokens or candidate refusal are returned to the tests.

Unknown exceptions, missing sources, duplicate manifest keys/control identities,
ambiguous mutation needles and any mismatch fail the helper. Equal lexer
refusals are recorded as differential results, never as source acceptance.
The receipt records frozen source/lexer identity, case hashes, counts and status;
an interrupted/incomplete receipt is not a pass. No verifier or proof authority
is invoked or selected. The frozen comparison scope above has run; later source revisions need their
own appropriate qualification.

After the differential, run the unchanged source-control tests and canonical
source preflight through the normal native gates. Existing assembly, imports,
private-token, primitive, full function/counter and deliberately broken control
inventories still require their normal checks; token equality cannot waive them.

## Native measurement and selection

Correctness qualification precedes timing. Record the exact candidate and
baseline source, Python/CI runtime, native instance type and region. Hold the
remaining source, manifests, corpus, runtime and host constant, and run one
worker so compilation/proofs cannot contend with the measurement.

Measure a fixed in-memory corpus of the same actual sources and every mutation,
loading modules and reading Git objects before timing. Warm each lexer once;
then alternate baseline/candidate and candidate/baseline order over repeated
rounds, retaining every sample and checking exact results. Measure canonical
preflight separately through its unchanged capture/assembly/inventory path,
before verifier/runtime startup. Its programmatic entry points are in
`check-acceptance-refinements.py`; do not invent or select a preflight-only CLI
mode, remove validation, or change timeouts to obtain a passing result.

Keep cold image/build, source preflight, verifier work and full native gate
timings distinct. A reduction in suffix allocation does not establish the
same reduction in the full gate or in Sail's verification runtime. Root owns
the quiet native differential, measurements, exact-source gate and selection;
final integrated selection remains pending. Raw receipts/logs stay outside git, and measured acceptance
belongs in the existing refinement qualification record.
