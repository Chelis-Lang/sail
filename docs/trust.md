# Trust model

How far each source of a verification result is trusted, and what is checked
before it is used. Charter rules 2, 3, 7, 9 and 10 apply.

Status: skeleton. No cache tier exists yet. Every result today comes from a
fresh run of the pinned Z3, through the pass-through shim.

| Source | Trusted for | Checks before use |
|---|---|---|
| Fresh pinned-Z3 run | Confirmed results | Solver identity (version string, binary hash and platform) matches the pin |
| In-memory tier | (not built) | |
| Local disk tier | (not built) | |
| Shared tier | Untrusted until verified (rule 9) | (not built) |

## Solver identity

A result is only as reusable as the identity it was produced under.
Identity binds:
- the solver name and the version string it reports;
- the SHA-256 of the solver binary;
- the platform (OS and architecture).

Rlimit counts have been observed to differ across machines for the same
binary (ADR 0004 update), so the shared tier may need to bind a machine class
too. That is decided by measurement before the shared tier is built.

**Owner decision (ADR 0010).** Shared-tier entries are scoped by solver
binary hash plus platform class. Hits across platform classes are allowed
only in a named opt-in mode.
