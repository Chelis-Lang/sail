#!/usr/bin/env python3
"""The verification step of the Sail GitHub Action.

Runs `sail verify --output json`, then renders the result with `sail report`:
annotations on standard output, the job summary, and a SARIF file. The step
itself always succeeds, so the steps after it (saving caches) still run; the
action's last step exits with the recorded `sail verify` exit code.

Standard library only. Inputs arrive as SAIL_ACTION_* environment variables.
"""
import hashlib
import os
import re
import shlex
import shutil
import subprocess
import sys
from pathlib import Path


def flag(name):
    return os.environ.get(name, "").strip().lower() == "true"


def command(level, message):
    """One workflow command; GitHub reads these from standard output."""
    data = message.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")
    print("::" + level + " title=Sail action::" + data, flush=True)


def report(sail, arguments, out=None):
    """Runs `sail report`; a rendering failure is a warning, never a verdict."""
    result = subprocess.run([sail, "report", *arguments], stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True, check=False)
    if result.returncode != 0:
        command("warning", "sail report " + " ".join(arguments[:2]) + " failed (exit "
                + str(result.returncode) + "): " + result.stderr.strip())
        return None
    if out is not None:
        with open(out, "a", encoding="utf-8") as stream:
            stream.write(result.stdout)
    return result.stdout


def workspace_version(manifest):
    """The `[workspace.package]` version of the Sail checkout the action is in."""
    text = Path(manifest).read_text(encoding="utf-8")
    section = text.split("[workspace.package]", 1)[1]
    return re.search(r'^version = "([^"]+)"', section, re.M).group(1)


def scope():
    """Names the arguments and directory: results compare only within one."""
    key = "\0".join(os.environ.get(name, "") for name in
                    ("SAIL_ACTION_ARGS", "SAIL_ACTION_WORKING_DIRECTORY"))
    return hashlib.sha256(key.encode()).hexdigest()[:16]


def baseline_note(found):
    """Says which saved result the comparison used, or that there was none."""
    key = os.environ.get("SAIL_ACTION_BASELINE_KEY", "")
    commit = os.environ.get("SAIL_ACTION_BASELINE_COMMIT", "")
    if not found or not key:
        return ("\n_No saved result was found for the base commit, so there is no baseline "
                "comparison. The action saves one on each push._\n")
    if commit and key.endswith(commit):
        return "\n_Baseline: the result saved for base commit `" + commit[:12] + "`._\n"
    return ("\n_Baseline: the most recent saved result (`" + key + "`), because none was saved "
            "for the base commit" + (" `" + commit[:12] + "`" if commit else "") + "._\n")


def main():
    if sys.argv[1:2] == ["--workspace-version"]:
        print(workspace_version(sys.argv[2]))
        return 0
    if sys.argv[1:2] == ["--scope"]:
        print(scope())
        return 0
    sail = os.environ.get("SAIL_ACTION_SAIL") or shutil.which("sail")
    if not sail:
        command("error", "sail is not on PATH; the install step did not complete")
        return 1
    work = Path(os.environ.get("SAIL_ACTION_OUT") or
                Path(os.environ.get("RUNNER_TEMP", ".")) / "sail-action")
    work.mkdir(parents=True, exist_ok=True)
    document, sarif = work / "result.json", work / "results.sarif"
    directory = os.environ.get("SAIL_ACTION_WORKING_DIRECTORY") or "."
    arguments = shlex.split(os.environ.get("SAIL_ACTION_ARGS", ""))

    # Verus's own output stays in the job log, byte for byte, on stderr.
    with open(document, "wb") as stream:
        verify = subprocess.run([sail, "verify", "--output", "json", *arguments],
                                cwd=directory, stdout=stream, check=False)
    code = verify.returncode
    # A signal has no exit status of its own; never report it as success.
    recorded = code if 0 <= code <= 255 else 1

    outputs = {"exit-code": str(recorded), "result": "", "sarif": ""}
    if document.stat().st_size == 0:
        command("error", "sail verify wrote no result document (exit " + str(code)
                + "); see the log above")
        recorded = recorded or 1
        outputs["exit-code"] = str(recorded)
    else:
        outputs["result"] = str(document)
        root = os.environ.get("GITHUB_WORKSPACE") or os.getcwd()
        common = [str(document), "--repo-root", root]
        if flag("SAIL_ACTION_ANNOTATIONS"):
            text = report(sail, ["--format", "github-annotations", *common])
            if text:
                sys.stdout.write(text)
                sys.stdout.flush()
        summary = ["--format", "github-summary", *common]
        if all(os.environ.get(name) for name in
               ("GITHUB_SERVER_URL", "GITHUB_REPOSITORY", "GITHUB_SHA")):
            summary += ["--link-base", "{}/{}/blob/{}".format(
                os.environ["GITHUB_SERVER_URL"], os.environ["GITHUB_REPOSITORY"],
                os.environ["GITHUB_SHA"])]
        baseline = os.environ.get("SAIL_ACTION_BASELINE", "")
        found = bool(baseline) and Path(baseline).is_file() and Path(baseline).stat().st_size > 0
        if found:
            summary += ["--baseline", baseline]
        target = os.environ.get("GITHUB_STEP_SUMMARY")
        text = report(sail, summary, out=target)
        if text is not None and baseline:
            note = baseline_note(found)
            text += note
            if target:
                with open(target, "a", encoding="utf-8") as stream:
                    stream.write(note)
        if text and not target:
            sys.stdout.write(text)
        if report(sail, ["--format", "sarif", *common], out=sarif) is not None:
            outputs["sarif"] = str(sarif)
        # The result the next default-branch run's pull requests compare with.
        save = os.environ.get("SAIL_ACTION_SAVE_BASELINE", "")
        if save:
            Path(save).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(document, save)

    limit = os.environ.get("SAIL_ACTION_CACHE_MAX_BYTES", "")
    if limit:
        # Bounds what the cache-save step uploads; never affects the result.
        subprocess.run([sail, "cache", "gc", "--max-bytes", limit], check=False,
                       stdout=subprocess.DEVNULL)

    target = os.environ.get("GITHUB_OUTPUT")
    if target:
        with open(target, "a", encoding="utf-8") as stream:
            for name, value in outputs.items():
                stream.write(name + "=" + value + "\n")
    print("sail verify exited with " + str(code), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
