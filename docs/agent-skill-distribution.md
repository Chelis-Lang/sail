# Agent skill distribution

The canonical consumer package is [skills/sail-verification](../skills/sail-verification/SKILL.md).
It teaches agents to use Sail; [AGENTS.md](../AGENTS.md) governs contributors.
The website links to this package instead of maintaining another set of
instructions. Supporting references travel inside the package, so installation
does not require a Sail source checkout.

Install it with:

```sh
npx skills add Chelis-Lang/sail --skill sail-verification
```

The skill does not install the Sail runtime. Use [installation](install.md) for
that. Read installed help and release provenance before choosing release-dependent
workflows; development builds can retain an older version string.

## Quality and maintenance

The description identifies Verus proof development and SMT work, including
people who ask for faster verification without knowing Sail. The body provides
the first useful action, then loads only the relevant workflow reference.
Preserve task-specific requirements such as proof currentness; do not copy a
reference manual, promise arbitrary Rust verification or add promotional slogans.

Check important commands against the implementation and tests. Validate the
frontmatter and test installation from a clean project. `scripts/check-links.py`
also rejects relative links that leave the skill folder: they would break when
an installer copies only that package. Its small regression test runs in the
existing workspace gate.

For meaningful revisions, use independent cold technical and devrel readers,
then test realistic prompts against the previous skill. Include proof iteration,
stale/partial evidence, repeated SMT requests and release-dependent file input.
Keep raw prompts, responses and execution traces outside the repository. Record
the source revision, runtime, actual result and limits in the PR. These model
evaluations are local review work, not CI jobs.

Standards and evaluation references:

- https://agentskills.io/specification
- https://agentskills.io/skill-creation/best-practices
- https://agentskills.io/skill-creation/evaluating-skills

## Publishing

GitHub is the source of truth. A directory listing points to this repository;
a registry-hosted bundle is exported from the merged skill folder with its MIT
license. Record the source commit, package version and returned listing/status
URL. Update hosted bundles from source when the skill changes, never by editing
their instructions separately. The skill's package version is distinct from
the Sail binary release.

Use a real installation to test usability. Do not fabricate installs, stars or
reviews. A submission acknowledgement is not a published listing. A listing is
not evidence that users completed verification successfully. Record these
outcomes separately; use install counts only where the service exposes them.

The canonical package describes the published Sail `0.2.0` workflows, including
standalone verification and repair, background preparation and cache budgets.
The refreshed exports below use merged source
`d8e6c3407d8194a6c39f25353364fe0eba52ea7e`; the skill metadata version remains
`1.0.0`. Registry wrapper versions are independent of both versions.

## Distribution landscape

Primary sources and actual registry operations checked on 4 November 2026.
Similar names below identify different services. Availability and publishing
requirements can change; verify the current flow before sending anything.

| Channel | Route and fit | Checked disposition |
| --- | --- | --- |
| [ClawHub](https://docs.openclaw.ai/clawhub) | Versioned folder upload with source provenance | Published `1.0.0`; normal CLI install passed and all four files match source |
| [Tessl](https://tessl.io/registry/jeffreykent/sail-verification) | Versioned hosted package | Updated to wrapper `1.0.3`; moderation passed and clean install matches source |
| [Agent Skill Hub](https://doc.agentskillhub.dev/guide/publishing) | Public GitHub import and source refresh API | Refreshed to `2026.11.04`; source revision, manifest and clean install verified |
| [skills.sh](https://www.skills.sh/docs/faq) | Public GitHub installation; genuine aggregate installs drive discovery | Clean GitHub install passed; owner-filtered search still returns no result |
| [Skillstore](https://skillstore.io/submit) | Anonymous skill-folder submission and automated review | Original listing is public; updated-source submission is in review |
| [Skill.Fish / MCP Market](https://github.com/knoxgraeme/skillfish) | One repository-submission channel serving both catalogs | Repository remains queued; resubmission reports already queued, search returns 403 |
| [Agent Skills CLI marketplace](https://www.agentskills.in/) | Repository discovery and CLI search | Search still returns the canonical skill, owner and package path |
| [Skills Directory](https://app.skillsdirectory.com/submit) | Account-based repository submission | Redirects to sign-in; no new account created |
| [SkillRouter](https://skillrouter.org/us/submit) | CLI repository submission after device pairing | Owner pairing requested; repository submission still requires login |
| [SkillsLLM](https://skillsllm.com/llms.txt) | GitHub submissions with a published 100-star minimum | Not eligible: Sail still has 1 star; do not manufacture stars |
| [SkillsMP](https://skillsmp.com/about) | Indexes public GitHub skills | Search fetch is challenged; no verified author-submission route |
| [Smithery](https://www.smithery.ai/) | Current service concerns MCP tools; old skills URL returns 404 | No verified skill-publishing route; do not substitute an MCP listing |
| [Playbooks](https://playbooks.com/) | Previous skills catalog | Skills route returns 404; current homepage serves a coming-soon shell |
| [SkillHub.space](https://docs.skillhub.space/docs/cli) | Advertises an OpenClaw registry and `shsc` CLI | Live CLI/API reference pages contain headings only; publishing/auth flow unverified |
| [SkillHub.club](https://www.skillhub.club/app/skills) | Live skill upload and publication UI | Live upload requires login; anonymous upload returns 401 |
| [skills.rest](https://skills.rest/contribute) | Public repository/folder form, automatic indexing | Submission succeeded; service says indexing occurs within 24 hours |
| [FindSkills](https://findskills.org/) | Indexes GitHub and ClawHub | Public search does not return the exact skill; no verified submission route |
| [Tencent SkillHub](https://skillhub.cn/dashboard/publish) | Individual publication requires account and real-name verification | Owner-only identity verification remains outstanding |
| [LobeHub](https://lobehub.com/skills) | Versioned package publication through `lhm skill publish` | Publishing route verified; owner login required |

New accounts, fees and outreach messages require the owner's involvement; do not
silently create accounts or buy promotion. A challenged or failed request does
not establish that a skill is absent from an index.

## Listing state

- **ClawHub: public and installed.**
  https://clawhub.ai/jeffreykentjr/skills/sail-verification
  The owner approved device authorization. CLI `0.25.0` uploaded version `1.0.0`
  from source `d8e6c340`, then the service completed its publication checks.
  A normal noninteractive install of the owner-scoped package succeeded without
  a scan bypass. All four installed files match source byte for byte. The public
  audit verdict is clean; one component retains a low-severity warning. ClawHub
  displays its platform license as MIT-0; the unchanged canonical MIT `LICENSE`
  is included in the installed package.
- **Tessl: updated, moderation passed and installed.**
  https://tessl.io/registry/jeffreykent/sail-verification
  Existing workspace `jeffreykent` now publishes wrapper `1.0.3` from source
  `d8e6c340`, replacing `1.0.2` as latest. Lint and publication dry run passed.
  A clean strict install with CLI `0.119.0` retained the skill and both
  references byte for byte. The export carries the canonical license text as
  `LICENSE.md`, because Tessl filters the extensionless filename. The wrapper
  adds repository/homepage/license/author metadata, without separate
  instructions. No paid evaluation campaign was requested.
- **Agent Skill Hub: refreshed and installed.**
  https://agentskillhub.dev/u/Chelis-Lang/sk/sail-verification
  Registry version `2026.11.04` reports exact source `d8e6c340`, the canonical
  directory and all four files. Every manifest blob hash and file size matches
  source. A clean `skhub` `0.14.1` install of that version retained all four
  files byte for byte.
- **skills.sh: GitHub installation verified; discovery still pending.**
  CLI `1.7.4` installed the current public GitHub package into a clean Codex
  project; all four files match source. `find sail-verification --owner
  Chelis-Lang` returns no result. The service populates its catalog from genuine
  aggregate usage; there is no verified manual-submission route. Validation
  installs use `DO_NOT_TRACK=1`, not manufactured installation counts.
- **Skillstore: original public listing; refresh submitted.**
  https://skillstore.io/skills/chelis-lang-sail-verification
  The original submission was published. The updated canonical folder is in review,
  tracked at
  https://skillstore.io/submissions/cb45db69-05bb-4d82-994f-b33a0a8be755.
  The service created [review PR #4080](https://github.com/aiskillstore/marketplace/pull/4080).
  A successful submission is not evidence that the public bundle has refreshed.
- **Skill.Fish / MCP Market: still queued.** CLI `1.0.39` says the repository
  is already in its queue. Its current search request returns 403, so public
  listing and current file contents remain unverified. Do not count the two
  catalog names as independent submissions.
- **Agent Skills CLI marketplace: searchable listing remains verified.**
  A current `search sail-verification --json` returns one exact skill with owner
  `Chelis-Lang`, branch `main` and path
  `skills/sail-verification/SKILL.md`. This is discovery evidence; that response
  does not expose a pinned package revision or establish a full bundle install.

The public [skills.rest contribution form](https://skills.rest/contribute)
accepted the canonical skill-directory URL and reported successful submission,
with indexing within 24 hours. It returned no versioned listing or status URL;
publication and installed contents remain unverified. The
[SkillHub.club upload UI](https://www.skillhub.club/app/skills) accepts a ZIP
selection but its API returns 401 and the UI displays “Login required.” No
anonymous skill was created. Skills Directory also redirects to sign-in.
SkillRouter uses owner device pairing before repository submission. LobeHub's
current CLI exposes `lhm skill publish --identifier ID --dir PATH`, but an
attempt with the canonical export reports “Not logged in. Run `lhm login`
first.” Account creation and identity checks on these remaining services need
owner involvement.

For Tessl updates, export the merged canonical folder outside ignored repository
paths, import it with `tessl skill import --workspace jeffreykent --public PATH`,
and retain repository/homepage/license metadata in its generated
`.tessl-plugin/plugin.json`. Copy the canonical `LICENSE` to `LICENSE.md` in the
export so Tessl retains the license text. Run lint and a dry run, then publish a new registry
version from that export. Do not independently edit the skill instructions.

For ClawHub updates, export the merged canonical folder and use
`clawhub skill publish PATH --slug sail-verification --version VERSION` with
the source repository, commit, ref and package path flags. Bump the registry
version for changed files, wait for publication, then test an ordinary
owner-scoped install. Do not bypass scans to call a queued version published.

For Agent Skill Hub updates, the documented refresh endpoint is
`POST https://agentskillhub.dev/api/v1/skills/refresh-from-source` with
`{"slugs":["Chelis-Lang/sail-verification"]}`. Recheck the resulting source commit
and bundled references after a refresh. For upload-based registries, publish a
new package version from the merged folder and record its source revision.

## What Deadpost does and does not establish

The adjacent Deadpost project shipped a long API/onboarding skill and recurring
social-participation instructions, plus registry integrations and outreach
templates. Those artifacts do not establish which listings went live or why
adoption failed. Sail's use case is a concrete developer task. Test successful
first use and keep actual submission/listing records rather than treating a
public skill file as distribution by itself.
