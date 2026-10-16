# 0052: Complete frontend inputs and current-source diagnostic binding

- **Status:** proposed
- **Date:** 2026-10-16

## Context

[#162](https://github.com/Chelis-Lang/sail/issues/162) calls for reuse of
rustc/VIR/AIR work across edits. The [current profile](../reports/2026-10-16-warm-frontend.md)
shows a substantial frontend floor. A raw hash of serialized pruned VIR
misses every measured body edit, including buckets whose measured SMT digests stay equal. Source spans, AST identities and deterministic serialization need
separate treatment; they cannot simply be dropped from a proof key.

`Span.id` is used by Verus resolution, erasure and temporary tracking. A
source locator can also refer to another crate. Reusing old spans after an
edit can change diagnostics even if a logical query remains equivalent.
The correctness contract requires the same pinned-solver verdicts and
current diagnostics. This proposal does not authorize query normalization.

## Proposed decision

1. Keep persistent frontend reuse disabled until a versioned, complete input
   boundary is established for a specific pure lowering operation. Bind its
   tool/runtime identity, every compiler/Verus option it reads, complete typed
   semantic input and all global data consulted by that operation. Missing or
   unsupported input coverage is a miss.
2. Treat identifiers consumed by resolution or emitted into solver symbols
   as semantic inputs. Do not erase or renumber them to obtain hits without
   a separate correctness argument and exact-query qualification.
3. Bind output diagnostics to the current source. A candidate may separate
   semantic input from location metadata only when each stored location has
   an exact, validated current-source mapping. Ambiguous, missing, changed or
   foreign-source mappings refuse reuse. No cached diagnostics are accepted
   merely because a function name matches.
4. Start with a measurement-only candidate: compute the proposed key, lower
   freshly, and compare the complete candidate output with fresh output on
   every would-be hit. This does not skip verification. Preserve misses and
   disagreements in the report. Only then implement restoring that exact
   lowering artifact, with integrity checks and a fresh-path fallback.
5. Require a measured native gain, including key and restoration cost,
   before selecting a runtime. The M8.0 gate remains: reused unchanged work
   must occupy at least 25% of the median edit cycle. CPU sums do not qualify
   a wall-time gate. Cold, warm, passing, failing, edited dependencies,
   changed options and moved diagnostics all need explicit coverage.
6. Run exact SMT-stream comparisons and full fast/standard conformance on
   both platforms' qualified runtimes before selection, plus determinism
   checks and the fork's full regression/deep suite before a re-pin. Any
   verdict or diagnostic disagreement blocks selection.

## Implementation status

No persistent frontend cache or qualified key from this proposal is
implemented or accepted. The first implementation candidate instead replaces discarded
identity rewrites in reachability pruning with read-only traversal of the
same current AST. It has no cross-invocation key and does not skip checks.
Its independent review, native performance and runtime qualification remain
separate gates. It does not complete #162.

A separate measurement-only fork prototype (`2371ca51`, based on `520f3b57`)
can project serialized spans to encounter-ordered identifiers while retaining
identifier aliasing, replacing actual packed positions and display strings
with empty fields. No proof makes those values irrelevant or rebinds current
diagnostics. It records an explicitly
unqualified key policy and always lowers and verifies freshly. Four tests
check ordinary serialization, alias distinctions, thread isolation and scope
restoration; `rust_verify` checks successfully. No native key-stability or
fresh-output equivalence result exists yet. This probe is not a cache key and
a match cannot authorize reuse. Other identifier consumers and exact current
location binding remain unresolved.

The next measurement source starts at combined fork `8a373f81`, excluding
the read-only pruning experiment. It records the ordinary pruned AST without
that projection, a bounded raw serialization digest, selected lowering
arguments and available global-context Debug observations, then fresh SST
Debug. Source `80a19b1a` builds and passes its two native unit tests, but
both edited project buckets refuse incomplete SST output at its first
64 MiB record cap. The single capacity revision `7812863f` allows 256 MiB
per record, still two records per process and a separate 64 MiB raw-hash
ceiling. No field or acceptance check changes. Its build, unit tests and ten
bounded project capture runs pass; its observations remain incomplete for
reuse. A build or capture is not complete-key qualification. Each
manifest states that neither the input nor output is qualified for reuse.
Opaque `RawSpan`, private global state, rustc callbacks/source maps and the
loss of span IDs/packed positions in existing SST Debug remain explicit
coverage gaps. `Span.data` contains a stable crate identifier and packed
source positions; it is not a source-file content digest. No complete key
has been established by hashing the pruned crate.

The offline `scripts/frontend-input-diff.py` describes positional differences
in ordinary AST JSON, preserving numeric tokens, object-pair/array order and
duplicate keys. Only literal raw equality returns success; formatting or
escape-spelling differences still fail. Its bounded summaries never become
keys or normalized input. The native capture plan and its remaining proof
obligations are in the [profile report](../reports/2026-10-16-warm-frontend.md).

All 12 native-capture AST comparisons now complete. Vest unchanged inputs
match, but IronKV contains a two-entry external-function order reversal.
Comment/body captures change position-bearing fields even in control
buckets. These are diagnoses of literal input misses, not permission to
sort/drop fields or reuse context; the [report](../reports/2026-10-16-warm-frontend.md)
records exact scope and raw hashes.

A later audit finds the captures' dependency patch was a no-op: the full
source API received a packaged runtime directory. Retained Cargo locks bind
original project macro/library dependencies, not the fork-built copies.
The corrected controller requires a full frozen checkout, a nonempty patch
receipt and exact Cargo metadata origins, but remains unexecuted natively.
No complete-key or fork-macro qualification follows from the old captures.

## Consequences

A key that misses ordinary edits is recorded as a failed performance design,
not weakened until it hits. A compiler-owned incremental mechanism remains
an alternative, but it must demonstrably reuse work and preserve Verus's
callback/query dependencies. Proc-macro optimization and avoiding repeated
AST allocation can reduce the floor independently while persistent reuse
is developed.
