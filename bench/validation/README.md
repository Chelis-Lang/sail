# Cache publication cost

`stderr-publication-cost-aarch64-macos-2026-10-09.json` summarizes three paired
fill/warm rounds of the 33-unit guide subset before and after ADR 0022.
Every comparison checks verdicts and diagnostics against plain Verus. All
396 unit comparisons matched; each round compared 311 functions.

The metric sums individual unit wall times, rather than parallel elapsed time.
Median fill changed from 70.9 s to 77.4 s (+9.2%), with ranges 40.6–82.9 s and
38.1–180.5 s. Median warm changed from 42.3 s to 35.3 s (-16.4%), with ranges
33.4–62.7 s and 30.4–71.8 s. Concurrent development, compilation and source
conformance ran on this secondary macOS machine. These overlapping ranges
establish neither a stable speedup nor a stable regression.

The after variants have identical active cached-session code; the later revision
also moves unused shared-store files to their new safe directory. Shared storage
was disabled during this probe. Source reports are retained locally; their
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
