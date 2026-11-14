# Kani executor integration

Status: tool-payload authenticator passes focused native checks, all15 original
tool controls (12 refusals/3 positives) and the full workspace gate at6e85f920.
The owned execution layout is implemented but uncompiled. Execution, CLI/backend selection and complete pilot qualification
remain planned. Tracks #712 G / ADR0152.

The Rust authenticator binds every original bundle/Rust file and executable bit;
it executes nothing. The canonical Rust commitment is derived from the same
complete original and independently installed maps accepted in ADR0149.

Pending: native compile/Clippy, original Kani and full Rust prefix positive,
modified/missing/extra/linked/special/unreadable controls, native platform checks,
source/lock guard review and full local gate. No check or baseline is waived.
The later executor still needs original process/bounds/output authentication,
source/tool/environment closure, bounded outcome attribution, process cleanup,
direct-arrayvec comparisons, conservative cache policy, CLI and Mac acceptance.

Independent lock-binding review approves the exact new lock only (ADR0152):
all78 controls,316 complete generated-driver/input-map pairs and39 focused
refusals pass; all934 raw review payloads are lead-rehashed and retained.
The contributor inspect_tools example performs original payload inspection and
reinspection only. Runtime authentication remains uncompiled/unqualified.

| Source / attempt | Actual result |
|---|---|
| 1662c2dd / native1 | CI image builds successfully; container setup refuses a target-volume mount under the read-only source because target/ was not created. No Rust compilation or authenticator execution occurs. Successor creates that mount point explicitly. |
| 4b0c9ae7 / native2 | Native all-target compilation refuses the new FIFO control's nonexistent rustix::fs::mkfifo convenience function. Successor uses the pinned API mkfifoat(CWD, path, mode); no test or bound is weakened. No passing focused suite or runtime authentication is claimed. |

The original tool15-control summary misrecords only the saved Rust mode after
reusing a harness variable for the later Kani file. Exact original summary,
script and read-only correction remain retained; actual restoration is0755,
all assertions use their correct scoped values, and final original authentication
passes. No result/assertion is changed. Complete raw closure is pending lead
verification; source, runtime and authority limits remain unchanged.

ADR0153 adds a private original-bundle staging candidate with one exact link to
the authenticated Rust prefix. Original-payload authentication stays strict.
Copy/route controls and native original-layout qualification remain pending;
there is still no harness executor or backend CLI.

| ff57045e / stage-native1 | All-target Clippy refuses the contributor example's rust/rest similar names. Successor renames the trailing argument binding; no admission or control changes. Stage native tests/real layout are not yet passed. |
