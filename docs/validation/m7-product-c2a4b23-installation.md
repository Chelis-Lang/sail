# c2a4b23 release and byte installation

Exact source `c2a4b2363b415aa2157fc38dc388192125b87109` completes an ordinary
release build in 106.164 seconds and the original owned installer in 2.111 seconds.
Both phases return zero within their unchanged 600/60-second limits. Known-group
closure records have no live, unknown or terminated-unreaped entry. Producer
container identity, image and birth match before and after.

The [portable record](m7-product-c2a4b23-installation.json) binds the source archive,
selected c77 runtime's 13 file seals, the preceding
[complete gate/solver result](m7-core-c2a4b23-validation.md), phase logs and both
installation identity records. Source archive SHA is
`590c4fd2de781a9312393d73d0bd0256bfe92b403be6640eaa6c321f11746a11`.
The independent host inventory rebuild matches all 1,593 extracted entries,
including modes and links.

| Executable | Bytes | SHA256 |
|---|---:|---|
| sail | 76,677,936 | `d198d931057be4a3f1e122eb053c30738a799379c161881ff1de75b7fc3aea50` |
| sail-z3 | 11,000,936 | `0abda23619c4147a8f0a7f595f7ae5954c5d98d7ba37e710473bad9f4c3e65ca` |

Separate acceptance transfer and fresh installation pass in 4.513 seconds.
Independent host hashes of both transferred ELF files match the producer and
acceptance records. Original checks verify source before/after, equal producer
files before/after and acceptance files, and unchanged births in both containers.
New executable copies and destination directory are service-owned UID/GID 0,
mode 0700; `CAP_LEASE` is absent. No existing sealed product is changed.

This is release and installation evidence, without proof, cache or receipt
transfer. The runtime seals are bound from original records; this host-only
review performs no fresh native census or runtime read. The fresh original
[120/30/5-second repair fixture](m7-repair-c2a4b23-terminal.md) passes separately;
it receives no result from installation alone. Real-project warmups, readers and
benchmark performance must be established in their own scopes.
