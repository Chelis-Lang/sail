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

Final corrected source bytes and actual controls are pending; this document does
not approve either draft for production activation.

Required sensitivity includes aborting a later generation, restoring external
bytes and mtime, and returning to both the old and a new proof context; changing
declared inputs during watcher setup; and two output contracts for one package
where the earlier contract becomes stale. Exact Current and receipt checks must
remain unchanged. No new compiler or proof actor was launched for this review.
