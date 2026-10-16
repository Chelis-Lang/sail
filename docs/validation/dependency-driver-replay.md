# Verified dependency replay at the compiler boundary

| Field | Value |
|---|---|
| Status | Implemented candidate; **not accepted**. The real macOS store matrix fails five strict reuse/publication assertions. Native Linux gates and final source qualification remain. |
| Defect | [#187](https://github.com/Chelis-Lang/sail/issues/187): copying native dependency artifacts can suppress a changed linker's error. Separate from [#184](dependency-store-output.md). |
| Source scope | Reproduced on main `0a7c1d0d` plus the standalone stdout correction `2f7102c4`; no shipped dependency feature. Runtime: pinned fork `73dea021`, primary Verus/Z3 pin. |
| Environment | Shared Apple M3 developer machine, macOS 26.5; correctness reproduction, not a benchmark. |
| Best evidence | Same-PATH linker reproduction archive SHA-256 `7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`. |
| Current focused checks | Complete integration release typecheck passes. Combined core/input-resolution harness passes 26 tests with ordinary parallel execution, including real rustc metadata restoration, actual repeated-emit union refusal, exact declared env absence-to-presence refusal, topology/race guards, FIFO/cache-reader refusal, partial-placement cleanup and original Report/proof-origin preservation. Direct pedantic and forbidden-operation clippy passes. Five earlier ordinary-parallel rounds passed all17 then-current cases. These are fixtures, not Verus/corpus acceptance. |

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
this replay created. An unsuccessful cleanup stops with `SAIL3111`, rather
than invoking a compiler over partial replay outputs. Cache and dep-info
readers use bounded, nonblocking regular-file descriptors and reject symlinks
and special files. The original proof run stays distinct from the current
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

| 2026-10-16 | v2 driver candidate with native resolution observer | Failed focused parallel test execution | 14 initial captures refused under concurrent fixture activity; all16 pass serially. Registration was corrected before the inventory epoch; ordinary-parallel rounds now pass. No gate was weakened. |


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

The caller review also required refusing repeated output controls (rustc unions
repeated `--emit` options), raw import forms outside the supported closure and
retaining the complete raw environment for declared reads. The candidate now
checks those cases directly; excluded routing variables remain exact when a
source declares them. The same real-compiler fixtures include a native `.rlib`
negative control and an `option_env!` absence-to-presence replay refusal.


## Nonblocking opens and filesystem admission

A source stat followed by `File::open` could wait forever if the file changed
to a FIFO before the open. The same race existed when attaching ancestor
watches. Safe descriptor opens now use nonblocking/no-follow/close-on-exec
flags, check regular-file/directory type and inspect the held filesystem before
hashing or registering a watch. Recursive inventory directories and every
streamed input file undergo admission, so an already-mounted unsupported
subtree cannot authorize replay merely because its package root is local.

Two bounded negative controls preserve the old blocking-open behavior: a
regular source replaced by a FIFO after stat, and a FIFO substituted at watch
registration. Each fails its two-second refusal assertion (exit 101); logs
have SHA-256 `025d6916b73611b719deba0460849818ea0c4e375a5c5799e346e55c8e5b4c7c`
and `266f26b07cf298b87a7c43827149dbd4801155fc109f212d446ab041c895f1e7`.
The corrected actual-source helper passes all 16 Mac tests serially and with
default parallel execution, including FIFO/no-follow refusal and admission
predicates. Logs have SHA-256
`0ec55a40e80c287c03d66e0057f84986bd077a3438c19aeb1280b9d11be67fc3`
and `4cacac22119f8ba997ee48f35b486c4d3e2d05116d12688362903f6aa7e55856`.
Strict direct pedantic clippy passes. Actual filesystem execution is APFS;
network/pseudo/FUSE/unknown rejection is predicate scope, not a mounted remote
filesystem experiment. Linux observer, supported filesystem and mount-event
qualification remains pending.

- 2026-10-16, old blocking-open variants: failed both bounded FIFO controls;
  the file reader and watch registration could wait for a FIFO peer.

The final combined 26-case macOS fixture run passes against the FIFO-safe
helper and cache-failure caller changes (log SHA-256 `85eac8e8d84f3a2fbc3f7e9b04e21a60c29a52ebc1426ee82f55531de804e62c`).
Strict direct clippy passes; the complete release integration typecheck also
passes. These checks do not replace the pending actual Verus store-hit and
native Linux gates.

The cache, dep-info and extern-byte readers also share the filesystem-admitted
nonblocking opener. The resulting 26-case run passes (log SHA-256 `15901e570a9e64fd810ec9aabc49d66de9e99bdd7725705d6c593bdce7b1e730`); strict direct clippy passes.

## Actual macOS candidate checks

Candidate `a1624c68`, before the #183 rebase and final declared-path correction,
was built with Rust 1.99.0 against the existing pinned `73dea021` runtime.
Its reported version is `0.1.0-store-driver-candidate.a1624c68`; all 507 tracked
source digests matched before and after building and the macOS host gate.
The staged Sail/shim and all 15 runtime-file digests are retained in the raw
manifest. These are correctness checks on a shared, loaded developer Mac,
not performance measurements.

- `ci/run.sh host`: workspace all-target clippy passes; 642 tests pass and
  35 configured cases skip. The final rebased source still needs its own gate.
- Same-path linker mutation: the donor verifies successfully. Replacing its
  `cc` at the same `PATH` then makes a fresh second project exit 101 and print
  the deliberate linker failure, exactly as fresh plain Cargo-Verus does.
- Plain output: the second project's stdout equals plain Cargo-Verus,
  including both verification summaries; its unchanged run prints no stdout.
- The strict store matrix **fails five assertions**: no donor entry was
  published; the second project and both concurrent projects did not reuse;
  without an entry the corruption test could not exercise warning/fallback.
  All measured exits, verdict summaries and compared stderr match plain.
  No assertion was relaxed. A subsequent live diagnostic identifies a regular
  executable access-time attribute event as one publication refusal, below.

The qualification archive (logs, manifests, comparisons and reproduction
scripts) has SHA-256
`d4e56284bdfe4183b1b46efc8a1287d2a275ce6a6416833e075432546e094559`.
Post-compile offline constructor and payload probes pass; those probes do not
establish that the live compiler bracket is valid or that reuse works.

| Date | Source | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | `a1624c68`, pinned fork `73dea021` | Failed real store matrix, five strict assertions | No entry published; cause remains under investigation. Archive above. |
| 2026-10-16 | Initial `a1624c68` staging attempt | Failed before verification | Runtime files had not yet been copied into the candidate stage; corrected before the actual matrix. Raw failure retained in archive. |

The final declared-path guard walks every literal and expanded component,
including components consumed by `..`. Unknown symlinks refuse; an unknown
intermediate ordinary directory is accepted only with its full exact native
stamp from the complete captured inventory. The terminal file still requires
its positively captured bytes and stamp, within the existing native mutation
barriers. A regression actually reads different bytes through a temporary
unknown-directory/symlink substitution, restores it, and requires refusal.
Unknown directory/file aliases also refuse; captured nested directories and
captured root aliases remain usable. All 27 combined focused cases and strict
direct clippy pass; independent static review is clean. The focused test log
has SHA-256 `5adc6dd68c82d750048a29203d55dcf20036f7f7171137fe51d9182b7b672bf4`.

## Regular executable observer correction

On source `8007192b` with a temporary, separately archived diagnostic overlay,
a live donor reaches metadata replay admission, then refuses publication on
macOS `Vnode(Attrib)` for the pinned regular `verus` executable. An isolated
observation followed by `verus --version` reproduces the event: exit 0, unchanged
bytes and guarded metadata, advancing access time. Raw live stderr SHA-256
`fb3984834d40de4f19384b0665831d27b0febaec3696af0920d5695cb7f459bd`;
isolated probe log SHA-256
`7b905bf325f72afe0206631af840dd5a491196f27b96f4cf0e640f5a7914295c`.
The diagnostic overlay was removed before implementing the correction.

The correction checks full regular-endpoint stamps in registration,
`covered_with` and `unchanged`, and omits macOS regular-file attribute watches
while retaining native rename/delete/revoke and directory attribute watches.
Source/tree, symlink, mount and native pre/post barriers remain mandatory.
Twenty actual-source helper tests and strict clippy pass, including an owned
rustc-built Mach-O execution positive, content/chmod ABA declared-coverage and
mutation refusals, and a replacement ABA with an actual bad read. Twenty
ordinary parallel rounds pass. The same execution-positive test on the old
`8007192b` helper fails (exit 101).

- Corrected helper test log SHA-256: `4c15b28294ce2b33d8a38f55a855b9c5487cf3bbff84b136494069032cf769f1`.
- Twenty parallel rounds SHA-256: `2a416334e2a3d38553375519687d81dbbbbe2539f3b0dbc9d7178b57a4d955f8`.
- Old-source negative SHA-256: `39037ac371d783474943b4ee301b695e243e1265d1163f708de3348b0e985bbe`.

Independent static review of this boundary is clean. These focused results
do not establish live publication/reuse or native Linux qualification; the
strict store matrix remains a failed acceptance gate until rerun successfully.

| Date | Source / attempt | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | Execution fixture attempt 1 | Failed before guard assertion | Copying Apple's platform `/bin/echo` produced an unexecutable copy; replaced with an owned rustc-built fixture. Raw attempt retained. |
| 2026-10-16 | Execution fixture attempt 2 | Failed expected-path assertion | Expected literal temporary path instead of the API's canonical result; corrected expectation, mutation checks unchanged. Raw attempt retained. |

An invocation of the old direct-check script overwrote its three `guard-open`
logs with a later fixture run. The original separate 16-case parallel log and
all source/runtime and native archives remain intact; the latest logs are
retained separately. No current acceptance depends on the overwritten files.
