#!/usr/bin/env python3
"""Steps of the Sail GitHub Action (action/action.yml).

Each subcommand is one step. Inputs arrive as SAIL_ACTION_* environment
variables, outputs go to $GITHUB_OUTPUT. Only the standard library is used,
so the script runs on every GitHub-hosted Linux and macOS image.

  setup     install Sail (or use the one on PATH) and compute cache paths
            and keys
  times     restore file times, knowing which caches were restored
  verify    run `sail verify --output json`, keep its exit code as an output
  report    write the job summary, annotations and SARIF; fetch a baseline
  gc        bound the solver-reply cache before it is saved
  finish    end with `sail verify`'s exit code
"""
import hashlib
import importlib.util
import json
import re
import os
from pathlib import Path
import shlex
import subprocess
import sys

ACTION = Path(__file__).resolve().parent
INSTALL = ACTION.parent / "scripts" / "install.py"


class Failure(Exception):
    """A step failure with a message for the job log."""


def env(name, default=""):
    return os.environ.get("SAIL_ACTION_" + name, default).strip()


def flag(name, default="false"):
    value = env(name, default).lower()
    if value not in ("true", "false"):
        raise Failure(f"input {name.lower().replace('_', '-')} must be `true` or `false`, not `{value}`")
    return value == "true"


def output(**values):
    path = os.environ.get("GITHUB_OUTPUT")
    lines = "".join(f"{key.replace('_', '-')}={value}\n" for key, value in values.items())
    if path:
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(lines)
    else:
        sys.stdout.write(lines)


def command(level, message):
    """A workflow command (`::notice::`, `::warning::`, `::error::`)."""
    escaped = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print(f"::{level}::{escaped}", flush=True)


def workspace():
    return Path(os.environ.get("GITHUB_WORKSPACE") or os.getcwd()).resolve()


def project_dir():
    directory = env("WORKING_DIRECTORY", ".") or "."
    path = (workspace() / directory).resolve()
    if not path.is_dir():
        raise Failure(f"working-directory {directory} is not a directory")
    return path


def state_dir():
    root = Path(os.environ.get("RUNNER_TEMP") or (Path.home() / ".cache" / "sail-action-tmp"))
    path = root / "sail-action"
    path.mkdir(parents=True, exist_ok=True)
    return path


def cache_root():
    """Sail's cache root, as sail_verus::cache_root resolves it."""
    if os.environ.get("SAIL_CACHE_DIR"):
        return Path(os.environ["SAIL_CACHE_DIR"])
    if os.environ.get("XDG_CACHE_HOME"):
        return Path(os.environ["XDG_CACHE_HOME"]) / "sail"
    return Path.home() / ".cache" / "sail"


def load_installer():
    spec = importlib.util.spec_from_file_location("sail_install", INSTALL)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def default_version():
    """The `sail-version` default in action.yml: the release this action ships with.

    A workflow that passes an empty `sail-version` (for example an unset
    `workflow_dispatch` input) gets this default too."""
    text = (ACTION / "action.yml").read_text(encoding="utf-8")
    match = re.search(r'^  sail-version:\n(?:    .*\n)*?    default: "([^"]+)"', text, re.M)
    return match[1] if match else ""


def installed_version(sail):
    try:
        result = subprocess.run([str(sail), "--version"], capture_output=True, text=True, check=False)
    except OSError:
        return None
    text = result.stdout.strip()
    return text.removeprefix("sail ") if result.returncode == 0 and text.startswith("sail ") else None


def install():
    """Install the selected archive, or reuse an identical earlier install."""
    installer = load_installer()
    try:
        host = installer.target()
    except ValueError as error:
        raise Failure(str(error)) from error
    archive = env("ARCHIVE")
    version = env("SAIL_VERSION") or default_version()
    if archive:
        archive_path = (workspace() / archive).resolve()
        name = archive_path.name.removesuffix(".tar.gz")
        selection = ["--archive", str(archive_path)]
    elif version:
        name = f"sail-{version}-{host}"
        selection = ["--version", version]
    else:
        raise Failure("no sail-version default found in action.yml; set sail-version")
    prefix = Path(env("INSTALL_PREFIX") or (Path(os.environ.get("RUNNER_TEMP") or Path.home()) / "sail"))
    binary = prefix / name / "bin" / "sail"
    if binary.is_file():
        print(f"Using Sail already installed at {prefix / name}")
        return binary
    arguments = [sys.executable, "-I", str(INSTALL), *selection, "--prefix", str(prefix)]
    if flag("INSTALL_TOOLCHAIN", "true"):
        arguments.append("--install-toolchain")
    if flag("REQUIRE_ATTESTATION", "true"):
        arguments.append("--require-attestation")
    print("+ " + shlex.join(arguments), flush=True)
    if subprocess.run(arguments, check=False).returncode != 0:
        raise Failure("installing Sail failed; see the messages above")
    if not binary.is_file():
        raise Failure(f"the installer did not create {binary}")
    return binary


def restore_mtime(top, since=()):
    """Set each clean tracked file's time to its last commit's time.

    `actions/checkout` gives every file the current time, so Cargo would
    consider every crate changed. A modified or untracked file keeps its time:
    its content is not the commit's, and an old time could make Cargo trust
    a restored build of different content.

    SINCE names the commits restored builds were made from. Cargo trusts a
    build whose fingerprints are newer than every input's time, whatever
    commit it was made from, so each file that differs between one of those
    commits and HEAD also keeps its current time. When such a commit is not
    in the clone, no time is restored: the build is then never trusted on
    time alone.

    Merges count as changing what they change relative to their first
    parent, and only the first-parent history is followed, so a file's
    time is never older than the commit that brought its content to this
    line of history."""
    def git(*arguments):
        return subprocess.run(["git", "-C", str(top), *arguments], capture_output=True, check=True).stdout

    def names(data):
        return {name for name in data.decode("utf-8", "surrogateescape").split("\0") if name}

    changed = set()
    for base in since:
        try:
            git("rev-parse", "--verify", "--quiet", f"{base}^{{commit}}")
            changed |= names(git("-c", "core.quotepath=off", "diff", "--name-only", "--no-renames", "-z",
                                 base, "HEAD", "--"))
        except (OSError, subprocess.CalledProcessError):
            command("warning", f"file times were not restored: a cached build was made from commit {base},"
                    " which is not in this clone, so a changed file could look older than that build."
                    " Check out with `fetch-depth: 0` to keep restored times")
            return 0
    try:
        tracked = names(git("ls-files", "-z"))
        status = git("status", "--porcelain=v1", "-z", "--untracked-files=no").decode("utf-8", "surrogateescape")
        log = git("-c", "core.quotepath=off", "log", "--pretty=format:%x01%ct", "--name-only",
                  "--no-renames", "--first-parent", "-m", "-z", "HEAD").decode("utf-8", "surrogateescape")
    except (OSError, subprocess.CalledProcessError) as error:
        command("warning", f"file times were not restored: git failed in {top}: {error}")
        return 0
    dirty = set()
    entries = status.split("\0")
    index = 0
    while index < len(entries):
        entry = entries[index]
        if len(entry) > 3:
            dirty.add(entry[3:])
            if entry[0] in "RC":  # a rename or copy names its source next
                index += 1
                if index < len(entries):
                    dirty.add(entries[index])
        index += 1
    remaining = tracked - dirty - changed
    restored = 0
    for record in log.split("\x01"):
        if not remaining:
            break
        if not record.strip():
            continue
        stamp, _, files = record.partition("\n")
        try:
            seconds = int(stamp)
        except ValueError:
            continue
        for name in files.split("\0"):
            if name in remaining:
                remaining.discard(name)
                path = top / name
                if path.is_file() and not path.is_symlink():
                    os.utime(path, (seconds, seconds))
                    restored += 1
    kept = len(dirty | (changed & tracked))
    print(f"Restored commit times of {restored} files in {top}"
          + (f"; {kept} modified files, or files changed since a cached build, kept their times" if kept else ""))
    return restored


def restored_build_commits():
    """Commits of restored caches that hold Cargo builds, when the cache was
    made for another commit than this one (a restore-keys match). The commit
    is the last part of the key (see `setup`)."""
    commits = []
    for kind, holds_build in (("CARGO", bool(env("CACHE_TARGET"))), ("PROJECTS", True)):
        matched, exact = env(kind + "_MATCHED_KEY"), env(kind + "_KEY")
        if holds_build and matched and matched != exact:
            commits.append(matched.rsplit("-", 1)[-1])
    return commits


def times():
    """Restore file times, after the caches are restored (`restore_mtime`)."""
    if not flag("RESTORE_MTIME", "true"):
        return
    project = project_dir()
    top = git_top(project)
    if top is None:
        print(f"{project} is not in a Git repository; file times are left as they are")
        return
    restore_mtime(top, restored_build_commits())


def digest_files(paths, base):
    digest = hashlib.sha256()
    for path in sorted(paths):
        digest.update(str(path.relative_to(base)).encode() + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest())
    return digest.hexdigest()[:16]


def project_files(project, names):
    """Files with one of NAMES under PROJECT, outside build and VCS directories."""
    found = []
    for directory, subdirectories, files in os.walk(project):
        subdirectories[:] = [name for name in subdirectories if name not in ("target", ".git", "node_modules")]
        found.extend(Path(directory) / name for name in files if name in names)
    return found


def toolchain_files(project, top):
    """rust-toolchain files that select the project's Cargo: in the project
    and its parents up to the repository root, and below the project."""
    names = ("rust-toolchain", "rust-toolchain.toml")
    found = set(project_files(project, names))
    directory = project
    while True:
        found.update(directory / name for name in names if (directory / name).is_file())
        if directory == top or directory.parent == directory:
            break
        directory = directory.parent
    return found


def install_project_toolchain(project):
    """Install the toolchain the project's rust-toolchain file names, once,
    before verification. Otherwise rustup installs it on first use inside
    `sail verify`, which failed on a fresh hosted runner (SAIL3104; see
    docs/known-issues.md)."""
    rustup = subprocess.run(["sh", "-c", "command -v rustup"], capture_output=True, text=True, check=False)
    if rustup.returncode != 0:
        return
    print("+ rustup toolchain install  (the project's rust-toolchain file)", flush=True)
    result = subprocess.run(["rustup", "toolchain", "install"], cwd=project, check=False)
    if result.returncode != 0:
        command("warning", "rustup could not install the project's toolchain; sail verify will try again")


def git_top(project):
    result = subprocess.run(["git", "-C", str(project), "rev-parse", "--show-toplevel"],
                            capture_output=True, text=True, check=False)
    return Path(result.stdout.strip()).resolve() if result.returncode == 0 else None


def setup():
    if flag("INSTALL", "true"):
        sail = install()
        path_file = os.environ.get("GITHUB_PATH")
        if path_file:
            with open(path_file, "a", encoding="utf-8") as handle:
                handle.write(str(sail.parent) + "\n")
    else:
        found = subprocess.run(["sh", "-c", "command -v sail"], capture_output=True, text=True, check=False)
        if found.returncode != 0:
            raise Failure("install: false needs `sail` on PATH")
        sail = Path(found.stdout.strip())
    version = installed_version(sail)
    if version is None:
        raise Failure(f"{sail} --version did not report a Sail version")
    probe = subprocess.run([str(sail), "report", "--help"], capture_output=True, text=True, check=False)
    if probe.returncode != 0 or "github-summary" not in probe.stdout:
        raise Failure(f"Sail {version} has no `sail report`; this action needs Sail 0.3.0 or newer")
    status = subprocess.run([str(sail), "engine", "status"], capture_output=True, text=True, check=False)
    try:
        verifier_toolchain = json.loads(status.stdout)["rust_toolchain"]
    except (ValueError, KeyError, TypeError):
        verifier_toolchain = "unknown"
    project = project_dir()
    top = git_top(project)
    if top is None:
        top = project
    selected = toolchain_files(project, top)
    if selected and flag("INSTALL_TOOLCHAIN", "true"):
        install_project_toolchain(project)
    prefix = env("CACHE_KEY_PREFIX", "sail") or "sail"
    runner = f"{os.environ.get('RUNNER_OS', sys.platform)}-{os.environ.get('RUNNER_ARCH', os.uname().machine)}"
    tools = digest_files(selected, top)
    locks = digest_files(project_files(project, ("Cargo.lock",)), top)
    # One entry per project and argument list, so matrix jobs over
    # different arguments do not race for one key.
    place = str(project.relative_to(top) if project.is_relative_to(top) else project)
    where = hashlib.sha256((place + "\0" + env("ARGS")).encode()).hexdigest()[:8]
    commit = os.environ.get("GITHUB_SHA") or "local"
    # Most specific first: this commit; then the same lockfile and
    # toolchains; then anything from the same Sail and verifier toolchain.
    base = f"{runner}-{version}-{verifier_toolchain}-{where}"
    keys = {}
    for kind in ("results", "projects", "cargo"):
        stem = f"{prefix}-{kind}-{base}"
        keys[kind] = (f"{stem}-{tools}-{locks}-{commit}", f"{stem}-{tools}-{locks}-\n{stem}-{tools}-\n{stem}-")
    root = cache_root()
    targets = [line.strip() for line in env("CACHE_TARGET").splitlines() if line.strip()]
    cargo_home = Path(os.environ.get("CARGO_HOME") or (Path.home() / ".cargo"))
    cargo_paths = [str(cargo_home / "registry" / "index"), str(cargo_home / "registry" / "cache"),
                   str(cargo_home / "git" / "db")]
    cargo_paths += [str((project / target).resolve()) for target in targets]
    state = state_dir()
    sarif = env("SARIF") or str(state / "sail.sarif")
    write_multiline("results-restore-keys", keys["results"][1])
    write_multiline("projects-restore-keys", keys["projects"][1])
    write_multiline("cargo-restore-keys", keys["cargo"][1])
    write_multiline("cargo-paths", "\n".join(cargo_paths))
    write_multiline("projects-paths", f"{root / 'projects'}\n!{root / 'projects' / 'jobs'}")
    output(sail=sail, sail_version=version, project=project,
           result=state / "result.json", sarif=Path(sarif).resolve(),
           results_path=root / "results",
           results_key=keys["results"][0], projects_key=keys["projects"][0],
           cargo_key=keys["cargo"][0])
    print(f"Sail {version} ({sail}); verifier toolchain {verifier_toolchain}; project {project}")


def write_multiline(name, value):
    path = os.environ.get("GITHUB_OUTPUT")
    delimiter = "SAIL_ACTION_" + hashlib.sha256(value.encode()).hexdigest()[:12]
    text = f"{name}<<{delimiter}\n{value}\n{delimiter}\n"
    if path:
        with open(path, "a", encoding="utf-8") as handle:
            handle.write(text)
    else:
        sys.stdout.write(text)


def clean_environment():
    environment = dict(os.environ)
    # Credentials stay out of Cargo, build scripts and recorded job state.
    for name in ("GH_TOKEN", "GITHUB_TOKEN", "ACTIONS_RUNTIME_TOKEN", "ACTIONS_ID_TOKEN_REQUEST_TOKEN"):
        environment.pop(name, None)
    environment = {key: value for key, value in environment.items() if not key.startswith("SAIL_ACTION_")}
    environment.setdefault("SAIL_BACKGROUND_PREPARE", "0")
    return environment


def verify():
    sail = env("SAIL")
    result = Path(env("RESULT"))
    arguments = [sail, "verify", "--output", "json", *shlex.split(env("ARGS"))]
    print("+ " + shlex.join(arguments), flush=True)
    with result.open("wb") as handle:
        code = subprocess.run(arguments, cwd=env("PROJECT"), stdout=handle,
                              env=clean_environment(), check=False).returncode
    save_projects = "false"
    try:
        document = json.loads(result.read_bytes())
        save_projects = "false" if document.get("no_reuse") else "true"
    except (OSError, ValueError, AttributeError):
        command("warning", "sail verify printed no result document")
    output(exit_code=code, save_projects=save_projects)
    print(f"sail verify exited with {code}")


def find_baseline(directory):
    """The newest result artifact from another run on the baseline branch of
    this repository (not of a fork)."""
    branch, name = env("BASELINE_BRANCH"), env("RESULT_ARTIFACT")
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    if not branch:
        return None
    if not name:
        command("warning", "baseline-branch needs result-artifact, the artifact name to look for")
        return None
    query = subprocess.run(
        ["gh", "api", "--method", "GET", f"repos/{repository}/actions/artifacts",
         "-f", f"name={name}", "-f", "per_page=100"],
        capture_output=True, text=True, check=False)
    current = os.environ.get("GITHUB_RUN_ID", "")
    repository_id = os.environ.get("GITHUB_REPOSITORY_ID", "")
    run = ""
    if query.returncode == 0:
        try:
            # Newest first, as the API lists them.
            for artifact in json.loads(query.stdout).get("artifacts", []):
                origin = artifact.get("workflow_run") or {}
                # A pull request from a fork can have a branch of the same
                # name; only a run of this repository's own branch counts.
                if (not artifact.get("expired") and origin.get("head_branch") == branch
                        and origin.get("head_repository_id") is not None
                        and origin.get("head_repository_id") == origin.get("repository_id")
                        and (not repository_id or str(origin.get("repository_id")) == repository_id)
                        and str(origin.get("id")) != current):
                    run = str(origin["id"])
                    break
        except (ValueError, KeyError, AttributeError, TypeError):
            run = ""
    if not run:
        command("notice", f"no baseline: no unexpired `{name}` artifact from branch {branch}"
                + (f" ({query.stderr.strip()})" if query.returncode != 0 else ""))
        return None
    download = subprocess.run(["gh", "run", "download", run, "--repo", repository, "--name", name,
                               "--dir", str(directory)], capture_output=True, text=True, check=False)
    candidate = directory / "result.json"
    if download.returncode != 0 or not candidate.is_file():
        command("warning", f"no baseline: downloading `{name}` from run {run} failed: {download.stderr.strip()}")
        return None
    print(f"Baseline: `{name}` from run {run} on {branch}")
    return candidate


def report():
    sail = env("SAIL")
    result = env("RESULT")
    common = ["--repo-root", str(workspace())]
    baseline = env("BASELINE")
    if baseline:
        baseline_path = (workspace() / baseline).resolve()
        if not baseline_path.is_file():
            command("warning", f"no baseline: {baseline} does not exist")
            baseline_path = None
    else:
        baseline_path = find_baseline(state_dir() / "baseline")
    server, repository, commit = (os.environ.get(name) for name in
                                  ("GITHUB_SERVER_URL", "GITHUB_REPOSITORY", "GITHUB_SHA"))
    summary = ["--format", "github-summary", *common]
    if server and repository and commit:
        summary += ["--link-base", f"{server}/{repository}/blob/{commit}"]
    if baseline_path:
        summary += ["--baseline", str(baseline_path)]
    annotations = ["--format", "github-annotations", *common]
    if env("MAX_ANNOTATIONS"):
        annotations += ["--max-annotations", env("MAX_ANNOTATIONS")]
    step_summary = os.environ.get("GITHUB_STEP_SUMMARY")
    failures = []
    with (open(step_summary, "ab") if step_summary else open(os.devnull, "wb")) as handle:
        if subprocess.run([sail, "report", *summary, result], stdout=handle, check=False).returncode:
            failures.append("github-summary")
    sys.stdout.flush()
    if subprocess.run([sail, "report", *annotations, result], check=False).returncode:
        failures.append("github-annotations")
    sarif = Path(env("SARIF_PATH"))
    sarif.parent.mkdir(parents=True, exist_ok=True)
    with sarif.open("wb") as handle:
        if subprocess.run([sail, "report", "--format", "sarif", *common, result],
                          stdout=handle, check=False).returncode:
            failures.append("sarif")
    if failures:
        raise Failure("sail report failed for " + ", ".join(failures) + "; see the messages above")


def gc():
    limit = env("CACHE_MAX_BYTES")
    if limit:
        subprocess.run([env("SAIL"), "cache", "gc", "--max-bytes", limit], env=clean_environment(), check=False)


def finish():
    code = env("EXIT_CODE")
    if not code:
        raise Failure("sail verify did not run")
    if code != "0":
        print(f"sail verify exited with {code}", file=sys.stderr)
    sys.exit(int(code) if code.isdigit() else 1)


def main():
    steps = {"setup": setup, "times": times, "verify": verify, "report": report, "gc": gc, "finish": finish}
    if len(sys.argv) != 2 or sys.argv[1] not in steps:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    try:
        steps[sys.argv[1]]()
    except Failure as failure:
        command("error", str(failure))
        sys.exit(1)


if __name__ == "__main__":
    main()
