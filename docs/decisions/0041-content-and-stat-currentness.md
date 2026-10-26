# 0041: Content and stat checks replace kernel leases for currentness

- **Status:** accepted
- **Date:** 2026-10-12
- **Supersedes:** the lease, observer and mount parts of ADRs 0029, 0031 and 0032, and the guarded bootstrap in ADR 0036

## Context

- **Readback could not cover Vest.** ADRs 0029, 0031 and 0032 kept
  Current results trustworthy with Python-held kernel read leases, precise
  inotify observers, mount-generation polling and a daemon sysroot
  handoff. The read-time projection needed leases and watches on every
  workspace file, which exceeds the 4,096-lease and 8,192-watch bounds on
  Vest. Every Vest result therefore read back Unknown.
- **Every run paid for it.** Managed runs also paid 0.3–0.9 s per run to
  hash and guard the toolchain.
- **Owner's definition of Current:** no source, manifest or lockfile that
  Cargo compiled or read for the target has changed.

## Decision

Source identity includes path resolution as well as content. Snapshots and
retained read-check records bind each observed source symlink and linked
ancestor to its canonical destination, including compiler-declared includes.
Retargeting a source path changes its logical revision even if the set of
canonical file contents is unchanged. Read-check format 2 requires those
bindings; older records cannot authorize Current. This repairs #343 without
changing the declared filesystem exclusions below.

- **Read time.** A Current run records the content digests of the selected
  closure's input files and the presence or absence of every Cargo
  configuration path. Readers rehash exactly those files: equal is
  Current, different is Historical, unreadable is Unknown.
- **Run time.** A run is Current only if its input content revision and
  dev/ino/size/mode/mtime/ctime stamps are unchanged across the run, no
  absent configuration appeared, and every compiled unit is inside the
  closure.
- **Toolchain.** Toolchain and sysroot files are identified by
  dev/ino/size/mtime; configuration is identified by content.
- **Removals.** Kernel leases, the precise observer, mount-generation
  polling and the sysroot handoff are removed. This supersedes the
  lease parts of ADRs 0029, 0031 and 0032 and the guarded bootstrap in
  ADR 0036.

## Consequences

- **Results.** Vest results read back Current. Warm runs are 0.4–1.6 s,
  and about 15,000 lines are deleted.
- **Undetected cases.** An edit made and undone during a run through an
  already-dirty shared writable mapping, or a bind mount added and
  removed (root only), is no longer detected. The same holds for an
  in-place toolchain rewrite that restores mtime. These are documented
  limitations.
- **Unchanged.** Default-mode verdicts and SMT cache keys are unchanged
  (charter rules 1 and 2). Only the Currentness label depends on these
  checks, and any doubt still yields Historical or Unknown.

Measurements and validation: natively, managed overhead on a HumanEval
edit fell from 3.03 s to 0.94 s and on Vest from 7.27 s to 3.40 s, and Vest
results read back Current instead of Unknown (`SAIL7102`), L7c (rescue lane report, tag `archive/rescue-m7-98f6292`).

## Note (2026-10-26): installed tools are identified by content (#421)

The proof context, compiler family and dependency-store family identified
installed tools (Sail, Cargo, Cargo-Verus, Verus and its engine, the solver,
rustc, rustup and sysroot libraries) by dev/ino/size/mtime. A restored
`projects/` cache or a reinstalled toolchain always has new inodes, so a new
CI runner never reused a managed target or proof receipt, even with
identical bytes at the same paths. Byte equality is the identity relation
([evidence acceptance core](../design/evidence-acceptance-core.md),
assumption 4), so a file's identity is now the SHA-256 of its bytes
(`sha256:<hex>`). Paths stay in the context exactly as before.

Reads are memoized per machine in `runtime-identities/tool-digests.json`
under the Sail cache: absolute path, complete stat stamp
(dev/ino/size/mode/mtime/ctime) and digest. A stamp equal to the recorded
one takes the recorded digest without reading; anything else reads the file.
A digest is recorded only under ADR 0082's settle rule: the stamps before and
after the read are equal and the ctime is at least 2 s older than the start
of the observation. A restored memo cannot match on another machine, because
restored files have new inodes and change times. The run-time check
(`runtime_current`) uses the same memo, so an in-place rewrite that restores
mtime is now detected (ctime advances); the remaining limits are ADR 0082's
(a filesystem whose ctime does not advance on write, or a clock set
backwards). Records written with `stat:` identities still compare by stat
and simply never match a new context. The engine runtime's revalidation
record (ADR 0043) keeps its stat record: a mismatch there repeats the full
content check and blocks nothing. Unchanged-call replay keeps its own full
stamps, so the first call after a restore takes the full path, which then
reuses the restored receipts.

Measured on native c7i.4xlarge (us-east-1), CapybaraKV with crates.io vstd,
`RUSTUP_TOOLCHAIN=1.99.1`, Sail's install, the Rust toolchain and
`~/.cargo/bin` recreated with new inodes and identical bytes, a fresh clone
with `git restore-mtime` and the whole Sail cache restored with `tar`: main
`1f812e12` took 40.7 s with 2 verifier runs and 0 reused targets; this
change 0.94 s, Current, 0 verifier runs and 2 reused targets (0.80 s on the
same machine without reinstalling). On a warm machine, ten alternating
unchanged full-path calls took a median 0.473 s against main's 0.478 s, and
replayed calls 0.030 s for both. The memo held 77 entries (21 KB).

## Note (2026-10-16): memoize successful Cargo scheduling (#174)

Cargo's resolved graph includes scripts the selected build never schedules.
A successful Current run may memoize those absent scripts for the same full
input digest map, resolved package graph, selection, invocation directory,
proof context and compiler identity (including arguments and environment).
The memo is a bounded, checksummed optional sidecar, not a script contract
or proof receipt. Missing, malformed, incomplete or changed inputs keep the
original cleaning behavior; even an ordinary source edit discards it.
The initial memo requires the existing unambiguous selected dependency
closure: default/member and `-p` selections qualify; `--workspace` and
ambiguous package selections retain original cleaning.

Every observed script's declared content and environment checks and the late
compiler-family recheck still run. The current invocation must independently
confirm that every exempted script remained unscheduled in a successful
Cargo build. A contradiction or changed binding prevents Current and proof
receipt publication. Failed, Historical and Unknown runs cannot establish
new scheduling facts. This avoids repeated destructive cleaning when an
unscheduled package shares a name with a scheduled version; even an exact
Cargo package spec does not isolate that clean on the pinned Cargo
([minimal reproduction](../../corpus/repros/cargo-clean-package-versions/README.md)).

The joined implementation records only scripts in the existing selected dependency
closure. Out-of-closure scripts were already exempt from cleaning and provide
no useful memo fact. Empty observations skip binding/publication; loading an
empty memo is a miss. The nonempty exact-input/current confirmation and all
ordinary proof checks above are unchanged. See the current
[qualification scope](../validation/unscheduled-script-cleaning.md).

## Note (2026-10-14): macOS absent-configuration rule

- **Problem.** On macOS, a run also stamped the nearest existing parent
  directory of every absent Cargo configuration path, standing in for Linux's
  inotify watch. Cargo updates its cache database in the Cargo home on every
  build, and `/tmp` and the home directory change constantly, so every macOS
  run read Historical. That withdrew the dependencies' build-script records,
  and every run rebuilt the macro dependencies and verified vstd again.
- **Decision (owner, 2026-10-14).** macOS checks absent configuration paths
  only at the start and end of a run, and on every read, as before. Parent
  directory stamps are dropped. No FSEvents observer is added.
- **Accepted limitation.** On macOS, a Cargo configuration file created and
  deleted again within one run is not detected. This joins the undetected
  cases above and is documented in
  [freshness](../freshness.md#what-these-checks-do-not-catch). Linux is
  unchanged.
