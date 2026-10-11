# Real HE retained-workflow prerequisite: Current admission failed

The one approved functional prerequisite used the sealed `1e8b633` product,
selected b346 source runtime, source-only pinned HumanEval template and frozen
`dccb41f` controller. It ran in Linux amd64 Docker Desktop emulation on macOS,
with concurrent development work. The precise product, runtime, template,
controller, public result and worker-output hashes are in
[the portable record](m7-retained-he-functional-1e.json).

Fresh source copies and a new private cache were used. No proof receipts were
copied. Separately recorded preparation preceded the original bounded checks;
it did not relax their 600-second limit, or the retained workflow's 120-second
whole, 30-second CLI and 5-second MCP exchange limits.

| Preparation | Outcome | Verifier invocations | Full obligations |
| --- | --- | --- | --- |
| Ordinary full | Exit 0, Unknown | 2 | vstd 1871/1871; HE 331/331 |
| First exact repair context | Exit 0, Unknown, accepted false | 2 | vstd 1871/1871; HE 331/331 |
| Existing declaration-establishment retry | Exit 0, Unknown, accepted false | 3 | vstd 1871/1871; HE 331/331 |

All target reports were complete, with zero failed, unknown, provisional or
unstable function outcomes. **Project currentness remained Unknown.** The
first repair report separately noted new build-script declarations and
`kernel-lease-acquisition-unavailable`; the retry retained the lease refusal
without the declaration note. Successful obligations do not establish Current
proof evidence. The controller stopped with `SAIL0942: repair warmup lacks
Current accepted full evidence`, after 1295.715 seconds including setup and
cleanup. No further retry was made.

The later bounded full checks, retained CLI/MCP workflow and both reader
collectors were not attempted. This result provides no reader latency,
one-driver performance, physical source-budget or overall M7 qualification.
The differing verifier counts are recorded rather than interpreted as stock
fallback evidence.

Both completed workers' original stdout, stderr, events and proof-source
streams were archived without changes. The owned daemon exited normally with
code 0; cleanup and archival error lists are empty. Product, runtime, template
and controller seals matched before and after execution. The raw archive is
68,444,160 bytes, SHA-256
`7a3e0c1e5cd43406cba1d50d0a4b4a5e518ae3a43064fba2ee6224ec8cf5f8c2`.

Release acquisition error paths did not retain the acquisition stage, refused
path or syscall errno. The historical conflicting holder and cause are
therefore unknown. A later reconstructed acquisition diagnostic must remain
separate from this original failure; it cannot retroactively supply missing
evidence or qualify this run.

For the product contract and its current limitations, see
[freshness](../freshness.md), [proof repair](../proof-repair.md) and
[the roadmap](../ROADMAP.md).
