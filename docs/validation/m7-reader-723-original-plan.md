# Prepared original reader workflow on detailed product 7238da8

This is a launch plan, not an executed workflow. The preceding
[single proof diagnosis](m7-sysroot-proof-diagnostic.md) establishes only its own
new complete Current proof. It supplies no receipt, baseline job, digest owner or
prepared Cargo directory to this plan. Root reviews the current complete gate
and grants the concrete new freeze before any installation, setup or proof.

## Exact product, compiler and controller inputs

Use byte-identical, freshly service-owned copies of the diagnostic product:

| Input | SHA-256 |
|---|---|
| Sail, 74,664,464 bytes | `731f581c95c0eb6d436bb045f177a6deaf9a3298faebdb7dfdcba9dad8335289` |
| Shim, 11,000,912 bytes | `ddfa36ab78d36d8e7fbfaf7740f19bb7c63a6bab9fa36f39a3e1bf82f3049636` |
| Product source archive, commit `7238da8406cd3a5d62b8c378949d36832446aed8` | `2c30f98ffdf5aae5881014801ac892cd1327516e17cdcb4e791650debf427096` |
| Installation-only original grant | `aa83d7fafd8aa8974813e9de2c1651c09b0ae4d832ec046e7e365c3aa389bbc3` |
| Selected b346 runtime manifest | `4b6f48d278e981f9f3ce40b5e8238ffed2409cac61f003c09fd8ee0c2c9a7c41` |
| Unchanged Python collector | `3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720` |
| Unchanged Rust collector binary, frozen SDK `02fd1fc` | `f1632772daacc28e4bb5e7f7747e0e048fce7973aeff31b6ed65c5bbbd0e0b4d` |
| Unchanged Rust collector source | `dcfba2d98259c04dd76327196331aeee478656aa81469fd4c09c04af2699eebe` |

The selected runtime is the original pinned Verus commit's
`bfc6b239222f190b` variant. Recheck all thirteen actual artifact hashes from its
manifest; no unselected source candidate or changed manifest is substituted.
The new copies use the reviewed [installer](../native-product-setup.md), a new
private destination and modes `0700`. Neither the installed diagnostic directory
nor any old binary is modified. Record exact original grant, installer hash,
new file metadata and byte hashes before and after copying. The new destination
gets a `freeze.json` containing the installer-returned `files` plus original
product/grant/source provenance. That metadata permits the original controller's
unchanged `Run.freeze`; it neither grants Current nor copies any proof authority.

Use the existing frozen `2d6539f` controller bundle, whose files have no Git diff
against `7238da8`:

| Imported script | SHA-256 |
|---|---|
| `m7-reader-acceptance.py` | `84966d77cd679653b96ab9f05019d51f805538f3d4dd509c1591da1679b7634d` |
| `m7-repair-bench.py` | `3931f922af8da8317a857955915754e1d43e7c905b6171f381d0b6096352de33` |
| `m7-retained-jobs-acceptance.py` | `ba1acf98c39dce9687a4a0e277234296459651dc1580316576c654ceb2154028` |
| `m7-job-read-bench.py` | `3d5dfa5d47bbf7377c2d6c79c8ae9618eae6c4a080d66d31bb858fbb8e611720` |
| `m7-native-product.py` | `f2d52accdb735a14de0e8ae09f6297271391f652db45622320a133fed23f0ff0` |

Git diff of `crates/sail-api/src/jobs.rs` and its modules between the frozen
`02fd1fc` probe and product `7238da8` is empty. Keep that exact probe binary and
source unchanged; no explicit cache opt-in or altered collector is introduced.
The unchanged repair catalogue is 5,510 bytes with SHA-256
`6ffc129735220bf53511ac6bba818c5c3c7b61bca92d59c17292a807979b18d6`;
record its identity even though this reader workflow does not apply edits.
The corrected retained harness keeps five seconds for each full MCP exchange,
30 seconds for each CLI call and 120 seconds for its whole workflow.

## New independent inputs and namespace

After grant, create a new source-only template from the closed original active
HumanEval workspace using the original `copy_source` helper and its actual Cargo
target exclusion. Copy only the 273 exact original template files plus its
existing 3,007-byte generated `Cargo.lock`, SHA-256
`a38eff2ab6773bcb3f8a8b32068b6b9687cae6d168b3a20f8002578649245525`.
Verify that exact source inventory before and after copying. The historical
lockfile hash was not retained; the new actual original Guard binds it freshly.
No old target, cache, receipt, report or leased digest is copied. The controller
then creates its own independent baseline and active source copies.

Concrete new basenames are `sail-m7-reader-owned-7238da8` for the product,
`sail-m7-reader-template-7238da8` for the source-only template and
`sail-m7-reader-original-7238da8` for the workflow output. The output owns its
`cache` and `cache.sock`; exact absolute native paths belong only in private
launch metadata. Every path must be absent before creation. Use a new native
output, cache, socket and product destination, with no existing path replacement. Private per-context Cargo output is allowed; the common Cargo
target and all other actors stay untouched. Preserve stock Cargo-Verus and Z3
hashes from the diagnostic seals, Rust 1.98.1, original managed force and the exact
selection `--package human_eval_verus --lib`, function
`human_eval_verus::human_eval_013::compute_gcd`. Do not change compiler or solver
flags, remove vstd reports or add tracing to this measured workflow. Setup timer
output remains outside the verification inputs; it does not authorize reuse.

## Original sequence and failure rules

Reuse the original external owned wrapper's sequence: retain the original
`Run` object and its actual `Popen` daemon handle through reader completion,
retained-harness invocation and final shutdown. The next wrapper changes only
exact installation/source/output identities and records its new source hash;
root reviews that concrete freeze before launch. It does not edit either helper
or collector. This prevents a successful kept service becoming an unowned
detached actor. Its outer retained-harness safety ceiling can include cleanup;
the harness's own original 120-second workflow, 30-second CLI and five-second
MCP clocks remain authoritative and unchanged.

Invoke the unmodified reader controller with `--prepare-contexts
--setup-timeout 600 --keep-daemon`, the exact product/runtime/probe/template,
package/target `human_eval_verus`, kind `lib` and function above. Record Linux
amd64 Docker emulation and observed concurrent work without assuming its cause.

1. The approved preparation flag runs separately recorded unmeasured ordinary
   full verification and exact repair-context preparation in each new workspace.
   All full reports, initial failures, job IDs, timings and total costs remain
   retained. Only the original controller's existing declaration-establishment
   transition may submit its one retry; no new retry is added. Preparation uses
   the explicit existing setup-zero seam; it is not a performance sample or a
   change to the subsequent 600-second checks.
2. Original bounded ordinary and exact repair checks follow in both workspaces,
   each under the unchanged 600-second limit. A genuine successful, independently
   complete, publicly Current accepted baseline is necessary. Ordinary incremental
   reuse is legitimate only from proofs just established in this same new run.
   Raw output cannot replace public Current admission.
3. Start the independently active exact selected-target repair and bind its
   worker PID/start time and source scope. Run the unchanged Python collector
   first, then the unchanged Rust collector. Each keeps four readers, 200 full
   status/result samples, five-second RPC bounds, a 600-second collector-process
   bound and both p95 values strictly below 200 ms. The same live non-stopped,
   non-zombie verifier with the exact selected crate must span each timed RPC.
   Complete reports and all failed samples are retained. No model, function or
   dependency data is dropped.
4. The first preparation, check or collector failure stops this attempt. No
   second census, proof retry or retained workflow is used to erase it. The
   original helper archives exact owned jobs and cancels only its independent
   active job; it retains closure, raw streams, setup timers and before/after
   seals. A Python failure leaves the Rust census unattempted, explicitly recorded.
5. Only if both censuses pass, retain the successful owned baseline/service using
   `--keep-daemon`. Then run the corrected original real retained CLI/MCP harness
   once, with the exact baseline job, workspace, revision, stock environment and
   source manifest: original whole 120 seconds, CLI 30 seconds and MCP five seconds.
   Preserve all responses/streams and stop on the first failure. Shut down only
   the owned service after the retained harness finishes, with normal/forced
   cleanup and immutable-input checks recorded. There are no endpoint reads or
   new product jobs from other diagnostics during either census.

## Scope

This would test the original reader and retained functionality of product
`7238da8` with the selected b346 baseline. It is not selection or qualification of
a newer source engine, a 512 MiB source physical-budget gate, the 2× edit-loop
benchmark, current full-corpus conformance or M7 overall. The earlier failed D20
workflow, every failed collector and the completed one-proof diagnostic remain
separate. No new proof or setup is authorized by this document itself.

See [reader acceptance](../reader-acceptance.md),
[freshness](../freshness.md), [the repair guide](../proof-repair.md),
[the roadmap](../ROADMAP.md) and
[the preserved diagnosis](m7-sysroot-proof-diagnostic.json).
