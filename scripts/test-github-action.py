#!/usr/bin/env python3
"""Off-GitHub controls for the GitHub Action's steps (action/sail_action.py).

A stand-in `sail` records its calls, so these check what the action runs and
the outputs it writes; the hosted `action-*` jobs in .github/workflows/ci.yml
run the action itself."""
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent.parent
HELPER = ROOT / "action" / "sail_action.py"
spec = importlib.util.spec_from_file_location("sail_action", HELPER)
action = importlib.util.module_from_spec(spec)
spec.loader.exec_module(action)

FAKE_SAIL = r"""#!/usr/bin/env python3
import json, os, sys
with open(os.environ["FAKE_SAIL_LOG"], "a") as log:
    log.write(json.dumps({"argv": sys.argv[1:], "cwd": os.getcwd(),
                          "token": "GH_TOKEN" in os.environ,
                          "prepare": os.environ.get("SAIL_BACKGROUND_PREPARE")}) + "\n")
args = sys.argv[1:]
if args == ["--version"]:
    print("sail " + os.environ.get("FAKE_SAIL_VERSION", "9.9.9"))
elif args[:2] == ["report", "--help"]:
    if os.environ.get("FAKE_SAIL_NO_REPORT"):
        sys.exit(2)
    print("FORMATS: github-summary")
elif args[:2] == ["engine", "status"]:
    print(json.dumps({"rust_toolchain": "1.99.1-x86_64-unknown-linux-gnu"}))
elif args[:1] == ["verify"]:
    print(json.dumps({"kind": "sail-project-result", "no_reuse": json.loads(os.environ.get("FAKE_NO_REUSE", "null"))}))
    sys.exit(int(os.environ.get("FAKE_SAIL_EXIT", "0")))
elif args[:1] == ["report"]:
    fmt = args[args.index("--format") + 1]
    print({"github-summary": "## summary", "github-annotations": "::error::x", "sarif": "{}"}[fmt])
    sys.exit(int(os.environ.get("FAKE_REPORT_EXIT", "0")))
"""

FAKE_GH = r"""#!/usr/bin/env python3
import os, sys
args = sys.argv[1:]
if args[:1] == ["api"]:
    print(os.environ["FAKE_GH_ARTIFACTS"])
elif args[:2] == ["run", "download"]:
    directory = args[args.index("--dir") + 1]
    os.makedirs(directory, exist_ok=True)
    with open(os.path.join(directory, "result.json"), "w") as handle:
        handle.write("{}")
"""


def outputs(path):
    """Parse $GITHUB_OUTPUT, including `name<<DELIMITER` values."""
    values, lines = {}, Path(path).read_text().splitlines()
    index = 0
    while index < len(lines):
        line = lines[index]
        if "<<" in line and "=" not in line.split("<<")[0]:
            name, delimiter = line.split("<<", 1)
            end = lines.index(delimiter, index + 1)
            values[name] = "\n".join(lines[index + 1:end])
            index = end + 1
            continue
        name, _, value = line.partition("=")
        values[name] = value
        index += 1
    return values


class ActionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="sail-action-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        sail = self.bin / "sail"
        sail.write_text(FAKE_SAIL)
        sail.chmod(0o755)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.git("init", "-q")
        (self.repo / "crate" / "src").mkdir(parents=True)
        (self.repo / "crate" / "Cargo.lock").write_text("lock 1\n")
        (self.repo / "crate" / "src" / "lib.rs").write_text("// one\n")
        (self.repo / "rust-toolchain.toml").write_text("[toolchain]\nchannel = \"1.99.1\"\n")
        self.commit("first", 1_600_000_000)
        (self.repo / "crate" / "src" / "lib.rs").write_text("// two\n")
        self.commit("second", 1_700_000_000)
        self.log = self.root / "sail.log"
        self.github_output = self.root / "output"
        self.summary = self.root / "summary.md"
        self.env = {
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "HOME": str(self.root / "home"),
            "RUNNER_TEMP": str(self.root / "temp"),
            "GITHUB_WORKSPACE": str(self.repo),
            "GITHUB_OUTPUT": str(self.github_output),
            "GITHUB_STEP_SUMMARY": str(self.summary),
            "GITHUB_SHA": "abc123",
            "GITHUB_SERVER_URL": "https://github.com",
            "GITHUB_REPOSITORY": "owner/repo",
            "RUNNER_OS": "Linux",
            "RUNNER_ARCH": "X64",
            "FAKE_SAIL_LOG": str(self.log),
            "GH_TOKEN": "secret",
            "SAIL_ACTION_INSTALL": "false",
            "SAIL_ACTION_INSTALL_TOOLCHAIN": "false",
            "SAIL_ACTION_WORKING_DIRECTORY": "crate",
        }

    def git(self, *arguments, **kwargs):
        return subprocess.run(["git", "-C", str(self.repo), *arguments], check=True,
                              capture_output=True, **kwargs)

    def commit(self, message, seconds):
        self.git("add", "-A")
        stamp = f"@{seconds} +0000"
        environment = dict(os.environ, GIT_AUTHOR_DATE=stamp, GIT_COMMITTER_DATE=stamp,
                           GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid",
                           GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.invalid")
        self.git("commit", "-q", "--no-verify", "-m", message, env=environment)

    def run_step(self, step, **extra):
        self.github_output.write_text("")
        environment = dict(self.env, **extra)
        return subprocess.run([sys.executable, "-I", str(HELPER), step], env=environment,
                              capture_output=True, text=True, check=False)

    def calls(self):
        if not self.log.exists():
            return []
        return [json.loads(line) for line in self.log.read_text().splitlines()]

    def test_default_version_is_the_workspace_version(self):
        workspace = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
        text = (ROOT / "action" / "action.yml").read_text()
        default = re.search(r'  sail-version:\n(?:    .*\n)*?    default: "([^"]+)"', text)
        self.assertIsNotNone(default)
        self.assertEqual(default[1], workspace,
                         "action/action.yml's sail-version default must be the release it ships with")
        self.assertEqual(action.default_version(), workspace)

    def test_every_input_reaches_a_step(self):
        text = (ROOT / "action" / "action.yml").read_text()
        inputs = re.findall(r"^  ([a-z-]+):\n    description:", text.split("\noutputs:")[0], re.M)
        self.assertIn("sail-version", inputs)
        for name in inputs:
            self.assertIn("inputs." + name + " ", text.replace("}}", " }}"), name)
        # Every SAIL_ACTION_* variable the helper reads is set by some step.
        for name in set(re.findall(r'env\("([A-Z_]+)"', HELPER.read_text())):
            self.assertIn("SAIL_ACTION_" + name + ":", text, name)

    def test_setup_restores_times_of_clean_files_only(self):
        (self.repo / "rust-toolchain.toml").write_text("[toolchain]\nchannel = \"1.99.0\"\n")
        result = self.run_step("setup")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        # Times are restored by their own step, after the cache restores.
        self.assertNotEqual((self.repo / "crate" / "Cargo.lock").stat().st_mtime, 1_600_000_000)
        values = outputs(self.github_output)
        result = self.run_step("times")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual((self.repo / "crate" / "Cargo.lock").stat().st_mtime, 1_600_000_000)
        self.assertEqual((self.repo / "crate" / "src" / "lib.rs").stat().st_mtime, 1_700_000_000)
        # Modified: its content is not the commit's, so it keeps its own time.
        self.assertNotEqual((self.repo / "rust-toolchain.toml").stat().st_mtime, 1_600_000_000)
        self.assertEqual(values["sail-version"], "9.9.9")
        self.assertEqual(values["project"], str(self.repo / "crate"))
        self.assertTrue(values["results-key"].startswith("sail-results-Linux-X64-9.9.9-1.99.1-"))
        self.assertTrue(values["results-key"].endswith("-abc123"))
        restore = values["results-restore-keys"].splitlines()
        self.assertEqual(len(restore), 3)
        for prefix in restore:
            self.assertTrue(values["results-key"].startswith(prefix))
        self.assertEqual(values["results-path"], str(self.root / "home" / ".cache" / "sail" / "results"))
        self.assertIn("!", values["projects-paths"])

    def test_keys_follow_lockfiles_toolchains_and_arguments(self):
        def key(**extra):
            self.assertEqual(self.run_step("setup", **extra).returncode, 0)
            return outputs(self.github_output)["results-key"]
        first = key()
        self.assertEqual(key(), first)
        (self.repo / "crate" / "Cargo.lock").write_text("lock 2\n")
        second = key()
        self.assertNotEqual(second, first)
        (self.repo / "rust-toolchain.toml").write_text("[toolchain]\nchannel = \"1.98.0\"\n")
        third = key()
        self.assertNotEqual(third, second)
        self.assertNotEqual(key(SAIL_ACTION_ARGS="-p other"), third)
        self.assertNotEqual(key(FAKE_SAIL_VERSION="9.9.10"), third)

    def test_setup_refuses_a_sail_without_report(self):
        result = self.run_step("setup", FAKE_SAIL_NO_REPORT="1")
        self.assertEqual(result.returncode, 1)
        self.assertIn("has no `sail report`", result.stdout)

    def test_verify_keeps_the_exit_code_and_no_credentials(self):
        result_path = self.root / "result.json"
        common = {"SAIL_ACTION_SAIL": str(self.bin / "sail"), "SAIL_ACTION_RESULT": str(result_path),
                  "SAIL_ACTION_PROJECT": str(self.repo / "crate"), "SAIL_ACTION_ARGS": "-p 'a b' --lib"}
        result = self.run_step("verify", FAKE_SAIL_EXIT="1", **common)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs(self.github_output), {"exit-code": "1", "save-projects": "true"})
        call = self.calls()[-1]
        self.assertEqual(call["argv"], ["verify", "--output", "json", "-p", "a b", "--lib"])
        self.assertEqual(call["cwd"], str(self.repo / "crate"))
        self.assertFalse(call["token"])
        self.assertEqual(call["prepare"], "0")
        self.assertEqual(json.loads(result_path.read_text())["kind"], "sail-project-result")
        self.run_step("verify", FAKE_NO_REUSE='{"code": "SAIL3112"}', **common)
        self.assertEqual(outputs(self.github_output)["save-projects"], "false")

    def test_finish_ends_with_the_verify_exit_code(self):
        for code in ("0", "1", "101"):
            self.assertEqual(self.run_step("finish", SAIL_ACTION_EXIT_CODE=code).returncode, int(code))
        self.assertEqual(self.run_step("finish").returncode, 1)

    def test_report_writes_every_format(self):
        result_path = self.root / "result.json"
        result_path.write_text("{}")
        baseline = self.repo / "baseline.json"
        baseline.write_text("{}")
        sarif = self.root / "out" / "sail.sarif"
        common = {"SAIL_ACTION_SAIL": str(self.bin / "sail"), "SAIL_ACTION_RESULT": str(result_path),
                  "SAIL_ACTION_SARIF_PATH": str(sarif)}
        result = self.run_step("report", SAIL_ACTION_BASELINE="baseline.json",
                               SAIL_ACTION_MAX_ANNOTATIONS="5", **common)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("::error::x", result.stdout)
        self.assertEqual(self.summary.read_text(), "## summary\n")
        self.assertEqual(sarif.read_text(), "{}\n")
        summary, annotations, sarif_call = (call["argv"] for call in self.calls()[-3:])
        self.assertEqual(summary, ["report", "--format", "github-summary", "--repo-root", str(self.repo),
                                   "--link-base", "https://github.com/owner/repo/blob/abc123",
                                   "--baseline", str(baseline), str(result_path)])
        self.assertEqual(annotations[-3:], ["--max-annotations", "5", str(result_path)])
        self.assertEqual(sarif_call[:3], ["report", "--format", "sarif"])
        failed = self.run_step("report", FAKE_REPORT_EXIT="2", **common)
        self.assertEqual(failed.returncode, 1)
        self.assertIn("sail report failed for github-summary, github-annotations, sarif", failed.stdout)

    def test_missing_baseline_is_a_warning(self):
        result_path = self.root / "result.json"
        result_path.write_text("{}")
        result = self.run_step("report", SAIL_ACTION_SAIL=str(self.bin / "sail"),
                               SAIL_ACTION_RESULT=str(result_path),
                               SAIL_ACTION_SARIF_PATH=str(self.root / "s.sarif"),
                               SAIL_ACTION_BASELINE="missing.json")
        self.assertEqual(result.returncode, 0)
        self.assertIn("::warning::no baseline: missing.json does not exist", result.stdout)
        self.assertNotIn("--baseline", self.calls()[-3]["argv"])

    def test_restore_mtime_leaves_renamed_files_alone(self):
        self.git("mv", "crate/src/lib.rs", "crate/src/main.rs")
        restored = action.restore_mtime(self.repo)
        # The renamed file and its source are both left alone.
        self.assertEqual(restored, 2)
        self.assertEqual((self.repo / "crate" / "Cargo.lock").stat().st_mtime, 1_600_000_000)

    def head(self):
        return self.git("rev-parse", "HEAD", text=True).stdout.strip()

    def test_times_follow_merges_by_their_first_parent(self):
        self.git("checkout", "-q", "-b", "side")
        (self.repo / "crate" / "src" / "lib.rs").write_text("// side\n")
        self.commit("side", 1_650_000_000)
        self.git("checkout", "-q", "-")
        stamp = "@1800000000 +0000"
        environment = dict(os.environ, GIT_AUTHOR_DATE=stamp, GIT_COMMITTER_DATE=stamp,
                           GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@example.invalid",
                           GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@example.invalid")
        self.git("merge", "-q", "--no-ff", "--no-verify", "-m", "merge", "side", env=environment)
        action.restore_mtime(self.repo)
        # The merge brought the side commit's content: the merge's time, not
        # the older side commit's.
        self.assertEqual((self.repo / "crate" / "src" / "lib.rs").stat().st_mtime, 1_800_000_000)
        self.assertEqual((self.repo / "crate" / "Cargo.lock").stat().st_mtime, 1_600_000_000)

    def test_files_changed_since_a_restored_build_keep_their_time(self):
        # The cached target was built from the first commit, later than the
        # second commit's time; lib.rs changed since, so a restored commit
        # time would make Cargo trust that build of other content.
        first = self.git("rev-parse", "HEAD~1", text=True).stdout.strip()
        head = self.head()
        lib = self.repo / "crate" / "src" / "lib.rs"
        os.utime(lib, (1_900_000_000, 1_900_000_000))
        common = {"SAIL_ACTION_CACHE_TARGET": "target",
                  "SAIL_ACTION_CARGO_KEY": f"sail-cargo-x-{head}"}
        result = self.run_step("times", SAIL_ACTION_CARGO_MATCHED_KEY=f"sail-cargo-x-{first}", **common)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(lib.stat().st_mtime, 1_900_000_000)
        self.assertEqual((self.repo / "crate" / "Cargo.lock").stat().st_mtime, 1_600_000_000)
        # The same for managed targets restored from another commit.
        result = self.run_step("times", SAIL_ACTION_PROJECTS_KEY=f"sail-projects-x-{head}",
                               SAIL_ACTION_PROJECTS_MATCHED_KEY=f"sail-projects-x-{first}")
        self.assertEqual(lib.stat().st_mtime, 1_900_000_000)
        # This commit's own build: every clean file gets its commit time.
        self.run_step("times", SAIL_ACTION_CARGO_MATCHED_KEY=f"sail-cargo-x-{head}", **common)
        self.assertEqual(lib.stat().st_mtime, 1_700_000_000)
        # Without cache-target, the Cargo cache holds no build.
        os.utime(lib, (1_900_000_000, 1_900_000_000))
        self.run_step("times", SAIL_ACTION_CARGO_KEY=f"sail-cargo-x-{head}",
                      SAIL_ACTION_CARGO_MATCHED_KEY=f"sail-cargo-x-{first}")
        self.assertEqual(lib.stat().st_mtime, 1_700_000_000)

    def test_a_build_from_an_unknown_commit_restores_no_times(self):
        lock = self.repo / "crate" / "Cargo.lock"
        os.utime(lock, (1_900_000_000, 1_900_000_000))
        result = self.run_step("times", SAIL_ACTION_CACHE_TARGET="target",
                               SAIL_ACTION_CARGO_KEY=f"sail-cargo-x-{self.head()}",
                               SAIL_ACTION_CARGO_MATCHED_KEY="sail-cargo-x-" + "0" * 40)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("::warning::file times were not restored", result.stdout)
        self.assertEqual(lock.stat().st_mtime, 1_900_000_000)
        result = self.run_step("times", SAIL_ACTION_RESTORE_MTIME="false")
        self.assertEqual(result.returncode, 0)

    def test_baseline_ignores_fork_branches_of_the_same_name(self):
        gh = self.bin / "gh"
        gh.write_text(FAKE_GH)
        gh.chmod(0o755)
        artifacts = [
            # Newest first: a fork's `main`, then this repository's.
            {"expired": False, "workflow_run": {"id": 30, "head_branch": "main",
                                                "repository_id": 7, "head_repository_id": 99}},
            {"expired": False, "workflow_run": {"id": 20, "head_branch": "main",
                                                "repository_id": 7, "head_repository_id": 7}},
        ]
        result_path = self.root / "result.json"
        result_path.write_text("{}")
        result = self.run_step("report", SAIL_ACTION_SAIL=str(self.bin / "sail"),
                               SAIL_ACTION_RESULT=str(result_path),
                               SAIL_ACTION_SARIF_PATH=str(self.root / "s.sarif"),
                               SAIL_ACTION_BASELINE_BRANCH="main", SAIL_ACTION_RESULT_ARTIFACT="sail-result",
                               GITHUB_REPOSITORY_ID="7", FAKE_GH_ARTIFACTS=json.dumps({"artifacts": artifacts}))
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("Baseline: `sail-result` from run 20 on main", result.stdout)
        # Only the fork's: no baseline.
        result = self.run_step("report", SAIL_ACTION_SAIL=str(self.bin / "sail"),
                               SAIL_ACTION_RESULT=str(result_path),
                               SAIL_ACTION_SARIF_PATH=str(self.root / "s.sarif"),
                               SAIL_ACTION_BASELINE_BRANCH="main", SAIL_ACTION_RESULT_ARTIFACT="sail-result",
                               FAKE_GH_ARTIFACTS=json.dumps({"artifacts": artifacts[:1]}))
        self.assertIn("::notice::no baseline", result.stdout)


if __name__ == "__main__":
    unittest.main()
