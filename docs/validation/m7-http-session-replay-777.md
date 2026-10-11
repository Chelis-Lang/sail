# HTTP whole-session replay failure

The complete solver suite at `777b86a` ran all 38 cases: 37 passed and one
failed. The failing HTTP fresh-reader case returned exactly the direct Z3
output, but started a solver: its start count increased from two to three.
The original assertion and all serving deadlines remain unchanged.

[The portable record](m7-http-session-replay-777.json) retains the exact cache
and signed-entry hashes. A private archive preserves the writer cache, fresh
reader cache, staging entries, HTTP store and configuration. The three HTTP
entries match the signed staging bytes, including the whole-session silence
certificate. The reader's three local entries have `pinned-solver` provenance;
they do not establish that the remote store served any reply.

One separate run of the unchanged frozen test executable reproduced the same
assertion failure. It is a diagnostic, not a replacement for the failed complete
suite. The original fixture did not retain transport phases or daemon counters;
its cause is still under investigation.

For users, a failed shared lookup falls back to the configured pinned solver and
can cost a solver startup even when a signed session is available remotely. The
[HTTP cache guide](../http-shared-cache.md) describes that fallback behavior and
the unchanged 250 ms default lookup deadline. No verdict or diagnostic mismatch
was observed in this case. This record establishes neither current whole-suite
qualification nor reliable cache-only cold replay.
