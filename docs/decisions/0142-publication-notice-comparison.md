# 0142: Compare exact optional publication explanations separately

- **Status:** accepted
- **Date:** 2026-11-14

## Context

The mutation benchmark requests `SAIL_EXPLAIN=1` and already separates named
Sail operational notices from exact Verus diagnostics. Main #745 adds sixteen
bounded publication-decline reasons. The native standard HumanEval sweep on
`efff77ec` consequently reports extra stderr despite matching outcomes in the
inspected samples. This is main bug #748; its failed attempt remains retained.

## Decision

Share the production publication stage vocabulary with the benchmark through
`sail_verus::managed::publication::Decline::from_note`. Accept only the exact
human note prefix, reason, suffix and canonical positive u64 operation count.
Recognize at most one publication explanation per fixed stage per invocation;
two different counts for one stage are still duplicate notices and fail.

Unknown text, invalid counts, altered suffixes, control characters and all
other verifier/progress diagnostics retain their existing strict handling.
Line splitting removes at most one LF or CRLF terminator. A lone terminal CR
or repeated CR bytes remain part of the diagnostic and cannot be hidden.
The original stderr stays captured. Production note wording, emission policy,
verification, receipt admission and solver input are unchanged. This is an
operational-notice schema update, not a Verus diagnostic exception.

## Consequences

Production and comparison use one reason vocabulary. Exact negative controls
and the owning mutation sweep must qualify this correction. Earlier failed
runs cannot be relabelled passed; source-identical corpus evidence must be
identified separately from a newly executed corrected sweep.
