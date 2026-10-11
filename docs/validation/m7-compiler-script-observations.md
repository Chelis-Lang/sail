# Managed compiler-family script observations

The reduced cdb6 real repair diagnostic passes its original success, established
followup and missing-selector controls in 77.046 seconds internally / 79.816
seconds externally. All three jobs use one compiler-artifact family and one input
revision. The selected and missing requests use different proof contexts.

Actual Cargo stderr reports 22.91 seconds for the first dependency build, 2.45
seconds for the established followup (only the selected project checked), and
17.68 seconds for the missing-selector request. The latter rebuilds proc-macro2,
quote, verus_syn, verus_prettyplease, syn, synstructure and builtin macros. Both
final local ledgers have four covered script records with identical per-package
content digests and declarations. The production source cleans every script
absent from the new context's ledger, despite sharing the compiler family.
This is an avoidable cleaning seam. It does not establish the whole cause of the
separate original 120.314-second functional timeout.

The change implements optional, consumed family generations of ordinary script
contracts. It does not share source observations or proof receipts. Its
[authority and fallback rules](../decisions/0036-managed-compiler-artifact-families.md)
include fresh output/declaration binding, package identity, effective environment,
post-watch recheck, all output units, failed-generation invalidation and a local
trusted-cache boundary. First-run observation paths remain available only as
uncovered roots when their generation is absent; they still force cleaning.
Missing reuse eligibility does not imply missing input coverage: the original
Current transaction uses freshly updated after-coverage and complete before/after
input guards. The script-coverage flag separately reports declaration status.

Linux controls exercise selector reuse, interrupted generation plus restored
input/mtime, actual post-change native observer acquisition, two-unit production
update, same-name distinct IDs, missing-generation establishment and next reuse,
changed content/output/declarations/registry/tool binding, injected environment
refusal, and malformed/oversized/symlink/FIFO refusal. Final Linux validation passes all 11 script controls (nine new plus two original)
in 0.10 seconds and all 13 existing runner controls in 0.57 seconds. Strict
all-target sail-verus Clippy passes in 14.139 seconds externally (13.65 seconds
reported by Cargo). The same private release target uses one compiler job, two
independent test threads and no incremental compilation. The final script test
invocation, including its package rebuild, takes 94.191 seconds externally.
The [portable record](m7-compiler-script-observations.json) binds source files,
the tested executable, commands and the private raw archive. It also retains the
initial visibility compilation failure and ten test-only strict lint failures.
Both corrections preserve authority and assertions.
No new project proof, source-runtime build, conformance or benchmark is implied.
