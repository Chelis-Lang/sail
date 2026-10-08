# 0002: AI-authorship guard and workflow mode

- **Status:** accepted
- **Date:** 2026-10-08

## Context

The owner requires that commits and PRs on Chelis-Lang repos never claim AI
authorship. A survey of the org's repos found several mechanisms:
- **Commit-msg hooks.** chelis and dive use a tracked `.githooks/commit-msg`
  wrapped in a brace group and installed into the git common hooks directory.
  octant uses a cargo-husky hook. beacon, c-note and nautilus use a minimal
  grep hook.
- **CI job.** A job named exactly `No AI authorship markers` checks messages,
  author and committer. It runs in chelis, dive, octant and c-note.
- **Policy text** in AGENTS.md.
- **Agent config.** In dive only, `.claude/settings.json` disables Claude
  Code's attribution.
- **Branch protection** that makes the CI job a required check, with admins
  enforced.

dive's `scripts/check-ai-markers.sh` is the most complete version. It is one
script shared by the hook and CI. It covers co-author trailers, "Generated
with/by" lines, AI vendor noreply addresses, and author and committer
identity, and it scans the full push or PR range.

## Decision

- Adopt dive's four layers unchanged in substance:
  1. `.githooks/commit-msg`, installed by `cargo xtask install-hooks` (common
     hooks dir, no `core.hooksPath`);
  2. the CI job `No AI authorship markers`, running
     `scripts/check-ai-markers.sh` over the full range;
  3. `.claude/settings.json` with empty `attribution` and
     `includeCoAuthoredBy: false`;
  4. the AGENTS.md "Commit messages and authorship" section.
- **Workflow mode.** Bootstrap pushes go directly to `main` during M0. Once both
  CI jobs are green on `main`, switch to PR mode:
  - `cargo xtask protect-main` requires `No AI authorship markers` and
    `Workspace gate`, with admins enforced, PRs required, and force pushes
    and deletions disabled;
  - every change after that is a squash-merged PR.
- `protect-main` derives the repository slug from the `origin` remote, so the
  copied xtask cannot target another repository.

## Consequences

- **Every clone must run `cargo xtask install-hooks` once.** CI catches
  anything a missing hook lets through.
- **PR bodies are covered by policy only.** The org has no automated PR-body
  scan yet.
- **M0 exit requires a negative test.** A scratch branch with a
  `Co-Authored-By: Claude` trailer must be rejected both locally by the hook
  and in CI.

## Update 2026-10-08

The negative test passed:
- **Local hook.** It rejected `Co-Authored-By: Claude <noreply@anthropic.com>`,
  matching both the co-author pattern and the noreply pattern.
- **CI.** The same commit, built with `git commit-tree` so it bypassed the hook,
  was pushed to `scratch/ai-guard-check`. CI run 37788662801 then failed
  `No AI authorship markers` on that commit. The branch was deleted afterwards.

After that, `main` was protected and the project switched to PR mode.
