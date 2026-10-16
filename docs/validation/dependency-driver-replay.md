# Verified dependency replay at the compiler boundary

| Field | Value |
|---|---|
| Status | **Blocked; not accepted.** The complete-event candidate incorrectly accepts a confirmed APFS memory-mapping mutation. The real macOS store matrix also fails five strict reuse/publication assertions. No shipped or local-store acceleration is qualified by this candidate. |
| Defect | [#187](https://github.com/Chelis-Lang/sail/issues/187): copying native dependency artifacts can suppress a changed linker's error. Separate from [#184](dependency-store-output.md). |
| Source scope | Reproduced on main `0a7c1d0d` plus the standalone stdout correction `2f7102c4`; no shipped dependency feature. Runtime: pinned fork `73dea021`, primary Verus/Z3 pin. |
| Environment | Shared Apple M3 developer machine, macOS 26.5; correctness reproduction, not a benchmark. |
| Best evidence | Same-PATH linker reproduction archive SHA-256 `7bea96ebf3bd10a292ec6508bb7322a8a799fbc25ef024a89cf256d90783f51c`. |
| Current focused checks | The complete-event core/input harness passes 32 focused tests; its actual APFS mmap control nevertheless fails, below. Focused publication-flow checks exercise the real private adapter and rustc with an explicitly synthetic verifier report. These are fixtures, not Verus/corpus acceptance. |

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
This architecture remains an unaccepted candidate: the input observation does
not yet cover every mutation visible to the compiler, as the mmap case below
demonstrates.

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
predicates. The retained original parallel log has SHA-256
`4cacac22119f8ba997ee48f35b486c4d3e2d05116d12688362903f6aa7e55856`.
The original serial raw log (`0ec55a40e80c287c03d66e0057f84986bd077a3438c19aeb1280b9d11be67fc3`)
was overwritten by a later diagnostic script invocation; its loss is retained
below and that missing log does not qualify the current source.
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

## Coarse-timestamp HFS counterexample

The exact `c053bb13` resolver (`8e9854925964bcef352d3851b3d6764fc68ff074515ac845268e822e1f7138f8`)
and inventory (`3929714d5bb319ed6e2dc231f04470e2fe5357f0f8fadb0b9fbfa627d8d973aa`)
were tested on an owned mounted HFS+ image. The rapid regular-file control
fails immediately (exit 101): it writes same-length bad bytes, actually reads
them, restores the original bytes and mtime, and the observation incorrectly
remains unchanged with an identical full stamp. Source/library/runtime scope
and the actual setup/test outputs are retained; manifest SHA-256
`ec1a98281b45aa7d7a697fd950850d79f2e76c725b417370c4c3a85afab24805`.

HFS admission is now removed and its predicate regression requires refusal;
actual new-source HFS capture refuses both regular roots and recursive trees
(exit 0). The frozen helper is `bb5cc037524ba9233a135a266db00f139fe0575d12cff4efbd72bd906fee1b46`;
manifest SHA-256 `fa901435dd3027be2d123c4611231f5a78bbc16e178f6de1e4868f1fd3ee431d`. No byte, topology or native
mutation check is relaxed. APFS passes 1,000 rounds each of regular-root and
recursive-child content/mtime restoration and permission ABA (4,000 total),
log SHA-256 `08b395339de362ce0fda7fa149c81270a3cbdc348339a2a5daac561bcd915cc1`.
The same 4,000 controls pass on actual native Linux overlay; results manifest
SHA-256 `98f8435d2ce16f73589083c2d6de2ca5a76d1b4887eec7a355ce9b14c458e11b`.
Those passes do not establish all admitted filesystems or complete native
write-event coverage, which remains under review. This candidate is unaccepted.

The `c053bb13` full Mac host gate passes 649 tests (35 skipped), with all 507
tracked source hashes unchanged; log SHA-256
`53a4c6e39168f7dbf07392b380bee38e160b262e0952cc9812d57cfe34f04854`.
This predates HFS refusal. Its live donor still publishes no entry. A separate
sidecar diagnostic again reaches admission and refuses changed inputs; no
strict store assertion was relaxed. Further native event/path tracing is
required to establish the next refusal.

| Date | Source / attempt | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | Exact `c053bb13`, actual HFS+ | Failed safety control (101), round 0 | A source ABA with restored mtime retained the same full stamp; manifest above. HFS now refuses. |
| 2026-10-16 | HFS wrapper setup attempts | Failed before qualified control | Obsolete image format flag and literal/canonical mountpoint mismatch; raw setup failures retained. |
| 2026-10-16 | Two HFS diagnostic-overlay attempts | Not source qualification | Actual failures retained, but helpers included temporary diagnostics; exact frozen-source attempt above is authoritative. |

## Complete input-event candidate

The structural correction enumerates complete metadata before byte reads,
activates one bounded queue for all inventory endpoints and resolution paths,
then requires identical topology/identities before hashing. File writes and
complete-directory membership changes now latch independently of timestamp
precision; symlink parents receive membership guards too. Linux uses union
masks for duplicate-inode watch roles. No native queue is restarted while
input bytes are read. HFS continues to refuse.

The first combined core/input harness passes 32 focused tests; log SHA-256
`95a1f7f83d449477c2569e252d6a8ef9d1bce2582600594916ddb3caa3b08584`.
The helper-only set passes 22 tests and strict clippy. Direct native-only tests
prove that file write/read/restore and temporary directory membership events
refuse without consulting any timestamp. A duplicate complete/ancestor watch
role must retain the stricter mask. These are fixtures, and the memory-mapping
counterexample below blocks acceptance despite their passes.

A real vstd plan enumerates 7,550 files and 1,452 directories across its existing
36 source/compiler roots (9,002 paths), excluding deleted extern/generated
paths from that diagnostic run. The 100,000-path cap is bounded and descriptor
exhaustion refuses; no resource limit is raised. No real store-hit or timing
acceptance follows from this enumeration or the focused fixture results.

## Memory-mapped input mutation blocks acceptance

Independent review tested the exact `43eaf7a9` helper on APFS. A writer maps
an observed input, writes different same-length bytes that the compiler can
actually read, and restores the original bytes while keeping the mapping
open at the guard barrier. All six regular-root/recursive-child cases pass
the guard incorrectly: the original full stamp is unchanged, and no subscribed
native event rejects the observation. The cases include a mapping created
before capture, a pre-dirtied mapping, and a mapping created after capture.
Fine timestamp precision and complete write-event registration therefore do
not establish this guard's correctness. No new acceptance claim or filesystem
exception follows from the ordinary-write fixture passes.

- Resolver SHA-256: `a7f1cfeb8568afa4c25f72b8751fedd6cf866f9a459a74e3f13c6db60ad52542`.
- Inventory SHA-256: `024de915154ed915e6fe25fdaf43a84b3c9b35d12e76cd6980ad591f740a9a41`.
- Actual-source harness SHA-256: `166fad1b0e17d23a9aa5e5616633605f622de7c8a436ae0d34c0701ef63af8e5`.
- Writer SHA-256: `ce35ebea184c3d2591925431de75d6589d9b1f07b5c530c76db768717ad77e75`.
- Actual APFS stdout SHA-256: `7b29bb01e1606945be21b1af3bb986b0ba0c7e4f9b2cde6d683b0657d983f18c` (exit 1).

This proves a defect in the unmerged candidate's input guard. It does not by
itself reproduce an ordinary managed-receipt defect on main or qualify a
particular cache replay through real Verus.

| Date | Source / attempt | Result | Cause / evidence |
|---|---|---|---|
| 2026-10-16 | Exact `43eaf7a9`, actual APFS mmap | Failed safety control, all six cases | Different mapped bytes were read and restored without a changed stamp or refusing event; stdout hash above. |

## Detected mutations invalidate ordinary receipts too

For an admitted active guard, a detected mutation now withholds both the v2
entry and the ordinary successful invocation. A durable per-run marker makes
project currentness Unknown and prevents pending receipts, including receipts
for another successful target in that run. Unsupported preflight remains the
ordinary verification path; cache publication I/O failure alone does not
invalidate a sound compiler observation.

The private-driver integration fixture actually invokes rustc for metadata,
then emits an explicitly synthetic verifier summary. A successful positive
control publishes both observations; changing source after rustc returns
withholds both reuse paths, while an unwritable store path preserves ordinary
success. A separate runner fixture proves the per-run marker also blocks
another target's pending receipt. These checks establish publication ordering
only. They do not repair or waive the mmap blocker above.
Both focused tests and scoped CLI/Verus all-target pedantic clippy pass with
the source manifest unchanged; raw manifest SHA-256
`0820aec790069fe18ac6246f7f0102ddace68dc9e173bb4b9a8f420b6aaa5b40`.

## Read-only fallback assessment

No code or test expectation changes follow from this assessment. Withholding
the v1 whole-target store would leave fresh Cargo/native compilation and the
existing exact-prefix Z3 cache intact. The actual verifier still establishes
each report; a cached solver answer does not become whole-target proof reuse.
This avoids a new source-observation boundary but forfeits most store gain.

The original native c7i.4xlarge [store measurement](dependency-store.md), fork
`73dea021`, recorded a donor-cache-warm new demo at 27.5 s before the store,
1.30 s with it and 29.5 s plain. The corresponding new Vest worktree was
52.3/25.7/66.1 s. These are committed historical medians, not a new candidate
benchmark; their cited archive is `c48fa5edbfbbdb08fcc6ee759af150155a98d962131704c3c6baeb0381bfe954`.

Preserved older native deep-tier raw logs were read directly: one direct
vstd invocation takes 23.83 s with the exact solver cache warm, against paired
plain stock's 26.75 s. Warm Rust time is 9.796 s and verification time 11.259 s,
including 5.897 s VIR; both runs verify 2,060 functions. This is an older
stock-plus-shim single-file corpus case, without Cargo's native-dependency
work, not current-fork qualification. Its warm/plain stdout hashes are
`b65d73e1f0a11b7380c6d58290d9ffb329f9871f46d6f691fffaea0c66fab8fb`
and `bcfcc3f1ba1dced65cb30cf808904b909a4eaeb8687772e4442185ee457a42d8`;
the time-log hashes are `d796a9c118def590f2a3adeae5210f370b8ba99be796ffe70e1ec3c090f40a94`
and `6e6dc07e78b0f185a0a52f2cfb50233774e490221ea21df8bfa7535c3bb7225c`.

An estimated fresh-vstd path of roughly 25–30 s on that historical host class
is therefore plausible; a 1.3 s first call is not. Strict store-hit assertions
remain failing requirements, never relabelled or skipped. Any temporary safety
hold still needs the actual linker/output reproduction and normal gates before
acceptance. Complete compiler-derived identity/private immutable inputs remain
unimplemented alternatives, and `.vir` alone cannot key proof bodies. Full
read-only assessment SHA-256: `a2c223ada6816fcecb2f86a289897991b137c8b0f138a98360aeee5d8bf3738a`.
