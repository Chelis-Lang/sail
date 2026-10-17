# Solver executable identity (#213)

**Correctness checks pass; final current-head CI pending.** The daemon now computes solver identity for every
HELLO instead of retaining it under `(path, length, mtime)`. Each handshake
runs the version command and reads and hashes the executable bytes. A launch,
version-parse or read error refuses the handshake; the shim's existing fallback
is unchanged. This removes the demonstrated stale-metadata memo, not every
possible concurrent executable-mutation race.

On shared macOS arm64, the retained private CLI built from `af339f8a` returned
the preceding solver SHA after a one-byte comment replacement at fixed path,
length and nanosecond mtime. `server.rs` is byte-identical at main `79e26818`.
The two real protocol connections returned
`b9c10bcc305463722b87534227458cb5fd0d8d168f5d68bbc665a9dc6ad7bcf0`;
the second executable's actual SHA was
`048a3e1560ee4d1cd082a30d08483a345bd8b2a3fb6e6fdaa58ac9abd5acb811`.
Its ctime changed. The fixture used only a private fake solver and daemon;
no installed runtime was changed and no wrong proof or verdict was tested.

Raw observation SHA-256
`3ef96d8617994a0d5abe37b2bd6f7d117ff8db6ee46552ab28677173d32aa34c`.
The retained private Mac CLI SHA-256 was
`d68e680cc50ddf50b0c98518315d2dad760566635c604c3ca6b0ad470361e947`.
The [standalone reproduction](../../corpus/repros/daemon-solver-identity/README.md)
returns nonzero for stale identity and removes its owned process and scratch.

The candidate adds real-client regressions for in-place byte changes with
restored mtime, atomic replacement, unchanged executables, removed executables
and removed execute permission. The accepted-runtime native Linux gates, solver tests and full fast/standard
conformance now pass, with independent source and final-integrity review.
The matched cost screen below records a material regression; final
current-head PR gates remain required. Fresh version processes and full binary reads add work to
each solver-session handshake. Measure that cost against the preceding build
on native hosts, including the warm-cache fresh-project workload, before
acceptance; no negligible-overhead claim is made.

## Accepted-runtime native checkpoint

Standalone source `51649fc2` keeps main `72acf29b`'s accepted runtime pins.
On 2026-10-17, native c7i.8xlarge in us-east-1 passed the real HELLO
reproduction, workspace gate (657 tests, 35 solver tests skipped, 2 doctests),
all 35 solver tests, all nine E2E phases plus local extra, and the PR
conformance subset. Full FAST then matched stock in every mode:

| Modes | Units matching stock per mode | Functions per mode | Differences / unstable / infrastructure / skipped |
|---|---|---|---|
| Pass-through, cache fill, cache warm, engine | 147 / 147 | 3,450 | 0 / 0 / 0 / 0 |

The HELLO reproduction returns the new executable SHA despite the unchanged
path, length and restored mtime; it exercises identity, not proof verdicts.
Full STANDARD also passes all four modes, each with 159 Same / 12,726
functions and zero Different, Unstable, infrastructure errors or skipped
projects. The pass-through arm records one rlimit-only drift under the
unchanged comparison policy; fill, warm and engine record none.

Independent final review checks all 579 archived source files against their
Git-bound manifest, clean initial/final source observations, the five actual
executable maps across every arm, and all 3,886 runtime files. All agree.
The native queue exits zero with no errors. Its performance qualification
remains explicitly false: the separate cost screen below measures overhead
on the earlier matched fork composition, and final current-head PR gates
are still required.
This is separate from the combined fork candidate; no runtime selection
follows from these results.

Raw E2E archive SHA-256
`a68bc44d075b6dacac5c26e5f4e6ea16e31673cb3611d59700660f9e943ef8cd`;
full FAST archive SHA-256
`36eb43b6f083ea51929262e02693a572c8f0f39cfc4df291af17616c6d696bce`.
Full STANDARD engine archive SHA-256
`793f3ed3b20441bcfcfe1e4d556187eb1102e2c7b1d19f1e15bf0c2bfa3b7e36`;
final archive SHA-256
`413294058b3f915a5f0fae05d6000ef3c7fc8900c606738b1dafe2aa1488eab3`.

## Exact-source macOS checkpoint

On 2026-10-17, source `e3322f4f` passed shared macOS arm64
`ci/run.sh host`: clippy on all targets, 646 tests passed and 35 skipped.
Its actual private CLI also passed the HELLO reproduction: replacing bytes at
the same path, length and mtime returns the new SHA. The CLI SHA-256 is
`2f53a554311e0c8e4c2692f1cdba9c0ee37a54f733d725a34e348661889d2d8e`.
This source retains the accepted runtime pins. The source changes since the
native checkpoint affect the managed-output test observer and documentation;
compiled Rust, Cargo inputs and pins agree with that checkpoint.

Independent review binds all 579 source files to Git and checks original and
private source observations plus installed toolchain, Cargo cache and stock
runtime before, after and in a separate late inventory. All agree. This is a
host correctness and identity check; it does not exercise proof verdicts,
ordinary installation or performance.

Raw host stderr SHA-256
`829fa5f2be18c9c4447c273e0da001e638d67246a76b208029130d11e638c311`;
actual HELLO stdout SHA-256
`8241fdb65094627565ae5231ac75abff3c316e82e51998299f0742670ae7739e`;
result SHA-256
`eb36ad00257204a9cefffb5490b771f84614f59174d8a8d8a77a7a4ee3824c76`.

## Matched native identity-cost screen

On 2026-10-17, c7i.4xlarge in us-east-1 completed all 50 fixed samples:
five rotated pairs in each workload. The preceding source `36f39a79` and
corrected source `dfafffcf` both use the earlier `8a373f81` fork composition;
the corrected daemon implementation is byte-identical to `e3322f4f`. This
is a matched identity-cost screen, not acceptance of that fork composition
or a measurement of the accepted-runtime candidate's complete performance.

| Workload | Previous median seconds | Fresh identity median seconds | Change |
|---|---:|---:|---:|
| Fresh project, warm solver cache, quiet and one Cargo job | 43.4012 | 48.2891 | +11.26% |
| Vest proof-body edit | 26.1111 | 27.3467 | +4.73% |
| Vest comment edit | 25.0607 | 27.0729 | +8.03% |
| Small project unchanged | 0.4428 | 0.4431 | +0.06% |
| Small project helper insertion | 1.2144 | 1.2636 | +4.05% |

Every pair agrees on complete verdict/diagnostic fields and literal solver
input multiplicities. Stdout matches byte for byte; stderr uses only the
existing E2E `Finished` numeric-time allowance, with raw bytes preserved.
All source, executable, runtime and final guards pass, with no replacement,
unrun or incomplete samples. The fresh-project case explicitly uses
`--quiet` and one Cargo job after earlier donor output-order failures; it
does not qualify default cold-start cost. The other four case arguments
are unchanged.

Daemon self-CPU medians over each full observation interval rise from
0.49 to 15.71 seconds for the fresh-project case and from 0.74 to 12.25
seconds for the Vest body edit. These are daemon self-CPU observations,
not whole-pipeline CPU or an attribution of wall time. The full fresh
identity correction remains necessary; performance recovery is separate
and must preserve fresh executable bytes and fresh version execution.

The final raw archive SHA-256 is
`9334a428976bd467e92f5ac2bf323efcee8e05c8626df98c2ed4c5eff33ebf71`.
Independent review verifies all 21 complete archives and recomputes all
50 timings, pair comparisons and daemon CPU deltas from the raw records.

Failed attempts remain indexed here:

- 2026-10-17, `af339f8a` private Mac CLI / daemon source equal to `79e26818`:
  actual stale identity reproduced; original outcome remains failed.
- 2026-10-17, `e3322f4f`, first Mac host attempt: bundle clone lacked a
  default HEAD; source admission refused before Rust ran. A later explicit
  detached checkout corrected setup without changing the tested source.
- 2026-10-17, `e3322f4f`, second Mac host attempt: clippy and 645 tests passed;
  the existing missing-engine diagnostic fixture rejected `private` in its
  temporary path. A later run used the equivalent owned `/tmp` alias with
  every assertion unchanged. Both failed attempts remain failed.
- 2026-10-17, matched `36f39a79` / `dfafffcf`, first cost attempt:
  interrupted before the full 50-sample cohort; retained as failed, with no
  complete performance acceptance.
- 2026-10-17, the same pair, second cost attempt: donor output-order
  mismatch before timed samples (0 / 50); failed, no samples replaced.
  The later quiet fresh-project variant is a separate prospective cohort.
