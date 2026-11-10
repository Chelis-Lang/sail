# GitHub Action

The Sail action verifies a Verus project in a GitHub Actions job. It
installs a Sail release, restores Sail's caches, runs
`sail verify --output json` and turns the result into a job summary,
annotations on the failing lines and a SARIF file. The job then ends with
the exit code of `sail verify`. The action does not change a verdict:
`sail verify` decides the result, and the report steps only read it.

**Status.** The action is implemented and tested on GitHub-hosted Ubuntu
24.04 runners: on every pull request, the optional `GitHub Action (cold)`
and `GitHub Action (restored)` jobs in [CI](../.github/workflows/ci.yml)
run it on [a small fixture](../ci/github-action-fixture). In
[run 42801635198](https://github.com/Chelis-Lang/sail/actions/runs/42801635198),
the cold job verified the fixture in 47.4 s, vstd included. A fresh runner
then restored the exact cache keys and verified in 0.9 s, reusing 2 targets
from receipts with no verifier run. A broken lemma then failed the action
step with exit code 101, the exit code of `sail verify`, and the SARIF file
located the error. That test installs
the current commit's Sail, not a published release, because `sail report`
is not in a release yet. Sail 0.2.0 does not have `sail report`, and the
action stops with an error when it finds a Sail without it. The first
release the action works with is 0.3.0. Its default `sail-version` is the
release it ships with, so `uses: Chelis-Lang/sail/action@v0.3.0` installs
Sail 0.3.0.

## Usage

```yaml
name: verify
on: [push, pull_request]

permissions:
  contents: read
  attestations: read   # checks the release archive's build provenance

jobs:
  verify:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0   # per-file commit times; see restore-mtime
      - uses: Chelis-Lang/sail/action@v0.3.0
        with:
          args: --lib
```

The action lives in the `action/` directory of the Sail repository, so the
reference is `Chelis-Lang/sail/action@<tag>`. GitHub Marketplace lists
only actions whose `action.yml` is at the repository root, so this action
is not listed there. Pin a release tag, or a commit SHA for an immutable
reference.

Sail supports Linux x86-64 with glibc 2.39 or newer (`ubuntu-24.04`) and
macOS on Apple silicon (`macos-14` and later). The action needs `python3`,
`git` and `rustup`, which GitHub's hosted images have. Its hosted test runs
on Ubuntu only; macOS runners are untested.

## What it does

1. **Installs Sail** with [`scripts/install.py`](install.md) into
   `$RUNNER_TEMP/sail`: it downloads the release archive, checks its SHA-256
   and its build-provenance attestation, installs the verifier's pinned Rust
   toolchain with `rust-src`, and adds Sail to `PATH` for later steps. When
   the project has a `rust-toolchain` file, it also installs that toolchain
   first (`rustup toolchain install`), since installing it on first use
   inside `sail verify` has failed on a fresh runner
   ([known issues](known-issues.md#product-limits)). A
   second use of the action in one job reuses that installation.
2. **Restores caches** with `actions/cache`: solver replies
   (`~/.cache/sail/results`), managed targets and proof receipts
   (`~/.cache/sail/projects`), and the Cargo registry plus any `cache-target`
   directories. See [keys](#cache-keys).
3. **Restores file times.** `actions/checkout` gives every file the current
   time, which makes Cargo rebuild and re-verify unchanged crates. The
   action sets each tracked file's time to the time of the last commit that
   changed it, like `git restore-mtime`, following the first-parent history
   (a merge counts as changing what it brought in). Files that differ from
   the commit (edited by an earlier step) and untracked files keep their own
   times. With the default shallow checkout, every file gets the checked-out
   commit's time; with `fetch-depth: 0`, each file gets its own. See
   [restored builds](#restored-builds-and-file-times).
4. **Verifies**: `sail verify --output json ARGS` in `working-directory`.
   Cargo's and Verus's messages go to the job log as usual. The run gets no
   GitHub token, and `SAIL_BACKGROUND_PREPARE` defaults to `0`, since a CI
   job ends after one call.
5. **Reports** with [`sail report`](ci-reports.md): the summary goes to the
   job's summary page with locations linked to the commit, up to
   `max-annotations` errors and warnings per level become annotations, and
   the SARIF file is written to `sarif` (and uploaded to code scanning with
   `upload-sarif: true`). With `baseline` or `baseline-branch`, the summary
   compares the result with an earlier one.
6. **Uploads the result document** as the `result-artifact` artifact, for a
   later run's baseline.
7. **Saves the caches**, after `sail cache gc --max-bytes` bounds the
   solver replies. Proof receipts are not saved when Sail reports
   `SAIL3112`: such a run reuses no receipts.
8. **Ends with the exit code of `sail verify`.** The report, upload and
   cache steps run whether verification passed or failed. A failed report
   step fails the job as well, so a failed verification never passes.

## Inputs

| Input | Default | Meaning |
|---|---|---|
| `sail-version` | the release the action ships with | Sail release to install (0.3.0 or newer) |
| `archive` | | A local release archive to install instead, with its `.sha256` beside it |
| `install` | `true` | `false` uses the `sail` already on `PATH` |
| `install-prefix` | `$RUNNER_TEMP/sail` | Where to install |
| `install-toolchain` | `true` | Install the verifier's Rust toolchain and `rust-src` with rustup |
| `require-attestation` | `true` | Refuse an archive whose attestation does not verify (needs `github-token` and `attestations: read`) |
| `github-token` | `github.token` | For the download, the attestation check and `baseline-branch`; never passed to `sail verify` |
| `working-directory` | `.` | The Cargo project or workspace, relative to the repository |
| `args` | | Arguments for `sail verify`, as for `cargo verus verify`; split like a shell's words, without expansion |
| `restore-mtime` | `true` | Restore commit times of unmodified tracked files |
| `cache` | `true` | Cache solver replies |
| `cache-projects` | `true` | Cache managed targets and proof receipts (tens to hundreds of MB) |
| `cache-cargo` | `true` | Cache `~/.cargo/registry`, `~/.cargo/git/db` and `cache-target` |
| `cache-target` | | Cargo target directories to cache, one per line, relative to `working-directory` |
| `cache-key-prefix` | `sail` | Prefix of every cache key |
| `cache-max-bytes` | `1G` | Bound for the solver replies before saving; empty to skip |
| `baseline` | | An earlier result document to compare with |
| `baseline-branch` | | Compare with the newest `result-artifact` from another run on this branch (needs `actions: read`) |
| `result-artifact` | `sail-result` | Artifact name for the result document; empty to skip. Use a distinct name per matrix job |
| `sarif` | under `$RUNNER_TEMP` | Where to write the SARIF file |
| `upload-sarif` | `false` | Upload the SARIF file to code scanning (needs `security-events: write`) |
| `sarif-category` | `sail` | Code scanning category |
| `max-annotations` | Sail's default, 10 | Annotations per level |

## Outputs

| Output | Meaning |
|---|---|
| `exit-code` | Exit code of `sail verify` |
| `result` | Path of the result document |
| `sarif` | Path of the SARIF file |
| `sail-version` | Version of the Sail that ran |
| `cache-hit` | `true` when the solver replies came from this commit's exact key |
| `cache-matched-key` | The solver-reply key that was restored, if any |

To keep the job going after a failed verification, for example to run a
later step on the result, set `continue-on-error: true` on the action's step
and read `steps.ID.outputs.exit-code`.

## Cache keys

Each cache's key is

```text
PREFIX-KIND-RUNNER_OS-RUNNER_ARCH-SAIL_VERSION-VERIFIER_TOOLCHAIN-PLACE-TOOLCHAINS-LOCKS-COMMIT
```

`PLACE` is a hash of `working-directory` and `args`, so matrix jobs over
different arguments keep separate entries. `TOOLCHAINS` hashes the
`rust-toolchain` files that select the project's Cargo and `LOCKS` the
project's `Cargo.lock` files. The key ends with the commit, so every commit
saves its own entry. When this commit has none, the restore falls back to
the newest entry with the same lockfiles and toolchains, then the same
toolchains, then anything from the same Sail on the same runner.

A restored entry from another commit is safe to use. Solver replies are keyed
by the exact query bytes and the solver's identity, and proof receipts by the
exact inputs and installed tools, so a stale entry is a miss, never a wrong
answer. [Caching Sail in CI](ci-cache.md) has the measured gains, the sizes
and the trust caveats. In particular, restore only caches written by workflows
you trust; GitHub keeps a pull request's caches from reaching its base branch.

### Restored builds and file times

A Cargo target (`cache-target`) restored from another commit's entry is
different: Cargo trusts it whenever every input file is older than the
build, without reading their contents.
Restored commit times are older than a build cached later, so a file changed
since the cached commit could look unchanged, and Cargo would replay that
commit's result instead of verifying the change. The action therefore restores
file times after the caches. When a target came from another commit's entry,
every file that differs between that commit and this one keeps its current
time, so Cargo rebuilds it. As a precaution the action does the same for
managed targets restored from another commit's `projects` entry, although
proof receipts are bound to file contents. That commit must be in the clone
(`fetch-depth: 0`); when it is not, no file time is restored and the job log
has a warning. If you restore a `target/` with another cache step, restore
it without commit times (`restore-mtime: false`).

## Comparing with a baseline

`baseline-branch: main` makes the summary compare this run with the newest
result uploaded by a run on this repository's `main` (a pull request from a
fork's `main` does not count):

```yaml
permissions:
  contents: read
  attestations: read
  actions: read          # find the baseline artifact

jobs:
  verify:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: Chelis-Lang/sail/action@v0.3.0
        with:
          baseline-branch: main
```

The run on `main` must use the same `result-artifact` name. When no such
artifact exists, or it has expired, the summary has no comparison and the job
log has a notice. The comparison is that of `sail compare`: two results that
ran with different settings, targets or arguments are reported as not
comparable, with the reason ([baseline comparison](ci-reports.md#baseline-comparison)).

## Code scanning

```yaml
permissions:
  contents: read
  attestations: read
  security-events: write

    steps:
      - uses: actions/checkout@v4
      - uses: Chelis-Lang/sail/action@v0.3.0
        with:
          upload-sarif: true
```

Code scanning must be enabled for the repository. Without `upload-sarif`,
the file is still written and its path is the `sarif` output.

## Limits

- **Platforms.** Windows runners are not supported, as for Sail itself.
- **Annotations and summary size** follow GitHub's limits; see
  [CI reports](ci-reports.md#limits).
- **Managed receipts need the same paths.** They bind the installed tools'
  bytes and absolute paths and the project's absolute path. GitHub-hosted
  runners give every run the same paths, so they are reused there. A
  self-hosted runner with several work directories may not
  ([caveats](ci-cache.md#caveats)).
- **Several uses in one job** share one installation. Give each use its own
  `result-artifact` name or an empty one, since artifact names must be unique
  in a run.
