# Requirement-attribution disposition

Status: five-function historical hand join complete; corrected active-entry
retirement qualifies on native Linux and receives independent approval at
`b8ec953b4e81d77ed27e655f6103645488d019ec`. This change preserves those exact
executable, test, lock and checker bytes; final documentation records the actual
results. Tracks #712 F / ADR0148. No outside-user benefit, CI demand, requirements
correctness, performance or release acceptance is claimed.

## Original hand join

The table reuses the first full baseline observation and selected catalogue
mutants from Sail `13b3311c` / normal Verus `5d034994`, runtime manifest
`36020404a456d3fd2af49c0c421cbecc0bf84441ac92f17a790f798b0782f9de`, on native
c7i.4xlarge/us-east-2. Tags below are illustrative unchecked grouping keys,
not user requirements. Margin is `100 * (1 - max(used/limit))` over the
non-advisory checks in that baseline observation, not a repeated stability or
performance claim. Functions and campaigns remain historical.

| Tag | Exact function | Baseline | Observed margin | Selected mutant evidence |
|---|---|---|---:|---|
| DEMO-1 | `ironsht::single_delivery_state_v::CSingleDelivery::send_single_cmessage` | Verified | 53.894809% | Arithmetic and constant mutations rejected by Verus |
| DEMO-2 | `ironsht::host_impl_v::HostState::parse_end_points` | Verified | 99.789823% | Off-by-one mutation rejected by Verus |
| DEMO-3 | `ironsht::cmessage_v::CMessage::serialized_size` | Verified | 99.232073% | No selected mutant; no mutation outcome follows |
| DEMO-4 | `vest_lib::combinators::star::exec::prepare_slice` | Verified | 98.522053% | Off-by-one mutation rejected by Verus |
| DEMO-5 | `vest_lib::cbor::format::parse_cbor_with_child` | Unknown / unstable | 0% | No selected mutant; no proof or mutation tier follows |

Each admitted rejection retains two repetitions and full confirmation in its
original campaign. Other unselected/unsupported edits remain distinct; a
missing selected mutant is neither a survivor nor a zero score. The grouping
changes none of these per-function observations. There is no outside-user
specification change or request for grouped CI output in this evidence. The
stated bar is therefore unmet; ADR0148 applies the planned active retirement.

All 26,616 original campaign payloads were independently lead-rehashed and
retained, archive SHA256
`e390defb1d8dee57d736836aecb6cb7059fc9388f53b900a9659508967fa2dc9`.
The table's complete exact identities, checks, mutation/detail bindings and
first-baseline scope are retained outside the repository as JSON, SHA256
`8d86767c1359e6d0905faa054408a721e09199b0597acce6ef014202244ef1db`.
The two original baseline stdout payloads are separately rehashed against that
archive/index: IronKV `486005598ed243e1996e7774f0063f3f2985484675c870ff2afdb9366b84f83c`;
Vest `67529d70ac2d81c9671c34f3207524d3afe63c4c2b008f46ee64d25d55b0d626`.
This is a hand join over existing native results, not a fresh verifier run or
whole-project acceptance.

## Retirement gates

Native c7i.4xlarge/us-east-2 qualification passes the complete workspace gate
(1,443 tests, 59 declared skips), all 55 pinned-solver tests, full local E2E
including Extra, all managed/copies/native/scripts paths and repair, plus all
four PR conformance arms. Each arm reports 33 Same / 311 functions, zero skipped
projects, unstable, different or infrastructure cases; seven units retain
resource-counter drift under the unchanged oracle. Source/book/build/link checks
pass; optional Node highlighting was unavailable in the native review image.

Independent actual-executable controls cover 56 before-`--` flag cases with no
worker/cache/project effects, four after-`--` controls reaching the stand-in,
four assurance/help cases, 21 MCP removed-input controls and three retired-tool
calls. Thirteen legacy raw RPCs refuse before work. Current SDK calls cannot
silently fall back; six older-service raw-wire scenarios retain negotiation and
canonical legacy payloads. Three actual service boots preserve original result
and tagged journal bytes and execute the historical ordinary worker only once.
Ordinary untagged jobs, function evidence, health and runtime tools remain.

All 35,899 original material payloads and five symlink records are independently
lead-verified and externally retained. Archive SHA256
`d79c16ddd580a9c8dc11519f427c6c1656ab1e4bf22ab2e423431f623acb5fa2`;
index `98bab50350d20a443662979c2d242a60b4f9e77aca390583d457bbcc46e5ef3a`;
review report `2ff48b4754d1a0b02c7529c33373e10301ccc98226142d387055d2097840a9ed`.
The original nanosecond file metadata stays in the index; tar float timestamps
round by at most 237 ns. This is evidence retention, not a closed runtime.

ADR0087 classification: optional grouping retirement and early refusal; ordinary
untagged solver interaction, replies, cache keys/reuse, verdict/diagnostic
production and the differential oracle remain unchanged. Full fast/standard/
deep corpus, model/refinement, performance, Mac and temporal suites were not
rerun for this bounded change and are not marked passed. Exact corpus entries
are the 33-unit PR subset; no entry source or pin changes. Final required PR CI
and post-merge green state remain separate from these local results.

## Retained attempts

| Source / attempt | Actual result |
|---|---|
| 5c0e7480 / native1 | Wrapper names the directory sail-cli rather than package sail-solve; Cargo refuses before compilation. Corrected wrapper retains the same source and inputs. |
| 5c0e7480 / native2 | Affected Clippy refuses an unused import left by deleted assurance rendering and an overlong migration test. Successor removes the import and extracts identical fixture construction; original assertions remain. Tests are not reached. |
| f896b6d7 / native3 | The import cleanup also removes the existing function-local fmt::Write import, causing two comparison-renderer compilation errors. Successor restores that local import only; no assertion or behavior is changed. |
| 34b66478 / native4 | Affected Clippy and API/CLI/MCP library tests pass. The new service test incorrectly expects Response::Error from Client::call, which correctly returns io::Error instead. Successor checks the same exact SAIL3161 refusal and no-worker/journal invariants through that actual API contract. |
| fcf99002 / native5 | Affected Clippy, 46 API, 70 CLI and 75 MCP library tests, plus the service historical-journal/no-worker integration test pass. This is focused Linux evidence, not the full gate. |
| 5c88c598 / independent review | Rejected: full gate finds a stale documentation anchor; supplemental nextest finds stale CLI-help and installed-MCP contracts; legacy E2E finds the same removed MCP tool. Fragility/portfolio routing bypasses the managed-only retirement guard and prepares work. Solver and all four PR conformance arms pass; remaining supplemental review evidence is retained separately. |
| 63816f00 / fixed-native1 | Clippy rejects the new final help-refusal empty-output assertion under assert_eq lint; tests are not reached. Successor uses the same exact empty byte expectation with assert_eq. |
| 2467a039 / fixed-native2 | Affected Clippy and all twelve verify-mode refusal/no-artifact controls pass. The final new help assertion incorrectly expects SAIL3161 although removed help topics correctly use SAIL0002; successor checks that exact unknown-help contract and separately checks actual assurance calls refuse with SAIL3161. |

The successor rejects retired flags at the public verify entry before mode
routing (preserving tokens after `--`), updates exact active installed-MCP and
help contracts to the intended retirement, and fixes the legacy error migration
link/action. Its integration control exercises both option spellings across
ordinary, background, engine, fragility, portfolio and standalone requests,
requiring the same retirement code and no project/cache artifacts. These fixes
qualify in the complete native successor review recorded above.

## Independent successor review integrity

The review's focused-wrapper restart races gate completion and produces an
invalid mixed/sparse aggregate log and a container collision. Original failures,
surviving complete per-case payloads and the explicitly reconstructed missing
wrapper remain retained. The authoritative valid3 run freshly compiles its
independent harness and repeats every unchanged assertion under separate names,
outputs and temporary directories, exit 0. Both earlier strict product failures
and reviewer setup mistakes stay visible; no assertion or suite is waived.
The original 5c88 rejection archive remains SHA256
`afc8a6a58de8d11a43b32135cd05a14b1939b660b4cf9095a91e5aca4ffd4ac5`.
