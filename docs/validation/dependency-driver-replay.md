# Verified dependency replay at the compiler boundary

| Field | Value |
|---|---|
| Status | Implemented candidate; **not accepted**. Native Linux gates, real store-hit/output/linker checks, macOS host gate and independent input-boundary review remain. |
| Defect | [#187](https://github.com/Chelis-Lang/sail/issues/187): copying native dependency artifacts can suppress a changed linker's error. Separate from [#184](dependency-store-output.md). |
| Source scope | Reproduced on main `0a7c1d0d` plus the standalone stdout correction `2f7102c4`; no shipped dependency feature. Runtime: pinned fork `73dea021`, primary Verus/Z3 pin. |
| Environment | Shared Apple M3 developer machine, macOS 26.5; correctness reproduction, not a benchmark. |
| Best evidence | Same-PATH linker reproduction archive SHA-256 `7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`. |
| Current focused checks | Complete integration `cargo check --release -p sail-verus` passes. Original core/inventory harness passes 11 tests and direct pedantic clippy. After native resolution observation was added, the combined 16 tests pass serially but parallel execution exposes initial-capture refusals under sibling activity; this is under correction, not a qualified pass. These include real rustc metadata restoration, added-module ambiguity, post-compile source mutation and corruption refusal; they are fixtures, not Verus/corpus acceptance. |

## Minimal reproduction

1. Create two identical registry-vstd projects, each with a fresh Sail target
   and the same Cargo home/cache. Put a private directory first on `PATH`.
2. Place an executable `cc` there which delegates to the system C compiler.
   Verify the first project, filling the dependency store.
3. Replace that same `cc`, keeping `PATH` unchanged, with a script that prints
   `fixture linker failed` to stderr and exits 42.
4. Verify the second project. The old store copies proc-macro/build-script
   artifacts, reuses vstd and exits 0 without the linker diagnostic. Plain
   Cargo-Verus on a fresh target invokes `cc`, prints its error and exits 101.

This is a confirmed diagnostic and exit-code defect in local store seeding.
It does not require a shipped bundle. The original rc.5 conformance scope did
not include changing an external native tool between two fresh-target runs;
those results do not establish correctness for this case. The release remains
unpublished and the draft asset has not been requalified with this correction.

## Candidate behavior

The candidate stops reading and filling the v1 whole-target store. Cargo
builds native dependencies normally. At the driver boundary, after that work,
only a successful metadata-only registry proof may be reused. A v2 entry
contains the verifier's `.rmeta`, `.vir` and dep-info, structured stdout,
replayable artifact notices and the actual original verification run.
No native executable, proc-macro library or Cargo fingerprint is restored.

The exact compiler arguments, effective per-package driver options, compiler
working directory, Cargo resolution/profile and relevant environment bind the
key. Only explicit output destinations and explicit extern artifact paths
have role-specific replacements; the externs' complete bytes remain bound.
Declared environment values are compared exactly, including path values.
Unsupported flags, escapes, implicit/transitive metadata searches, git inputs
and incomplete source coverage safely compile and verify normally.

Complete source-package and generated-input inventories bind file bytes,
names, directory topology and absence. Relevant installed verifier/sysroot
bytes join that identity. Local mutation observations bracket compiler start
through recording and input validation through placement. Corrupt streams or
payloads refuse before placement; failed placement removes only files that
this replay created. The original proof run stays distinct from the current
receipt/publication run, including later receipt reuse.

## Remaining gates and limits

- The synchronous input-resolution observer now has focused macOS fixture
  coverage, below. Independent review, native Linux execution and full
  integration qualification remain required.
- Native Linux must demonstrate an actual new-project vstd hit, exact stdout
  and stderr ordering, unchanged-run silence and the changed-linker failure.
  Every required gate plus full fast and standard conformance must pass.
- macOS embeds native output paths in proc-macro dylib bytes. Freshly compiled
  dylibs at different targets may therefore correctly miss. No normalization
  or omission of those bytes is permitted; any performance limit must be
  measured and reported before acceptance.
- The source inventory uses bounded streaming reads, never a partial accepted
  identity. Excessive, unreadable or unsupported inputs decline reuse.
- This record does not qualify shipped builds (#161), a fork re-pin, or a
  rebuilt rc.5 asset.

## Indexed attempts

| Date | Source | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | `0a7c1d0d` + `2f7102c4`, fork `73dea021` | Failed local-store correctness | Same-PATH changed linker bypassed; archive SHA-256 above. |


## Path-resolution guard qualification

The first inventory helper, source SHA-256
`99659a2f3922a2319f895a129b131415f74780f1d2f7981a7243c103f378eb3a`,
incorrectly accepted both of these compile-bracket mutations:

- Rename an ordinary ancestor away, put a different tree at the same name,
  read its different source bytes, then restore the original ancestor.
- Keep the literal root symlink unchanged but retarget an intermediate symlink
  within its target path, read different source, then restore the intermediate.

Both negative controls fail against that old helper (raw test log SHA-256
`b31004e8ce68fcefe88287e259193059e847d7e7703f35c1006bfcd491832d25`).
The candidate resolves the complete target chain and brackets input observation
with synchronous native inode and mount events. Watches bind held, checked
file descriptors; path and symlink identities are checked after activation,
before any inventory read. Event refusal stays latched after draining.

On the shared Apple M3/macOS 26.5 machine, a direct harness compiling the actual
helper sources with Rust 1.99.0 passes 13 tests, both serially and in 20 rounds
of default parallel execution. These include both negative
controls, repeated refusal after event draining, a temporary ancestor permission
change, refusal without an observer, unexpected opened-inode refusal, and
benign sibling Cargo file and directory output. Existing source-mutation, topology, relocation, declared
coverage, corrupt digest, alias, oversized and unsupported-input cases also
pass. The focused test log SHA-256 is
`f71e9968f336097ae9d6415c3b312d5d7345f34b87941cc613986cf55d42fbc5`;
the parallel-run log SHA-256 is
`176b1c0098a05967fdb65b61d56511948d4f98ae5db15d92e0dbdcded67bbd48`.
Direct pedantic clippy passes. The harness uses existing release dependencies
and a small build of the exact pinned safe kqueue crate; it does not run Cargo,
Verus or a real store hit and is not a full macOS host gate.

| Date | Source | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | Initial inventory helper SHA-256 above | Failed two mutation regressions | Ordinary ancestor ABA and indirect symlink-target ABA were invisible; negative-control log above. |
| 2026-10-16 | Candidate resolution helper, 13 focused Mac tests | Passed fixture scope only | Test log above; Linux, independent review and full integration pending. |

| 2026-10-16 | v2 driver candidate with native resolution observer | Failed focused parallel test execution | 14 initial captures refused under concurrent fixture activity; all16 pass serially. Registration contention is under investigation; no gate weakened. |


An additional real macOS mount fixture overmounted the observed nonempty
source directory with a small APFS image, read different source bytes there,
then detached it and confirmed the original bytes had returned. Vnode-only
observation accepted this ABA (negative-control assertion exit 101; log SHA-256
`a638cb31329bd16f8bcc04a604b88418096970571f0f61ceedaf93ea4030d54f`).
The synchronous global mount guard refuses and remains latched (exit 0; log
SHA-256 `869b8b4ed68bc84231e145e770f416324e8619123d4fa9dae830d765ae28acd4`).
This is an actual filesystem mutation test, not a Verus/store-hit gate. Linux
mount generation and inotify behavior still require native execution.

The mount-event behavior is supported by the primary implementations:
[XNU filesystem-event filter](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/vfs/vfs_subr.c)
latches future matching filesystem events, and
[Linux proc mount polling](https://github.com/torvalds/linux/blob/master/fs/proc_namespace.c)
reports a mount-namespace generation change even after the mount table's text
has returned to its earlier state. Errors and events refuse; no metadata or
output is normalized.
