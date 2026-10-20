# Solver executable identity (#213)

**Native and macOS correctness checks complete; digest memo selected by PR #219.** The daemon computes solver identity for every
HELLO instead of retaining it under `(path, length, mtime)`. Each handshake
runs the version command and reads the complete executable bytes, reusing a
digest only after full fresh-byte equality as described below. A launch,
version-parse or read error refuses the handshake; the shim's existing fallback
is unchanged. Since #232 the comparison reads the file through a 256 KiB buffer
while the version command runs, with no executable-sized allocation; a
mismatch still reads and hashes the whole file
([cold first run](cold-first-run.md)). This removes the demonstrated stale-metadata memo, not every
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
  failed before the full 50-sample cohort; retained as failed, with no
  complete performance acceptance.
- 2026-10-17, the same pair, second cost attempt: donor output-order
  mismatch before timed samples (0 / 50); failed, no samples replaced.
  The later quiet fresh-project variant is a separate prospective cohort.

## Fresh-byte digest memo

The implementation selected by PR #219 retains one immutable executable body
and its SHA-256. Every HELLO still launches the version command and reads the
complete executable anew. A digest is reused only after literal equality of
all freshly read bytes; path, metadata and previous version output never
substitute for that comparison. A poisoned memo falls back to hashing the
fresh bytes. Bodies larger than 64 MiB are hashed without retention. The
one-entry limit is not a process-wide memory bound: concurrent readers can
retain older snapshots, and every handshake still allocates its fresh body.

On 2026-10-17, candidate `fd31d747` passed the full shared macOS arm64
host gate: clippy and 651 tests, with 35 solver tests skipped, plus the real
HELLO reproduction. Targeted controls also cover unchanged bytes with changing
version output. The initial clippy failure remains retained. These are
correctness checks, not full conformance or performance acceptance.

The matched native c7i.4xlarge screen in us-east-1 compares accepted-runtime
sources `4ee320a5` and `fd31d747`: all 50 fixed samples pass, five rotated
pairs per case. Source, runtime, executable and final guards agree. Complete
verdict/diagnostic fields and literal solver-input multiplicities agree;
stdout is literal, and stderr uses only the inherited E2E `Finished`
numeric-time allowance with all raw bytes retained.

| Workload | Baseline wall seconds | Memo wall seconds | Baseline daemon self CPU seconds | Memo daemon self CPU seconds |
|---|---:|---:|---:|---:|
| Fresh project, warm solver cache, quiet and one Cargo job | 46.4141 | 44.8362 | 14.54 | 9.16 |
| Vest proof-body edit | 26.4238 | 26.1265 | 11.18 | 7.33 |
| Vest comment edit | 26.0934 | 25.3924 | 11.19 | 7.32 |
| Small project unchanged | 0.4166 | 0.4168 | 0 | 0 |
| Small project helper insertion | 1.1944 | 1.1821 | 0.04 | 0.03 |

These are medians over observation windows, not whole-pipeline CPU or default
cold-start performance. Waited-child CPU is reported separately: fresh-project
medians are 0.53 and 0.54 seconds; Vest body and comment medians are 0.43 seconds
in both arms. Daemon RSS after observation rises by roughly 34 MiB in the
active cases; this is an observed tradeoff, not an exact memo-allocation bound.
Wall improvement is modest (about 1–3% for the active verification cases),
while daemon self CPU falls about 34–37%. No full solver-output coverage is
inferred from provenance records. These earlier-source measurements qualify
the cost screen; the separate current-main join below qualifies correctness.

The current-main join `bdc6f39a` passed native x86-64 qualification on a
c7i.8xlarge in us-east-1. Its four production Rust files, Cargo inputs and
runtime pins are byte-identical to `fd31d747`; the earlier macOS host gate
therefore covers the same implementation. The native queue passed the full
workspace gate (662 tests, 35 solver tests skipped, two doctests), all 35
solver tests, all nine E2E phases plus extra, and PR conformance. Full FAST
passed all four arms, each 147 Same / 3,450 functions. Full STANDARD passed
all four arms, each 159 Same / 12,726 functions, with no Different, Unstable,
infrastructure errors or skipped projects. Rlimit drift was 0/0/0/1 in
pass-through/cache-fill/cache-warm/engine order and remains recorded.

Final guards confirmed all 591 tracked source files, the complete 3,886-file
runtime map and identical initial/final byte, size and mode inventories for
all five built executables. Their actual bodies are retained with source
bindings. All 25 stage archives were strictly verified. Local merge
qualification is complete; lead review selected the change for PR #219,
subject to its final current-head CI. This is not a new performance measurement. The
raw controller's inherited `qualification_for_digest_memo` field remains
`false`: it is initialized once and never updated. Its actual success fields
are `status = selected-queue-pass`, `passed = true` and exit 0, backed by the
executed command results and final guards above. Tiny transport markers,
including `after/status.json`, always retain `diagnostic_only = true` and
`qualification = false`; their context reports the queue result. Those
transport fields supply no independent qualification or selection decision.
The frozen records are unchanged. The
qualification final archive SHA-256 is
`77e923780b77eefd9c10b5c8cdc8ad1a45269e61513349e114aba0ac31a33fa9`;
the STANDARD aggregate is
`2c268aeedbd34ea9d2f6be94f1b48e70118ffaf049aaee692b5486ff9d88ee27`.

Raw final archive SHA-256
`e5ca4c9666c5ea1ffb597f21ec2201bd4d89476b90355ad4d6b3840df8a1c5be`;
all-sample cost summary SHA-256
`f1af9f9409b0cd54bd4c705fc2d8ee3ef5f80faab60e249296ee20a9507a7dd0`.
Initial native attempts that never started the controller remain unrun and
are not performance samples. The original screen preserved native binary
inventories but not executable bodies. The later qualification above used a
genuine rebuild and retained its five executables; it does not reconstruct the
original measured binaries.
