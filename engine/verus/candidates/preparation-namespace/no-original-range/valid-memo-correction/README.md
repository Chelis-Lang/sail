# Admitted stale memo fixture correction

The [previous source scope](../test-integration-correction/source-failure.json)
remains failed. Its new fixture supplied a non-64-hex string, correctly refused
by the existing memo admission, then unwrapped an absent entry and poisoned the
shared test lock. This correction changes only that new test fixture to a valid
64-hex stale digest and adds explicit successful insertion assertions before the
production provenance call. The control still requires fresh provenance to
exist and differ from the admitted stale value, and requires the stale entry to
remain intact. Production guards, memo bypass, authority, domain, budgets,
broker retirement and existing role assertions are unchanged.

`identity.json` binds the two changed test-only source files, full candidate,
narrow delta and complete source controller. Narrow application and full reverse
application were checked. The controller retains the second failed scope as a
required input and launches all unchanged serial library cases and both strict
checks in the closed private target with two compiler jobs. This is source
validation only; ordinary runtime build and original qualification remain
separate. After a positive scope, a separate targeted counterfactual will remove
only the production absence memo bypass, demonstrating that the successfully
admitted stale entry makes the unchanged inequality fail. No counterfactual has
run yet. The runtime stays unselected and the source feature experimental.

Actual corrected complete source controls now passed: VIR 42/42, driver 108/108 and both strict all-target checks, 37.197 seconds. `source-controls.json` retains both tested executables, raw phase logs, unchanged source/lock and four prior runtime inventories, plus zero-holder dual-container closure. This remains a source result, without a new ordinary runtime or previous-revision restore. The separately frozen `memo-bypass-counterfactual/` plan removes only the absence bypass for one targeted case, requires the existing inequality to fail with the admitted digest on both sides, restores the exact positive source, then repeats that one case. It has not run yet.
