# Validated freshness request review

Independent static review of candidate
`9ae6056247808e50e435a17e6e5b1915510885a5` found no authority or resource
blocker. The actual `jobs/store.rs` SHA-256 is
`8c9d482da07e6bd2ce8b39f760466b56f531ae0eaf103cdfefb061395a3307d4`;
`jobs/worker.rs` is
`57cb2666833e0c928d746d8ed63e3badfecea659394dc1e4279ac3c6929712c2`.
Both match the candidate's frozen source record.

Only successful `execute` supplies the already parsed document, after its
existing process-exit, report-error, kind and version checks. Repair uses the
FULL document. The worker borrows it for the same synchronous freshness
establishment before terminal publication; no parsed graph clone, extra owner,
FIFO change or cancellation change is introduced. Cache selection still uses
the original submission and project directory, with the exact document
workspace/context and original submission environment.

This selects an independent input-freshness request. It does not attest retained
output bytes: completed Result, Status and frame paths keep their original
projection seal. Corrupted or substituted stdout cannot retarget the borrowed
request and remains refused by those paths. Exact restored bytes with missing
input-cache identity still yield Unknown.

The added controls exercise the actual bounded document reader and store seam,
including repair FULL versus distinct focus identity, malformed initial input,
no second read, independent request fields and RPC seal refusals. They do not
spawn a full `execute` process and do not substitute a synthetic Current input
oracle. Native compilation, strict checks and product measurements remain
pending for these reviewed bytes. Avoided parsing is a concrete seam; its effect
on the original 120-second fixture is unmeasured.
