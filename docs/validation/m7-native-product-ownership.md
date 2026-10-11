# Owned native product installation controls

The [portable record](m7-native-product-ownership.json) covers installation and
setup metadata, not proof, reader latency or M7 qualification. The historical
[reconstructed refusal](m7-retained-he-lease-refusal-1e.md) remains separate.
No sealed product, runtime, receipt or original collector was changed.

Nine native Linux Docker-emulation controls passed, including a real kernel
ownership control. A fresh fixture's UID-501 file refused `F_SETLEASE(F_RDLCK)`
with `EACCES` while the actual service filesystem UID was 0 and effective
`CAP_LEASE` was absent. The installer created a new UID-0, mode-0700 copy with
exactly identical granted bytes. Its read lease succeeded, `F_GETLEASE`
confirmed READ, and the lease was explicitly released. Source hashes and owners
remained unchanged. These were fresh tiny fixture files, not proof inputs or a
replacement sealed product.

The controls also refuse existing destinations, symlinks, FIFOs, mismatched
size/digest, duplicate or stale inventory bindings and malformed/bounded input.
Partial content failures clean up only the new owned destination. Host tests
passed seven portable controls with the two Linux-specific cases explicitly
skipped. The initial host implementation used a `realpath` keyword unavailable
in Python 3.9; its two errors were corrected without weakening canonical binding
or regular-file checks. The initial native nine-test control passed before the
structured kernel record was added; the final source hashes and records are
listed independently.

Twenty reader-controller regressions passed on host and Linux. The new binary
ownership preflight fails before any setup command on foreign ownership without
the capability. Optional supplied-inventory validation binds every recorded
path/canonical/device/inode, but cannot establish completeness or Current.
Matching ownership or capability is only a prerequisite; original Guard
acquisition and validation remain mandatory.

See [native setup instructions](../native-product-setup.md) for exact grants,
private fresh copies, complete-inventory auditing and the dedicated-container
capability alternative. Root review, integrated gates and a new product seal
are required before a real-project launch. No proof retry or acceptance is
included in this record.
