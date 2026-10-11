# Complete fresh-result cost audit

The existing staged SDK decoding reuse reduces repeated whole-frame parsing,
but its retained Linux-emulated four-reader replay still measured 126.33 ms
median and 209.76 ms p95. That eight-sample offline replay exceeds the separate
200 ms live target. It does not establish live reader acceptance. The
[staged-client record](m7-staged-result-client.json) retains all samples,
corrected build provenance and earlier failed attempts.

[This audit](m7-fresh-frame-hot-cost-audit.json) binds the inspected source and
file hashes. On the daemon's established Result path, each request reads the
complete bounded worker output, verifies its validated SHA-256 seal and performs
fresh input/runtime inspection. An encoding-cache hit then selects one complete
preencoded Current, Historical or Unknown frame through an Arc. It does not
rebuild or serialize the function graph. Status likewise checks the output seal
and fresh inspection without parsing that graph. Failed output validation
evicts the encoding entry. Cache misses and older records still use full
validation and encoding. Adding another immutable encoding memo would duplicate
the existing hot-path mechanism; there is no measured hot serialization to
remove in these records.

The SDK receives the complete fresh body before comparing it to an entry
captured at request entry. Changed bytes use the ordinary versioned parser.
Repeated exact bodies return an independent complete Response clone. The
[component record](m7-result-client-components.json) measured four-reader body
allocation/copy at 0.96 ms median / 1.15 ms p95, entire-body equality at
0.34 / 0.84 ms, and independent Response cloning at 17.75 / 28.00 ms under Linux
emulation. The typed-read component's 3.512-second tail remains recorded. These
component measurements and the whole-client replay came from different runs;
subtracting them cannot identify the remaining whole-call cost or prove a
scheduling cause. They do not justify a new buffer pool or a byte-comparison
shortcut. Independent graph ownership remains part of the API contract.

The [client guide](../result-client.md) describes its 128 MiB charged retained
ownership bound across one family, including in-flight old entries. The server's
separate encoding table limits sixteen retained frame sets and 128 MiB of wire
lengths. A frame held by a sender can outlive table eviction; that table bound
is not a process RSS or aggregate in-flight ownership claim. Neither cache
authorizes Current, omits dependencies, or waives fresh RPC validation.

No production change follows from this audit. Attribution of a remaining hot
cost needs measurements of the same current service calls, with complete
responses and actual freshness inspections. The original Python census failure,
Rust-not-reached result and [M7 roadmap](../ROADMAP.md) acceptance requirements
remain unchanged. A future result must retain all four readers, 200 samples,
the 200 ms p95 goal and original RPC limits.
