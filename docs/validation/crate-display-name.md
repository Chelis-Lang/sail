# Crate display names

Status: bounded independent review and combined native Linux qualification pass.
Current-main candidate remains subject to all required hosted checks.

Main-relative defect #746 is established on current main `0cbb2093`: the
attribution helper names a crate from its first friendly timing function.
Independent native baseline reproduction changes only row order and obtains
`alloc` versus `consumer_review`; every function record and verdict is unchanged.
Original failed broad assertion and baseline outputs remain in the 345-file
archive `a1e67ae750d48f3998a06da3839609f70b8daf4898d16cb9210ca761344fb4a5`,
independently lead-rehashed and externally retained.

ADR0141 prefers consistent compiler-path namespaces. If none are present,
legacy friendly namespaces must agree. Mixed, incomplete or conflicting evidence
leaves the optional name unavailable. Existing selector projection uses this
namespace, so actual selected reports can change: the shipped compiler's
`alloc*` control checks two methods, both incorrectly omitted by the old
report and retained by the corrected namespace. Unscoped function records,
diagnostics, query keys, attribution status and physical target/receipt
admission policy remain unchanged.

New source controls cover foreign-type implementations, all-foreign friendly
names, row reversal, incomplete/conflicting/empty/unqualified paths, empty
function sets and the consistent legacy fallback. The final successor is executed and independently checked as recorded below.
The initial rejected candidates remain failures.


| Attempt | Result / cause |
|---|---|
| c1a4bd75, 2026-11-14 | Native lint and 17 tests pass; independent probes reject three malformed path acceptances and expose selector coupling. Actual `alloc*` compiler results establish the desired two-row correction; original failures remain preserved. |

The successor validates namespaces and qualified components, includes explicit
whitespace/control/colon/invalid-root cases and updates the scope above. The intermediate successor is rejected below; its results are not promoted.


The 7112 candidate passes 17 focused tests but fails independent actual
Unicode/digit-leading crate controls: `démø_名` and `123review` verify, while
its namespace guard rejects their legitimate Verus `$~` spellings and drops
selected `good*` rows. The baseline retains one checked function; 7112 retains
zero. This local-candidate defect is fixed in this work, not filed as a main bug.
The successor follows the pinned compiler's generated-namespace grammar and
adds positive and malformed-escape controls. Native results for the final successor are recorded below.

## Final successor and combined qualification

Exact069ff3d0 passes native Clippy and18attribution tests. Fresh independent
review executes36actual compiler controls,2selector tests and2,176exact encoded
namespace forms. Malformed/missing/conflicting evidence refuses; Unicode and
digit-leading namespaces preserve legitimate selected rows. Equal-friendly tie
ordering remains unchanged. All5,006review originals are independently lead-
rehashed and externally retained, archive:
`9f5b38604e1ec88e9eb10dbe1b2c392f9f126014b1de89712dac0843982710b6`.
Earlier invalid compiler/fixture assertions remain retained and excluded.

The exact attribution files are then qualified in combined3952fc0b: workspace
1,423/58declared skips,54solver, complete local E2E/Extra/managed/repair, four PR,
four fast and four standard arms pass; all21owning mutation/revert samples are
Same with no false verified. Zero corpus differences/unstable/infra/skips.
Complete32,680-payload originals are independently lead-rehashed and externally
retained, archive:
`98b5694b8c602e6164efd481b8e7eddf9217f2d5547cda200bdcc2046896945a`.
See [combined record](compiler-owned-air-identities.md) for source/tool scope.
Current-main rebase preserves all executable and runtime pin bytes. No Mac,
performance, new runtime selection or release acceptance is claimed.
