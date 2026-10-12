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

## Create new owned copies

Use the same service UID that will launch the daemon, CLI and lease helper. Work
in a native Linux directory. Obtain the expected sizes and SHA-256 values from
an independently verified product grant; this tool does not qualify the source
or selected engine. `product-content.json` has exactly this shape, with actual
granted values replacing the placeholders:

```json
{
  "sail": {"bytes": 123, "sha256": "<64 lowercase hexadecimal characters>"},
  "sail-z3": {"bytes": 123, "sha256": "<64 lowercase hexadecimal characters>"}
}
```

Create a fresh destination and retain the installation record outside it:

```sh
python3 scripts/m7-native-product.py install \
  --source sealed-product --destination owned-product \
  --expected product-content.json > installation.json
```

The destination must not exist, even as an empty directory or symlink. The tool
creates the directory and executables with mode `0700`, copies bytes under the
current filesystem UID, compares the full content to the grant, then re-reads
each destination. It does not replace an existing file, change the sealed source
owner, copy old proof receipts, or grant capabilities. Source symlinks, FIFOs and
other nonregular final components are refused before content reads. Partial
failure removes only files created in the new destination. The local service
UID remains the trusted filesystem boundary.

Limits are two binaries, 128 MiB per binary, a 1 MiB transfer chunk and a 4 KiB
content-grant input. These are byte/work bounds, not a promise that filesystem
I/O can be interrupted at a wall deadline. The installation record is not a new
source/runtime freeze: retain the original grant and record the new ownership
and product identity in a separately reviewed freeze before acceptance.

## Audit the complete candidate inventory

If the original Guard capture for this exact candidate is available, export its
complete binding array, including runtime, stdlib, source, build, configuration
and helper files. Each row has `path` and `canonical` arrays of native path bytes,
and numeric `device` and `inode` fields. Audit it before an acceptance launch:

```sh
python3 scripts/m7-native-product.py audit \
  --inventory candidate-guard-bindings.json > ownership-prerequisite.json
```

The audit checks every supplied canonical path and device/inode binding, records
current filesystem UID and effective capabilities, and refuses stale bindings,
missing/nonregular files, duplicate paths and malformed inventories. Limits are
4,096 rows, path lengths below 4 KiB and 64 MiB of JSON input. They bound the
supplied document; they do not describe RSS. Output includes UID distribution and
foreign-owned files. A failed prerequisite exits nonzero.

The tool cannot discover or certify completeness from an arbitrary list. An old
inventory cannot stand in for new product paths or future generated inputs.
Even a complete successful audit is only setup evidence: production Guard
acquisition and content checks remain unchanged. Retain the exact inventory hash
and its original capture identity privately; paths may disclose local details.

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
