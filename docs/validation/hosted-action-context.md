# Hosted Action context evidence

Status: the hosted control's runtime difference is established (#737); fixture
correction is implemented, hosted acceptance pending. The original exact-cache,
zero-verifier and verdict assertions remain.

On main `df49f95f`, hosted run 43286029482 restores all three exact current
cache keys. Both fixture functions verify, but both cold and restored arms
invoke two verifiers and reuse no target. The input revision is identical;
the context differs. That initial result alone does not identify which context input changed. All six required checks pass; the hosted
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
acceptance. The later hosted comparison below establishes the differing tool identities. The consumer
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

## Hosted tool identity and fixture correction

Diagnostic run 43293225697 produces both complete context records. Cold image
20261101.360.1 and restored image 20261111.370.1 have the same 147 identity
paths, the same recorded caller selector hashes and the same source revision.
The only differing identity entries are the Rustup-backed `cargo`, `rustc`
and `rustup` proxies. Their hashes are respectively
`5be191e8c70a5007074254cb74d78dbfdd25cd5a61c27de7b8e5410122ec24f4`
and `b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55`.
The changed executable bytes are sufficient to require a new proof context;
restoring a cache does not authorize reusing receipts across that change.

Original context record SHA256 values: cold
`c0eed9ea0765e1aa3a0ed3005d84299b02f4ece29131e387b288bc4590e7ce16`,
restored `954b63a479c88ccffab151f7102cde7393944e986fdfb7ebe20fb561efeb7793`.
All original failures stay failures.

The Linux hosted cache fixture now installs official Rustup 1.29.0 in both
arms and disables self-update before installing its existing pinned Rust
toolchains. The [official archive method](https://rust-lang.github.io/rustup/installation/other.html#manual-installation)
supports selecting an exact version. Installer and all three resolved proxy
paths must match SHA256
`4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10`.
This configures the acceptance fixture only. Consumer installation, default
proof-context hashing and all reuse guards are unchanged. An isolated native
bootstrap with private Cargo/Rustup homes passes and confirms all three
installed proxy identities. Hosted zero-invocation acceptance remains pending.

Native private-home controls also replace an existing proxy with actual
`b1c8cb14...` bytes and confirm all three resulting identities equal the
pinned installer. A fake downloader supplies corrupt bytes; the exact helper
rejects the checksum before creating any installed Rustup file. The host's
real Cargo/Rustup homes are not modified by these controls.
