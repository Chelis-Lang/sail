# Metadata component replay proposal

The first native scope remains failed at a test-only error format expectation.
The corrected scope passed eight metadata tests, thirty freshness tests and
all-target strict checks. Its original rewriting counterfactual exited 101 at
the exact unchanged `Arc::ptr_eq` assertion, with zero passing and one failing
case. The controller nevertheless rejected it: `--nocapture` inserted the panic
between the case prefix and `FAILED`. The restored-positive phase was unattempted.
See the [complete classifier failure record](m7-preserve-freshness-metadata-classifier-failure.json)
and [literal actual output](m7-preserve-freshness-metadata-libtest-failure.txt).
Neither historical scope is relabeled as a whole pass.

Independent closure found no holders, unreadable processes or surviving owned
process groups in either container. Protected product/runtime files and earlier
tested images matched their full inventories. All 1,361 source files retained
exact bytes and links. `git apply` changed only `files.rs` permissions from `0664`
to `0644`; this difference is retained, not silently repaired. The actual positive
and negative executables and all eleven raw members are in a verified archive
bound by the failure record. Private environment values were not exported.

The corrected classifier requires exit 101, exactly one running case, the exact
case's failed status and failure-list entry, the unchanged assertion line, and a
zero-passed/one-failed summary. It accepts both compact and actual interleaved
libtest output. Ten controller tests include the literal output and negative
variants; the original compact-line classifier rejected that actual output.
Five additional pure replay-lineage tests reject changed images, compiler logs,
historical controllers and incomplete compile records. Early pure-test drafts
exposed host Python's unsupported `zip(strict=...)` and a temporary-directory
alias; both were corrected before freezing. No native actor ran for these drafts.

The [replay controller](m7-preserve-freshness-metadata-replay.py) proposes one
**new complete component scope**, after the lead's separate grant:

```sh
python3 docs/validation/m7-preserve-freshness-metadata-replay.py \
  --source "$SAIL_METADATA_SOURCE" --target "$SAIL_METADATA_TARGET" \
  --prior-scope "$SAIL_METADATA_PRIOR_SCOPE" \
  --prior-controller "$SAIL_METADATA_PRIOR_CONTROLLER" \
  --out "$SAIL_METADATA_NEW_OUTPUT" --execute
```

The binding fixes the source's six hashes and lock, historical controller/source
record/counterfactual, actual compile logs and prior result, both executable sizes,
bytes and modes. Those images remain in their immutable prior scope, outside the
reused target. This is explicit compiled-image reuse; it has no warm-build claim.
The source is the byte-exact restored positive source, with the recorded mode.
The lead's independent inventory must bind its complete files and current modes
before and after. Existing negative-source lineage is the original exact patch
and compiler execution, not a newly invented receipt or proof authority.

All component tests execute freshly: metadata `8/0/0`, complete freshness
`30/0/1`, original all-target strict check, exact negative case and exact positive
case after source restoration. The one ignored case remains
`managed::freshness::tests::identity_guard_tests::complete_inspect_profile_retains_all_original_barriers_and_exact_output`;
its original reason is an explicit local profiling comparison, not a live reader
or proof gate. No previous test result substitutes a fresh execution.

Each test retains its original 180-second ownership cap; strict compilation retains
600 seconds, release, `--locked`, one job and `CARGO_INCREMENTAL=0`. These caps do
not change any request or Rust test deadline. Shared owned-process cleanup still
makes timeout/interruption uncertainty sticky; subsequent phases refuse. The
replay performs no source patch, restoration, installation or verifier proof.
First unexpected failure stops the new scope. Closure remains independently
verified; reaping a leader is not an orphan-absence certificate.

The replay is prepared only. Product integration, the complete original repair
fixture and live reader census remain pending. Metadata preservation alone cannot
keep the unified owner through an actual source edit; any repair-edit benefit
requires a new independent owner and actual post-child timing evidence.
