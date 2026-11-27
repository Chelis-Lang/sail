# Original Kani/Rust payload preparation

Status: corrected source/data, native component and full native workspace gates pass;
complete originals are independently checked and externally retained. No selected CLI/backend, proof receipt, source
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
`4f107e13a121b7ea5239b599beebf390d35136a64a92111a3ea0fe88a0f089d4`
approves the bounded helper component after independently streaming every native
packet file and checking the original outcomes. Full workspace at `df6b7ec1`
passes 1,452 tests / 59 declared skips, fmt, Clippy, doctests, rustdoc, layering,
dependency policy and generated benchmarks. This is separate from frontend
acceptance.

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

All original captures are independently lead-rehashed and externally retained:

| Packet | Complete files | Archive SHA-256 |
|---|---:|---|
| Native materializer/download attempts 1–4 | 14,740 | deeb74278c85f69c63660aa38ab2d06b82cb20f7527bf1cc1245ee702a366dab |
| Corrected native downloader 5 | 7,159 | 34ef5e76e1b2337a914a2642ce5090789581c971500bd6a77a887c5fe5df34da |
| Exact source/full gate and initial gate setup failures | 2,961 | 261913977256827a47495be1373e43b576c6c633f39db3a3e3751bf72cb80e18 |

Their indexes are respectively
`01c30122151bd1e9e5210f93978c8a1a337aaebdc900935dd0cb0ebbac8221e8`,
`7374c0c21e86fa69258d4e79870ebf6b2c97403d861b4a289ecf8f484793e32c`
and `05c9a1cf9be68b19ab883ef32d1c22d49902f297891468a3e1c172c9d9b7d6bd`.
The gate packet includes complete source/Git metadata and logs; regenerable Docker
build intermediates are excluded. All 7,246 initial review files, including the
7,137 independently reconstructed payload bodies, and all 185 native-addendum
files are separately lead-verified and retained.

The original project/harness/lock files remain exact. The prepared Cargo home's
`.global-cache` database changes after use; its before/after hashes are preserved
by independent review. No immutable whole-input or source-closure claim follows.
The first gate image pull refuses unauthorized registry access. The second
builds the image but correctly rejects new files absent from the copied Git
index. The third uses the exact committed source and passes. An initial local
archive check started before its transfer finished fails; complete transfer and
full independent stream verification then pass. None of these attempts is
silently relabelled.
