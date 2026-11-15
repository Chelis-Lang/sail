# 0155: Prepare original Kani and Rust payloads without installer execution

- **Status:** implemented; source/data review and native component checks pass; evidence closure pending
- **Date:** 2026-11-15
- **Extends:** [ADR 0149](0149-original-kani-rust-payload.md), [ADR 0153](0153-kani-owned-execution-tool-layout.md)

## Context

The bounded original-driver runner has independently qualified tool identities
and a private execution layout. Product setup still needs to prepare the original
Rust prefix from its six authenticated component archives. Running `install.sh`
or trusting Rustup-generated metadata would introduce unnecessary execution or
authority. The existing read-only payload verifier must retain its contract.

ADR 0128 permits eventual fresh bounded backend execution while result caching
stays disabled until input-closure authority exists. Payload preparation is a
prerequisite, not that backend or a source-currentness certificate.

## Decision

Add a separate contributor preparation helper, `ci/kani-install.py`. It reuses
the unchanged original bundle and Rust validators, authenticates all Rust archive
payloads, and copies only their exact named file bodies into a new private prefix.
Each copied body, size and executable bit is checked again. Identical component
overlays are allowed only because the complete original inventory already agrees;
conflicting overlays refuse. Archive links, special files, noncanonical paths,
unknown payload names and incomplete or changed payloads refuse.

Prepare the original bundle and Rust prefix in one private staging directory,
then use the existing Linux atomic no-replace publisher. Existing destinations,
including dangling links, remain intact. Failures remove only the newly created
private staging directory. No downloaded installer or verifier is executed.
The input archives and private parent must remain caller-owned and immutable;
this does not protect against concurrent same-user mutation.

Use `SAIL6305` for stable refusals. No Rust dependency, package graph, runtime
selection, CLI/MCP availability, result cache or proof-authority rule changes.
The existing read-only validator remains read-only. A separate contributor
`ci/kani-fetch.py` wrapper downloads the exact original manifest and archives,
checks HTTPS requests/redirects, strict byte bounds and hashes, and calls the
same materializer. It uses private temporary downloads and the same no-replace
publication. Product setup integration and `sail verify --backend kani` remain
separate work.

## Validation and limits

Local Python fixtures check exact payload-only publication, existing file/
directory/dangling-link preservation, invalid original archive, changed copied
bytes, identical overlays and nonprivate-parent refusal. The original bundle
and Rust validator suites remain unchanged and pass. These fixtures execute no
Rust/Kani code. Eight additional transport tests check exact bytes, truncated/
excessive/modified streams, per-hop HTTPS and no downgrade contact, invalid bounds and existing destinations.

Independent source/data review passes 32 Python controls and compares every
original file in a newly materialized prefix: Rust 7,017 files / 1,586,050,167
bytes and Kani 120 files / 507,989,223 bytes. Native Linux then materializes the
originals, authenticates them with the previously qualified Rust tools, stages
them and runs all three original arrayvec harness cases with the expected
bounded-success/counterexample/insufficient-bound outcomes. The downloader
wrapper separately fetches and materializes the originals on native Linux and
passes Rust tool authentication. Independent downloader review rejects the
initial final-URL-only check; the exact successor checks every redirect before
contact and passes 40 Python controls plus 30 independent urllib redirect cases.
Original rejection and interrupted setup attempts remain recorded in
[the validation record](../validation/kani-payload-installation.md). Complete
capture closure and the full workspace gate remain pending.

These are component checks, not selected backend acceptance. CLI/direct-arrayvec
comparison, Mac support, input closure and cache authority remain separate gates.
