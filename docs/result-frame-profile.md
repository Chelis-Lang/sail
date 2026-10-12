# Whole job-result phase profiling

The live-reader workload still requires the complete public status/result graph,
four concurrent readers, 200 samples, matching Current input revisions and a p95
below 200 ms. Its five-second RPC deadline is unchanged. Offline phase profiles
explain costs; they do not satisfy or replace that workload.

The daemon already retains three immutable, preencoded frames per cached job,
for Current, Historical and Unknown. A cache hit selects an `Arc` after checking
the exact retained stdout SHA and the live input projection. It does not clone,
parse or serialize the full proof graph again. These cached bytes cannot authorize
Currentness. Modified stdout, a lost freshness generation, changed job metadata
or an unavailable projection still prevent a Current response.

The standalone Rust example exercises the production framing codec with the
entire original response:

```sh
cargo run --release -p sail-api --example result_frame_profile -- \
    --frame target/current-result.frame
```

The input includes the four-byte length prefix. Loading is bounded by the public
frame limit; truncation, trailing bytes and additional frames are refused.
Decode stages retain the exact captured bytes and JSON member order. Cold
encoding is checked against the complete original graph, because serialization
can legitimately choose a different member order. Alternatively, pass a JSON file
containing the public Response body without `--frame`. The example checks that
its typed response preserves the original whole JSON graph before measuring
one- and four-thread typed decode, generic Value decode, full-response clone,
cold frame encoding and cached `Arc` selection. It never reads a job environment,
starts a daemon or verifies sources.

The Python diagnostic uses the same complete captured frame:

```sh
python3 scripts/m7-result-frame-profile.py \
    --frame target/current-result.frame \
    --stdout target/owned-worker-stdout \
    --output target/result-frame-phases.json
```

`--stdout` is optional and measures the exact file's read and SHA costs. Use only
an owned worker's report file, never its private record or environment. Socketpair
measurements include a new owned producer thread and full JSON decode. The
experimental `recv_into` variant is reported separately; it changes neither the
production client nor the frozen collector. Every decoded graph must equal the
original response. Run `python3 scripts/test-m7-result-frame-profile.py` for its
bounded-frame and whole-graph checks.

## Observed payload and costs

The retained genuine human-eval baseline frame contained 4,314,946 payload bytes.
Its `result` was 2,155,446 bytes and exactly equalled `repair.batch.full`, which
repeated those bytes. The focus report was 3,044 bytes. Each full graph included
a complete dependency vstd target of 1,827,990 bytes and the requested human-eval
target of 326,915 bytes. This was a real cold two-driver baseline; its dependency
report must remain in that evidence. A separately established future workload
may produce a different actual report, but no receipt copying, dependency removal
or smaller-frame assumption is allowed.

On the archived frame, full Python decoding alone exceeded 200 ms in several
samples under Linux emulation. Isolated stdout read and SHA phases were much
smaller. Release Rust typed decoding was also measured independently on the host and
in the Linux container. These
small offline samples and different platforms are not a live latency comparison,
and mixed `recv_into` results do not establish an improvement.

The [portable phase record](validation/m7.md)
retains the exact captured frame hash, field-size counts, all preliminary and
actual-frame results, executed source/binary hashes and qualifications. The Linux
actual-frame Rust typed decoder measured 37.14 ms median for one thread and
117.35 ms for four, with an eight-sample four-thread maximum of 398.12 ms. The
same frame's Python four-thread decode measured 314.43 ms median. These are
separate offline stages, not improvements or proof of a live census passing.

The duplicate full graph is a current API representation cost. Removing it or
trimming dependency obligations would change the public contract. A future
negotiated representation needs explicit design review and complete evidence
preservation; this profiler makes no such change. Likewise, avoiding a stdout
SHA read requires continuous ownership of that exact file and its namespace
from the first validated content hash. Stat metadata or an unrelated runtime
input guard is insufficient, including for restored-mtime and replaced files.

The initial Rust profiler reencoded the typed response before timing its decode
stages. Its recorded whole-graph comparisons remain evidence, but an original-byte
decode claim additionally requires verifying that reencoded frame's byte identity.
The current tool uses the captured frame directly and tests a different JSON
member order, extra frames and truncation. The earlier measurements remain
offline diagnostics with their original implementation identities.

A subsequent [codec equality check](validation/m7.md)
found that typed reencoding exactly equals the archived 4,314,950-byte framed
response on the host. The production codec, manifest and dependency lock are
unchanged from the historical profiler. This establishes byte equality for that
input with the current codec; it does not rerun the historical native timing
measurement. The profiler now reports this equality explicitly while continuing
to decode the original captured bytes.
