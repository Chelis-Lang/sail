# Metadata publication integration checklist

Candidate `8b3e4f61296669bb8621f1e85a3d9f1b41a2625b` has completed its
component controls; combined product acceptance remains pending.
The [first scope](m7-preserve-freshness-metadata-first-failure.json) stopped at a
fixture error. The corrected scope passed eight metadata tests, thirty freshness
tests and all-target strict; its actual rewriting sensitivity failed as intended,
but the old controller rejected interleaved libtest output. That whole scope stays
failed and restored-positive was unattempted. The [new full replay proposal](m7-preserve-freshness-metadata-replay.md)
reused sealed compiled images and completed every component check freshly.
Its [complete terminal result](m7-preserve-freshness-metadata-replay-result.json)
is PASS, including the unchanged negative assertion and restored positive case. Production bytes remain the original candidate. No proof, daemon,
product or live reader acceptance actor ran for these component controls.

1. After the lead grants an exclusively owned **private Linux** source/target,
   use the frozen [control runner](m7-preserve-freshness-metadata-controls.py),
   Historical SHA `097b7a7f7a612d0de837e75def6016a6583db38e7ae52cd80551efdcae07e9a2`
   used the rejected compact-line sensitivity classifier. Its corrected bytes
   and the separate image-reuse replay are bound by the native plan.
   It checks source identity and counterfactual applicability before mutation.
   Preserve failed scopes, every actual tested executable and the independent
   actor/holder closure census. The exact affected commands are:

   ```sh
   cargo test --locked -p sail-verus --release --lib --no-run --message-format=json-render-diagnostics
   "$SAIL_METADATA_TEST_ELF" metadata --nocapture --test-threads=1
   "$SAIL_METADATA_TEST_ELF" managed::freshness::tests:: --nocapture --test-threads=1
   cargo clippy --locked -p sail-verus --release --all-targets -- -D warnings
   ```

   The runner then removes only the no-op early return, requires the unchanged
   exact case below to fail at its same-owner assertion, restores positive source
   bytes, and requires that case to pass. Negative and restored-positive tested
   images are retained before later rebuilds. Each compile or strict phase has
   a 600-second ownership cap; each test executable has a 180-second cap.
   These do not change any request or Rust test deadline. Jobs remain one,
   release profile, with `CARGO_INCREMENTAL=0`. Timeout, interruption or forced
   group termination makes closure uncertainty sticky, even after leader reap.
   No later actor, patch or source restoration is allowed before an independent
   lead audit. TERM and KILL each have a two-second cleanup grace. Ten fake-child
   and classifier controls plus five replay-lineage controls passed without
   launching a native actor. See the
   [native plan](m7-preserve-freshness-metadata-native-plan.json).

   ```text
   managed::freshness::tests::unchanged_metadata_publication_keeps_same_continuous_freshness_owner
   ```

2. Integrate the production source only after actual affected controls and strict
   checks close. The lead builds and seals the combined product from its exact
   source commit, including matching `sail` and `sail-z3` bytes/modes and the
   selected complete source-engine runtime. The candidate does not change the
   qualified c77 runtime manifest `54109b76e8bf79d92332194431e5b4dd83038b9f285994c319b5a2e3391c3e9d`.
   The original fixture's engine installer still verifies the supplied runtime,
   and its pinned Verus source checkout must match the installer commit.
   Freeze actual container birth, private environment and native installation
   ownership before execution; reject unintended engine/portfolio/trace selectors.
   No old reports, receipts or caches are copied into the new fixture workspace.

3. Bind the original fixture through the **separately labeled** timing diagnostic
   using immutable files. `ci/test-repair.py` remains SHA
   `9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7`.
   The timing launcher takes the sealed product path/SHA, engine build and pinned
   source checkout as explicit arguments, so it can select the combined product
   without changing the fixture. It calls the original `run` with timeout 120;
   original per-operation 30-second, MCP cleanup five-second, daemon cleanup
   ten-second and post-kill drain ten-second limits remain unchanged. Only the
   existing `SAIL_JOB_RPC_TIMINGS=1` observation switch is enabled. Original
   cleanup runs before bounded metadata retention and TemporaryDirectory cleanup
   on ordinary exit or Python exception; the wrapper always delegates the original
   TemporaryDirectory cleanup when its exit handler runs. Forced process death or
   VM failure can bypass those handlers. This does not certify orphan absence or
   add cancellation authority.

   Choose a wrapper whose exact schema matches the sealed product **before**
   launch. The db4 timing wrapper from `628f145` is SHA
   `3d4c4cb6237a55b4215e453131b158ee80f7e05b042a49f38abc838d1b932837`.
   A product incorporating C's `output_guarded` field needs the wrapper frozen in
   `92e3068`/`a8f2b1a`, SHA
   `bd6997ed223e0b1588e91637e80143ae2571bcbcd41811d97147f901350bee3a`.
   Both bind the same original fixture. Do not edit a running or historical
   wrapper; old diagnostic records retain their old schema and product binding.

4. Preserve full original outcomes, including selection-missing rejection,
   independent full evidence, uncertain declaration establishment, negative
   full-proof regression, MCP, late EXEC/E0499 and all original acceptance/cursor
   checks. The original single declaration-establishment follow-up stays inside
   its same whole-loop budget. No failure is waived or stitched with a later pass.
   Timing diagnostics provide no acceptance credit; keep the unchanged fixture's
   outcome distinct from retention completeness.

5. Compare actual retained `worker_publication` fields: `child_wait_us`,
   `child_exit_observed_us`, `document_read_us`, `freshness_establish_us` and
   `status_save_us`, plus managed setup result/receipt/compiler-family publication
   phases. Missing, overwritten or unfinished rows are unavailable, not zero;
   the original 256-row ring, 64-row setup files and retention bounds are not
   enlarged. The previous 120.137879-second diagnostic failed at missing
   foreground and overwrote its initial cold row; it remains failed.

The same-owner control proves unchanged-input owner preservation only. A real
source edit invalidates the previous continuous owner; metadata no-op alone
cannot avoid that cold fill or establish repair-edit improvement. Post-child
measurements and the complete original fixture must establish any benefit.
Completion remains synchronous, and independent freshness, restored-write,
alias, helper-loss, context/environment and cancellation behavior remain mandatory.
The outstanding original standard timeout, reader p95 failures and full workspace
HTTP deadline failure receive no credit from these component controls.
