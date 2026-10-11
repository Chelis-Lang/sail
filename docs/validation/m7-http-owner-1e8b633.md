# HTTP runtime and server owner observation

One separately frozen test-only observation runs the original paused-initiator
fixture from `1e8b633`. It passes, retaining the 75 ms server delay and 250 ms
serving assertions. This is not a retry of the
[failed complete workspace gate](m7-http-gate-1e8b633.md). The
[record](m7-http-owner-1e8b633.json) binds the original archive, diagnostic
archive, bounded [patch](m7-http-owner-1e8b633.patch), raw log and unchanged
operations.

Unlike the earlier helper-only measurement, this control samples the actual
reqwest runtime thread, calling helper and fixture server. It reads bounded
Linux task CPU and `schedstat` fields at existing phases, with at most 64 task
entries and 16 phase samples. Output occurs after the request. The observation
adds proc reads; its own overhead is not subtracted or treated as free.

| Observed phase | Wall time | Calling helper CPU | Reqwest runtime CPU | Runtime runqueue wait |
|---|---:|---:|---:|---:|
| Send snapshots | 138.004 ms | 5.204 ms | 49.506 ms | 2.166 ms |
| Body snapshots | 12.556 ms | 8.270 ms | 2.565 ms | 9.174 ms |

The send interval includes the server's unchanged delay. The same runtime task
and kernel start time span both samples. The complete first lookup-to-publication
phase takes 198.353 ms. Guest per-thread accounting excludes blocking sleep,
network waits and unobserved hypervisor scheduling; it cannot establish the
complete failed gate's cause or identify a dependency hotspot.

Runtime CPU is material in this one diagnostic. The separately frozen, single
package-only [reqwest optimization control](m7-http-owner-reqwest-1e8b633.json)
also passes the fixture, but **does not demonstrate a performance improvement**.
It preserves debug assertions, overflow checks, features, TLS, signatures and
every original fixture bound. Send runtime CPU rises to 85.313 ms, send snapshot
wall time rises to 185.739 ms, and the first lookup-to-publication phase takes
208.290 ms. Body helper CPU drops to 3.457 ms, but that alone does not establish
a whole-request gain. First-use, scheduling and hypervisor effects are not
causally separated by this pair.

The [package-only profile patch](m7-http-owner-reqwest-1e8b633.patch) remains
unselected. The workspace profiles are unchanged. Neither diagnostic establishes
the failed complete gate's cause, authorizes a broader profile change, or
replaces the original failed controls.
