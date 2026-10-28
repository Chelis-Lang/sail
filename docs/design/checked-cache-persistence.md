# Shared cache record framing and write sequencing

Status: source candidate extracted from the existing acceptance work. Direct
Stock discovery, failure controls and native qualification remain pending;
see the [qualification record](../validation/checked-cache-persistence.md).

`DiskCache::put_scoped` calls the shared `cache_write::write_record` transaction
through the private native `persist::NativeWriteHost`. The transaction creates
a new temporary file, serializes the header, writes the complete record,
observes metadata on that descriptor, closes it and requests rename. A failed
exclusive create returns its original error without removing the preexisting
name. Later failures attempt cleanup of the name this call created.

`cache_record` shares the actual header check, record encoding and first-newline
split. Production `parse_entry` uses that split before its existing JSON,
version, key and reply-digest checks. The encoding preserves the original reply
bytes, including newlines and binary data. A header containing a raw newline is
refused. The normal JSON serializer escapes embedded newlines.

The complete `persistence` proof root includes these operations, the transaction
and its executable scripted host. It also includes the original record
roundtrip and nonempty witnesses. Three record-framing controls and six
transaction controls remove required checks, change the separator, omit or
reorder I/O calls, remove an unowned name or substitute an error. All must fail
ordinary whole-root verification at unchanged default resources.

`PersistenceHost` states low-level call effects and records the exact bytes
passed to writes and associated with successful rename calls. Failed rename
attempts remain in the operation trace and produce no completed result.
The native file and serializer adapters
are reviewed and hash-bound, not Stock-compiled. The scripted implementation
is compiled and checked by Stock; its results do not prove the native adapter.
The source guard locks both the trait contract and that implementation and
requires the implementation to remain inside a checked envelope.

`WriteCommit` is only a transaction result. It grants no solver-origin or
acceptance authority. It does not establish which object a pathname denotes at
rename, filesystem durability, globally unique metadata stamps, the absence
of concurrent mutation, or historical validity of the protected store.
Recovery still needs the existing integrity checks and the full design's
writer-origin and filesystem refinement. Shared and bundled writers remain
separate obligations.

Host trait syntax lives in a separately captured module. Existing proof roots
do not load it, so this extraction need not change their compiled programs.
Every new trait assumption still requires explicit review. The production
dependency graph and default cache format remain unchanged.

This is one part of stage D of the
[acceptance design](evidence-acceptance-core.md), not the composed service
theorem. Selection still requires complete direct Stock evidence, native gates,
scoped full fast/standard conformance and a measurement of the added header
validation cost.
