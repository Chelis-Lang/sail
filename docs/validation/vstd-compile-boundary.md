# vstd compiler-boundary replacement

| Gate | Current state |
|---|---|
| Implementation | Prototype wired into the real Cargo compiler wrapper; disabled unless `SAIL_VSTD_BOUNDARY_RESEARCH=1` |
| Native prerequisites | Nonverification invocations return directly through the original compiler before plan loading or lookup |
| Reuse scope | Pinned, nonprimary vstd library only; root outputs, no native units or Cargo fingerprints |
| Identity | Fresh linked-library/VIR bytes, compiler options, runtime/sysroot, actual solver bytes, source and declared environment |
| Proof provenance | Complete successful document required before recording; nonlocal origin preserved in reports and receipts |
| Review | Initial static review confirms the structural boundary; macro input completeness and publication races remain unqualified |
| Execution and performance | Initial native source compiles and all 15 shipped/boundary tests pass on `f6fdb636`; full clippy stopped on one documentation backtick lint, corrected next. Actual first-project reuse/performance remains unrun |
| Default/release selection | Neither selected nor qualified; v1 store remains quarantined |

The prototype ports the original shipped-build serializer and proof-origin
plumbing onto current Sail. Its schema and kind differ from all-unit shipped
bundles. It explicitly binds the actual solver's bytes independently of the
runtime's recorded manifest. Focused, engine and loader-variable setups keep
the ordinary compiler path.

The native experiment must record vstd once, then use a different fresh project
whose native units are genuinely compiled. Compare the actual dependency bytes
and every lookup outcome before attributing any saving to reuse. Repeat with
the private native compiler replaced at the same path: fresh stock and Sail
must both fail, and no vstd hit may hide that failure. Retain failures and
misses. Compare complete per-function verdicts and diagnostics, including a
failing precondition naming the user's actual registry source, against stock.

Research-only commands use `SAIL_SHIPPED_RECORD` for a recording directory,
`SAIL_SHIPPED_DEPS` for a candidate bundle and `SAIL_SHIPPED_TRACE` for keyed
lookup observations, together with the explicit research flag. A recording
is not an accepted release artifact. No public asset, runtime pin, ordinary
verification behavior or conformance comparator changes.

The environment allowlist and dep-info source model inherited from the older
prototype do not prove arbitrary proc-macro input coverage. Publication also
needs an execution input boundary. Those limitations remain blockers to default
enablement even if a timing experiment succeeds.

The first source run used a native c7i.4xlarge in us-east-1 with Rust 1.99.0.
It compiled the actual product crate and passed all 15 serializer/boundary
unit tests. The broader clippy run failed on `doc_markdown` for the research
variable in module documentation; this attempt is not a passed gate. Raw logs
remain outside the repository. Later source adds whole-package file-set
before/after guards and rechecks linked dependency bytes before publication;
these detect ordinary mutation, not an immutable execution or ABA proof.
Actual linked-byte digests are retained in research traces for the first-project
experiment. No timing or complete-native-input qualification follows from unit
fixtures.

- `930dfea8`, 2026-10-17, native source attempt: failed compilation in the
  new publication recheck (`String` environment pairs passed where `OsString`
  pairs were required); application and timing stages unrun. Corrected next.
- `930dfea8` plus the environment-pair correction, 2026-10-17, bounded macOS
  source check: all 17 boundary/serializer tests pass. Workspace compilation
  found a missing optional origin field in the existing fallback renderer;
  corrected next. This was not a passed workspace gate.
