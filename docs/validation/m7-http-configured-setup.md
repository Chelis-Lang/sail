# Configured HTTP transport setup

Daemon binding now starts one asynchronous transport constructor when an HTTP
store is configured. It performs no request, reads no signing trust, and waits
for nothing on the foreground path. Solo and directory-only stores create no
HTTP client. The constructor shares the existing initialization gate; failure
retains the existing five-second reload cadence. Shutdown abandons publication
and joins only an already finished setup owner. The
[HTTP guide](../http-shared-cache.md) and [ADR 0025](../decisions/0025-http-shared-cache-and-local-first-team-workflow.md)
describe this product behavior.

[The portable record](m7-http-configured-setup.json) retains the exact source,
compiled artifact and output hashes. These are local Linux x86-64 Docker tests
under emulation and shared development load. Private direct `rustc` builds reuse
the original frozen dependency fingerprints; they are not a complete Cargo
gate or corpus run. The unchanged whole-session test source passed three
separate cold runs: 4.651, 2.694 and 2.534 seconds. Each retained exact direct Z3
output, no additional solver startup, and at least three signed shared hits.
The private builds exercised the committed production implementation; a module
documentation clarification followed the first compile, and the later
credential diagnostic adds assertion messages only. They are not claimed as
exact Git-archive release artifacts. Compiler arguments and dependency hashes
remain with the private raw evidence.

All four new setup ownership tests passed, including a paused constructor,
abandoned publication, failed-construction cadence, and no runtime for
unconfigured/directory stores. Twelve transport integration tests passed,
including forged/misfiled replies, signature trust, truncation, authentication,
TLS hostname/CA rejection, configuration reload and unchanged stalled-response
bounds. Host strict Clippy and the four setup/ten HTTP unit tests also passed;
the initial test-only Clippy failure is retained.

The native unit batch passed eight of ten cases with two independent case
threads. Separate runs with one case per process passed thirteen of fourteen
controls. The credential/disconnect control failed its initial required hit;
the same unchanged control also failed against the original frozen executable.
An assertion-message-only diagnostic classified the miss as `Timeout`. Its
constructor had completed before lookup. The server consumed the request
209.364 ms after lookup, leaving less than its required 75 ms response delay
inside the unchanged 250 ms serving window. No entry classification was
reached. This remains an unresolved comparator failure; neither the response
delay nor the serving window was changed.

The [original complete solver failure](m7-http-session-replay-777.md) remains
37/38, including exact output equality and an extra solver startup. These
isolated passes do not replace it or qualify the whole suite. The unchanged
baseline also passed native CI in a separate platform scope. A complete gate and
solver rerun on the integrated fix is still required.
