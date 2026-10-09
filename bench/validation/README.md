# Cache publication cost

`stderr-publication-cost-aarch64-macos-2026-10-09.json` summarizes three paired
fill/warm rounds of the 33-unit guide subset before and after ADR 0022.
Every comparison checks verdicts and diagnostics against plain Verus. All
396 unit comparisons matched; each round compared 311 functions.

The metric sums individual unit wall times, rather than parallel elapsed time.
Median fill changed from 39.6 s to 39.1 s (-1.1%), with ranges 35.5–41.1 s and
37.7–55.5 s. Median warm changed from 29.9 s to 34.9 s (+16.7%), with ranges 29.5–49.9 s and
32.7–35.3 s. Concurrent development, compilation and source
conformance ran on this secondary macOS machine. These overlapping ranges
establish neither a stable speedup nor a stable regression.

The after variant includes immediate `(exit)` completion as well as safe publication
and exact complete-input silence records. Shared storage was disabled during this
probe. Source reports are retained locally; their
SHA-256 hashes and summaries are in the artifact, without local paths.

To regenerate from a directory containing `old-1-fill/report.json` through
`new-3-warm/report.json`:

```sh
python3 bench/validation/stderr_cost.py REPORT_DIRECTORY > bench/validation/stderr-publication-cost-aarch64-macos-2026-10-09.json
```

Each report comes from `sail-conformance run --tier fast --project verus-examples
--filter /guide/ --jobs 3`, using the appropriate shim and daemon and a fresh
cache per variant and round. Every fill is immediately followed by its warm run.
Long probes remain local-only.

## Initial product acceptance

`initial-product-acceptance-2026-10-09.json` retains the summaries, unstable
case identities, plain failures and SHA-256 hashes of the final acceptance
reports. Long standard, source and Singular suites ran locally, independently
of fast CI. No local paths are embedded in the generated artifact.

Organize the raw reports under `REPORT_DIRECTORY`, matching the relative paths
in `initial-product-acceptance-input.json`, then regenerate with:

```sh
python3 bench/validation/acceptance.py bench/validation/initial-product-acceptance-input.json REPORT_DIRECTORY > bench/validation/initial-product-acceptance-2026-10-09.json
```

The generator refuses incomplete coverage, a disagreement, an infrastructure
error or an unexpected skip. Deep instability additionally requires that the
entire Sail result match one complete observed plain result. Preserved plain
failures and ignored upstream cases do not count as successful proofs.

## HTTP shared-cache runtime

`http-cache-runtime-linux-2026-10-09.json` records the tested runtime image and
Sail binary hashes, Linux platform, glibc and unprivileged user. It checks actual
server startup, reader/writer authentication, private credential permissions and
credential persistence after replacement. The test publishes only a loopback
host port, captures credentials privately and removes its containers and volume.
Docker is needed for this deployment probe, not normal Sail verification.

After building `deploy/cache/Dockerfile` with the Linux release binary, run:

```sh
python3 bench/validation/http_runtime.py --image sail-team-cache --out bench/validation/http-cache-runtime-linux-2026-10-09.json
```

Signed publication, conflicting writes, certificate-based full-session reuse and
a fresh HTTP reader's exact equality with pinned Z3 are covered by the daemon
and ignored real-solver integration suites. The runtime probe complements those
checks and does not claim to validate TLS termination in an operator's proxy.
