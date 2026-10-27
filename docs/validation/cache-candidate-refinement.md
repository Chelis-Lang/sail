# Candidate write refinement

Status: full-root Stock positive proofs cover the shared generation and
declared-charge accounting operations. Ordinary-negative and native qualification
remain incomplete; this is not a complete production transition or cache-origin
theorem. Native map/lock/LRU/charge correspondence remains unfinished; neither
Q40 architecture is selected.

| Source | Actual result | Limits |
|---|---|---|
| `f9fc1d34` stock Arc support | Whole-file direct stock exits 1 after 9.001848591 seconds; all five actual `Arc::ptr_eq` calls are unsupported. No resource marker; final source/runtime checks pass | Zero proofs or ordinary-negative credit. A new assumed pointer-identity specification is not adopted ([#473](https://github.com/Chelis-Lang/sail/issues/473)) |
| Generation replacement | Actual MemoryTier/Slot, writer FIFO, raw PUT/import paths, disk-argument projection, returned-Result dispatch and stale-read removal use the source-shared candidate records and generation operations; nonempty witnesses and 40 full-function controls are prepared | Source only and unrun. Canonical closure is extended with all inherited controls retained; actual new function/count inventories await whole-root discovery. Native map/lock/accounting proofs, three separate native mutation controls and complete ordinary/native/differential gates remain required |
| `37a38135` first native attempt | Model/source suites pass 12+62; full query Stock stops at executable endpoint getter inside ghost assertion; ordinary Clippy stops at two explicit-refusal style lints; final source/runtime guards pass | Inputs/authority discovery and native tests unrun. No new inventory, proof, negative or complete-component credit ([#483](https://github.com/Chelis-Lang/sail/issues/483)) |
| Owned accounting follow-up | Actual native counters and install/apply/remove/disable paths use shared Accounting; two nonempty witnesses, 13 additional Stock controls and one additional actual native omission control prepared | Source only, unrun. Supplied-charge accounting is distinct from native map/lock/LRU/charge correspondence and solver origin |
| `494c091c` shared operations | Source suites pass 12+62; all three complete Stock roots pass: query 519 obligations/908 functions, inputs 308/538, authority 510/895. Every previous function is retained; final source/runtime guards pass | Positive proofs only. All 214 query ordinary controls remain unrun; native Clippy stops at explicit guarded subtraction's style lint, so native tests are unrun. No complete component/service theorem |
| `a6e5208e` native preflight | Shared/native production compilation clears the prior errors; Clippy stops at cfg(test) settle helper's intentional owned-handle argument; final source/runtime guards pass | Source suites, fresh proofs and native tests unrun at this head. A reasoned fixture-only allowance preserves every statement/assertion and the consumed original handle; correction needs qualification |

The closed `37a38135` attempt is reconstructable from the source bundle and raw
logs in object `experiments/2026-10-27/acceptance-candidate-write-37a38135-native.tgz`,
7,345,414 bytes, SHA-256
`62a542a385fcbe8096d638c4cad0837ac84609ef8c010dd3d4a93e97f8e229a8`.
PUT and exact GET readback pass; independent lead download remains separate.
Its first query failure occurs before verification, so no ordinary negative or
candidate proof result is inferred. The source correction captures the actual
executable endpoint borrow before ghost code, and adds narrowly documented
Clippy allowances retaining the exact checked refusal bodies and all controls.

The closed `494c091c` result and complete source bundle are in object
`experiments/2026-10-27/acceptance-candidate-write-494c091c-native.tgz`,
7,597,201 bytes, SHA-256
`82d1baff2fffcfa066f2cebe324bda9d405ea5e4d75fb540cf38c9709d5473a2`.
PUT/exact GET readback pass; independent raw audit is pending. It uses the same
native c7i.4xlarge/us-east-1 lane, pinned image/runtime, direct whole-root stock
command, default resources, one thread and external 120-second limit. Query,
inputs and authority verifier wall times are 31.9581, 27.5139 and 19.5287 seconds.
There is no function focus. The manifest locks the actual observed inventories;
the subsequent source adds only a reasoned local lint allowance to the proved
accounting body, retaining all mutation contexts. That new head requires its own
fresh full-root/final-guard and native qualification before controls launch.

The closed `a6e5208e` preflight is object
`experiments/2026-10-27/acceptance-candidate-write-a6e5208e-native.tgz`,
7,090,063 bytes, SHA-256
`21b017c803cb4b5de1de1d45097c8e524588d0d6d41a10a56ce65795d0b27bf7`.
The source bundle, exact native stop and final guards are retained; PUT/exact GET
readback pass. It stops before the remaining suites, so it earns no fresh proof
or native-test credit and does not replace the historical `494c091c` positives.

The Arc probe is `proofs/support/candidate_write_arc.rs` at source
`f9fc1d34bc156a3749dc0c26359cddf75a352a69`, SHA-256
`d469be720fc4600f2bfa1b7e3a669f0e4918018fb7002b7682893e4399b76bca`.
It uses pinned stock Verus `0.2026.10.18.7d82994` / `7d829946`, Rust `1.99.1`
/ `58cf8f9c`, default resources, one thread, `--no-cheating --output-json`
and the unchanged 120-second external deadline. Its unsupported diagnostic
names `alloc::sync::impl&%25::ptr_eq` at lines 17, 25, 35, 46 and 47. The tool's
suggested `assume_specification` is not used. The closed raw report's SHA-256 is
`2b48ac9f7171da43e9ca082de1879211cfb1ce42e082d251179a986774dd103c`;
the lead retains it with the key-support probe for archival review.

The replacement uses a checked lifetime clock for insertion generation and
LRU ordinal. Clearing must preserve it; exhaustion must permanently refuse
new generations and memory reuse, without stopping disk writing. This also
addresses the existing unchecked native tick increment
([#474](https://github.com/Chelis-Lang/sail/issues/474)); native lookup/insertion now use that checked clock; disablement exhausts and
clears data without reconstructing Inner or clock. The source reproduction
recipe and new native regression have not executed. The native source policy
closes the reviewed constructor/field-access surface and supplements exact
native bindings; it is ordinary reviewed tooling, not a Mutex/map proof.

The same source review found an unrepresentable public Config.queue_wait
panicking during Instant addition ([#478](https://github.com/Chelis-Lang/sail/issues/478)).
Checked deadline construction now drops the cache write before queue or pending
mutation; its bounded Duration::MAX regression is prepared and unrun.

Native actual-Arc preservation mutations are recorded separately in
`proofs/support/candidate_write_native_controls.json`. Their actual Rust test
failures cannot count as any of the 53 new required Stock negatives. The
canonical query plan contains all 161 inherited plus 40 generation and 13
owned-accounting controls (214 planned negatives); inputs
and authority retain their complete existing lists. Verified counts and
function arrays now retain the actual complete `494c091c` observations. No long
control matrix may run before the corrected lint head passes fresh discovery and
native compilation.

The 13 additional runtime descriptors are also retained independently in
`proofs/support/candidate_write_accounting_controls.json`. All 40 generation
descriptors remain present, with identical lint attributes applied to original
and mutant contexts. Four native controls are separate from Stock credit.

The [design](../design/cache-write-candidate-refinement.md) identifies the real
callers, expected ABA refusal, metadata charges and pending native/proof gates.
Source-support roots and planned controls are not canonical acceptance cases.
No branch freshness, source hash, payload equality or plausible reply shape
supplies a solver-origin fact.
