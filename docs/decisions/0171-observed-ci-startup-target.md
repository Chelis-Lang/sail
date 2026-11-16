# 0171: Accept the observed CI image startup target

- **Status:** accepted
- **Date:** 2026-11-16

## Context

The v0.3.0 plan in [#712](https://github.com/Chelis-Lang/sail/issues/712)
sets a CI image pull p95 target of at most two minutes for
[#195](https://github.com/Chelis-Lang/sail/issues/195). Earlier slow starts
remain real; their registry, network or runner cause is unresolved.
Packaging parity and a smaller image alone did not establish a latency gain.

The [startup record](../validation/ci-image-startup.md) now includes all
43 main/push CI runs created from November 10 through November 15 UTC,
all job attempts, and all 570 successful container initialization intervals.
For the current image variant, all 195 actual pull logs bind the intended
image tag, manifest and successful pull status.

The population is retrospective: its calendar window was chosen after a
single-run observation and before collection of the complete window.
It is not a preregistered benchmark or a guarantee about future tails.

## Decision

Accept the bounded observed operational target and close #195 on that basis.
For all 195 current-image samples, the time from the logged Docker pull
command to the next logged matching Docker create command has nearest-rank
p95 **40.2 seconds** and maximum **60.9 seconds**. Sequential runner commands
make this a conservative observed upper bound including pull completion
and subsequent command overhead, rather than a direct process-exit timer.
Every sample is below the 120-second target.

Retain every attempt and historical slow start. Make no causal claim that
image-size changes produced the observed times, no empty-layer-cache claim,
and no permanent registry or runner latency guarantee. Three separately
reported later successful workflows supply 39 whole initialization intervals;
they do not extend the original pull population.

This measured disposition uses charter appendix entry 18's engineering
delegation and independent review. It changes no workflow, check, timeout,
baseline, selected toolchain or correctness condition.

## Consequences

The observed startup target no longer blocks v0.3.0. Reopen investigation
if actual slow starts recur, retaining their raw logs and complete job
population. The earlier unexplained tails remain documented. Other cold-path
performance gates, Mac validation and exact-candidate publication approval
are unchanged.
