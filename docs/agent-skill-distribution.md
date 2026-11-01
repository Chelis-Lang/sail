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
the current flow before sending anything. The canonical package was published in
[PR #640](https://github.com/Chelis-Lang/sail/pull/640), source commit
`ec052397ea6bd96b72bb6c163be8c417b893dd42`, package version `1.0.0`.

| Channel | Route and fit | Checked disposition |
| --- | --- | --- |
| [Tessl](https://tessl.io/registry/skills/submit) | GitHub repository analysis/submission or `tessl skill publish`; registry review and versioned packages | Ready for submission; existing workspace and owner login required |
| [skills.sh](https://www.skills.sh/docs/faq) | GitHub installation through `npx skills`; aggregate installs drive discovery | GitHub install verified; catalog page still unavailable |
| [ClawHub](https://docs.openclaw.ai/clawhub) | Installed CLI 0.9.0 uses `clawhub publish` to upload a versioned folder | Held for publisher namespace confirmation |
| [Skill.Fish](https://github.com/knoxgraeme/skillfish) | `skillfish submit owner/repo`; repository review | Submitted through CLI; MCP Market is the same submission channel |
| [Skillstore](https://skillstore.io/submit) | Public GitHub repository or skill-directory form; anonymous submission supported | Submitted; automated processing completed, final review pending |
| [Skills Directory](https://www.skillsdirectory.com/submit) | GitHub sign-in and reviewed submission | Sign-in required; no new account created |
| [SkillRouter](https://skillrouter.org/us/submit) | Repository submission, optional ownership proof and author statistics | Account connection required; held for owner login |
| [Agent Skill Hub](https://doc.agentskillhub.dev/guide/publishing) | Documented public GitHub analyze/import API, snapshots package files | Published; source revision and all four package files verified |
| [SkillsLLM](https://skillsllm.com/llms.txt) | Community submissions via GitHub OAuth; publishes a 100-star minimum | Not currently eligible: Sail has 1 star; do not manufacture stars |
| [Agent Skills CLI marketplace](https://www.agentskills.in/) | Repository form and `agent-skills-cli submit-repo owner/repo` | Published in search; canonical source path verified |
| [SkillsMP](https://skillsmp.com/about) | Indexes public GitHub skills | No Sail result found in checked search; no verified submission route |
| [Smithery](https://github.com/smithery-ai/cli) | Skills catalog and upstream installer; documented publishing commands concern MCP servers | Check catalog; no verified skill-publishing route found |
| [Playbooks](https://playbooks.com/skills) | Skill catalog with GitHub-based installation | Check catalog; no verified submission route found |
| [SkillHub.space](https://docs.skillhub.space/) | OpenClaw skill registry; `shsc publish` described in docs | Secondary: verify live publishing/auth flow; CLI reference is sparse |
| [SkillHub.club](https://www.skillhub.club/) | Evaluated skills catalog advertises publishing | Discovery API and installer documented; author-publishing route unverified |
| [skills.rest](https://skills.rest/) | Skills library and GitHub package installation | Secondary: no verified submission route found |
| [FindSkills](https://findskills.org/) | Search/index API with GitHub and ClawHub sources | Secondary indexing check; no verified author-submission route found |
| [Tencent SkillHub](https://skillhub.tencent.com/) | Login and real-name verification for individual publication | Held: requires Tencent face verification |
| [LobeHub](https://lobehub.com/skills) | Skills marketplace | Secondary: public fetch refused; publishing route not verified |

This list separates verified publishing flows from sourcing leads rather than
assuming every catalog accepts submissions. Reassess a service if its concrete
route or audience fit improves. New accounts, fees and outreach messages require
the owner's involvement; do not silently create accounts or buy promotion.

## Listing state

Checked on 1 November 2026 after source publication:

- **Agent Skill Hub: published.**
  https://agentskillhub.dev/u/Chelis-Lang/sk/sail-verification
  The public API reports the canonical repository, `skills/sail-verification`,
  source commit `ec052397ea6bd96b72bb6c163be8c417b893dd42` and all four package
  files. Its registry version is `2026.11.01`; the upstream skill version remains
  `1.0.0`. Refresh from source rather than editing the hosted copy.
- **Skillstore: in review.**
  https://skillstore.io/submissions/72564182-6044-4efb-b1d7-574d3762597c
  Processing created https://github.com/aiskillstore/marketplace/pull/4036;
  final review is pending. The submitted source is the canonical skill folder.
- **Skill.Fish / MCP Market: accepted submission.** `skillfish` 1.0.39 reported
  success and discovered `sail-verification`. Public listing not yet verified;
  do not count the acknowledgement as publication.
- **Agent Skills CLI marketplace: searchable listing verified.**
  https://www.agentskills.in/api/repos/submit?repo=Chelis-Lang%2Fsail
  A subsequent `search sail-verification --json` returned the canonical skill
  name, owner, main branch and `skills/sail-verification/SKILL.md` path. The
  repository status reports two entries; Sail maintains one consumer skill
  package. Do not count both entries as maintained consumer skills.
- **skills.sh: installation verified, listing pending.** CLI 1.7.4 installed
  the public GitHub package and all files matched source. The catalog URL still
  displayed “isn't available” when checked, despite returning HTTP 200.
  Validation installs used `DO_NOT_TRACK=1` rather than generating install counts.
- **Tessl:** CLI 0.119.0 and live form require login. Continue after the owner
  authenticates with `tessl login --device` and chooses an existing workspace.
- **ClawHub:** publication is held for owner confirmation of the publisher
  namespace. No bundle has been uploaded.

Skills Directory and SkillRouter also require account access. SkillsLLM's
published minimum is not met. Tencent SkillHub requires identity verification.
The remaining channels are sourcing leads, not completed submissions.

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
