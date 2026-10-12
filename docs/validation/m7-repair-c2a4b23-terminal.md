# Complete original repair fixture on c2a4b23

The fresh, unchanged `ci/test-repair.py` fixture passes on exact c2a4b23 product
and selected c77 runtime. Its original 120-second whole scenario, 30-second CLI
operations and 5-second MCP exchanges remain unchanged. It completes 325
operations in 99.719 seconds, with `progress_failures=[]` and `cleanup_errors=[]`.
The enclosing native phase takes 103.209 seconds and the host controller 109.101
seconds; these include different supervision/archival work and are not the inner
proof budget. No publication timing wrapper is used.

The [portable terminal record](m7-repair-c2a4b23-terminal.json) binds exact fixture
SHA `9880f200f99007e048727dbe8348eb93f9e53937df601eb16f2d9fdc9ceaebd7`,
[installed product](m7-product-c2a4b23-installation.md), source archive, runtime
manifest, original controller and all host streams. Independent host hashing
verifies all 678 regular archive members; the archived summary equals the
terminal summary. The 72,405-byte raw archive has SHA
`76437de2827bad0e83fc407973cf716de88483f631089b526ab0bdb7100f4176`.

Original checks bind source, fixture, protected product/runtime bytes and PID1
before and after. Container identity/image/birth match, transport and fixture
exit zero, and the known-group closure has no live, unknown or terminated-unreaped
entry. This host-only review performs no new native census and does not certify
arbitrary orphan absence.

This qualifies the complete original repair fixture on this product. It neither
explains the old 730 failure nor establishes a paired speedup, full reader result,
three-round HumanEval/Vest benchmark or overall M7 acceptance. Earlier failed
scopes remain failed; benchmark preparation is a separate scope without measured
performance credit here.
