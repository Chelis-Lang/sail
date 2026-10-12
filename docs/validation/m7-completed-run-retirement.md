# Completed-run retirement validation (2026-10-12)

The corrected cleanup helper retired the closed native gate scratch after full
host evidence verification. Twenty pure controls pass. Native capture, holder
checks in both Sail containers and exact-binding removal all passed. The retained
archive contains 486,743 full evidence bytes; nine original entries were removed.
The selected products, runtime and shared compiler target were untouched.

The first capture refused removal because unrelated parent-directory timestamps
changed. No deletion occurred. The correction binds the parent by device, inode,
mode and ownership; exact root/member identities remain checked. Sibling activity
and replaced parent/root/member controls exercise the distinction.

The original e95 gate remains FAIL: 674 of 675 tests passed, solver execution was
unattempted, and its ownership observation remained uncertain. Two separate
bounded present-ownership audits established idle containers and no target/source
holders before retirement. They authorize cleanup only and do not revise that
historical gate result. The corrected Rust fixture and repair generation migration
still require their own complete native gate.

See [the machine-readable summary](m7-completed-run-retirement.json) and
[workspace hygiene](../workspace-hygiene.md). Full evidence is retained under the
Sail cache's `evidence/completed-runs` directory, outside disposable scratch.
