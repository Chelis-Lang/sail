# Ledger half-limit arithmetic

| Field | Current state |
|---|---|
| Tracking | #719 |
| Change | Replace saturating doubling with the equivalent overflow-free integer half comparison |
| Scope | `Effort::near_limit`, propagated to existing check/function instability metadata; answers and proof verdicts unchanged |
| Native focused evidence | Mac arm64, Rust 1.99.0: all nine ledger tests pass; the exact old arithmetic fails the new wide-integer boundary control |
| Linux source | `bc142bf05cc248a8b34858544c7fa2d11e7b660f`; native EC2 c7i.4xlarge, us-west-2, Ubuntu 24.04 |
| Linux qualification | Workspace gate (1,328 tests, 53 skips), solver (50), full end-to-end including extra; PR conformance and full fast/standard conformance pass |
| Corpus results | Each of four PR arms: 33 Same / 311 functions; full fast: 147 Same / 3,451 functions; standard: 159 Same / 13,166 functions. Zero different, unstable or infrastructure errors; no skipped projects |
| Independent review | Exact ledger source approved; nine native tests, 627,105 independent wide arithmetic cases and three old-expression must-fail controls |
| Mutation controls | All four catalogues regenerate identically; 18 HumanEval mutants plus cold/revert/forced-revert: 21 Same, zero unstable/different/infra errors or false verified |
| Rebase | 405 selected crate/extension/xtask Rust, Cargo and toolchain paths are byte-identical after the scanner-only main rebase |
| Rebased workspace gate | `032eac09` on main `f4d7c56e`: fmt, clippy, 1,328 tests (53 skips), doctests, rustdoc, layering, deny and generated benchmarks page pass |
| Remaining | Protected PR checks |

The regression compares 72 combinations of requested limits and used effort
against an independent `u128` multiplication oracle, including zero, odd
limits, the half boundary, and `u64::MAX`. Missing effort remains false and
zero remains unlimited. Separate native and cached unsat cases retain
`Verified` while reporting the actual maximum effort as unstable. The old
saturating expression misses that instability; the corrected comparison does
not. These are arithmetic/reporting controls, not a claim that a real pinned
Z3 invocation reached maximum effort.

The native Mac command compiles the exact dependency-free ledger source as a
Rust test binary. Candidate source SHA-256 and old-expression control are
retained outside the repository. The exact candidate ledger source SHA-256 is
`fb8aa51d9d2092907b6816524b5fe374865550b8c7dceab6fe68d5f0f851befa`;
the old-expression control source is
`4e997b66a2d748cbe72c9d874b0cddab8ba07000a84612333fee5c6a127ed0be`.
Passing test log SHA-256:
`a159e212b369b543d2513b68bf78a28c9eb1d8dba7819f01c961a219513f1a9a`;
old-expression must-fail log:
`e7af1e63787d2e62d71990890efc68f94acc40a67cf0a613589b1347be1a00ef`.
An initial focused-check setup used an incorrect exact test filter and ran zero
negative tests; it is invalid evidence and is not credited. The corrected
command selects the full `tests::` name and demonstrates the expected failure.

No dependency, solver identity, cache key, model, checker or proved body changes.
ADR 0087 classification is selected runtime reporting behavior; actual
conformance results are recorded above. The main rebase adds scanner/build tooling only; identical selected Rust/Cargo source is checked separately before carrying the corpus evidence forward. No corpus entry is changed.
The changelog records the corrected instability flag; the consumer skill and
command guides need no instruction changes.

Native Linux report archive SHA-256:
`a3607e153030889bd0d460a3a897c63fc14778fa6f6d504225ed0373bb325950`.
Rebased workspace-gate log:
`20927e4b0f1c33b6985a2df02a1eeb20c961181d88ef9fcfa63b66b763309dcf`.
Independent bounded arithmetic-review report:
`96f60770f4459d4d9bf08e8a80bab541689557fbdd82befa29b274c81719ea94`.
These raw artifacts are retained outside the repository. The mutation run is
one correctness round, not a release performance matrix. This final evidence
amendment changes documentation only; the qualified ledger source is unchanged.
