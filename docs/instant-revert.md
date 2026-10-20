# Instant revert

Status: automatic selection is implemented and qualified at `83128ad0` for the
narrow Linux/public dependency profile below. PR #224 merged as `3892881a`,
with all required checks and the main workflow passing. Native ordinary and genuinely bundled Vest A → B → A controls pass,
along with the complete default gates. See the
[validation record](validation/instant-revert.md).

No extra workflow command is needed: `sail verify` can restore an
already verified workspace revision after an A → B → A edit. `SAIL_MANAGED_FORCE=1 sail verify`
continues to require fresh verification. Restored results keep the original
establishing run, while the current invocation separately checks currentness.
An unsupported context runs the genuine compiler instead.

Automatic eligibility is restricted to x86-64 Linux and the exact qualified
normal runtime manifest. Every invocation uses the existing complete runtime
validator on fresh file bytes before admitting a plan. Source, features, native
compiler-read certificates and complete private dependency graphs must also
satisfy a complete closed policy. A cheap metadata prefilter routes only the
public alloc/std dependency profile used by current-pin Vest to full admission. It uses the same authority
selector as full admission and grants no authority by itself. The public default
feature profile used by a new library and the 718-file path-provider profile
remain available only through explicit retained-content research. Public profiles
bind all 15 archive identities, 662 file/checksum entries and exact
feature/dependency vectors. This is a dependency-profile restriction, not an
estimate of every target's proof cost; another small library can share that
profile. Other Cargo resolutions remain fresh. macOS, other runtimes,
unsupported macro/generated inputs, foreign imports and unsupported
source namespaces remain fresh. Loader, engine and focused contexts keep their
existing refusal rules.

Retention also has to pay for itself. Its checks read the target's whole
private dependency graph (vstd and the macro crates) before and after the
compiler runs, about 1.5 s on the current runtime. Each verifying compile
records, beside the proof context's receipts (`retention-costs/`), how long
the compiler and, when engaged, retention took. A run prepares no retention
for its selected targets when every one of them last compiled in less than
four times its measured retention, so a one-function crate compiles normally
on every edit and revert, while a Vest-class target (about 26 s against
1.5 s) keeps instant revert. A target without a record, or with an unreadable
one, keeps retention; a skipped plan compiles normally and changes no verdict,
receipt or check. The records authorize nothing.

The explicit vstd-boundary experiment suppresses automatic retention. Explicitly
requesting both research modes still refuses with `SAIL3135`. The runner passes
an admitted plan through the existing managed runtime file; it does not add a
public switch to the compiler child's environment. Archived declared-variable
checks use the fresh outer caller snapshot, including variable absence.

The pending release-integration candidate extends ordinary routing to the exact
closed public default-feature profile and allows requested-target retention
alongside an admitted adjacent vstd proof. It independently validates shared
native observations and keeps a distinct release witness for served vstd;
requested targets still require their own compiler-read evidence. This extension
is not yet qualified or selected. Its bundled and unbundled A → B → A controls,
late-authority mutation checks and existing complete Vest regression must pass
on the final matching runtime. See the [candidate decision](decisions/0048-shipped-dependency-build.md#candidate-requested-target-coexistence)
and [qualification record](validation/vstd-compile-boundary.md).

The durable retained arena admits at most 8 GiB and 65,536 entries, with at most
four archived revisions and 1 GiB of outputs per target. Fullness takes the fresh
compiler path and preserves unknown live graph references. These are not global
bounds on all managed-run sidecars, project targets or logs. Whole-project
storage growth and scan costs are included in the native cycle measurements below.

Three rotated native rounds at the final source compare a complete four-call
Vest cycle: cold A, changed B, returned A and unchanged A. Candidate ordinary
Sail takes 113.08 s versus the earlier Sail build's 130.66 s (13.45% less) and
stock's 148.10 s. Returning to A falls from 26.54 s to 2.51 s. Cold, changed-B
and unchanged costs rise; final cache size grows from about 560 MB to 1,080 MB.
The small control's cycle is essentially unchanged against earlier Sail and
remains slower than stock. This does not meet the overall 25% M8 gate.

All 72 fresh rows, their exact function/diagnostic comparisons and complete
storage inventory pass. Timed Sail actors share an explicit matching runtime
override; a separate genuinely bundled, ordinary CLI control passes without
runtime or research overrides. The earlier 72-call cohort remains failed and
is indexed in the validation record; its retrospective comparisons are not
substituted for this fresh qualification.

A separate unchanged-run optimization has passed a 36-call native screen: it
omits optional archive preparation only when existing receipt headers cover all
requested current revisions. Unchanged Vest falls from 0.895 s to 0.544 s
(39.29%); the complete four-call sum improves 0.73%. It preserves ordinary
verification and Current checks and adds no workflow switch. Mac host and native
gate, solver, all end-to-end phases plus extra, PR conformance and full FAST
and STANDARD checks pass in all four arms, with zero verdict or diagnostic
differences. The optimization is qualified for the existing default profile;
Mac performance and general CPU savings remain unclaimed.
The paired validation record retains all ranges, cold/edit/revert costs,
footprint, failed attempts and the Linux evidence export limitation.
