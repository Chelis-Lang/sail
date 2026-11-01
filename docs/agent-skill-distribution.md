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

## Distribution landscape

Primary sources checked on 1 November 2026. Similar names below identify
different services. Availability and publishing requirements can change; verify
the current flow before sending anything. The initial canonical-source PR is
not yet merged, so no listing is claimed below.

| Channel | Route and fit | Initial disposition |
| --- | --- | --- |
| [Tessl](https://tessl.io/registry/skills/submit) | GitHub repository analysis/submission or `tessl skill publish`; registry review and versioned packages | Priority: submit public canonical source; requires login |
| [skills.sh](https://www.skills.sh/docs/faq) | GitHub installation through `npx skills`; aggregate installs drive discovery | Priority: verify discovery/install; no manual upload form |
| [ClawHub](https://docs.openclaw.ai/clawhub) | `clawhub skill publish` uploads a versioned folder | Priority: publish canonical bundle if existing authentication permits |
| [Skill.Fish](https://github.com/knoxgraeme/skillfish) | `skillfish submit owner/repo`; repository review | Submit; its docs also mention MCP Market, so do not count that as a second independent submission |
| [Skillstore](https://skillstore.io/submit) | Public GitHub repository or skill-directory form; anonymous submission supported | Submit with canonical source and task-specific context |
| [Skills Directory](https://www.skillsdirectory.com/submit) | GitHub sign-in and reviewed submission | Submit if existing account access permits |
| [SkillRouter](https://skillrouter.org/us/submit) | Repository submission, optional ownership proof and author statistics | Submit; account/claim step may need owner access |
| [Agent Skill Hub](https://doc.agentskillhub.dev/guide/publishing) | GitHub analyze/import web/API flow, snapshots package files | Submit if authenticated access permits |
| [SkillsLLM](https://skillsllm.com/llms.txt) | Community submissions via GitHub OAuth; publishes a 100-star minimum | Not currently eligible: Sail has 1 star; do not manufacture stars |
| [Agent Skills CLI marketplace](https://www.agentskills.in/) | Repository form and `skills submit-repo owner/repo` | Submit canonical repository |
| [SkillsMP](https://skillsmp.com/about) | Indexes public GitHub skills | Check indexing; no verified public submission endpoint found |
| [Smithery](https://github.com/smithery-ai/cli) | Skills catalog and upstream installer; documented publishing commands concern MCP servers | Check catalog; no verified skill-publishing route found |
| [Playbooks](https://playbooks.com/skills) | Skill catalog with GitHub-based installation | Check catalog; no verified submission route found |
| [SkillHub.space](https://docs.skillhub.space/) | OpenClaw skill registry; `shsc publish` described in docs | Secondary: verify live publishing/auth flow; CLI reference is sparse |
| [SkillHub.club](https://www.skillhub.club/) | Evaluated skills catalog advertises publishing | Secondary: live route needs verification; public fetch timed out |
| [skills.rest](https://skills.rest/) | Skills library and GitHub package installation | Secondary: no verified submission route found |
| [FindSkills](https://findskills.org/) | Search/index API with GitHub and ClawHub sources | Secondary indexing check; no verified author-submission route found |
| [Tencent SkillHub](https://skillhub.tencent.com/) | Redirects to skillhub.cn | Secondary: publishing route not verified |
| [LobeHub](https://lobehub.com/skills) | Skills marketplace | Secondary: public fetch refused; publishing route not verified |

This list separates verified publishing flows from sourcing leads rather than
assuming every catalog accepts submissions. Reassess a service if its concrete
route or audience fit improves. New accounts, fees and outreach messages require
the owner's involvement; do not silently create accounts or buy promotion.

## Listing state

Pending canonical-source publication. Replace this paragraph with actual
submission receipts, live URLs and access blockers after the source PR lands.

## What Deadpost does and does not establish

The adjacent Deadpost project shipped a long API/onboarding skill and recurring
social-participation instructions, plus registry integrations and outreach
templates. Those artifacts do not establish which listings went live or why
adoption failed. Sail's use case is a concrete developer task. Test successful
first use and keep actual submission/listing records rather than treating a
public skill file as distribution by itself.
