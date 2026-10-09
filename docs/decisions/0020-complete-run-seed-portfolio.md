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
Protocol version 4 requires the silent-session safety rules (ADR 0022) and negotiates the mode in HELLO; stale daemons fail safe and the
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

Function-only timing data can include skipped successes (VI-0009). Structured
reports apply the explicit selector using pinned Verus matching rules before
complete-run acceptance or MCP storage. Exact names take priority over unique
substring fallback; explicit leading/trailing wildcard rules are preserved.

## Small end-to-end proof

`corpus/repros/z3-nla-seed-portfolio/` records a complete one-function
nonlinear proof. At the pinned release's `--rlimit 0.01`, the unchanged
baseline exhausts its budget, and seed 4 independently proves the crate.
Fresh and warm host runs and a Linux CI-image run retain the proving
configuration and solver hash. This is separate from page-table's allocation
census and does not imply a complete page-table proof.

The fast end-to-end step runs this small case cold and warm. It checks both
attempts, a single complete confirmed proof, pinned solver identity and seed
provenance, genuine warm proof hits, and re-solving the exhausted baseline.
Long corpus checks remain local-only.

## Cargo coverage

A successful exit and one package report do not establish a whole workspace
proof. Metadata resolves requested verification-enabled targets from default
members, `--workspace`, exact package selectors and exclusions, and library or
binary selection. Every selected target must produce a report in the same
attempt. Custom crate names are respected. Crate names shared by multiple verifying packages are also ambiguous: a
dependency report cannot stand in for a missing requested package. Ambiguous
crate names, nonexact
package selectors and test/example/benchmark target selection are rejected
before verification; choose an explicit library or binary scope instead.
Cleaning still covers every verification-enabled workspace package in the
owned directory, including packages outside a selected subset.

The regression omits one requested workspace member's report: the original
code accepted the remaining member as a complete proof. The corrected code
rejects it, while `-p input` still accepts that explicitly smaller scope.
