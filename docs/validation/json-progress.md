# Compiler-capture JSON progress qualification

| Field | State |
|---|---|
| Tracking | #726; existing owner-approved ADR 0064 |
| Classification | Differential oracle change; native gate, solver, E2E, fast and standard required; no runtime/solver/corpus-input change or performance claim |
| Exact reviewed implementation | `2c6cd8cc697345e85c9aeec0cb125e9190899013` |
| Focused native checks | EC2 c7i.4xlarge/us-east-2, Ubuntu 24.04, Rust 1.99.0: 65 library tests and all-target conformance clippy pass |
| Fresh independent review | Approve: 2,351 cases in both directions (4,702 production caller comparisons), all 20,000 original-slice reconstruction probes, all three original 143-function compiler-capture pairs |
| Broad gates | Native workspace, solver, complete E2E, PR, fast and standard pass at `077ba9a5`; current-main gate/PR pass at `688c20a1` |

The original Mac Anvil failure retains three stock controls and a Sail
cache-warm capture. Every selected compiler stream contains the same 45
complete JSON progress envelopes with identical bytes and counts, but a
different order. The original failed capture archive is
`d7c006dc9abfe9ee9a836b098f153cbd35cc6986c4883634e12934ac2e1e2f71`.
The committed fixtures preserve those original note bytes, without local paths.

Recognition requires the complete pinned diagnostic envelope, note level,
null code, empty spans and children, and exact standalone rendered progress
text. Duplicate/unknown fields, partial records and other note shapes remain
strict. The production caller compares original opaque gaps and exact
sequences of complete diagnostic runs separately. Probes, non-UTF-8 bytes,
changed/missing/repeated envelopes and crossings over other output refuse.
The global ADR 0042 diagnostic comparison remains unchanged.

Independent raw review SHA-256:
`f1e86813883821a19c2e06fb5b75bc371b4d36c110009a7149a6f4d0d308e8d6`.
Raw captures and review executions remain outside the repository. This
qualifies the bounded comparator, not a runtime, release or full corpus.
No CLI/MCP command, consumer skill instruction or setup behavior changes.

At exact `077ba9a5216bb36821716d24e1a38d3ce1da4940`, native EC2
c7i.4xlarge/us-east-2, Ubuntu 24.04, CI Rust 1.99.0: the workspace gate
passes 1,382 tests (53 skipped), solver 50, and complete E2E including extra.
Each of four PR arms reports 33 Same/311 functions/seven rlimit-drift units.
Each of four fast arms reports 147 Same/3,451 functions/25 drift units.
Each of four standard arms reports 159 Same/13,166 functions/32 drift units.
All have zero different, unstable, infrastructure or skipped-project results.
The complete standard mutation probe retains all 18 HumanEval mutants and
three original/revert controls, all Same. This is the legacy soundness probe,
not general campaign qualification or clean performance acceptance.

The merge onto main `33e6078d` as `688c20a1` preserves every conformance and
verification production source byte from that qualified candidate. Main adds
the separately qualified benchmark-only notice parser (#729). The current
workspace gate passes 1,384 tests (53 skipped); all four current PR arms again
report 33 Same/311 functions/seven drift units with zero bad/unstable/skipped
results. Original long-run evidence stays attributed to `077ba9a5`; it is not
relabelled as a new standard execution on the merge.

The earlier `a22ea2d0` pipeline is retained separately: three standard arms
have 159 Same, while its engine arm has 158 Same and one Unstable, with the
HumanEval probe Same. That source lacks the final lossless caller fix and
does not qualify this implementation. Original failures are preserved.

## Failed attempts

| Source/date | Result and cause |
|---|---|
| `bd403073`, 2026-11-12 | Native gate clippy refuses four empty-vector assertions; corrected without changing production acceptance. |
| `6aa084f5`, 2026-11-12 | Fresh review rejects: helper recognizes progress, but unconditional compiler stderr guard still reports failure on all three actual captured comparisons. Review `1e7f39a78164a2a4dbd6dd7c60730b23a8977c3a6510bf4b347de7d14bd5ed7e`. |
| `0835e36e`, 2026-11-12 | Fresh review rejects four diagnostic-crossing controls: a flat message vector loses placement between collapsed runs. Review `ef998f4da8d1fcc4b5d434c3574293e41cc46266f9af74546e4b7f638c96c7b2`. |
| `2cc32edf`, 2026-11-12 | Fresh review rejects five literal-marker collisions, including a full native stream. Typed original gaps replace synthetic-marker comparison in the caller. Review `cdaa1c6e924ea0bd1ae8d98f8e9d6a8b3d8c456f568f19cbc5a0841841c62d27`. |

VI-0044's trigger-summary source attachment remains a separate unresolved
strict failure and owner question. This change permits no attachment change.
