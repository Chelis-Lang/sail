# Trust model

How far each source of a verification result is trusted, and what is checked
before it is used. Charter rules 2, 3, 7, 9 and 10 apply.

Status: skeleton. No cache tier exists yet. Every result today comes from a
fresh run of the pinned Z3, through the pass-through shim.

| Source | Trusted for | Checks before use |
|---|---|---|
| Fresh pinned-Z3 run | Confirmed results | Solver identity (version string and binary hash) matches the pin |
| In-memory tier | (not built) | |
| Local disk tier | (not built) | |
| Shared tier | Untrusted until verified (rule 9) | (not built) |
