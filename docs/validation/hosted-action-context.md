# Hosted Action context evidence

Status: the original hosted proof-receipt reuse failure is unresolved (#737).
The collector adds diagnostic evidence; it is not a reuse fix or acceptance
waiver. The existing exact-cache, zero-verifier and verdict assertions remain.

On main `df49f95f`, hosted run 43286029482 restores all three exact current
cache keys. Both fixture functions verify, but both cold and restored arms
invoke two verifiers and reuse no target. The input revision is identical;
the context differs. No particular tool, environment variable or timestamp
change is established as the cause. All six required checks pass; the hosted
Action failure keeps the overall workflow red.

| Arm | Context SHA256 | Result |
|---|---|---|
| Cold | `6719c4c9b1ecb25a58161e92bbf6557483aa0e9386f5fc346c869356d4e60433` | Verified; 2 invocations / 0 reused |
| Restored | `2771130dc2cdc25de15aeaf1d763b6a8c99dcaf07b439a26148330d13cded6c5` | Verified; 2 / 0; strict reuse check fails |

The original result artifacts and complete failed/job logs remain external.
The diagnostic collector records only bounded runtime identity paths/digests
and hashes of named noncredential caller selector values. It does not copy
source environments, proof receipts or cache entries. Missing or malformed
metadata stays explicitly unavailable and cannot authorize proof reuse.
Both hosted arms upload their own context record, including on check failure.

Native checks ran on c7i.4xlarge, us-west-2, Linux x86-64. The full local
`ci/local.sh gate` passes, including all 1,393 workspace tests, clippy, docs,
layering and dependency policy. Book source/build/link checks pass. The
wrapper's final optional bytecode-file write fails on a root-owned generated
cache directory; this is a setup failure after those completed checks, not a
passed wrapper. Original log SHA256:
`18914b9e375c9dfed8b250335c04e38dd2b530e655f5ae224d27479371b7b426`.

A separate native probe reads an original retained compiler context with 348
identity entries and correctly refuses a traversal-shaped context, FIFO and
linked result file within the existing short supervision bound. Collector
source SHA256:
`2a33ca1fa60d01e328b91c272d65a67e5d6ba4c2109ffb2a50c3e49bc87fdbf8`.
This is metadata/collector validation, not new compiler, corpus or performance
acceptance. Final hosted diagnostic comparison remains pending. The consumer
skill and CLI/MCP behavior are unchanged.

The first hosted collector attempt records both contexts as unavailable: the
ordinary vstd-inclusive result exceeds its initial 2 MiB metadata limit. Keep
that failure visible. The corrected reader uses the explicit 64 MiB project
result bound for the original result document and retains the 2 MiB cap for
identity/link metadata. Regular-file, no-follow, nonblocking and stable-size
checks remain. This changes diagnostic availability only; the original hosted
zero-verifier failure remains a failure.

The corrected collector reads an original 2,608,947-byte native result with
333 runtime identities successfully, under a five-second external bound. Its
source SHA256 is
`3fbea8258cd28874f2c642f1fbf23c67cd8e1e91b874e5c92761173dbbde6253`.
The Rust/product source is unchanged from the completed full local gate; this
follow-up changes only the collector's result-file allowance and runs its
affected native metadata check.
