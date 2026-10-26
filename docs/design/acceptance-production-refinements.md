# Production acceptance refinements

Status: source sharing, concrete refinement proofs and production callers pass
native macOS and Linux checks. Native pre-push gates, hosted checks and full
fast/standard conformance pass. The owner accepts the bounded native timing
result; its measurement limits and the remaining service-wide theorem are
recorded separately. The owning
[validation record](../validation/acceptance-production-refinements.md) separates
checked source from accepted integration.

The [acceptance design](evidence-acceptance-core.md) targets four independent
risks: wrong query/reply association, stale input reuse, incomplete requested
scope and misleading verified reports. This implementation connects selected
production operations to those models. It does not establish the proposed
service-wide theorem.

## What production executes

`crates/sail-acceptance` is a dependency-free leaf with five direct callers:
`sail-api`, `sail-verus`, `sail-shim`, `sail-cache` and `sail-daemon`.
[ADR 0079](../decisions/0079-source-shared-acceptance-refinements.md) records
that graph. The existing shim isolation rules still apply.

The leaf's syntax envelopes preserve executable arguments, types, conditions
and bodies. Ordinary Cargo erases contracts and proof hints. Direct stock
Verus checks those same bodies with a fixed `sail_verus` configuration. This
is source sharing; neither copying a predicate into a proof nor asserting that
an arbitrary candidate is valid supplies the connection.

| Boundary | Checked executable source and concrete relation | Production callers |
|---|---|---|
| Query association | Owned FIFO and bound request/reply pairs; replay cursor; permutation-checked publication order; sticky publication eligibility; recovery field admission | Shim pairing, replay and publication; cache and daemon recovery admission |
| Input reuse | Exact byte/key comparisons, complete readback ledgers, checked permutations, original capture/guard preservation and selected-scope coverage | Input snapshots, read-time refresh, receipts and parallel file/artifact observations |
| Scope and authority | Actual typed report fields; all summary counters; confirmed verdict/provenance/tier checks; complete one-to-one target coverage; distinct receipt, whole-target and repair policies | Receipt reuse, runner completion, repair results, retained job projections and compact MCP completeness |

`proofs/refinement/{query,inputs,authority}.rs` states concrete-to-model
relations. For example, a factory returning `CompleteReport` guarantees the
actual borrowed report satisfies the checked predicate; projection of any
selected function then satisfies the authority model's good-function relation.
Private fields prevent ordinary callers from constructing those authority
tokens. Sorting outside the checker only proposes an order: checked forward
and inverse mappings must independently establish complete unique coverage.

The input ledger records actual worker-returned descriptors and raw
Present/Absent/Unreadable outcomes. Expected input identities are constructed
independently. Relabeling an observed value with an expected path would defeat
the correspondence; readers preserve the returned descriptor through admission
and fold hashes only after keyed validation, in the original requested order.
This is native adapter hardening. It does not prove that the inventory contains
every compiler input or that the operating system observed an atomic snapshot.

## Verification must actually run

A successful process and an empty function inventory do not distinguish a
legitimate zero-function proof from `--no-verify`. The pinned stock verifier
reports success for a deliberately false assertion when that option skips
verification; [VI-0035](../verus-issues/VI-0035-no-verify-success.md) contains
the reproduction.

Acceptance must record the effective per-package verification setting and
refuse compile-only evidence for proof authority. Inspecting only the outer
command misses package-specific overrides. Genuine successful zero-function
verification remains admissible. Current input identity is a separate fact:
compile-only or failed observations may be Current without being verified.
Retained records from an earlier producer require conservative admission; a
new envelope marker alone cannot establish that an old report was verified.
CLI job labels and MCP proof headlines consume checked target authority and
distinguish current, historical and unknown input observations.

The original lifecycle model omitted whether verification was enabled.
Comparing its completion event with the actual driver exposed that missing
premise; the direct stock reproduction established that the verifier emits that metadata.
The model now records `VerificationSetting`, and the checked production
predicate requires verification to be enabled. Removing that guard makes the
selected proof controls fail. The reproduction establishes the real behavior;
the amended proof checks the acceptance rule over its stated inputs. Native
option decoding and the complete service correspondence retain their explicit
refinement boundaries.

## Independent checking

Commit the source first, then run Python 3.11 or newer:

```sh
python3 scripts/check-acceptance-refinements.py \
  --stock-archive "$SAIL_STOCK_ARCHIVE" \
  --stock-dir "$SAIL_STOCK_RUNTIME" \
  --out "$SAIL_REFINEMENT_OUTPUT"
```

The output is a new directory outside the checkout. `ci/local.sh refinements`
uses the same runner in the core CI image. The separate
**Acceptance production refinements** job feeds **CI completion**. Its
native wrapper takes 197 seconds on the qualified Linux host; the hosted
production-refinement and model-proof jobs also pass.

The runner captures canonical Git blobs with inherited Git redirection and
replace refs disabled. Fixed drivers compile captured source, never reopened
working paths. It validates the pinned stock archive, every extracted runtime
file, Rust identity, module/dependency bindings, complete function inventories
and exact result counts. Source discovery, file kinds and auto-build-script
absence are checked again after verification. Stock runs directly with pinned Z3, sanitized settings,
`--no-cheating --output-json --num-threads 1`. Sail, its fork and its caches do
not certify these proofs.

Actual report types include serialization and presentation code outside stock
Verus's supported subset. A conservative declaration stencil captures the
actual authority-read fields, types and enum variants. It records all omitted
fields and hashes of the full original declarations and binding environment.
Conditional fields, custom derives, type substitutions and unsupported shapes
refuse checking. Omitted presentation fields are not assumptions of validity.
The stencil generator, syntax envelopes, compiler, ordinary sort forwarders
and Python runner remain reviewed and tested trust dependencies.

Every selected guard mutation changes the captured production dependency.
It must fail through ordinary verification errors with complete unchanged
scope. Syntax errors, missing modules, signals, malformed JSON, resource
limits and timeouts fail the gate. The report distinguishes checked bodies,
declaration projections and surrounding runtime source bound only by hash.
Binding a file's hash does not prove its behavior.

## What remains to prove

| Remaining obligation | Why these refinements do not discharge it |
|---|---|
| Parser/framing and exact raw SMT prefix | Checked owned pairs cannot establish that native parsing assigned the right bytes to each pair |
| Cache writer, persistence and initial origins | Field admission and association preservation cannot prove an arbitrary plausible `unsat` came from a truthful solver for that query |
| Actual compiler-input closure | Complete coverage of a supplied ledger does not prove the supplied inventory contains every input actually consumed |
| Filesystem and launch observations | Tagged read outcomes and executable hashes do not establish atomic path/process identity or observation |
| Request/output correspondence | Typed whole-report authority does not by itself prove every schema-two report is bound to the retained original request |
| All public success surfaces and composition | Individual proved operations and native callers do not constitute a service-wide transition refinement |
| Native integration and cost | Proof checking does not establish conformance, diagnostic parity or performance |

Stages C–F in the original design therefore remain open as integration gates.
Stage C has actual source-shared logic and callers; D and E have selected
concrete operations and hardened adapters. Their remaining obligations cannot
be replaced with an assumed successful-result Boolean. Shared/bundled writers,
cryptography and trusted Verus translation retain their named trust boundaries.
