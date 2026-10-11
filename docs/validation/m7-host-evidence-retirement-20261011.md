# Closed host evidence archived before retirement

Only the expressly authorized closed `target/namespace-drain` and
`target/result-cache-margin` trees were retired. Every regular file and symlink,
including tested images and raw validation evidence, remains in verified
archives under `target/evidence-retirement-20261011`.

[The inventory and retirement record](m7-host-evidence-retirement-20261011.json)
binds both archives, allocated bytes and ownership checks. Each archive member's
digest or link target, mode and ownership was verified; source inventories were
unchanged immediately before removal. No outside hardlinks were present. Host
FD inspection and both Sail containers' process command, cwd and FD inspection
found no holders or unreadable entries. An initial native census matched only
its own observer command; the corrected final census explicitly excluded that
PID. All-container mount inspection preserved foreign projects and confirmed
the applicable host aliases.

Removing uncompressed duplicates reclaimed 366,854,144 allocated bytes. The
archives occupy 106,164,224 allocated bytes, recovering 260,689,920 bytes before
small inventories. Active reader paths, native compiler targets, pinned runtimes
and other agents' evidence were untouched. This is storage maintenance, with
no validation or acceptance credit.
