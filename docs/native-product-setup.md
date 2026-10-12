# Native Linux product ownership

Install Sail, its selected Verus runtime, project sources and Python helper
closure on a filesystem with supported native observation and kernel read
leases. Each existing file in the complete guarded inventory must either belong
to the service's filesystem UID or be accessible to a service with effective
`CAP_LEASE`. The runtime and helper inventory matters as well as `sail` and
`sail-z3`. Merely running as root does not supply every Linux capability.

This is an environmental prerequisite, not freshness authority. Ownership or
capability alone cannot certify Current: Sail still requires its original
namespace, file-binding, mount-generation, helper and lease barriers. A writable
mapping, conflicting writer or unavailable observation still refuses coverage.
Verification can succeed while its report remains Unknown.

Linux checks the mapped inode UID against the caller's filesystem UID, or
requires `CAP_LEASE`, before granting a lease. See the primary
[kernel ownership check](https://github.com/torvalds/linux/blob/v6.17/fs/locks.c#L1893-L1906).
The [reconstructed retained-HE refusal](validation/m7.md)
found exactly two foreign-owned binaries in its complete 1,415-file inventory.
The helper returned `EACCES`; no capability or UID was inferred from a proof
verdict. The earlier proof failure did not retain the exact refused path, so this
separate reconstruction does not replace its historical evidence.

## Capability alternative

For an otherwise valid inventory containing system-owned runtime files that
cannot be installed under the service UID, a new dedicated Linux container may
use `docker run --cap-add LEASE` with the original selected image and mounts.
This is narrower than `--privileged`; no other privilege is required by this
ownership correction. Confirm the service actually has effective `CAP_LEASE`
and audit the entire candidate inventory. Do not change capabilities of an
existing acceptance actor or alter an old sealed product. Capability availability
does not override unsupported filesystems, mapped writers, missing watches or
any freshness check.

For the reconstructed inventory above, fresh service-owned copies of both
product binaries suffice for the observed UID mismatch; its other 1,413 files
already matched the service UID. The [installation and kernel fixture controls](validation/m7.md)
passed without changing any sealed product. No new real-project acceptance has
been claimed for this installation correction. See [reader acceptance](reader-acceptance.md)
and [the roadmap](ROADMAP.md) for the remaining product gates.
