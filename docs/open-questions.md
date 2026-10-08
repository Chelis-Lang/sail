# Open questions for the owner

Numbered and append-only. When a question is answered, record the answer
inline with the date and link the ADR that captures it.

1. **License.** The workspace has no license set, and `publish = false`. Which
   license should `sail` use, and when?
2. **Reference platform.** Baselines are first measured on the lead's machine
   (macOS arm64). Is macOS arm64 the reference platform for published
   benchmarks, or should Linux x86-64 be? This affects which Verus and Z3
   artifacts are pinned as primary.
