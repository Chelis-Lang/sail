# Namespace refusal boundary observation

Status: file-only, unselected diagnostic candidate. Actual source compilation,
strict checks, feature-bound runtime construction and the separate diagnostic
remain pending. The original qualification remains
[failed](../visibility-correction/qualification-failure.json): eight passed,
one failed and four unattempted stages out of thirteen, using fourteen frozen
scripts. Its revision 0 had zero hits/checks and exact stock verdicts/stderr.

The existing import trace occurs after used-binding namespace validation, so it
cannot identify the first rejected binding. This delta adds failure-only records
at three exact boundaries, without changing the validator:

- Worker binding: borrowed full id/data/spelling and actual raw owner kind
  (`SpanData`, unit or unknown), including malformed/generated unit forms.
- Compiler validation: the unchanged validator's exact `CaptureFailure`, packed
  words, frozen classification role and current file count. The equal-raw-hash
  Foreign count is computed only when the current file namespace has at most
  4,096 entries, under the same borrowed guard. It is an observed namespace count,
  not a new authority or a claim that any matching file is eligible.
- Service admission: missing queue, busy active-owner admission, disconnected
  queue or failed Ready. The Ready event preserves its existing combined
  disconnection/poison failure classification.

Packed words correlate the main validator with the worker binding. A malformed
worker raw owner can fail before there is a main validation request. Main events
identify immutable late classification explicitly; they never substitute a
frozen role for successful current validation. This does not resolve, load,
register or clone any source file and does not inspect a current resolver.

The selector remains the existing `SAIL_ENGINE_IMPORT_TRACE=1`; the separately
proposed diagnostic leaves its target filter unset. Its exact launcher must bind
the one-byte selector and bounded observation path. Default qualification has no
trace selector. All new records share the existing 4,096-record/4 MiB limits,
32 KiB per-record cap, 512-byte unavailable reserve, sticky stop and admitted
borrowed writer under the emitter mutex. Oversized spelling/data or serialization
refusal publishes only unavailable, with no partial identity. No new buffer,
namespace owner, queue or observer allowance is introduced. The existing bounded
writer drops physically before its mutex/admission ends. Its allocation cap is
1 MiB, so adding the single LF cannot overflow `usize` on either pinned 64-bit
target; the checked-size error is an unreachable extra safe refusal.

Output is only the existing event sidecar; stdout, stderr, stock reporter FIFO,
compiler flags, key/domain, codec, raw bindings, role eligibility, validator,
current namespace guard and normal fail-safe return values remain unchanged.
Ordinary reporter source loading never runs under this guard. Mere instrumented
compilation or diagnostic execution does not establish preparation reuse.

Three additional source-only controls check borrowed exact identities (including
invalid unit/unknown owners), unchanged raw-owner/validator refusals and local
acceptance, and joint record/byte caps, one unavailable marker, skipped encoding
and publication failure. They have not compiled yet. Every preceding role,
memory, domain, broker and oracle assertion is retained. A separate availability
correction is being reviewed independently; it is not mixed into this delta.

`identity.json` binds the full and narrow patches, eight complete-source hashes
and unchanged source lock. Only the engine observation/adaptor file changes.
