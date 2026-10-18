# Borrowed receipt checksum candidate

| Field | Status |
|---|---|
| Issue | #156, remaining managed overhead |
| Source | Original candidate `9d0a59d1`; production code ported as `0bc5724a` on optional-plan candidate `e130f632`, based on main `3892881a` |
| Qualification | Mac five focused controls and all-target Clippy pass on `c66049a0`; matched native Vest screen passes all 36 calls on `7ced406f`; full delivery gates not pursued |
| Selection | **PARKED / unselected**: no useful latency gain in the completed matched screen |
| Scope | Receipt checksum serialization only; no dependency, schema, key or verdict-policy change |

The candidate borrows every persisted receipt field and serializes it in the
existing order, representing `checksum` as the empty string. A bounded buffer
feeds SHA-256 instead of cloning the embedded report and allocating the entire
serialized receipt. Exhaustive receipt destructuring requires any future field
addition to make an explicit checksum decision. Serialization failure retains
the original empty-bytes digest behavior.

The compatibility oracle independently uses the original algorithm: clone the
receipt, empty its checksum, serialize it with the derived `Serialize`
implementation, then hash those bytes. Controls include the existing rich
report golden, nested portfolio and engine records, escaped and non-ASCII
strings, maps, declared environment absence and raw values, and a non-UTF path
serialization failure. Existing corruption, commit, context, environment and
artifact checks are retained. Full receipt parsing and content reads remain
unchanged; this candidate carries no file digest across observations.

## Matched native screen: parked

On 2026-10-18, one quiet c7i.4xlarge (16 vCPU, us-east-1) completed all
36 required calls with zero unrun or refused rows: three rotated rounds,
three actors and A1 cold / B edit / A2 return / A3 unchanged. Candidate
`7ced406f` has the exact production sources tested as `c66049a0`; the
baseline uses the actual executed e0 Sail/shim bodies from warm archive
`087ac26c877ff0af15d8c906d80fc1b032fcb295d31ed73ef27d79e5e07087bb`.
The third actor is genuine stock `426d8b0`. Both Sail actors use the same
e536 normal runtime archive `689004c0`, ordinary current-pin Vest source
and public dependency lock. Sail uses plain default verbosity; stock requests
`--output-json --time`. Source, runtime, binary, complete function/verdict,
diagnostic, Current, original establishing-run, returned-A/no-op verifier-count
and complete whole-owned growth guards all pass.

Wall seconds below are median [minimum, maximum], with all three samples kept:

| State | Stock | Baseline e0 | Checksum candidate |
|---|---|---|---|
| A1 cold | 82.556 [82.390, 83.149] | 94.943 [94.879, 95.132] | 94.732 [94.651, 95.448] |
| B edit | 45.757 [45.519, 46.117] | 33.936 [33.558, 34.053] | 33.942 [33.851, 34.006] |
| A2 return | 45.780 [45.647, 45.792] | 2.931 [2.913, 2.945] | 2.937 [2.924, 2.950] |
| A3 unchanged | 0.236 [0.219, 0.247] | 0.577 [0.572, 0.577] | 0.588 [0.580, 0.590] |
| Four CLI calls | 174.712 [173.775, 174.921] | 132.337 [132.193, 132.486] | 132.182 [132.123, 132.894] |
| Observed cycle, including preparation/validation | 182.515 [181.468, 182.742] | 147.561 [147.077, 148.126] | 147.378 [146.984, 147.536] |

The four-call median changes by about 0.117%, and the broader observed cycle
by about 0.124%, with overlapping ranges. A3 is 1.85% slower; every candidate
A3 sample is slower than every baseline A3 sample. Cold, edit and return
medians are essentially unchanged. This is negative evidence for pursuing
this optimization for user-visible latency. It remains parked and unselected;
there is no general CPU, memory, default or full-gate qualification claim, and
no further micro-tuning or broad qualification is planned.

Every command, report, receipt, certificate, manifest, compiler-read document
and raw stream is retained. Regenerable compiled graphs/blobs have exact
byte-hash/mode/size indexes rather than full bodies: this is performance
evidence, **not independent artifact-restoration qualification**. The complete
per-row and whole-owned growth censuses remain recorded. Both full archives
fit the prospective 2 GiB raw / 512 MiB compressed caps without truncation.

External evidence is under
`receipt-checksum-matched-native-2026-10-18/compact-caps/attempt01/`:

- `native-results/final.tgz`: SHA-256
  `b3d01de03b00eb2f57a24f01d7076356120c34b0061765bec784435ec0185d45`.
- `selected-final/evidence/performance/status.json`: SHA-256
  `1a988436880c1ee43e520ea48cbb7d1d3ee1719e304b70c5af9ff83917e152c3`.
- `actual36-summary.json`, all samples/ranges and observed growth: SHA-256
  `72215c3442c74be2608d3e5a8b33a4e694a9c33f4966c5e5454677f5fe8667dc`.

The earlier optional-plan result qualifies only that older implementation.
This comparison adds no acceptance to old `7090` or to the checksum candidate.

## Focused Mac checks

On 2026-10-18, source `c66049a0` passed all five receipt compatibility,
corruption and publication controls, followed by all-target Clippy with warnings
denied, using Rust 1.99.0 on native macOS arm64. The complete bounded run took
76.71 s. It used one compiler job, retained the original serialization oracle,
and left source, protected runtime and evidence bodies unchanged. This is a
focused correctness result, with no performance claim or full-gate acceptance.
The exact command, actual unit-binary hash and complete logs are retained in
external `receipt-checksum-current-2026-10-18/mac-focused-02/`; its result
SHA-256 is `97f063c44a8f9aa655308a400b085d74adb3624106f08d451a35da110e328805`.

## Indexed interrupted attempts

- `0bc5724a`, 2026-10-18: the first Mac focused compile was stopped at its
  operating disk floor after 22.9 s. The five receipt controls and Clippy are
  unrun. Source and 3,290 protected files remain unchanged, and the compiler
  is reaped. The final free-space guard also fails; no product result is
  inferred. External `receipt-checksum-current-2026-10-18/mac-focused-01/result.json`
  has SHA-256 `c0e37c92496ff299820699d2f926b877e5afedd68c8565535df4417e3c807496`.

This is an internal implementation candidate, with no command, configuration,
output or supported-workflow change. The existing
[managed-overhead record](managed-overhead.md) retains its original source and
measurement scope.
