# 0032: Sticky kernel mount-generation coverage

- **Status:** accepted
- **Date:** 2026-10-11

## Context

Managed proof currentness and run-local sysroot digest reuse retain file leases
and namespace watches from before establishing content reads. Their previous
mount check compared `/proc/self/mountinfo` snapshots. A bind mount followed by
unmount can leave both final path bindings and mountinfo exactly unchanged;
ordinary inode watches and leases alone do not establish which mounted file a
compiler opened in that interval. This gap matters before extending digest
reuse across managed commands under proposed ADR 0031.

Linux's [proc namespace implementation](https://github.com/torvalds/linux/blob/v6.12/fs/proc_namespace.c)
captures the namespace event generation when opening mountinfo. Its poll handler
reports priority/error events when that generation changes and consumes the
notification for that descriptor. A consumer must retain its descriptor and
latch the first event; repeated snapshot equality cannot renew authority.

## Decision

Each Linux namespace observer and read-lease owner retains a procfs mountinfo
descriptor, initial mount snapshot and exact namespace/root device-inode
identities. Acquisition brackets the snapshot with an initial zero-wait poll;
every subsequent clean barrier checks the identities and polls once without
waiting. Any event, poll error, changed identity or prior failure permanently
invalidates the owner. Existing path, inotify and kernel-lease checks remain.

Use the safe `rustix` event/fs interface as a Linux-only `sail-verus` dependency.
No unsafe Rust, new observation thread, notification acknowledgement or elapsed
grace period is introduced. The guard reads at most 1 MiB plus the overflow byte
and admits at most 2 MiB of snapshot storage; each owner adds one descriptor.
Unavailable or uncertain coverage follows the existing conservative fallback.

The real regression creates a separate private child mount namespace, performs
bind and unmount before any consumer barrier, and verifies that final contents
and mountinfo match their originals while all three consumers remain dirty.
The solver job explicitly runs this fault test with isolated mount privileges;
`ci/local.sh solver` supplies the same options. Other jobs receive none. The
whole suite retains its existing job timeout. No test mutates a verifier's or
another project's mount namespace.

The solver container explicitly permits the isolated control through both
seccomp and AppArmor. Docker's [default AppArmor policy](https://docs.docker.com/engine/security/apparmor/)
denies mount operations even when the container has `SYS_ADMIN`; merely adding
the capability would leave native Linux CI unable to exercise this check.
These exceptions apply only to the solver container, and the control still
creates a private child namespace before making any mount change.

## Consequences

Restoring a mount no longer restores retained proof or digest authority. Any
mount event in the retained namespace conservatively invalidates that generation,
including changes unrelated to a proof. The guard supplements content leases
and path observation; it neither establishes a proof nor attests a hostile
kernel, procfs bootstrap or externally modified compiler process. Managed
commands keep their original verdict, freshness and no-receipt rules.

The [initial validation record](../validation/m7.md)
retains the real isolated fault control and affected checks. The subsequent
[complete workspace gate](../validation/m7.md)
passes all 502 tests and remaining steps; the [complete solver suite](../validation/m7.md)
passes all 39 controls, including the same real fault.
This decision does not qualify the optional source engine or M7 performance.

## Exact per-barrier path bindings

The Linux read-lease binding check opens each current path with `O_PATH`,
compares its device/inode and compares the exact kernel pathname through an
owned procfs descriptor directory. This repeats at every barrier; it retains no
identity memo. The existing namespace, mount, lease, helper and interpreter
checks remain. Any procfs failure, mismatched identity, full pathname buffer or
` (deleted)` suffix falls back to the original canonical-path/metadata predicate.
The suffix refusal also covers live filenames containing that literal suffix.

Linux's [descriptor lookup](https://github.com/torvalds/linux/blob/v6.12/fs/proc/fd.c#L165)
returns the opened file's path; its [pathname renderer](https://github.com/torvalds/linux/blob/v6.12/fs/d_path.c)
can append the deleted marker. A deleted descriptor and a live hardlink named
with that marker must not certify the same binding. Native regression controls
exercise this collision, symlinks, hardlinks, parent traversal, native byte names
and unavailable procfs fallback against the unchanged predicate. This change is
under validation; it does not establish the real reader latency target.
