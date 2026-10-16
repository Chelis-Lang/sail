# Original output failure retention

## Status

Diagnostics-only candidate: original stdout/stderr comparisons and exit-code
assertions are unchanged. No receipt, freshness, solver, verifier or memo
algorithm changes. Source `b0ddd460` passed its native workspace, solver, full
E2E and PR-subset gates, followed by full FAST and STANDARD conformance in
all four modes. Required PR checks are enforced separately. No corpus entries
or runtime pins change.

The source controls execute the actual shell harness with controlled child
programs. They check passing cleanup, an injected byte mismatch, original setup
child exit 23 (including a relative observer destination), collector failure
and simultaneous child/collector failure.
Filesystem controls retain binary stdout bytes, sources and producer records,
reject special/external selected files, and index bounded or incomplete copies.
These fixture results are not real Verus acceptance. The capacity correction
also preserves a JSON-sized output above 1 MiB exactly, rejects per-file output
above 8 MiB, and retains only the remaining shared-budget prefix when the 32 MiB
command-snapshot allowance is nearly exhausted. Refusal metadata stays in the
outer log without printing payload bytes.

## Native feature checkpoint

Source `b0ddd460` on main `3c0498c3`, native c7i.4xlarge in us-east-1,
passed the workspace gate (655 tests, 35 skipped, 2 doctests), all 35 solver
tests, all seven E2E phases plus local extra, and every PR conformance arm
(33 Same / 311 functions each; zero different, unstable, infrastructure errors
or rlimit drift). Actual native gate execution includes the snapshot filesystem
and six shell controls. The full E2E log contains no snapshot/command-recording
warnings, including the ordinary JSON cases that exceeded the former bound.

The unchanged MCP image context built its real pinned client; all 3,886 runtime
hashes and the client identity matched before and after. Four source observations
each retained 558 unchanged tracked files. This source-only queue included no
memo binaries or additional original-path investigation.

Raw full E2E archive SHA-256
`3a281d6e718d4ba58bbacfeaf7a55712932470b3dc64a50f95c56225cc602d13`;
final archive SHA-256
`15792fde09df46f9f4d51417e0715af6c6d2ebfd572b732daad622cbdabc32a9`;
outer final archive SHA-256
`c2ff2222873a850c99eda4eeadc4c8f6bd2e23b49f5dbfd08d1f729dc0a41462`.
Every actual member hash, type, mode and symlink target in all thirteen stage
archives was independently rechecked; verification summary SHA-256
`80e3b078572131e3cccbd9542a81b08a605b3b1a89e6daf7528ed53ccad9189b`.

- 2026-10-16, earlier `3372eb1f`: the same basic gates passed, but the 1 MiB
  duplicate-snapshot limit warned for `json-sailwarm` and `loader-json`. A live
  observation of the former recorded 1,631,095 stdout bytes, a 1,048,576-byte
  prefix, truncation and no concurrent change; only a tool-response transcription
  was retained before passing cleanup removed the fixture. The loader warning's
  cause remains unconfirmed. This motivated the 8 MiB/logging correction;
  raw E2E archive SHA-256
  `8188538e132b2aae9d79ef4cd828934f3f816825b25212ba568b5f0f529d1591`.

## Full conformance checkpoint

The same `b0ddd460` implementation on main `3c0498c3` then ran on a separate
native c7i.8xlarge in us-east-1, with the same pinned fork, stock oracle, Z3
and MCP image context. Every mode passed:

| Tier | Modes | Units matching stock per mode | Functions per mode |
|---|---|---|---|
| FAST | pass-through, cache fill, cache warm, engine | 147 / 147 | 3,450 |
| STANDARD | pass-through, cache fill, cache warm, engine | 159 / 159 | 12,726 |

All modes reported zero differences, unstable units, infrastructure errors and
skipped projects. STANDARD engine alone recorded one resource-count drift:
`verified-node-replication`, function
`lib::spec::cyclicbuffer::log_entry_alive_value_wrap_around`, 51,948 versus
51,945; its verdict and diagnostics matched. All other drift lists were empty.
The final source inventory (558 files), three binaries, 3,886 runtime files and
actual MCP client identity were unchanged. Controller and outer terminal
records completed successfully with no errors. These checks qualify this
observer; the unresolved memo candidate remains unaccepted.

Raw STANDARD engine archive SHA-256
`75a48501056a9caba73caa6db11876f8348005d43da1e3f62de321750d249305`;
final archive SHA-256
`5aed314234b5535b24017882123d2223c47fb3ee874283d1d6a13c676869f8cc`;
outer final archive SHA-256
`f7959784c0e4bf0bc40419753f75c8ee5829cb640112cb23bce633c43d521e65`.
The observed summary, including every arm and terminal result, has SHA-256
`c72f830de8ded02895bb093607a6edf68d9516c36d455598dba6952e09d17f9c`.

## One original-path investigation

An earlier bounded queue using observer source `3372eb1f` ran exactly one
original automatic-daemon loop using retained `9341e827` Sail/shim binaries. It passed in 46.55 seconds; the first
combined failing-proof outputs were exactly equal (347 bytes each, SHA-256
`880e50e8192bbf2d5409a3030a6dfb9a0fb0f1d713192429b30bccafe8641cd2`).
The post-outcome-only external observer retained 220 selected files with a
complete index, real proof triggers, unchanged original/staged binary identities
and exact original/generated-script reversal. It did not reproduce the original
disagreement. The memo candidate stays unaccepted; a passing investigation is
not algorithm qualification. Raw attempt archive SHA-256
`120fd020cafc246b431528de91dcfa2dc3da4b857907257d731889f925b68bad`.

## Unresolved original disagreement

On 2026-10-16, candidate `9341e827` passed the native workspace gate (642 tests,
35 skipped, 2 doctests) and solver tests (35), then failed the original output
E2E check: the failing proof's project summary appeared before Cargo's error in
plain combined output and after it in Sail. Cause unknown. Original temporary
streams were deleted by the previous trap; the retained log records the diff,
not the complete original producer streams. PR/full-fast/standard conformance
never started. Raw E2E archive SHA-256
`30dc726c06842b9ef26cda8ce774b2e104d438a8632949c6d522d48666f94c3aa52`;
failed-attempt index SHA-256
`40bca9722f9c53bb95f4354509f5a02c310a6902615d22a88db5b778b4b23ebc`.
The memo candidate remains unaccepted ([#174](https://github.com/Chelis-Lang/sail/issues/174)).

A separate bounded native c7i.4xlarge/us-east-1 experiment ran the actual
`9341e827` and preserved `b0eb7534` binaries against the pinned stock oracle:
81 retained rows (6 setup and 75 measured), all valid. Across 20 interleaved
combined-output rounds and 5 split-output rounds, all 40 combined and 10 split
Sail-to-plain comparisons matched exactly. Real proof-trigger admission,
source restoration and original/staged executable plus runtime inventories
passed. It did not reproduce the original failure and always recorded
`qualified_for_adoption=false`. Explicit owned daemons and the fixed proof-edit
workload differ from the original automatic-daemon/warmup path. No current-main
or newly built binary qualification follows from this experiment.

Raw final archive SHA-256
`da19a6ec5a883135f32a6439f80f9ab30ca20e97967c8a192d2dc86d500ce146`;
outer final archive SHA-256
`df1eb6ffbc812ed23189f1f1b890559b4866cc457c63a42c4cbd3dc64cdf4f51`;
independent raw comparison SHA-256
`c328c3bdff58840682756e173e1ec5943730636eae1192418b6fc0cf03eb3096`.

This observer retains original bytes and producer records without changing
what passes. No additional original-path runs are part of this qualification.
See the [artifact guide](../e2e-output-evidence.md) for limits and commands.
