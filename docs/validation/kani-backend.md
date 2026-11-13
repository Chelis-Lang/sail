# Bounded Kani observation decoder

Status: bounded native Linux decoder acceptance and independent review pass on
the development branch. The executor, CLI, complete direct-Kani conformance,
cache authority and Mac qualification remain open. No Sail backend availability
is selected. [ADR 0128](../decisions/0128-kani-bounded-observations.md) defines
the library-only boundary.

## Accepted component

Exact implementation `24286578d8a722c7baba00c96e846b49e8c82e7e` runs on native
EC2 c7i.4xlarge/us-east-2, Ubuntu 24.04, Sail Rust 1.99.0. It decodes a single
hand-written Kani 0.68.0 / CBMC 6.11.0 proof-harness observation. It creates no
execution, currentness, proof-receipt or cache authority, and never reports an
ordinary verified verdict. Consumer skill instructions remain unchanged because
CLI/MCP and released availability are unchanged.

The native direct-tool baseline uses arrayvec 0.7.8 and Kani's pinned
nightly-2026-08-21. Its unchanged-lock original captures distinguish a bounded
try-push pass, an actual push-panic property failure and an insufficient unwind
bound. The decoder preserves failed checks exactly and requires the selected
harness, exit, single textual verdict, structured execution summary/error,
complete check inventory and native property totals to agree.

| Check | Actual result and limit |
|---|---|
| Focused decoder tests | Four pass natively, including original review regressions |
| Affected lint and layering | All-target clippy and 18-main/3-extension layering pass |
| Three original native exports | Bounded success: 88 Success/6 Unreachable; counterexample: 66 Success/3 Unreachable/1 Failure; insufficient bound: 1 Failure/302 Undetermined, no tier |
| Full workspace gate | 1,397 tests pass, 53 skipped; fmt, clippy, doctests, rustdoc, layering, dependency policy and generated benchmark page pass |
| Independent decoder review | Approves the exact bounded slice: four libtests, 88 contract cases (83 refusals/five positives), five explicit text-boundary probes and one annotated inconsistent historical positive |
| Existing acceptance models/refinements | All 35 model cases and 316 production-refinement cases pass natively, with eight model and 350 production/runtime source bindings. These concern existing selected components and the reviewed lock binding, not a formal theorem about Kani |

The independent reviewer audits and reuses the exact-source compiled libtest and
example, then executes its own cases with source/target mounted read-only. It
performs no concurrent compilation, lint or installation while the separate
formal suite runs. All 811 review payloads are independently rehashed by the lead.

## Limits

Only assertion-only counterexamples and unwind-only bound failures are admitted
in this first slice. Other or mixed failed categories refuse. Generated,
contract, should-panic and loop-contract modes refuse. Duplicate JSON keys,
missing contract records, inconsistent native counts and contradictory native
error fields refuse before an observation is returned.

Text agreement means one exact `VERIFICATION` verdict line. The decoder does not
parse the complete human-output check inventory or summary; five synthetic
text-only contradictions are retained as explicit scope-limit probes. The three
untouched native captures independently agree across those textual inventories
and JSON. Raw output is preserved; this component does not establish a complete
human-output oracle, execution authentication or direct-backend conformance.

Under ADR 0087 this unpublished diagnostic decoder changes no selected Verus
verification or solver path. No corpus entries are affected and no unrun corpus
suite is described as passed. Default Z3 input/key behavior is unchanged. The
contributor-only `inspect_export` example reads supplied captures and does not
invoke Kani. Broader backend, process/cancellation, source/bounds attribution,
cache, Mac and release gates remain required.

## Binding and evidence

The independent exact lock review approves only Cargo.lock SHA-256
`f1affe98cd0fe1de3b8b0d0fdcc8534c1eb92f9005471d1a2379caf16fee9b0e`:
removing the new unpublished sail-kani record yields identical parsed old lock
content. Existing package/source/version/dependency and all 26 selected
parser/command bindings remain exact. Only PARSER_NATIVE_SHA256's Cargo.lock
entry advances to those reviewed bytes; no proof or gate is waived.

Raw logs, exports, cases and audited artifacts stay outside the repository.
Rejected d2 review archive, all 255 payloads rehashed:
`9b5b7d1490f14c5702e8ef41ff08b23c80d69617d11b7caef1e126901ac4560a`.
Accepted successor review, all 811 payloads rehashed:
`deb4778299462cee5717a5379c769fc5182fc6cd294e010eff5145163976bbf4`;
index `52141ce5f4e11163df0de648fe8b1155872d56c2246201d73b4dcc4394c3f6a9`;
report `7a5704a4aa486ff5febbc1692e406f67e85392f2b9a43b80ebcd9dbfa22ae368`.
The complete lead-run acceptance archive contains 745 independently rehashed
material entries, including all original direct-tool captures, decoder outputs,
compiled decoder artifacts, full gate/model/refinement logs and reports:
`e028f5d5aa7062a46f63beb8c6a54fec956fc7de9550ca3daeb4fe439f0458cb`;
index `ddd27ffba2ba8e84ea0c89fe72d9c850e70d39bc802f5e2f8c2b2154b63e03ec`.
The existing models and refinements preserve their explicit conditional and
component scope; surrounding I/O, origin and service composition remain open.

## Attempts retained

| Source/date | Result and cause |
|---|---|
| First direct wrapper, 2026-11-13 | Unsupported `cargo kani --locked` refuses all three calls before execution. Rerun uses supported locked Cargo fetch, offline Kani execution and exact before/after dependency lock |
| `e8d1d553`, 2026-11-13 | Three unit tests pass; clippy refuses test empty-slice assertion style. No actual-export decoder acceptance follows |
| `032d37ae`, 2026-11-13 | Revised empty JSON slice assertion is type-ambiguous, E0282; successor supplies its explicit Value slice type |
| `d2be6c50`, 2026-11-13 | Focused/lint/layering and three original exports pass; full gate correctly refuses unknown lock bytes. Independent review rejects 14/34 adversarial cases: contradictory property/error records, unsupported failures, missing contract metadata and duplicate keys |
| Initial successor review harness, 2026-11-13 | Deleting harness metadata exposes a runner KeyError before a valid verdict check. Original retained; corrected runner keeps the original requested name, with no product change |

Original strict failures and the rejected implementation remain retained. The
successor's acceptance does not promote the earlier attempts or the unimplemented
backend stages.
