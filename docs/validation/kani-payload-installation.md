# Original Kani/Rust payload preparation

Status: corrected source/data review and native component checks pass; full workspace
gate and complete original capture closure are pending. No selected CLI/backend, proof receipt, source
currentness, result cache, Mac or release acceptance follows.

The contributor materializer copies exact original component payloads without
executing `install.sh` or trusting generated Rustup metadata. Both original
bundle/Rust trees publish together through the existing Linux atomic no-replace
publisher. Immutable caller-owned inputs and a private parent remain preconditions;
hostile same-user mutation, crash recovery and durability are outside scope.
The separate downloader checks fixed original identities, HTTPS redirects,
bounded lengths and every digest before calling that materializer.

| Source | Actual scope and result |
|---|---|
| Materializer `152706d7b2c1567ebc013e356e7b0712b0dee2e9d7682baf75069077dc72fe33` | Seven local filesystem fixtures pass, including preservation of an existing or racing destination |
| Original validators | Unchanged seven Rust and eleven bundle tests pass |
| Downloader `a25f07ac15804fc33b3492789317fceaaf11c84b6500587676e800e5aab1071c` | Eight local and native fixtures pass, including intermediate downgrade refusal before contact |
| Independent source/data review | Seven reviewer negatives pass; every file in current and prior local prefixes matches a separate original-archive oracle |
| Native materializer + existing qualified components | Original tools authenticate, Stage creates/rechecks its exact route, three original arrayvec harnesses give expected bounded success, counterexample and insufficient bound; source files unchanged |
| Native downloader | Exact original network downloads and materialization pass; existing qualified Rust tools authenticate the resulting prefixes |

Data-only checks cover Rust 7,017 files / 1,586,050,167 bytes, canonical inventory
`f2fdf9b82a0521dd8c88fc10cef8e217cf42d855252786492564d82fa7cc4dbe`,
and Kani 120 files / 507,989,223 bytes. Local checks execute Python only.
The earlier executed helper snapshot
`3ebe199bcc349fbf592206bdc0c061090e37edf340ed1391adfd80e9d1a3f66e`
differs from the current materializer only in scope wording; independent review
also materializes the exact current source.

Native environment: x86-64 EC2 c7i.4xlarge, us-east-2, Ubuntu kernel
`7.0.0-1014-aws`. Rust inspector executables are the unchanged qualified
`9f379750fc49e8bb8d52f9ed358667f1f8aef580` build. This checks them against a
newly assembled original payload prefix; it is not a new workspace build or
CLI/direct-Kani conformance run. All observations keep proof and input-closure
authority false. Full arrayvec/direct comparison, cancellation/product integration,
Mac support and cache authority stay separate.

Independent factory review SHA-256
`90a20fbcecfbd5874bf064f593ee36c8b902399d90bed863810d414a7cbd3cd2`
approves only source/data materialization and requires native qualification
separately. The initial downloader was independently rejected because an HTTPS → HTTP →
HTTPS chain bypassed its final-URL guard. Corrected review
`16641298bd3345ea37be7d3481c83527b3847a62c50efafd1fb1199415e36f87`
approves the exact successor after 40 Python tests and 30 actual urllib redirect
cases. Every redirect is checked before following it. Native raw-packet review
remains pending.

| Attempt, 2026-11-15 | Result and cause |
|---|---|
| Native materializer 1 | Interrupted by user-service shutdown after SSH logout (`Linger=no`); shell status zero lacks completion evidence and is **not** a pass |
| Native materializer 2 | Pass after persistent user service is enabled and separately witnessed; exact source and original archive guards remain unchanged |
| Native downloader 3 | Correctly refuses nonprivate fixture parent with `SAIL6305` before network/materialization; invalid setup, not a producer failure |
| Native downloader 4 (`be8efb0b`) | Original download/materialization and authentication pass; source later rejected for an intermediate redirect downgrade, never treated as final helper acceptance |
| Native downloader 5 (`a25f07ac`) | Corrected-source fixtures, original network download/materialization and tool authentication pass |

The consumer skill is unchanged because no consumer command or backend becomes
available in this slice. ADR 0128 permits eventual fresh uncached bounded checks;
input-closure authority remains mandatory before result caching.
