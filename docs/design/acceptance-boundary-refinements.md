# Acceptance boundary refinements

Status: follow-up implementation is in progress above the production acceptance
operations in [PR #362](https://github.com/Chelis-Lang/sail/pull/362). The earlier
PR's validation checkout remains frozen. This work does not establish the
service-wide theorem or close the remaining provider assumptions by declaration.

The [acceptance design](evidence-acceptance-core.md) states the intended theorem.
The [production refinement guide](acceptance-production-refinements.md) records
the executable operations already checked with stock Verus. This follow-up
connects more of the actual producers and consumers of their evidence.
Verus translation and the pinned solver remain trusted as in the original
design. Default-mode verdict and diagnostic conformance remains mandatory.

## Obligations and first implementation slices

| Obligation | Concrete next slice | Completion still requires |
|---|---|---|
| [Raw SMT interpretation](https://github.com/Chelis-Lang/sail/issues/371) | An owned exact transcript and checked parser-item endpoints feed the original complete prefix; malformed, overlapping or omitted nontrivia bytes refuse caching | Relate actual command/reply delimiter recognition and solver/mode/key encoding to the exact model binding |
| [Cache origins](https://github.com/Chelis-Lang/sail/issues/372) | Preserve actual immutable publication payloads through the writer boundary; relate admission to observed live-run history | Refine local initialization, physical persistence/recovery and every shared/bundled writer contract; a hash or signature does not prove solver truth |
| [Compiler input closure](https://github.com/Chelis-Lang/sail/issues/373) | Connect independently recorded compiler-boundary observations and complete keyed event accounting to input evidence | Establish that the provider records every actual compiler input, including generated input and effective configuration; completeness of a supplied list is insufficient |
| [Reads and launches](https://github.com/Chelis-Lang/sail/issues/374) | Preserve the actual read/path/process observations consumed by the factories under a named supported provider contract | Establish real observation/launch identity and the supported atomicity boundary; hash-then-unrelated-path-launch is insufficient |
| [Original request correspondence](https://github.com/Chelis-Lang/sail/issues/375) | Retain independently resolved request scope and pre-execution identity, then check the actual retained result against that request | Relate submission, argument normalization/effective package options, compiler target selection and worker-side capture; result-selected flags cannot supply the expected inventory |
| [Success surfaces and composition](https://github.com/Chelis-Lang/sail/issues/376) | Inventory each CLI, JSON, job, repair and MCP success path and require the appropriate request-bound authority | Connect complete physical observations and original requests through a concrete service transition refinement into the composed model |

These are separate deliverables under
[#368](https://github.com/Chelis-Lang/sail/issues/368) and
[#346](https://github.com/Chelis-Lang/sail/issues/346). A bounded implementation
may discharge part of one row without completing that deliverable. Each
handoff and qualification must state the exact boundary it establishes.

## Evidence flows and construction authority

Raw transcript bytes precede their interpreted commands. A parser's offsets
are candidates: the checked operation must validate their correspondence to
the actual immutable bytes. Failure disables cache interpretation and preserves
the original forwarding and pinned-solver path. Query normalization is not part
of this work.

Input observations retain the descriptor actually returned by the producer.
Independent expected records establish required coverage. Missing events,
duplicate keys, unreadable outcomes and unmatched identity cannot become a
successful complete observation through relabeling, sorting or a fresh stamp.
The original capture guard remains bound through selection and refresh.

The independently resolved request precedes its result. Retained request
records bind the selected package/target/kind, invocation, workspace, run,
input/context identity, arguments and runtime/mode where applicable. They are
not proof authority merely because they deserialize or carry a checksum.
A checked immutable borrow token establishes the relation between the actual
request and actual result. Missing older records withhold completeness while
preserving historical diagnostics and input-currentness observations.

A caller requesting repair requires the stricter repair policy. A whole-target
proof cannot be manufactured from an exact partial receipt. A compile-only
observation may match a request and remain Current while supplying no proof
authority. Genuine enabled zero-function proofs remain admissible.

Digest equality retains the existing cryptographic assumptions; it does not
replace full logical input equality in a theorem. Public report headers do not
establish an observed launch, an exhaustive compiler inventory, an admitted
solver origin or the original request by themselves.

## Composition and review criteria

The composed model already proves its conditional invariant over its permitted
execution histories. The missing production refinement must identify the real
event producer for every model fact and the exact checked operation consuming
it. The proof ledger distinguishes:

- Source-shared executable bodies checked directly by stock Verus.
- Native adapters that construct or consume candidates, with concrete tests.
- Physical compiler, filesystem, launch and store-provider contracts.
- Runtime success surfaces still outside the established transition relation.

An accepted component must include a nonempty constructive witness, explicit
representation relations and guard-removal controls that fail through ordinary
verification. Timeouts, syntax errors, missing source, malformed output,
focused verification and missing obligations fail qualification. Closed source,
import, module, type and function inventories must include every new authority
constructor; new fields must remain tied to the actual declarations.

Native macOS/Linux gates and solver faults remain separate from proof checking.
Changes to shim, cache, daemon, keys or harness also require the original full
fast and standard differential suites. Measure any added validation cost on a
native host and retain every sample. Existing comparator, corpus selection,
deadlines and baselines remain unchanged.

The follow-up remains a draft until its implemented slices have source-bound
proof receipts, independent review and the required integration gates. Broader
obligations remain open even when those slices pass.
