# Compiler-family script generation review

This is an independent source review of the in-progress ordinary build-script
sharing implementation, not a compiler run, proof, repair result or performance
qualification. The original repair timeouts remain failed. The compiler-family
key excludes only the focused function; proof contexts, runtime sidecars, source
observations and proof receipts retain their original full selector binding.

## Reviewed draft

The initial output-seal draft is based on `73c0370`. Its source digests are:

| File | SHA-256 |
| --- | --- |
| `crates/sail-verus/src/managed/build_inputs_family.rs` | `d9404a655d9e32449ca1384821a2071bcac688e47dbf891c62b3f25e67d99bc3` |
| `crates/sail-verus/src/managed/build_inputs.rs` | `cafad97cc1c29e9b914f1f56e33beb01139bba8dd37108a1d4306096665f824d` |
| `crates/sail-verus/src/managed/runner.rs` | `35ad44e4691964922cbc6f80eceda09ec1b90bd92797ed4b5f2f1cfcb0deb729` |

The draft consumes the bounded, versioned, checksummed family export before any
target cleaning or invocation, under the existing exclusive family lock. A
missing, malformed or stale export replaces even an older proof context's local
script ledger with an empty ledger. Failure to remove the export stops the
transaction. This prevents an interrupted later generation from leaving an old
declaration record beside changed generated output.

An imported record must match current package identity and manifest, a Cargo
script output file within the family target, the exact output digest and parsed
declaration contract, and independently read declared contents and environment.
Output-less legacy observations cannot authorize sharing. This is local compiler
artifact integrity within the existing managed-cache trust boundary; it is not
authentication against an actor trusted to modify Cargo's artifacts.

Optional family publication occurs only after the original report is Current,
verification exits successfully and guarded receipt publication succeeds. No
proof receipt or source-input observation is imported from another selector.

## Required corrections identified in review

1. **Watcher acquisition interval.** Initial declaration hashing precedes input
   observer acquisition. The runner subsequently replaces its initial revision
   with the monitored snapshot. A changed declared file could therefore establish
   new guarded bytes while old generated output remains reusable. Recheck the
   imported contents, effective environment and output/declaration contract after
   the original owners and Input barrier are live, before Cargo reuse. A mismatch
   must clean the affected script or refuse sharing. The original final barriers
   remain mandatory.
2. **Effective build-script environment.** Parent environment values do not
   attest Cargo-injected script variables. Pinned Cargo-Verus sets
   `RUSTC_BOOTSTRAP=1`; an explicit audited effective value can bind that key.
   Other injected declarations must be refused unless their actual values are
   independently established. This includes `OUT_DIR`, `HOST`, `TARGET`,
   `PROFILE`, `OPT_LEVEL`, `DEBUG`, `NUM_JOBS` and per-run Sail variables.
3. **Multiple script units.** The original update path retains only the last
   observation per package ID. Multiple Cargo units/output directories for one
   package can have different external declarations. Retain and validate complete
   per-output coverage, or conservatively decline sharing for such packages;
   one last record cannot authorize every cached output of that package.

The next reviewed draft implements all three corrections: live-owner recheck,
effective bootstrap binding with explicit invocation, and distinct output
contracts with whole-package refusal if any unit is invalid. Its file digests
are `27af110d5793def9df50cae3645821b811cca3b408be6f7a0fc6e9b61af0fc1e`
(`build_inputs.rs`),
`327d4f605f3416e067649172e7beadf9d029e206a5bd34703f504e7a3081e489`
(`build_inputs_family.rs`), and
`b77325aac006c1ccfc8ea13d2e11cdac566de81e8c2e6982265d9f87c687798f`
(`runner.rs`). The six new Linux controls are drafted but not executed at this
review point.

One further correction is required: late recheck computes lost package names by
set difference. If two distinct package IDs share a name, keeping one valid ID
can hide the other ID's invalidation and skip cleaning. Compare removed package
IDs first, then collect their clean names; an ambiguous Cargo package selector
must fail safely rather than authorize old output. A same-name, different-ID
control must cover the invalidation of only one package.

## Final static review

The corrected source closes the findings above:

| File | SHA-256 |
| --- | --- |
| `build_inputs_family.rs` | `bc9384861154398b54e557bed0960bb202a947de56024d17976cba23a695f125` |
| `build_inputs.rs` | `16809219e7094287fafb416f23e4b1980d3aa45d312fcd31f1f6c02f0d7db5fb` |
| `runner.rs` | `b77325aac006c1ccfc8ea13d2e11cdac566de81e8c2e6982265d9f87c687798f` |

Late cleanup now compares package IDs before collecting clean names. Every
distinct output contract survives the actual update path; invalidating one unit
refuses the package's complete export. Recheck remains after the original live
Input barrier and before Cargo invocation, under the same exclusive family lock.

Missing generations retain previous declarations solely as uncovered observation
paths: `covered=false`, `output=None`, and exact current metadata membership.
These records force cleaning and cannot authorize old output. Keeping their
paths lets the existing observers cover the fresh declaration-establishment run.
Removed dependencies are discarded rather than forcing an invalid Cargo clean.
Recheck retires these observation-only records after their paths have entered the
original inventory; fresh execution supplies new output-bound records.

The initial `script_coverage` flag affects the context-ready event and declaration
note, not `before.complete` or the final Current predicate. Final Current still
requires fresh `after_coverage`, original source coverage, equal revisions and
every original barrier. The drafted initialization control exercises absent
begin, forced clean observation, actual update, publication and a second begin
that admits complete observations.

Older supported CLI binaries cannot share this writer generation merely by
selecting the same engine: `managed_cli::config` selects the actual Sail
executable, and the compiler-family digest includes its independently read path
and contents along with the original tool identities. The seven-field borrowed
engine identity omits only the focused function.

Nine drafted Linux controls cover generation abort, watcher setup, multiple
output units, binding refusals, effective environment, malformed/nonregular
exports, same-name package IDs, initialization and dependency removal. They have
not executed at this review point. The static review has no remaining generation
or authority blocker; affected compilation, controls and strict checks remain
required. This does not establish product repair timing or proof acceptance.

Required sensitivity includes aborting a later generation, restoring external
bytes and mtime, and returning to both the old and a new proof context; changing
declared inputs during watcher setup; and two output contracts for one package
where the earlier contract becomes stale. Exact Current and receipt checks must
remain unchanged. No new compiler or proof actor was launched for this review.

## Final affected validation review

Commit `201d084` preserves `build_inputs.rs` and `runner.rs` above. The final
family helper is
`28b263c7f03281e3e4b29c37c962faf89cb525656eee1de3e441047c27512e90`;
managed-only function visibility and test-style corrections close the retained
initial compilation and strict failures without changing production eligibility.
The added ordinary `LANG` negative complements the effective bootstrap control.

The independent review rehashed the 13,440,057-byte archive
`05acea43c8e98614e9ad007c754ba308096b18df41d880177bafef922c60782e`
and its 17 retained files, then read the final logs: 11 script tests, 13 original
runner tests and strict all-target `sail-verus` Clippy passed. The archived
34,085,920-byte tested executable independently hashes to
`0fc1bc403e269f1428dfdd6b3d4169d48cb3c87a54396f638b705eacd2c6b1ef`.
The [implementation record](m7-compiler-script-observations.json) retains exact
commands, source and failed scopes.

This closes affected integration review. A combined gate, rebuilt product and
unchanged original repair workload remain separate requirements; no project
proof or timing acceptance was run by the reviewer.
