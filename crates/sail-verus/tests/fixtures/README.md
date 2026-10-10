# Kernel lock classifier fixture

`kernel-locks-frozen.txt` is the original independently captured Linux
`/proc/locks` body used for the M7 ASCII classifier experiment: 155,700 bytes,
2,883 rows, SHA-256
`b8270fe47c1efcf3ccffb529cab23b49eb26c4f50715f9c24d56c1dd6c2d38aa`.
It contains kernel identifiers, not source paths or user credentials.

The ignored timing diagnostic compares every row against the original Unicode
whitespace classifier for three PIDs and measures 20 alternating rounds. The
solver gate runs all ignored tests, so this retained fixture is the default.
`SAIL_LEASE_ROWS_FIXTURE` still selects an explicit local capture; unreadable
or oversized captures fail. The generated separator, truncation and owned-row
negative controls run separately in the ordinary workspace gate. These timing
samples establish classifier behavior only, not live-reader product latency.

See the retained experiment in `docs/validation/m7-kernel-ascii-parser.json`
and the first complete solver attempt in
`bench/validation/m7-linux-solver-b36-emulated-2026-10-11.json`.
