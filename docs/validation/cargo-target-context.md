# Owned metadata target selection

Status: implementation and bounded macOS checks pass; native Linux gate and
full end-to-end/conformance remain pending. Fixes #166 and the compiler false
reuse in [#189](https://github.com/Chelis-Lang/sail/issues/189).

Source: candidate over `0a7c1d0d`; `crates/sail-verus/src/adapter.rs` SHA-256
`39d0c48dea35b208c58af62bc122643f0d92bbd75b5c7f0ea2f34872613b0120`.
The cache-overlap guard additionally qualifies `configuration.rs` SHA-256
`3c6fb9f3b5aa07b265bafd261401766c705b2e0ea4d35ecad3de55a7416fed50`,
on the metadata correction `6c03eaa5` (stacked on #183).
Runtime: installed source build variant `c1eeaec18522f7e7`, upstream
`426d8b01`, fork lineage `73dea021`, pinned Z3 4.16.0. Correctness-only local
checks used a no-vstd library and the pinned verus_builtin package; these
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
| Native Linux/full host/e2e/conformance | Pending; no acceptance claim |

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

Failed attempts remain visible:

- `c753d7cd`, 2026-10-16, native gate (630 tests) and solver passed;
  end-to-end stopped on the strict unchanged generated-input assertion
  (`ci/test-managed.py:587`). The script declared `CARGO_TARGET_DIR`, but
  family import refused every `CARGO_*` variable, dropped its contract and
  rebuilt it again. The narrow correction admits this caller-preserved
  variable while keeping exact present/absent binding and all other
  Cargo-injected variable refusals. Native correction qualification is
  pending. Failed-attempt archive SHA-256
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
