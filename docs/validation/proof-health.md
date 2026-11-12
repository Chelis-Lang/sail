# Recorded proof health qualification

| Field | State |
|---|---|
| Decision | ADR 0116; descriptive metadata, no solver, key, reuse or verdict change |
| Exact implementation | `6c0a18b8d0ee3a5aba966aa5aa809cabe4c55bab`, based on main `33e6078d` |
| Classification | Diagnostic tooling and additive report metadata; native gates, affected assembly proofs and focused interface controls |
| Native Linux | c7i.4xlarge/us-west-2, Ubuntu 24.04, CI Rust 1.99.0: gate 1,386 passed/53 skipped, solver 50, complete E2E including extra pass |
| Real acceptance | Actual managed verification compares every CLI/file/MCP health evidence field and saved historical baseline; passes |
| PR conformance | All four arms 33 Same/311 functions/seven rlimit-drift units; zero different, unstable, infrastructure or skipped-project results |
| Native Mac | EC2 Mac m4/us-east-1: strict host clippy, 1,373 tests/53 skipped, book source/build/links pass |
| Independent review | Exact `6c0a18b8` approved; raw review SHA-256 `071a6372f7414f90b826d03ad0f769cb05274d829dbef42e1c874c6736011f0c` |
| Acceptance models | All 35 actual stock-Verus model cases pass, including 31 broken controls |
| Acceptance refinements | All 316 cases pass: seven positive, 309 broken controls; 345 source bindings and 3,873 runtime files |
| Remaining | Final current-main gate and protected PR checks |

Health derives only from original per-check records and the existing strict
ADR 0009 threshold. Missing attribution differs from an explicitly empty
inventory; missing and unlimited measurements never become finite margins.
No stability, fragility or proof tier is inferred. Requested target identities,
original verdicts, provenance, establishing run/reuse, project exit and
currentness remain visible. Reference jobs keep their own recorded currentness;
immutable named baselines require Historical and their retained job origin.
No corpus input, solver transcript or benchmark baseline changes; no performance
result is claimed. Standard corpus is not required for this diagnostic-only
change under ADR 0087.

Independent review executes public MCP dispatch, file and framed-service CLI
controls, complete baseline/currentness boundaries and strict schema children.
It also executes the actual managed comparison assertions against 48 independent
leaf mutations each, extra evidence fields and a wrong wrapper summary. Only
the known MCP wrapper summary is checked separately; every evidence field is
compared. The lead's complete E2E run additionally executes those assertions
against real managed jobs and a saved immutable baseline.

The production report generator adds only health to eight golden functions.
Removing only those eight members restores every prior verdict, diagnostic,
check, model, timing and provenance exactly; an independent integer oracle
recomputes their health. All four resource copies agree. All three exact tool
registries add only `proof_health`; the real client checks its required identity,
bounded limit and exclusive reference fields. The new book chapter links the
authoritative guide. Optional Node highlighting was not run on the Mac.

At exact `6c0a18b8`, actual selected-body/component refinements pass with the
pinned stock archive and Rust 1.99.1; every positive exits zero and every broken
control exits one. Surrounding I/O, origin and service composition remain explicit
obligations. Raw proof/refinement, gate, solver, real E2E and PR captures are
retained outside git in archive SHA-256
`5ffc38ee73b26a51a6296f5df0b97c1a122958fa589cd448a0cd03af38b60708`.

The current-main rebase adds the independently qualified JSON comparator
(#731) and installed-shell test fixture (#732). Original qualification remains
attributed to `6c0a18b8`; health production, checked stencils and acceptance
checkers are unchanged. Three bound surrounding files differ: the independently
qualified conformance comparator/caller and the MCP test fixture. None is a
checked production stencil. Its final current-main gate is recorded separately.

The API assembly root advances to
`59f7668d93becc3c7cdf8677832307ce65b5714b562b0a52546f3798cded8e07`
only for the added health module and health-bearing report declaration. Fresh
review passes all 78 checker controls plus four altered-forwarder refusals.
The checked output/project stencils remain respectively
`a7e4a6475ac12f1e598778714017a70dd7f6d8ca7c802c1fcb0f1174b8f045ef`
and `1b368edc81a33567cbb3fcb9bdd6747d8fa15ee338ed80c24f1ea4fae20b733f`.
This establishes no health theorem or runtime selection.

## Failed and interrupted attempts

| Source/date | Result and cause |
|---|---|
| `c88630b2`, 2026-11-12 | Generic required-field fixture included the deliberately optional member; the existing assertion stays intact and new dedicated controls cover optional compatibility and every required health child. Initial filter omitted the external contract test; corrected explicitly. |
| `331e685a`, 2026-11-12 | Focused native controls pass; clippy refuses a long helper and nested conditional; split/collapse preserves inputs. |
| `368667b0`, 2026-11-12 | CLI clippy refuses two missing semicolons; corrected. |
| `5f7c9752`, 2026-11-12 | Fresh review rejects immutable-baseline freshness and incomplete human identity/provenance. Mac host has three stale tool-list/golden failures. Review `e33b8fa91debb8ef0cc09260a2214a6f8f87d554751a02d58d01eb8912d3acde`. |
| `0eeef9e8`, 2026-11-12 | Focused controls and repaired fixtures pass; external dispatch test needs a documented local lint allowance for a private cancellation type. |
| `2f262908`, 2026-11-12 | Review closes production findings but rejects managed comparison's unhandled MCP wrapper summary; corrected without removing evidence fields. Full gate separately refuses missing book-source symlink before Rust tests. Review `7d5991e0ad1bd5c5632316363715ba861b9d69f5363361d61afdfbacd2b17ffd`. |
| `68a91f55`, 2026-11-12 | Gate passes 1,386 Rust tests, then real-client tool contract refuses its stale list; exact registry and new health argument checks updated. |
| `6c0a18b8` review, 2026-11-12 | Two broad package attempts hit ETXTBSY in unchanged fixtures; isolated reruns pass. Separate current-main MCP reproduction establishes #730; twelve current-main no-reuse suites all pass, cause of that distinct observation remains unknown. These interrupted attempts are not broad-suite passes. |

Raw captures and all failed attempts remain outside git. Documentation changes
include the health guide, output schema, MCP reference, consumer skill,
README/book navigation, ADR, roadmap, changelog and affected refinement binding
record. Retained sessions, stability experiments, fragility graduation and
profiling remain separate gates. No publication is authorized by this record.
