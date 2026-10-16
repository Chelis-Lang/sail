# Native-artifact store quarantine

**Status:** containment policy accepted; integrated tests and ordinary gates
pending. No release or runtime pin changes.

[ADR 0053](../decisions/0053-disable-unqualified-native-store.md) disables
v1 dependency-store reads and publication until native input identity
is qualified. This addresses the mechanism behind
[#187](https://github.com/Chelis-Lang/sail/issues/187) by making fresh targets
execute normal native compilation.

## Native assessment

On 2026-10-16, a dedicated EC2 c7i.4xlarge (16 vCPUs, us-east-1) compared actual
release binaries from baseline `3c0498c3` and containment candidate `19034f8d`,
using the original pinned runtime. Full source and runtime inventories and
binary identities were checked before and after. Both binaries were built
normally from their exact sources. Each round retained a successful donor,
fresh target, same-PATH compiler replacement, independent fresh stock failure
and stock fresh-project cost. All 48 commands and every raw output were kept.

| Round | V1 seeded fresh project | Containment fresh project | Fresh stock |
|---|---:|---:|---:|
| 1 | 1.2819 s | 32.8740 s | 34.3641 s |
| 2 | 1.2864 s | 32.5718 s | 34.8324 s |
| 3 | 1.2803 s | 32.0525 s | 34.3114 s |
| Median | 1.2819 s | 32.5718 s | 34.3641 s |

In all three rounds, replacing the private `cc` at the unchanged `PATH` made
fresh stock and the containment candidate exit 101 with the replacement's
literal diagnostic. V1 still exited 0 and reported Current. Positive store
capacity could not re-enable the candidate. Final source, binary and runtime
guards passed; owned scratch cleanup completed. The assessment itself remained
nonzero and unqualified because its unchanged default-enabled unit test was
red (654 tests passed, one failed, 35 skipped). It did not waive that gate.

Raw probe archive SHA-256
`0430cf3843d13a550b068403a898ffeccdc8e1afe66b2a95a11f6d56bf1c6ed9`;
complete assessment archive SHA-256
`9cc057c0e8ac331b8ecfb1185c800df935fa3452d85a1ef6cc3835840bd843f3`.
The original macOS reproduction remains cited by ADR 0053. This native
assessment is a bounded small-project comparison, not a whole-corpus or
full-output-equivalence qualification.

## Integration and remaining work

After seeing the safety reproduction and cost, the owner authorized repair
sequencing while requiring real performance wins. The integrated checks now
require disabled publication/reuse even with positive capacity, fresh vstd
compilation in new targets, correct changed-compiler failure, preserved
per-function verdicts, and subsequent reuse of each project's own receipts.
The old requirement and its red assessment remain visible above. No
conformance comparator or benchmark baseline is changed.

Full native gate, solver tests, every end-to-end phase plus extra, full FAST
and STANDARD conformance, and independent review of the integrated source are
still pending. Earlier assessment results do not qualify the later test and
documentation changes. A sound fast replacement and first-run speed recovery
remain open work; containment alone does not complete them.

| Source / date | Attempt | Result and limit |
|---|---|---|
| `19034f8d`, 2026-10-16 | Initial native assessment | Default-enabled unit test red; controller refused its actual indented failure format before any cost probe. Raw gate SHA-256 `a642953da8426972bc6def7d2e369bda0cbed9db7c65081e94208c905ea2ad3c`. |
| `19034f8d`, 2026-10-16 | Corrected assessment controller | Same gate remained red; all three reproduction/cost rounds completed, with final identity guards. No merge qualification. |
