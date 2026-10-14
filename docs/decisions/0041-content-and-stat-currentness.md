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
