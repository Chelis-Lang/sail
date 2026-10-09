# 0020: Complete-run seed portfolio

- Status: accepted
- Date: 2026-10-09

## Decision

Implement `sail verify --portfolio accept-first-proof` as a sequential portfolio
of complete, independent Verus runs: unchanged baseline first, then seeds 1–7.
`--portfolio-seeds 4` selects a smaller ordered list; baseline always remains
first. Both Z3 random seeds are set on alternative attempts. An attempt wins
only with a successful process exit, nonempty verification results, no errors,
and every reported function verified. Proofs from different runs are never
combined. With no winner, preserve the baseline failure and its diagnostics.
Missing reports, timeouts and cancellation are infrastructure failures.

Each request owns a Cargo target directory. Clean verification-enabled packages
there before every attempt, retaining dependency compilation: changing Verus
flags alone does not force Cargo to reverify (VI-0005). Reject external target
directories, conflicting seed flags and disabled verification. Partial scopes
use cargo-verus focus. A worker process group and a parent-liveness pipe ensure
cancellation or parent death stops compilation, Verus, shims and solver children.
A detached cache daemon remains a reusable service.

Require the pinned Z3 hash. Store every attempt report and verbatim stderr under
the local cache portfolio directory; JSON includes the same evidence and the
winning configuration. Human output labels the selected mode and seed. MCP
exposes the mode explicitly; watch forwards the same CLI options.

The baseline and alternatives all use `Mode::OptIn("accept-first-proof")` keys.
Protocol version 2 negotiates the mode in HELLO; stale daemons fail safe and the
default socket changes with the version. Disk and memory metadata bind the mode;
old disk records default to default mode. Portfolio sessions never read the shared
store, and publishing filters their entries. The shim's mode negotiation uses
only the existing thin API client; no dependency graph change is needed.

## Consequences

This mode can change a verdict and is registered in `docs/divergences.md`.
Sequential attempts limit resource contention and preserve complete-run evidence.
Direct Verus corpus entries use `--verus-source FILE -- <flags>` with the
pinned launcher and the same complete-run acceptance rule. The 50% unknown serving threshold remains unchanged by owner decision. Seed
census observations remain measurements, rather than changes to default policy.
