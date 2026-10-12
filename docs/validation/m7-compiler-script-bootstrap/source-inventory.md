# Compiler-family declared source inventory candidate

This implementation addresses [Sail issue 77](https://github.com/Chelis-Lang/sail/issues/77).
Affected native controls and strict checks pass within the failed `43d1c29`
combined gate. Complete gate/solver, a new installed product and the unchanged
full benchmark remain pending. No original Historical/Unknown refusal, deadline,
warmup allowance or receipt predicate changes.

The first three combined native gates remain failed: `2ba2a32` stopped at the
MCP argument helper's 104/100-line lint in 28.533 seconds; `3a3657f` stopped at
the baseline pruning/restart control's 106/100-line lint in 40.867 seconds;
`238a2d4` stopped at the extracted assertion's `Box<Baseline>`/`Baseline` type
error in 52.280 seconds. Each gate exited 101 before Rust tests or the solver
suite, with complete known-group closure and no uncertainty. The existing
[source inventory record](source-inventory.json) binds each complete result and
raw log. Root's helper extraction and payload-reference correction preserve
the original request encoding and full baseline equality assertion.

The later `43d1c29` gate passes strict checks, then runs 673 workspace tests in
236.369 seconds: 672 pass, one fails, and 41 solver tests are skipped. All four
new inventory controls pass, including actual Cargo compilation in 4.182 seconds;
the baseline wire/parser/dispatch, provisional/corrupt-record, unfinished-save,
and pruning/restart controls also pass. Compact package and feature-alias controls
pass. The sole failure is the existing CLI MCP handshake's exact tool inventory:
its expected twelve names omit the three authorized baseline tools. Root's
`e0bc26e` correction adds only those names, retaining the original order and
equality assertion. The whole gate remains failed (exit 100, 458.667 seconds);
solver and remaining gate steps were unattempted. Known groups close without
uncertainty. The fresh corrected full gate remains pending; individual control
passes do not supply whole-gate or installed-product acceptance.

The closed original `b89546c` benchmark failed before measured samples. Its second
GCD repair was Current, while the next concatenate selector was Historical in the
worker's own stdout, without declaration-transition notes. The retained
context-ready rows show source observations present for GCD and absent for
concatenate. Both terminal private ledgers have the same 551 declared paths,
including one generated `serde_core` include under their shared compiler family.
The ledger capture is 646,596 bytes, SHA-256
`30acca50d2eaada504bf78bb20be2dc32938bd0d89a1db21c89a322ba71b2635`.
Terminal ledgers are end-state evidence, not a historical kernel-owner capture.

The code establishes the missing inventory seam: a selector without a private
source ledger begins with no explicit generated input beneath its excluded
target. After Cargo supplies dep-info, the final snapshot adds that path and
can correctly change the revision to Historical. The fix moves only a previously
validated path list before that original first snapshot. It does not transfer
content freshness or a proof receipt. The three setup drivers remain separately
explained by exact-context verified dependency receipts; no cross-selector
receipt reuse is introduced.

The typed v2 envelope binds package identity and only admits canonical regular
package source files or generated includes under that consuming package's
validated script output directory. Unknown external roles and cache escapes use
the original fallback. It is consumed before cleaning/invocation. Import merges
earlier private paths and preserves incomplete coverage. All original source
hashes, native owners, input barriers, currentness classification and cancellation
remain unchanged. Independent Bootstrap publishes script observations with no
source inventory, so no extra reads enter its final-barrier publication interval.

Draft controls exercise actual bounded offline Cargo compilation with a generated
include, normal guarded publication and a second selector's original snapshot.
An otherwise intact envelope with only the inventory removed must still change
that revision when dep-info reveals the include. Restored bytes/mtime must still
fail the actual continuous guard. Additional controls cover package/role changes,
ambiguous IDs, generated aliases and target escapes, bounded counts, incomplete
or legacy attribution, corrupt local ledgers, preserved private roots, stale
ordinary contracts, checksum and v1 refusal. These source controls pass in the
failed combined scope above; original product and benchmark qualification remain separate.

The resource policy remains the existing 1 MiB encoded family envelope and
regular-file read cap, plus at most 128 package entries and 4,096 attributed paths
of at most 4,096 OS-string bytes each. These limits bound this new inventory,
not total managed allocation or RSS. See [ADR 0036](../../decisions/0036-managed-compiler-artifact-families.md).
