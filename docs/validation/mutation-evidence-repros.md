# Mutation and rlimit reproduction preservation

| Field | State |
|---|---|
| Tracking | #712 package A, #664 minimal examples; PR #205 rlimit salvage |
| Sources | Nine byte-identical Rust inputs from `75c86cb8a546146632d973ef51f7050aee5a6b77`; four from PR #205 at `104ca5499318a49eb7f872af45fe837e10bdae57` |
| Classification | Dormant reproductions and diagnostic helper; no selected corpus tier, cache/reuse or production verdict change |
| Exact qualified helper | `88b7f8cca2c995c9dae9f050d739e9371adc6558` |
| Native Linux | c7i.4xlarge/us-east-2, Ubuntu 24.04, Rust 1.99.0: full gate 1,384 passed/53 skipped |
| Native Mac | EC2 Mac m4/us-east-1: host clippy and 1,371 tests pass/53 skipped |
| Independent review | Bounded approval `2e0e395dc965ac8bf7c5f72560a9ba98a10d715382e512a479b5a223e7c5767a` |
| Remaining | Current-main final gate and protected PR checks |

Both native platforms execute the exact helper on all nine minimal inputs:
three bases finish with exit zero, six edits with exit one. Original stdout,
stderr, command, source hash and elapsed time remain recorded; the helper
assigns no verification classification. Four historical rlimit inputs run
with `--rlimit 1` and a 15-second cap: delta times out, both kepler layouts
finish with resource-limit diagnostics, and the floating-point collection
finishes with six verified/zero errors on both platforms. This does not
reproduce its old platform difference on the current stock pin
`0.2026.10.18.7d82994`/Z3 4.16.0, or qualify long default-limit experiments.
Earlier three-repetition minimal runs stay attributed to their actual earlier
helper source; they are not relabelled as new executions.

Version probes and source attempts share group supervision and retained
initial/final records. Expected signals and deadlines kill the owned group;
source hash, command, actual parent exit and raw streams survive interruption.
Descendants remaining after normal parent completion are killed and recorded
as infrastructure error, preserving the parent exit. A nonterminal record
following uncatchable interruption remains incomplete. No process status or
exit alone supplies a proof, solver answer or campaign classification.

Independent native controls execute normal-parent-exit descendants for source
and version commands, SIGINT/SIGTERM, the full 30-second version cap, source
caps, startup failures, exact binary output, source hashes and distinct attempt
directories. All sixteen fake PIDs/eight groups are absent after cleanup; three
real stock controls finish with actual exits zero/one/one. All thirteen Rust
inputs compare byte for byte with their original commits. FPBench-derived
inputs retain the pinned copyright/permission notice and exact originating
benchmark links; only redundant ending blank lines of the notice are omitted.
External translation fidelity is not established by preserving its output.

Raw Linux archive SHA-256:
`be1bf4787166045c03408067ac12f46d2ae8aec04e01c156d3f5f1a27f1a211e`.
Raw Mac archive SHA-256:
`45b9f859026eb64c4243e209c32cf55704f3f2deee1c44e20340a45f2fddc9bf`.
Review raw inventory SHA-256:
`7498d0da59cbfe29363242c20b61da30a2e0ddf5d5eb70b9f9bf6e865f5c278b`.
All original successful, failed and interrupted attempts remain outside git.

## Failed attempts

| Source/date | Result and cause |
|---|---|
| `7740137d`, 2026-11-12 | Fresh native review rejects missing interrupted metadata, SIGTERM child abandonment, version-timeout child leakage and absent FPBench notice. Review `cfebcf61be6c7c3db491296c697c4d466b6b8930e20cc03f3e946aad1a93cab1`. |
| `5b79f432`, 2026-11-12 | Prior failures pass; fresh native review rejects descendants outliving a normally exited source/version parent. All leaked PIDs are cleaned and verified absent. Review `b9fe4b5b283427a397ff897767b913889697a06d395f261892d7717dfc357ae6`. |

Historical tables keep their original platform, pin and scope. Neutral-generated
minimal sources remain historical examples; no removed frontend returns.
No new user CLI/MCP command, installation flow or consumer skill instruction
changes. Documentation consists of the three reproduction guides, historical
records, this qualification record and index. The distinct IronKV collateral-
limit and focus/full-diagnostic reproductions remain separate package-A work.
