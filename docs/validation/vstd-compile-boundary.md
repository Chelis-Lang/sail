# vstd compiler-boundary replacement

| Gate | Current state |
|---|---|
| Implementation | Prototype wired into the real Cargo compiler wrapper; disabled unless `SAIL_VSTD_BOUNDARY_RESEARCH=1` |
| Native prerequisites | Every nonverification invocation executes the original compiler with inherited streams; optional native observations may establish provenance but never substitute outputs |
| Reuse scope | Pinned, nonprimary vstd library only; root outputs, no native units or Cargo fingerprints |
| Identity | Fresh linked-library/VIR bytes, compiler options, runtime/sysroot, actual solver bytes, source and declared environment |
| Proof provenance | Complete successful document required; typed release/machine-local origin and actual establishing invocation preserved through receipts |
| Review | Initial static review confirms the structural boundary; macro input completeness and publication races remain unqualified |
| Execution and performance | Unrestricted literal-local `dcc2bb9e` hit in 8.456 s versus 37.892 s baseline (single sample); native compiler replacement still failed. New closed-source/native-provenance revision passes macOS lint, 23 focused tests and origin test; its native run remains pending |
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

On native c7i.4xlarge in us-east-1, `05c4377e` completed the initial four
application cases. Baseline took 41.388 s; recording failed with SAIL3134 after
43.420 s because `libvstd-8c4d8ba33f7ad30d.vir` still named the registry
directory. The fresh-project serving attempt therefore missed and took
42.081 s. Replacing the same private `cc` path with an exit-42 wrapper caused
actual native build scripts to fail (Cargo exit 101, 0.677 s, no verifier
invocations). This establishes the failure boundary only. The bundle was never
populated, so fresh dependency-byte equality remains unmeasured and no reuse
speedup is claimed. Raw source, compiler streams, keyed extern digests and all
cases are retained outside the repository in the application screen.

Literal-registry research now uses `SAIL_VSTD_BOUNDARY_LOCAL` together with the
research flag. This fixes the specific path scope without rewriting VIR bytes:
keys bind the registry root, and compiler inputs remain literal. Independent
source review also confirms that existing exported `.vir` is not a complete
proof-input key: Verus deliberately prunes restricted functions and proof/exec
bodies. Comparing fresh exported files alone cannot qualify skipped verification.
A complete compiler proof-input boundary or an audited complete input closure is
still required. The local mode is not a replacement qualified for default use.

- `05c4377e`, 2026-10-17, native application screen: portable recording refused
  registry-bearing VIR; fresh attempt missed; changed native compiler failed
  before verification. Workspace lint and 17 focused tests passed.
- Local-mode working source, 2026-10-17: first macOS lint stopped on the expanded
  serving function's length; an unchanged retry also failed. Extracting proof
  provenance into a helper corrected it. Final lint, 19 focused tests and the
  receipt/store provenance test pass; private build target removed.

## Literal-local hit and next input boundary

The second actual application screen, `dcc2bb9e`, used a fresh c7i.4xlarge in
us-east-1 and normal cc7 runtime (`717680b4` archive) with Rust 1.99.0 for Sail
and 1.98.1 for Verus. Four independent physical projects had identical source
and lock bytes, separate Cargo targets and proof caches. Baseline was 37.892 s;
local fill was 39.216 s; the next fresh project was 8.456 s with one verifier
invocation rather than two. Its vstd result retains the fill invocation as
`dependency_build.kind = machine-local`, not a release-built origin. Actual
linked native bytes and compiler-boundary key agree between fill and serve.
The compiler replacement still returns Cargo 101, no verifier invocations,
in 1.599 s. This is one prototype observation, not a median or stock-parity
qualification. Summary SHA-256:
`0ac364feda5de2928f272249db2768d1395770cbb337acb9833af4b91817712e`.

That source could admit an arbitrary first-fill macro, so its speed alone does
not justify default use. The next source binds the explicit 15-package,
662-file source policy in `engine/verus/vstd-boundary-inputs.json`, generated
from local crate archives whose full SHA-256 matches the retained resolution.
The policy excludes `contrib-hooks` and its external provider, randomized case
features, path patches and extra dependencies. Fresh Cargo extraction can omit
`.cargo-checksum.json`; direct pinned source hashes remain authoritative in both
layouts, and any present sidecar must agree. Actual vstd invocation must point
at the specific admitted vstd directory, not a copied-identity sibling.

Optional native provenance now records actual successful library compiler
invocations. It rejects foreign compiler hashes, source/crate shapes and
backend/linker overrides, checks the actual compiler feature set (including
features injected through rustflags), generated inputs and transitive current-run
native output records. It observes only declared library outputs absent before
that invocation, preventing a metadata-only compile from claiming an old matching
library. The real compiler always executes with inherited output and status.
First-fill and serving require current-run native provenance as well as actual
linked bytes. Per-output records and separate research traces are retained.

The complete transitive macro/compiler builtin/span/diagnostic input audit and
native qualification remain open. Before/after checks detect ordinary mutation;
they are not advertised as immutable execution or protection against arbitrary
host changes. The new revision has passed shared macOS workspace clippy,
23 focused tests and the existing proof-origin/receipt test. No Mac performance
measurement or default enablement follows.

- `b3f23535`, 2026-10-17, first closed-source/native-provenance screen:
  workspace lint and 23 focused tests pass; baseline 39.095 s and fill 40.625 s
  both verify completely, but no entry is published. Seventeen native library
  observations refuse the presumed sysroot executable identity. The driver source
  shows Cargo's leading rustc argument is discarded: native units execute
  `rustc_driver` inside Verus's `rust_verify` through its launcher. Remaining
  fresh-pair/compiler-replacement cases were not run. Screen completion is not
  reuse qualification. The next source binds the actual launcher and embedded
  compiler driver, with a regression checking their changed bytes.

Native argument admission also confines dependency-search and incremental paths
to the owned target, rejecting parent-directory escapes. Each research native
trace now retains the actual argument vector alongside per-output provenance;
this is diagnostic evidence, not a replacement for execution. The final source
continues to pass workspace lint, 23 focused tests and receipt-origin coverage.
Its repeated native measurements and full host/workspace qualification are
pending.
