# Source-supplied progress option (#203)

**Implemented; corrected native qualification pending.** The runner omits only
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
compiled as a Verus regression or counted as native proof coverage. Formatting
and source review do not qualify the corrected runtime execution. The next
frozen Sail source must pass native workspace/solver/all e2e/PR gates, the
original regression, actual selector controls and the full genuine fork suite.
Combined-runtime corpus, determinism and performance gates remain separate.
There is no fork rebuild, accepted-pin change or publication in this correction.

| Attempt | Result / limits | Raw SHA-256 |
|---|---|---|
| 2026-10-16, Sail `168ab7abc7cd44196326f141aeba6284b9f84573`, genuine fork `8a373f81a507b7ef4f144767ccd237d3d3de6c48`, native c7i.8xlarge/us-east-1 | Full source suite **failed strictly**: 4,709 selected (58 ignored), 4,708 Same, one Different, zero Unstable/infrastructure errors, 1,177 s. Original VI-0012 case failed on both arms with `Option 'no-report-long-running' given more than once`; eleven plain runs were stable and matched the shim. Identical failed nonignored tests remain Different. No determinism/standard/deep followed. | `c4e3826df19353f4935ad4d2b80689d2b1c754f61233f9b9aa15d1026608624a` |

The original runner independently omitted its quiet default from `reproduce.txt`;
that note cannot reproduce the duplicate without restoring the effective
`VERUS_EXTRA_ARGS`. All actual plain/shim output, materialized input, original
note, expected/actual 531 source hashes, runtime and 154 test executable
identities remain in the failed archive. The original note is retained, not
rewritten. The [combined candidate record](combined-fork-candidate.md) identifies
the real builder, source lock, solver, manifest and preceding BASIC/JSON scope.
See [#203](https://github.com/Chelis-Lang/sail/issues/203) and
[ADR 0019](../decisions/0019-rust-verify-test-conformance.md#effective-progress-option-supplied-once).
