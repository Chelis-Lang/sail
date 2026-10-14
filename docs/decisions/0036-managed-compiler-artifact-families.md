# 0036: Managed compiler artifact families

- **Status:** implemented; its guarded build-script bootstrap is superseded by 0041, and its per-repair-function targets by the L23 amendment below
- **Date:** 2026-10-11

## Context

Before compiler families, the managed runner placed Cargo's target directory
beneath the complete proof context. That context binds the source engine request,
including the chosen function. A different focus selector therefore chose a
different dependency build tree, even with identical compiler, engine, solver,
configuration and arguments. The rebuilt source-engine repair fixture exhausted its original total
budget; a separate retained diagnostic exhausted the same budget in dependency
compilation before submitting a missing selector. These observations establish
avoidable target separation in the code, not the cause or cost of each timeout.

## Decision

Keep the existing proof-context serialization and digest unchanged. Add a
separate, domain-tagged compiler-artifact digest over the same exact arguments,
engine request, runtime identity and byte-exact environment, clearing only the
engine request's function field. Keep stock and source-engine requests distinct.
Compiler artifacts and executable aliases live in this family. Runtime sidecars,
content observations, proof receipts, committed reports and runs stay under the
complete proof context. Local build-script observations also remain there; an
optional family export certifies ordinary generated-output contracts for reuse,
never proof receipts or Currentness.

Repair requests with an explicit function now use a versioned target generation
inside the compiler family, keyed by their unchanged complete proof context.
Their ordinary generation export uses the same private generation directory.
Local build/source observations use a matching versioned namespace within the
proof context. Legacy ledgers can contain generated paths in the former shared
target, so they are retained as historical files rather than merged into the new
repair generation. The fixed input-state file records the observation layout at
both initial and final metadata publication; independent freshness refresh and
its native inventory use that same namespace. Unsupported layout versions refuse
refresh. Proof keys, receipts, runtime sidecars and run directories remain unchanged.
The family executable aliases and nonblocking ownership lock remain shared.
Stock calls and source-engine full calls without a function retain the original
shared target and export. No old shared artifact or another selector's proof
receipt is copied into a repair generation.

This separation addresses a concrete collision in the original `387ba80`
benchmark: three ready repair selectors had vstd receipts with the same input
revision, but different artifact hashes at the same shared paths. A later
selector's full verification correctly invalidated the earlier selector's
byte-bound receipt. The first measured repair then required three drivers and
failed the unchanged one-driver check. Dependency preflight alone could reduce
a repeat invocation but would still need a dependency driver; it cannot satisfy
that check. See the [isolation candidate and controls](../validation/m7.md).

Each repair selector therefore incurs its own cold dependency compilation and
uses additional disk space. After its own declaration/receipt establishment,
other selectors cannot overwrite its certified dependency bytes. Actual
one-driver reuse, original repair deadlines and real-project speed remain
validation requirements, not consequences inferred from this layout.

The version-2 export may also carry a typed compiler-declared source **path
inventory**, attributed to exact Cargo package identities. It is published only
by the original final Current transaction; the independent script bootstrap does
not certify newly learned Rust source paths. Before a new selector's original
snapshot, `family::begin` consumes and revalidates the export, then merges an
admitted inventory into that selector's private source ledger. Earlier private
paths and incomplete coverage are retained. Every imported file is read and
guarded anew by the original snapshot, namespace, kernel and final input checks;
the export contains no reusable source content observation, Current verdict or
proof receipt. This addresses [Sail issue 77](https://github.com/Chelis-Lang/sail/issues/77):
a new selector can otherwise learn an unchanged generated include only after
verification and correctly become Historical.

The inventory binds package ID, name, version, source, manifest and targets.
Names must be absolute canonical regular-file paths within that package's source
directory, or within its own already validated build-script output unit's `out`
directory under the exact family target. Other cache paths, external unknown
roles, aliases, missing files, ambiguous package identities and incomplete or
unattributed inventories refuse sharing. Refused source inventories keep the
original fresh-cleaning path; absent optional inventories retain the existing
ordinary-script-only behavior. Newer source discoveries can still require the
original declaration transition. No filesystem glob or another context's raw
ledger supplies authority.

The existing 1 MiB envelope/read bound also covers the optional inventory. It
admits at most 128 package entries and 4,096 attributed paths, each at most 4,096
OS-string bytes. Local merge uses the same bounds and refuses corrupt ledgers.
These are bounded new serialization/inventory limits, not an allocator or RSS
quota. Version-1 envelopes are refused rather than promoted. Checksums and
compilation records retain ADR 0036's trusted private Cargo-artifact boundary;
they do not authenticate fabricated journals against a trusted local writer.

The optional daemon sysroot hint selection also uses a distinct versioned domain
and the same borrowed function-independent request view. This changes selection
only. Each caller still creates its own namespace/kernel Baseline before import,
checks exact scope and content identities, validates at the original barriers,
and falls back to original local hashing on refusal or uncertainty. No caller
digest owner or proof context is shared. The 256 KiB streamed selection bound
remains unchanged; request buffers are borrowed rather than cloned.

A nonblocking family ownership lock accompanies the existing proof-context lock
through package cleaning, Cargo invocation, artifact validation and receipt
publication. Acquisition always proceeds from proof context to compiler family.
A busy family returns the existing SAIL3104 error. No process waits for another
family owner or extends a verification deadline.

Ordinary Cargo freshness controls dependency reuse. Every selected repair
package is still forced fresh. A skipped verification artifact without an intact
receipt for the exact proof context follows the existing clean-and-reverify path.
An old receipt for another selector cannot establish Current. No compiler, solver,
source engine, selected package, proof result or freshness guard is substituted.

## Consequences

Ordinary and full-engine calls retain compiler-family artifact sharing. Explicit
repair selectors retain their own artifact generations; changing the selector
requires cold setup for that generation. The family lock still serializes
contexts that share the family. Other request fields, arguments,
runtime bytes and environment changes isolate families. Source and Cargo
configuration changes retain their existing content and Cargo freshness guards.

A compiler family can reuse an ordinary script contract only after independently
rehashing all declared files and environment, matching the exact package ID and
manifest in current metadata, and checking Cargo's actual `output` file bytes and
parsed declarations under that family's target. The export binds the compiler
family and a checksum, and is limited to a 1 MiB encoded envelope. Linux reads
use nonblocking, no-follow regular-file opens; unsupported platforms, malformed,
missing, incomplete, changed or oversized records take fresh script cleaning.
These bounds cover the new envelope and each output file read, not total managed
memory or RSS. Existing local Cargo observation loaders retain their original
loading behavior.

The exclusive owner consumes the export before any cleaning or invocation.
Interrupted, infrastructure-failed, incomplete and non-Current runs cannot leave
an earlier generation certifying rewritten output through the original proof-bound publication path. An absent generation forces cleaning even when an
older private context has observations. Its old declaration paths remain only
as uncovered watch roots, so the original declaration-establishment transition
can acquire the necessary inventory; they grant no output reuse. A fresh export
is atomically published only after the original final Current transaction.

Successful runs retain the existing publication rule. A source-engine run with
ordinary proof errors may also retain fresh ordinary contracts when its input
declarations were already covered at entry, Cargo passthrough arguments contain
no partial verification-scope filters, and
every selected target has exactly one observed completed full compiler outcome.
Every recorded compiler start must have a corresponding completion, including
dependencies. The negative branch requires Cargo exit 101, normal driver exits
0 or 1, an exact sealed full terminal and whole-crate JSON, explicit absence of
VIR errors, matching report counts and success fields, and at least one ordinary
proof error. A source-engine focus selection may accompany this complete full
run; the full document, not the focus result, establishes completion. Generic exit 101, partial scope, cancellation, missing output,
duplicate starts, declaration establishment and uncertain input coverage refuse
this branch. The original guarded receipt commit must still finish Current.

This is a completed negative compiler outcome, not successful verification.
`TargetResult.complete`, invocation success, proof receipts and function verdicts
keep their existing meanings. Per-function attribution uncertainty does not
change the explicit terminal whole-crate proof error; it grants no proof reuse.
The exported records independently revalidate each ordinary script contract and
every output unit at publication and before the next mutation. A repair still
reverifies its selected crate and cannot borrow a negative proof receipt.

Amendment (2026-10-13, rescue L19): stock runs take the same negative branch.
Without it, a stock run with proof errors consumed the generation, and the next
run cleaned every build-script package, which re-verified vstd: about 27 s on
HumanEval and 26 s on Vest, where plain `cargo verus verify` keeps vstd fresh.
A stock driver has no sealed terminal. Its completion evidence is this run's
recorded completion of the same invocation, a normal driver exit 0 or 1, and
one whole `--output-json` document that agrees with that exit. Every other
condition above still applies. Stock Verus can fold a caught internal panic into
the same exit-1 shape. That does not matter here: the branch retains only
ordinary dependency script contracts, which are revalidated independently, and
it grants no proof. Measured by L19 (rescue lane report, tag `archive/rescue-m7-98f6292`).

A second full contract check runs after the original native owners and monitored
snapshot are live, before Cargo can reuse output. Any newly invalid package is
cleaned. This closes a restored-mtime change between preliminary checks and
watch acquisition. Every distinct output unit for a package is retained in local
observations and checked; any invalid unit disqualifies the entire package. A
valid package version cannot hide another invalid version with the same name.

Cargo-injected or proof-specific declared environment variables conservatively
refuse sharing. The one audited exception is `RUSTC_BOOTSTRAP=1`, set by the
pinned Cargo-Verus and explicitly bound by the managed child command; old
parent-value observations become stale. No process environment is mutated.
Sharing also refuses Cargo's dynamic-library path variables, resolved `RUSTDOC`
and `RUSTC_LINKER`, removed `RUSTFLAGS`, and `DEP_` metadata: their child values
cannot be certified from the parent environment. The bootstrap and ordinary
publication paths use the same refusal policy. All other actual environment and
tool changes retain family isolation. The
compiler identity includes the actual Sail executable bytes/path; an older
supported CLI cannot write into a newer binary's family.

These observations have the same local trusted-artifact boundary as existing
managed Cargo artifacts and proof receipts. The checksum detects accidental
corruption; it is not authentication. A writer able to fabricate both private
metadata and arbitrary Cargo output is outside this boundary. Generated OUT_DIR
artifacts retain Cargo's original integrity/freshness rules, with exclusive
Sail mutation and export invalidation. No historical path list alone certifies
current output, no proof observations are transferred, and first-run incomplete
declarations remain incomplete.

The [retained diagnostic](../validation/m7.md)
shows a repeated dependency build in the same family after selector change. It
does not prove the cause of the earlier whole-loop timeout. Product and unchanged
benchmark validation remain required; no fixture or acceptance bound changes.

## Unchanged declaration metadata publication candidate

A source-only Linux candidate compares the exact original serialization before
replacing local declaration ledgers and context runtime links. Only a complete
regular-file read with private mode, single link and unchanged named/opened
identity can avoid the original atomic rename. Every uncertainty uses that
original publication. This is a write avoidance mechanism, not digest reuse or
Current authority: existing continuous native owners and all final barriers
remain mandatory. Family export consumption/publication, receipts and per-run
configuration retain their original transaction. See the
[uncompiled candidate and pending controls](../validation/m7.md).


## Independently guarded declaration bootstrap candidate

The original Current-only publication can fail to establish an ordinary script
export when cleaning recreates an explicitly tracked generated Rust include.
The proof namespace refusal remains correct. A source-only Linux candidate adds
an independent package/configuration/runtime input owner before cleaning for
missing or rejected generations. It supports only freshly compiled, single-unit
ordinary scripts with no build dependencies and pre-acquired regular-file
compilation/execution declarations. Runtime, output lineage and effective
injected environment are independently checked; final exact output contracts
are reread before the final native input barriers. Within the existing trusted
Cargo-artifact boundary, this owner may publish ordinary contracts after a
normal exit-zero invocation while the proof stays Unknown. It never publishes
proof authority or weakens the original Guard.

Empty or wholly/partially rejected generations do not suppress this optional
bootstrap. No supported metadata script means no optional acquisition or runtime
hashing. All other refusals keep original script cleaning. The implementation is
uncompiled and unselected; requested retained admission and cooperative streaming
bounds do not claim an enforced construction/RSS limit or measured speedup.
See the [candidate, limits and required real-Cargo controls](../validation/m7.md).

## Disabled metadata scripts and retry compilation evidence

Cargo metadata includes dependencies behind the exact always-false target
predicate `cfg(any())`. An unobserved non-workspace, non-resolve-root script can
be omitted from required coverage only when its unique resolve node has an
incoming edge and every incoming dependency kind has that exact predicate.
Workspace/root packages, explicitly selected packages, malformed graphs,
duplicate node identities, ambiguous selectors, empty edge kinds and alternative
predicates retain original cleaning and
incomplete coverage. This scheduling fact does not normalize SMT queries,
remove input roots, synthesize observations, or change proof authority.

A missing-target retry replaces the artifact journal. When the existing optional
independent bootstrap owner is held, retry cleaning also includes exact package
IDs for supported scripts. Cargo must compile them again under that same owner.
Final-journal `fresh=false`, exact binary/dep-info/output roles, effective
environment, independent input barriers and the original proof Guard remain
mandatory. First-pass journals are not merged; `fresh=true` remains refused.
Without the optional owner, retry cleaning retains its original behavior.

The follow-up real Cargo controls cover disabled dependency scheduling,
first/reused/rebuilt journals and subsequent original input-barrier readiness.
They remain uncompiled and unexecuted at source freeze. Product and unchanged
benchmark recovery are pending; see the
[source record](../validation/m7.md).

## Amendment (2026-10-13, rescue L23): one Cargo target per compiler family

This supersedes the per-repair-function target generations, the separate
`repair-observations-v1` namespace and the two-lock ownership above.

**Why.** With a target per repair function, the first `repair-check` of each
new function verified vstd again. On a native c7i.8xlarge, a function's first
repair took 40.7 s and 38.2 s on HumanEval (6.4 s and 6.5 s for its second),
and 61.4 s and 61.9 s on Vest (29.4 s and 29.8 s). An agent repairing *n*
functions paid about *n* × 32 s. The separation was introduced because the
VI-0005 fork fix bound forwarded Verus arguments, including the repair
function, into each crate's dep-info, so one function's run rewrote another's
vstd. That fork fix was dropped (ADR 0039, VI-0005); on the current pin vstd's
dep-info declares only `VERUSDOC`, and the function no longer reaches any
Cargo fingerprint. L23 checked this: the second function's repair found vstd
fresh in the shared target.

**Decision.**

- **One target per workspace and compiler identity.** Every proof context of
  a compiler family builds in `compiler-artifacts/<family>/target`, as plain
  `cargo verus` shares one target directory. That covers every repair
  function, and full engine runs whose arguments are otherwise identical.
  Cargo's own fingerprints decide what is rebuilt, as in plain Cargo (rule 1).
  Arguments that change results, such as `--rlimit`, are in the family digest
  and therefore get their own target.
- **One owner.** The family lock serialises every use of the target. A proof
  context determines its family, so the same lock also serialises each
  context's receipts and runs; the separate context lock is removed.
- **Receipts stay per proof context**, with one addition. A dependency (a
  target the run did not select) that Cargo reports fresh may be accepted on
  a receipt from another proof context of the same family. That receipt must
  pass every existing check, including its own context's commit marker, the
  input revision, the declared environment and the current bytes of every
  artifact. A dependency's full verification does not depend on which
  function a repair selects. The family records its contexts as empty files
  under `compiler-artifacts/<family>/contexts/`; only those are consulted.
  The reused target keeps the establishing run's id as provenance (rule 7).
- **Selected targets** still need a receipt of their own proof context.
  Repairs force them fresh, as before.
- Build-script observations and the family export keep their existing rules;
  the observations live directly in the proof context again.

**Consequences.**

- A function's first repair takes about as long as its second: HumanEval
  8.9 s and 6.5 s (after one engine verify of the same family), Vest 29.6 s
  and 29.8 s. Each first repair used one driver and was Current.
- One target per family uses less disk than one per function.
- A repair rebuilds the selected crate in the shared target. A later
  unfunctioned engine verify of the same family, with unchanged source,
  therefore verifies that crate again instead of reusing its receipt
  (HumanEval 6.2 s against 0.6 s, Vest 27.5 s against 1.8 s). After an edit
  it would verify the crate in either layout.
- If a repair function ever reaches a dependency's build again, Cargo rebuilds
  the dependency, its new bytes invalidate every older receipt, and the cost
  is a re-verification, never a wrong reuse.
- Existing `repair-targets-v1` and `repair-observations-v1` directories are
  no longer read; deleting `projects/` reclaims them
  ([workspace hygiene](../workspace-hygiene.md)).

Measured by L23 (rescue lane report, tag `archive/rescue-m7-98f6292`).
