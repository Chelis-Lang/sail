# Owned metadata target selection

Status: native gate, solver, all seven end-to-end phases plus extra,
PR/full fast and full standard conformance pass on `7cc62383`. Joined
current-main source `682ddd98` also passes gate, solver, all end-to-end plus
extra and PR conformance. Required PR checks are enforced separately.
Fixes #166 and the compiler false reuse in
[#189](https://github.com/Chelis-Lang/sail/issues/189).

Source: native candidate `7cc62383`, with the qualified #183 product changes.
Joined candidate `682ddd98` is based on main `8bd1948e`.
`crates/sail-verus/src/adapter.rs` SHA-256
`39d0c48dea35b208c58af62bc122643f0d92bbd75b5c7f0ea2f34872613b0120`.
The cache-overlap guard additionally qualifies `configuration.rs` SHA-256
`3c6fb9f3b5aa07b265bafd261401766c705b2e0ea4d35ecad3de55a7416fed50`,
on the metadata correction `6c03eaa5` (stacked on #183).
Runtime: upstream `426d8b01`, fork lineage `73dea021`, pinned Z3 4.16.0.
The native engine manifest is
`4f0d0d3f4315a3b0c2c4d2a977a712db8db5ba2c3f7f72c39899cf9fd50fccdd`;
Mac boundary controls used installed source variant `c1eeaec18522f7e7`.
Correctness-only local checks used a no-vstd library and the pinned verus_builtin package; these
compiler/source-boundary tests contain no SMT obligations.

Runner and Cargo-Verus's own metadata both select the absolute
`cargo-metadata/` namespace under the configured Sail cache. A highest-priority
`build.target-dir` override prevents either the caller's environment or Cargo
configuration from making metadata exclude source directories. It contains no
compiled project artifacts. The private adapter marker is removed before
subprocess execution, preserving ordinary nested Cargo commands in build
scripts. Compilation/clean environment bytes and public fallback snapshots
retain their existing behavior. Only the exact `CARGO_TARGET_DIR` environment
key is omitted from managed context identity; declared reads still bind values.

The cache must also be physically disjoint from every resolved package input
root. Canonical aliases and existing ancestors of a future cache suffix are
checked before prepare, proof receipt reads or dependency seeding. A package
inside the cache, or a cache inside any package, takes the ordinary Cargo-Verus
SAIL3112 fallback without managed receipts. Unknown/dangling paths are refused
conservatively; the default external cache keeps managed reuse.

| Check | Result and scope |
|---|---|
| Caller target unset → external → project root → source directory → root → unset | Same context/revision, Current and zero fresh verifier calls after establishment; no stray `.rustc_info.json` |
| Source edit while caller target names project root | Compile failure, fresh invocation, changed input revision; restored source verifies Current |
| Declared build-script target read | Unset succeeds; setting it fails the script with exit 101 under the same context |
| Existing `.rustc_info.json` included as user source | Its bytes remain untouched; unchanged proof reused; editing it changes revision and starts verification |
| Config target `src` and project root | Exact new CI fixture executed locally: after warm reuse, new alternative module forces E0761, fresh invocation and changed input revision |
| Sail cache outside package roots | Current, unchanged zero verifier invocations; adding an alternative module forces fresh E0761 and changed revision |
| Sail cache in source, alias to source, future source suffix, or ancestor of package | One SAIL3112 fallback note, no Current claim or managed proof state; fresh ordinary-target error control fails E0761 |
| Physical boundary unit controls | Both containment directions, every package, nonexistent disjoint suffixes, similar-name siblings, aliases and dangling/unidentified roots |
| Selected Rust tests | 177 sail-verus tests pass with one test thread |
| Selected clippy | sail-verus, release profile, all targets, warnings denied |
| Native workspace gate | `7cc62383`: 631 pass; joined `682ddd98`: 638 pass; both skip 35 solver-dependent tests |
| Native solver | Both sources: 35 pass |
| Native end-to-end | Both sources: all seven phases plus local extra pass |
| PR conformance | Both sources, all four modes: 33/33 Same, 311 functions; no Different, Unstable, infrastructure error, counter drift or skipped project |
| Full fast conformance | `7cc62383`, all four modes: 147/147 Same, 3,450 functions; no Different, Unstable, infrastructure error, counter drift or skipped project |
| Standard conformance | `7cc62383`, all four modes: each 159/159 Same and 12,726 functions; one rlimit-counter drift per mode, zero Different, Unstable, infrastructure errors or skipped projects |
| Joined current-main qualification | Gate, solver, all end-to-end plus extra and PR conformance pass; full fast/standard evidence remains tied to `7cc62383` |

Raw evidence is outside the repository in the target-context evidence set:

| File | SHA-256 |
|---|---|
| `audit-summary.json` | `50442d0349ac3d74ce618fbbba7fc1c3411f32227feec0d4118625add7d860fd` |
| `config-summary.json` | `21fecf1aa2b0652a7f05cc98cff8d51971efeb2c5015686a61a94f537d74a0c9` |
| `ci-regression-executed.py` | `3868f17c40588fb07ec4f2ecf219371e5cd956a81d256152194f05f0c38814e9` |
| `library-tests-serial.log` | `fc4a64d71cf62bfa7ca4385b4b34be27460305825a57e41953d8635b2d2e8d46` |
| `clippy.log` | `8f16f54d9ae2159dff898ccd27501810b6a37db6a34564541e9cf1bcba38662a` |
| `cache-source-overlap-2026-10-16.tgz` (pre-fix false Current and outside-cache control) | `7c2491bf63dd6d9d2e820425be1d905710b8b0972ce069c7831ff66b3e3eaed2` |
| `cache-overlap-guard-fixed-2026-10-16.tgz` (source/binary identities, 177 tests, clippy, exact CI blocks and old-binary negative control) | `a1fadb954956c1d985d6006db1334e7996706a261e6b829e594b102f7c9845cf` |

Native acceptance on `7cc62383` ran on a dedicated EC2 c7i.8xlarge in
us-east-1. All 502 tracked-file hashes match before and after except excluded
local agent settings. Internal packages were cleaned and all Rust/Cargo mtimes
refreshed before compiling. Joined `682ddd98` retains the complete accepted main input/receipt overlap,
foreign closure bindings and dependency-output replay; its own 508 tracked
hashes are also checked. Both runs exclude only local agent settings. The
pre-integration source has no current-main full fast/standard claim; joined
checks are recorded separately. Corpus pins and
conformance comparisons are unchanged.

Native archive `cache-overlap-native-evidence.tgz`, SHA-256
`0797ec4e190512e1d201f88adfb4aa7cc0337322afa9b2670305448b7df16070`.
Qualified binaries `cache-overlap-linux-binaries.tgz`, SHA-256
`8e0cf9f72c145894f48c5c81c6dbfbe0afbd0eabe1d2dc5028ca2c87af41b6d5`.

Joined archive `cache-overlap-joined-native-evidence.tgz`, SHA-256
`b6ee8a9fb786291a02e11e1f9837297c4c02b909c0db2e5379687938fbbe8572`.
Joined binaries `cache-overlap-joined-linux-binaries.tgz`, SHA-256
`4864eb5e47519c1a67eaaa2f331d1af18dcaaee51dc2cf33d1dabb29a9525230`.
Both archives were downloaded, checksummed and their report/source manifests
verified. Raw evidence remains outside git.

Failed attempts remain visible:

- `51e3b806`, 2026-10-16, native gate (631 tests) and solver passed;
  end-to-end passed unchanged reuse and generated-value invalidation, then
  its expected caller-target build-script failure reached a helper that
  required a requested artifact even though Cargo never built it. The actual
  report is Unknown, exit 101, with no requested target; stderr contains the
  deliberate caller-value assertion. The corrected negative control requires
  exactly these failure properties; every other helper call still requires
  one requested target. Failed-attempt archive SHA-256
  `2f3aa2bbbdd71935a9eefe602c22d5313d646b2fea368199e554cd967b16fe1c`.
- `f28a12bf`, 2026-10-16, native gate stopped on the new regression's
  `assert_is_empty` lint; the assertion was rewritten with the same empty
  value expectation. Archive SHA-256
  `d41b3ea40190136de57a11bbd514fc08ff06774f6e2dd9e473502d1b8f3cd2eb`.
- `c753d7cd`, 2026-10-16, native gate (630 tests) and solver passed;
  end-to-end stopped on the strict unchanged generated-input assertion
  (`ci/test-managed.py:587`). The script declared `CARGO_TARGET_DIR`, but
  family import refused every `CARGO_*` variable, dropped its contract and
  rebuilt it again. The narrow correction admits this caller-preserved
  variable while keeping exact present/absent binding and all other
  Cargo-injected variable refusals. This failed attempt is superseded by the qualified `7cc62383` correction. Failed-attempt archive SHA-256
  `c220d947efae3edf465034d23df45b64e0a924c84c80f4368ffaed9b1052c0ba`.
- `a4ba5014`, 2026-10-16, native Linux gate: 629 tests passed and the
  engine-selection fixture failed because its cache was inside its package
  root. The overlap guard correctly selected fallback before that fixture's
  intended engine-identification check. Its cache now lives in an owned
  sibling directory; every assertion is unchanged. Failed-attempt archive
  SHA-256 `216e2b61510422036ccd3738b26a46bad81b09759b84b2665aa46bf5f710b3ef`.
- Main-derived #184 staged binary, 2026-10-16: `SAIL_CACHE_DIR=workspace/src`
  hides a new alternative module after warm establishment: exit 0, Current,
  reused and zero verifier calls with unchanged revision. The outside-cache
  control instead recompiles and fails E0761. The new exact cache CI block
  also fails against the pre-guard #166 binary on its missing fallback note.
- Main-derived #184 staged binary, 2026-10-16: configured source target reused
  success after an alternative module appeared; fresh fork compilation exited
  101 with E0761. Fresh stderr SHA-256
  `df9285db195855345e48d85533a656210e6678a9a142e8b20a0217d484e219c7`.
- Initial #166 candidate, 2026-10-16: first caller-root run was Historical
  because Cargo metadata wrote a real `.rustc_info.json`; the strict zero-work
  assertion exposed this. Removing the caller override alone still excluded
  configured source directories. Negative-control summary SHA-256
  `f97962c5a6dcbcba236a872f35d1ef69f8f8e14dfce5080db1f8193dd31692d6`.
- Parallel cargo-test, 2026-10-16: 170 pass, 2 fail; existing engine and adapter
  fixtures share `sail-engine-fallback-<pid>` and collide in one process.
  Serial execution passes all 172. Raw failure SHA-256
  `8b1f24000c91621aac7189c72c4646988e429dc1c03c0886ee8d1e54a5f5f149`.
- First selected clippy attempt: the draft declared-environment test used two
  redundant clones; corrected to reference slices and rerun successfully.

The fix may conservatively inventory unrelated user output directories that
were previously excluded by metadata's target selection. No source-tree
exclusion is restored for speed; native performance measurement is pending.
