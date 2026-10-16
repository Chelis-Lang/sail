# Source-supplied progress option (#203)

**Implemented; native BASIC and all 4,709 fork source cases pass on private source.**
The main-based harness join passes its native workspace gate; overall
combined-runtime acceptance remains separate. The runner omits only
its own redundant `--no-report-long-running` default for
`sail_vi0012_progress_order::sail_vi0012_bucket_start_notes_in_bucket_order`
when the original 1,986-byte source has SHA-256
`046f7fb964c3a6dcb4cbccca1ec9cd61e4d4ed603d09b7976353f930fb6d8a63`.
Changed or missing source, other binaries and other cases retain the default.
Explicit thread overrides remain intact, even if a test's own arguments would
conflict. Arbitrary test options are neither parsed nor deduplicated. Both
arms and exported reproduction commands use the same effective environment.
The original fork source, executable and assertions are unchanged.

The licensed original-source fixture checks this narrow boundary; it is not
compiled as a Verus regression or counted as native proof coverage. On private
Sail `af339f8a`, native BASIC passed: workspace gate 645 tests (35 skipped),
solver 35, all seven end-to-end phases plus extra, PR four arms each 33 Same,
eight exact proof-note repetitions and all 18 actual-binary selector controls.
The original VI-0012 test passed on both arms (one Same), retaining all 20
internal assertions/rounds; nine JSON regressions also passed. The complete
genuine fork suite passed strictly: 4,709 Same (58 ignored), all 4,651
nonignored plain tests passed, zero Different, Unstable or infrastructure
errors, in 1,225.58 s. These results identify that private
source, not a native gate of the subsequent main-based harness-only join.
The main-based harness-only join `2bd59014` separately passed its full native
workspace gate and all controls in image `08bd13d8`, built from main `c89e29d3`,
on a c7i.4xlarge in us-east-1. Raw archive SHA-256
`9d417731ece59058f380072fb9171fb54c8120c2c76cd646c47cc5689fbc65cf`.
That result qualifies the workspace join, not the private fork runtime or any
source-suite run at this later source. Combined-runtime corpus, determinism and
performance gates remain separate.
The subsequent integration with #202's test-executable digests requires a new
native workspace/routing gate; it remains pending. Its selector fixtures bind
their newly written list executables and reject legacy digestless or incorrect
digests. The expanded 22 controls also distinguish nondefault ordinary and
Singular stock suites from the default by their actual listed markers, and
reject fork/engine feature variants under the stock selector. These restore
the preexisting stock source-override behavior; the earlier list controls did
not distinguish installer admission from selection of the actual suite.
The completed #202 integrity/rebuild and full fast/standard runs qualify
`6ba30636` separately, as recorded in the
[source-suite integrity record](source-suite-integrity.md).
There is no fork rebuild, accepted-pin change or publication in this correction.

| Attempt | Result / limits | Raw SHA-256 |
|---|---|---|
| 2026-10-16, Sail `af339f8a`, same genuine runtime/environment | Native BASIC passed; original VI-0012 regression 1/1 Same, both arms passed in 6.56 s; nine source-JSON regressions Same. Full suite 4,709/4,709 Same (58 ignored), all nonignored plain tests passed. | BASIC `dd39093b23b8c970c687db18014edd90d4c41e8baadb2bd22365ab26fb86e569`; original regression `cc9c1e0087778d3c66dd30fa6810d313c0da8996a62cbaf89e6e07908d32d67a`; JSON `884eab935f931dd26e4ef82eb5ec1b38bb0f9195c2cfacabda817bef42d16d1f`; full suite `b2304b6dd70355fd69a0ad6241c3f2a00c25156e8443ffb357b80005ce165faf` |
| 2026-10-16, Sail `af339f8a`, replacement remaining-gates controller | Complete fast corpus fetched; determinism helper failed before verifier/SMT execution because its subprocess recording proxy lacked `DEVNULL`. No determinism/standard/deep result follows. The corrected helper is independently tested through the entire committed oracle and TEE; this fixture coverage is not native acceptance. | `5f7d7fdfb1d21f0dcd4dc615b58524a5d72ec1d113b3107bb8dc2636c6aea37f` |
| 2026-10-16, Sail `af339f8a`, subsequent determinism phase | Preparation failed before any SMT execution: full fast corpus had not been fetched on the fresh host. No determinism, standard or deep result follows. The source-suite pass above remains its own completed scope. | `eec84add58f8e7bc103ec3c5e940019fa976d5cf39623415ba016dbd48971fc2` |
| 2026-10-16, Sail `168ab7abc7cd44196326f141aeba6284b9f84573`, genuine fork `8a373f81a507b7ef4f144767ccd237d3d3de6c48`, native c7i.8xlarge/us-east-1 | Full source suite **failed strictly**: 4,709 selected (58 ignored), 4,708 Same, one Different, zero Unstable/infrastructure errors, 1,177 s. Original VI-0012 case failed on both arms with `Option 'no-report-long-running' given more than once`; eleven plain runs were stable and matched the shim. Identical failed nonignored tests remain Different. No determinism/standard/deep followed. | `c4e3826df19353f4935ad4d2b80689d2b1c754f61233f9b9aa15d1026608624a` |

The original runner independently omitted its quiet default from `reproduce.txt`;
that note cannot reproduce the duplicate without restoring the effective
`VERUS_EXTRA_ARGS`. All actual plain/shim output, materialized input, original
note, expected/actual 531 source hashes, runtime and 154 test executable
identities remain in the failed archive. The original note is retained, not
rewritten. The [deep-tier record](deep-tier.md) retains the independent stock failure
and earlier corpus/source-suite scope.
The genuine suite manifest SHA-256 is
`a765909b9286eca63d8fc67e37ee3aa3180e15b2af2bba8115ee736ae8b97e90`,
variant `64fd48a31c902e49`, source lock
`80c71fdd00ef5073cd93edf6b5e37a635f7b4114fba8f6553ba2125eecb1395e`,
and pinned Z3
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
The unchanged genuine runtime/test artifacts were restored and checked before
and after every phase. The harness-only branch retains accepted `73dea021`
pins; the tested `8a373f81` candidate is neither selected nor published.

See [#203](https://github.com/Chelis-Lang/sail/issues/203) and
[ADR 0019](../decisions/0019-rust-verify-test-conformance.md#effective-progress-option-supplied-once).
