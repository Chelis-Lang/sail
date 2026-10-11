# Small known HTTP body controls

The root-granted native scope passed in 320.849 seconds on candidate `e6f6c7c`
with frozen corrected runner `db31164`. It ran 38 affected tests (16 shared HTTP,
6 refresh and 16 full integration) and all-target daemon strict Clippy. The
source is selected for integration as `827fecd`; installed product and complete
workspace acceptance remain pending. The original failed concurrent-reader
gates remain retained. No deadline or oracle changed, and no unexpected failure
or retry occurred.

The candidate uses the existing fixed 8192-byte allocation for exact small
body lengths, reading exactly that framed length without a separate EOF read.
Large or unknown lengths retain the original capped stream. Actual controls
cover zero/small/boundary bodies, truncation and missing-final-byte timeout,
malformed length headers, chunked bodies and the existing oversize/trust routes.
Cargo reported `fresh=false` for every one of the six test-image builds;
existing target files were not treated as proof of warmth. Pinned reqwest
features were exactly `__rustls`, `__tls`, `blocking`, `rustls-no-provider`.

The isolated counterfactual removed only the fast branch. The unchanged
zero-body read-count test failed with code 101 and exact `left: 1, right: 0`.
After byte-exact source restoration and ordinary rebuild, the same case passed.
Both actual images were retained before overwrite. This proves the read-count
control detects the optimization; it does not establish a deadline cause.

## One matched observation

Baseline and candidate each passed the original eight-reader lookup once:
eight Hits, one GET and seven coalesced followers, with the original 75 ms
server delay and 250 ms serving deadline. The synchronous bounded observers
were identical in both arms; all interpreted helper/I/O intervals were available
with matching task birth and complete traversal.

| Observed interval | Baseline | Candidate |
| --- | ---: | ---: |
| Whole lookup to publication | 114.514 ms | 107.617 ms |
| Full caller range (eight readers) | 116.269–117.280 ms | 107.693–108.782 ms |
| Whole transport | 101.764 ms | 97.213 ms |
| Body read wall interval | 1.094 ms | 0.949 ms |
| Body helper CPU | 0.946 ms | 0.569 ms |
| Body I/O runtime CPU | 1.239 ms | 1.366 ms |
| Body total observed CPU | 2.184 ms | 1.935 ms |
| Helper to publication | 110.114 ms | 103.684 ms |
| Helper to publication total CPU | 28.884 ms | 25.225 ms |

Observed runqueue deltas were zero. Helper-start observer costs differed
(2432 µs baseline, 812 µs candidate); observations themselves perturb timing.
This one sequential pair shows no observed aggregate wall/CPU regression and
is not a distribution, stable speedup, benchmark or historical timeout
attribution. In particular, the I/O runtime body CPU did not decrease.

## Closure and retained evidence

The exact candidate source (1355 regular files and one symlink), source/lock
pins, both container births and protected bytes/modes were rechecked. Independent
postflight cmd/cwd/fd/maps scans in both containers found zero holders or
unreadable live processes. A separate fixed-PGID stat-only scan found zero
survivors or unreadable rows for the 15 actual command groups in both containers;
it did not access the returned source or target. The 4154 acceptance-container protected files and
20 handoff-container protected files, including installed products, complete
13-artifact engine inventory/manifest, six C tested images and all outside-target
C evidence, matched their preflight inventories. No selected runtime or product
was mutated or invoked by this HTTP scope.

The runner reported no process-group uncertainty, reaped all command leaders
and restored exact source. The private target and sole compilation slot were
explicitly returned to the lead before documentation work; there is no further
source/target access from this scope. Its last target executable is an
instrumented diagnostic image, so subsequent builds must process changed source
normally.

The verified gzip archive has 41 regular members, including six actual test
image copies (five distinct SHA identities), all raw phase streams, terminal
records and bounded comparison. Every phase/image size, mode and SHA matches
the archive inventory. The archive SHA is
`64e0e85a6f6af405796e51c51417f3ba525b99c0d9425d70e6a195b7cd7fc51c`
and its size is 86,998,674 bytes. Full portable hashes, phase limits/outcomes,
image and raw member inventories are in the
[terminal JSON](m7-http-small-known-body-actual-controls.json).

The earlier [source freeze](m7-http-small-known-body-source.json), runner inputs
and `2264076` freeze remain immutable historical preparation records; this
terminal report supersedes their pending execution status. The correction's
ten pure controls and A's independent review remain separate from this native
scope. Full workspace, installed product, original repair fixture and genuine
reader/performance acceptance remain independent gates.
