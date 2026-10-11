# Complete default standard attempt: e0ca / 012

The new complete 159-unit standard pass-through attempt closed with 158 Same,
zero Unstable, zero Different and one infrastructure error. It compared 12,479
functions, found zero units with rlimit drift and skipped zero projects. Exit
code was 2 after 17,144.675 seconds. This is a complete failed scope, not a
conformance pass. [The machine record](../../bench/validation/m7-linux-default-standard-e0ca-failure-2026-10-11.json)
binds the frozen e0ca harness, 012 shim, stock Verus/Z3, original flags,
controller identity and every unit's classification.

The plain `verismo_tspec` arm reached its original 3,600-second deadline and was
classified `SAIL3022`; its process group was killed and no Sail arm was started.
Its retained stdout, stderr and time files are empty. The
[sleeping-stage observation](m7-standard-tspec-sleeping-observation-e0.md)
shows a running stock verifier with pinned-Z3 children, followed by sleeping
threads and no queued pipe bytes at the sampled instant. It does not identify a
lock owner, prove a deadlock or establish emulation/overlapping work as cause.
No retry, new limit or source-engine selection was made.

The remaining units completed: Verismo main was Same; page-table was Same on its
expected verification failure; Anvil was Same with 143 compared functions.
Page-table wall times were 2,249,061 ms plain and 1,367,425 ms pass-through;
Anvil was 864,897 ms plain and 967,097 ms pass-through. These are single-run
measurements, not performance qualification.

All 964 terminal raw-inventory entries, totaling 99,139,758 bytes, were copied
outside scratch and rechecked by SHA-256 and length. All six runtime artifacts
match their preflight identities afterward. The original controller, harness
leader and original observer are absent at closure. Observer disappearance and
the supplementary observer's original 24 MiB truncation remain retained;
complete descendant coverage is not inferred from them.

The [earlier complete 1dc failure](../../bench/validation/m7-linux-default-standard-1dc-failure-2026-10-11.json)
and [isolated bitflags Same result](m7-verismo-bitflags-e0ca-diagnostic.json)
remain separate. Nothing is stitched into this report. No cache-fill or warm
round ran. Any future complete cache comparison requires a separate grant for
the exact current product and conformance bytes; this old scope does not imply
equivalence to newer binaries.
