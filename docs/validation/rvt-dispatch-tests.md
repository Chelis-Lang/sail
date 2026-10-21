# Deep source-suite dispatch regressions

**Status:** complete native source-suite run passes with the corrections.

Source `d8e62b38`, native c7i.8xlarge in us-east-1, runtime f8589ed4 on
upstream 7d82994; fork test variant `7d829946…-5c434c1cc6390ee4`.
`ci/local.sh deep-rvt cache` exits zero.

| Arm | Result |
|---|---|
| Fork pass-through | 4,901 Same; 4,838 nonignored libtests pass |
| Fork cache fill | 4,901 Same; 4,838 nonignored libtests pass |
| Fork cache warm | 4,901 Same; 4,838 nonignored libtests pass |
| Stock control | 4,864 Same, 3 stock-unstable; zero Different or infrastructure errors |

Every arm has 63 ignored cases. The stock control retains two existing stock
libtest failures; the fork has none. Stock-only retries reproduce unordered
internal debug output in the three unstable cases. No additional comparison
exception was added for them.

The runner avoids injecting a duplicate progress option into its exact pinned
test. The owner-approved temporary-log-path exception is limited to six random
characters in that exact test's rerun command (ADR 0042). Actual query-byte and
other test assertions remain intact.

- Failed attempt, 2026-10-21, composed source `36cbfdec`: first fork arm
  4,899/4,901 Same, duplicate-option and random-log-path failures; later arms
  did not run. This attempt remains failed.
- Corrected complete run, 2026-10-21, `d8e62b38`: results above. Raw reports
  and logs: `experiments/2026-10-21/deep-rvt-complete.tgz` in the S3 evidence
  bucket, SHA-256
  `59a996cf0743835403e56302ffd0feefdf2790350ee03f83d56562bfe636777f`.

The final combined release still requires its corpus deep tier and installed
Linux/macOS benchmark qualification; this record does not substitute for them.
