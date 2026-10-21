#!/usr/bin/env python3
"""Compare Cargo verification scenarios with stock Verus, fork Verus, and Sail."""

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import resource
import shutil
import signal
import subprocess
import sys
import tempfile
import time

from bench_cargo_compare_report import (
    LIBRARY_KINDS, MODES, SCENARIOS, markdown_report, parse_proof, parse_workflow_proof,
    summarize, trial_order,
)


COUNTERS = ("sessions", "gets", "hits", "puts", "memory_hits")
MEMORY_BYTES = 256 * 1024 * 1024
WORKFLOWS = ("unchanged", "proof-edit")


def absolute(value):
    """Keep the executable name that a rustup dispatch shim requires."""
    return Path(os.path.abspath(os.path.expanduser(str(value))))


def executable(value):
    candidate = str(value)
    if os.sep not in candidate:
        candidate = shutil.which(candidate) or candidate
    path = absolute(candidate)
    if not path.is_file() or not os.access(path, os.X_OK):
        raise ValueError("Executable is not available: " + str(path))
    return path


def bytes_digest(value):
    return hashlib.sha256(value).hexdigest()


def digest(path):
    value = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def identity(path):
    path = absolute(path)
    return {"path": str(path), "resolved_path": str(path.resolve(strict=True)),
            "sha256": digest(path)}


def write_json(path, value):
    with Path(path).open("w", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write("\n")


def load_average():
    try:
        return list(os.getloadavg())
    except OSError:
        return None


def child_cpu():
    usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    return usage.ru_utime + usage.ru_stime


def stop_group(process):
    """Stop only the process group that this runner created."""
    if process is None:
        return
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except (ProcessLookupError, PermissionError):
        if process.poll() is None:
            raise
        process.wait()
        return
    try:
        process.wait(timeout=2.0)
    except subprocess.TimeoutExpired:
        pass
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except (ProcessLookupError, PermissionError):
        if process.poll() is None:
            raise
    process.wait()


def controlled_environment(cargo, cargo_home, rustup_home, home, temp):
    # Exclude Cargo, Rust, Verus, Sail, and loader controls from the caller.
    allowed = ("PATH", "LANG", "LC_ALL", "LC_CTYPE", "SYSTEMROOT", "SDKROOT",
               "MACOSX_DEPLOYMENT_TARGET", "CC", "CXX", "AR")
    env = {key: os.environ[key] for key in allowed if key in os.environ}
    env.update({
        "PATH": str(cargo.parent) + os.pathsep + env.get("PATH", os.defpath),
        "HOME": str(home), "CARGO_HOME": str(cargo_home),
        "RUSTUP_HOME": str(rustup_home), "CARGO": str(cargo),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_CACHE_HOME": str(home / ".cache"),
        "XDG_DATA_HOME": str(home / ".local/share"),
        "TMPDIR": str(temp), "TMP": str(temp), "TEMP": str(temp),
        "RUSTC_WRAPPER": "", "RUSTC_WORKSPACE_WRAPPER": "",
        "CARGO_BUILD_RUSTC_WRAPPER": "", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER": "",
        "CARGO_INCREMENTAL": "0",
        "CARGO_TERM_COLOR": "never", "GIT_CONFIG_NOSYSTEM": "1",
        "GIT_CONFIG_GLOBAL": os.devnull, "RUSTUP_AUTO_INSTALL": "0",
    })
    return env


def ancestor_config_problems(directory):
    problems = []
    for parent in (directory, *directory.parents):
        for name in ("config", "config.toml"):
            path = parent / ".cargo" / name
            if path.exists() or path.is_symlink():
                problems.append("Unpinned ancestor Cargo configuration: " + str(path))
    return problems


def local_source_problems(metadata, worktree):
    problems = []
    for package in metadata["packages"]:
        if package.get("source") is not None:
            continue
        manifest = Path(package["manifest_path"]).resolve(strict=True)
        try:
            manifest.relative_to(worktree)
        except ValueError:
            problems.append("Local dependency is outside the pinned worktree: " + str(manifest))
    return problems


def link_downloads(source, destination):
    links = []
    for relative in ("registry/cache", "registry/index", "registry/src", "git/db", "git/checkouts"):
        src = source / relative
        dst = destination / relative
        dst.parent.mkdir(parents=True, exist_ok=True)
        if src.is_dir():
            dst.symlink_to(src.resolve(), target_is_directory=True)
            links.append({"path": str(dst), "source": str(src.resolve())})
    return links


class SourceEdit:
    """Apply one exact source replacement in the private worktree."""

    def __init__(self, project, worktree, tracked, spec):
        if not isinstance(spec, dict) or any(not isinstance(spec.get(key), str)
                                             for key in ("path", "before", "after")):
            raise ValueError("The edit spec needs path, before, and after strings")
        raw = spec["path"]
        relative = PurePosixPath(raw)
        if (not raw or relative.is_absolute() or "\\" in raw
                or any(part in ("", ".", "..") for part in raw.split("/"))
                or relative.suffix != ".rs"):
            raise ValueError("The edit path must be a relative Rust source path without traversal")
        self.path = project.joinpath(*relative.parts)
        resolved = self.path.resolve(strict=True)
        try:
            resolved.relative_to(project.resolve(strict=True))
        except ValueError as error:
            raise ValueError("The edit path escapes the Cargo project") from error
        # A symlink edit can change a second tracked path. Refuse that ambiguity.
        cursor = self.path
        while cursor != project:
            if cursor.is_symlink():
                raise ValueError("The edit path must not contain a symlink")
            cursor = cursor.parent
        self.relative = self.path.relative_to(worktree).as_posix()
        if self.relative not in tracked or not self.path.is_file():
            raise ValueError("The edit path must identify a tracked Rust source file")
        before, after = spec["before"].encode("utf-8"), spec["after"].encode("utf-8")
        if not before:
            raise ValueError("The edit match must not be empty")
        if before == after:
            raise ValueError("The edit replacement must differ from its match")
        self.base = self.path.read_bytes()
        offset = self.base.find(before)
        if offset < 0 or self.base.find(before, offset + 1) >= 0:
            raise ValueError("The edit must match exactly once in the pinned source")
        self.edited = self.base[:offset] + after + self.base[offset + len(before):]
        self.hashes = {"base": bytes_digest(self.base), "edited": bytes_digest(self.edited)}
        self.state = "base"
        self.metadata = {"path": raw, "before_sha256": self.hashes["base"],
                         "after_sha256": self.hashes["edited"], "spec": spec}

    def problems(self, state):
        if state not in self.hashes:
            return ["Unknown source state: " + str(state)]
        try:
            if self.path.is_symlink() or digest(self.path) != self.hashes[state]:
                return ["The edit file differs from the declared " + state + " source state"]
        except OSError as error:
            return ["Cannot read the edit source: " + str(error)]
        return []

    def transition(self, state):
        if state not in self.hashes or state == self.state:
            raise ValueError("The source edit needs one change of state")
        problems = self.problems(self.state)
        if problems:
            raise RuntimeError(problems[0])
        self.path.write_bytes(self.edited if state == "edited" else self.base)
        self.state = state
        problems = self.problems(state)
        if problems:
            raise RuntimeError(problems[0])


class Runner:
    def __init__(self, args, out):
        self.args = args
        self.out = out
        self.rows = []
        self.metadata = {
            "schema_version": 2, "rounds": args.rounds, "modes": list(MODES),
            "scenarios": list(args.scenarios), "root_package_id": None,
            "root_crate": None, "edit": None,
            "problems": [], "arguments": vars(args).copy(), "argv": sys.argv,
            "started_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "host": {"architecture": platform.machine(), "os": platform.platform(),
                     "system": platform.system(), "release": platform.release(),
                     "hostname": platform.node(), "cpu_count": os.cpu_count(),
                     "load": load_average(), "isolation": "No host isolation is enforced"},
            "policy": {"fresh_target_per_invocation": ["fresh", "daemon-restart"],
                       "managed_workflows": list(WORKFLOWS), "frozen": True,
                       "serial": True, "daemon_memory_bytes": MEMORY_BYTES,
                       "timing_scope": "Cargo and complete verification. Restart includes new daemon startup, but excludes old daemon shutdown.",
                       "conformance": "Proof scope checks are not formal per-function conformance"},
            "setup_commands": [], "daemons": [],
        }
        self.daemon = None
        self.daemon_record = None
        self.daemon_streams = []
        self.scratch = None
        self.worktree = None
        self.repository = None
        self.env = None
        self.setup_index = 0
        self.scenario = None
        self.scenario_cache = None
        self.edit = None
        self.source_state = "base"
        self.warmups = {}
        self.samples = (out / "samples.jsonl").open("w", encoding="utf-8")

    def save_metadata(self):
        write_json(self.out / "metadata.json", self.metadata)

    def tool(self, label, command, env=None, cwd=None, check=True, timeout=None):
        self.setup_index += 1
        stem = "setup-{:03d}-{}".format(self.setup_index, label)
        command = [str(arg) for arg in command]
        result = self.capture(command, stem, env or self.env, cwd, timeout or self.args.timeout)
        self.metadata["setup_commands"].append(result)
        self.save_metadata()
        if result["interrupted"]:
            raise KeyboardInterrupt
        if check and (result["exit_code"] != 0 or result["problems"]):
            raise RuntimeError("Command failed: {}. See {}".format(label, result["stderr"]))
        text = (self.out / result["stdout"]).read_text(encoding="utf-8", errors="replace")
        return text, result

    def capture(self, command, stem, env, cwd, timeout):
        result = {"command": command, "cwd": str(cwd) if cwd else None,
                  "stdout": "logs/" + stem + ".stdout", "stderr": "logs/" + stem + ".stderr",
                  "exit_code": None, "wall_seconds": None, "client_cpu_seconds": None,
                  "problems": []}
        process = None
        interrupted = False
        with (self.out / result["stdout"]).open("wb") as stdout, (self.out / result["stderr"]).open("wb") as stderr:
            start_cpu = child_cpu()
            start = time.perf_counter()
            try:
                process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                           stdout=stdout, stderr=stderr, start_new_session=True)
                try:
                    result["exit_code"] = process.wait(timeout=timeout)
                except subprocess.TimeoutExpired:
                    result["problems"].append("Command timeout after {} seconds".format(timeout))
                    stop_group(process)
                    result["exit_code"] = process.returncode
            except KeyboardInterrupt:
                interrupted = True
                result["problems"].append("Command interrupted")
                stop_group(process)
                if process is not None:
                    result["exit_code"] = process.returncode
            except OSError as error:
                result["problems"].append("Cannot execute command: " + str(error))
                stderr.write((str(error) + "\n").encode())
            finally:
                result["wall_seconds"] = time.perf_counter() - start
                result["client_cpu_seconds"] = child_cpu() - start_cpu
        result["interrupted"] = interrupted
        return result

    def prepare(self):
        args = self.args
        self.metadata["implementation"] = {
            "runner": identity(Path(__file__)),
            "report": identity(Path(__file__).with_name("bench_cargo_compare_report.py")),
        }
        self.cargo = executable(args.cargo)
        self.sail = executable(args.sail)
        self.git = executable("git")
        project = absolute(args.project).resolve(strict=True)
        stock = absolute(args.stock_runtime).resolve(strict=True)
        cache_source = absolute(args.cargo_home or os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
        rustup = absolute(args.rustup_home or os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup"))).resolve()
        self.metadata["paths"] = {"project": str(project), "stock_runtime": str(stock),
                                  "sail": str(self.sail), "cargo": str(self.cargo),
                                  "cargo_home_source": str(cache_source), "rustup_home": str(rustup),
                                  "output": str(self.out)}
        self.scratch = Path(tempfile.mkdtemp(prefix="sail-cargo-", dir="/tmp")).resolve()
        if self.scratch == Path.home().resolve() or Path.home().resolve() in self.scratch.parents:
            raise RuntimeError("The private worktree must be outside the real home directory")
        configuration_issues = ancestor_config_problems(self.scratch)
        if configuration_issues:
            raise ValueError(". ".join(configuration_issues))
        home = self.scratch / "home"
        cargo_home = self.scratch / "cargo-home"
        temp = self.scratch / "tmp"
        for path in (home, cargo_home, temp):
            path.mkdir()
        self.metadata["download_cache_links"] = link_downloads(cache_source, cargo_home)
        self.env = controlled_environment(self.cargo, cargo_home, rustup, home, temp)
        self.env["SAIL_CACHE_DIR"] = str(self.out / "cache" / "setup")
        if args.engine_build:
            self.env["SAIL_VERUS_BUILD"] = str(absolute(args.engine_build).resolve(strict=True))
        discovery = self.env.copy()
        # Engine discovery can use the operator install, but not its configuration.
        if not args.engine_build:
            discovery["HOME"] = str(Path.home())
            discovery["SAIL_CACHE_DIR"] = str(absolute(os.environ.get(
                "SAIL_CACHE_DIR", str(Path(os.environ.get("XDG_CACHE_HOME", str(Path.home() / ".cache"))) / "sail"))))
        text, _ = self.tool("engine-status", [self.sail, "engine", "status"], env=discovery, cwd=self.scratch)
        status = json.loads(text)
        self.metadata["engine_status"] = status
        self.env["SAIL_VERUS_BUILD"] = str(absolute(status["build"]).resolve(strict=True))
        self.env["RUSTUP_TOOLCHAIN"] = status["rust_toolchain"]
        stock_version = stock / "version.json"
        stock_manifest = json.loads(stock_version.read_text(encoding="utf-8"))
        self.metadata["stock_version_manifest"] = {
            "identity": identity(stock_version), "contents": stock_manifest}
        if stock_manifest["verus"]["commit"] != status["commit"]:
            raise RuntimeError("Stock and fork upstream source commits differ")
        fork = absolute(status["verus"]).parent
        self.runtimes = {"stock": stock, "fork": fork}
        self.metadata["runtimes"] = {}
        suffix = ".dylib" if platform.system() == "Darwin" else ".so"
        runtime_files = (
            "cargo-verus", "verus", "rust_verify", "z3", "vstd.vir", "libvstd.rlib",
            "libverus_builtin.rlib", "libverus_builtin_macros" + suffix,
            "libverus_state_machines_macros" + suffix,
        )
        for mode, runtime in self.runtimes.items():
            files = {}
            for name in runtime_files:
                path = runtime / name
                if name in ("cargo-verus", "verus", "rust_verify", "z3"):
                    executable(path)
                files[name] = identity(path)
            version_env = dict(self.env, VERUS_Z3_PATH=str(runtime / "z3"))
            versions = {}
            for name in ("verus", "z3"):
                output, record = self.tool(mode + "-" + name + "-version", [runtime / name, "--version"],
                                           env=version_env, cwd=self.scratch, check=False)
                versions[name] = {"stdout": output, "exit_code": record["exit_code"],
                                  "stderr": record["stderr"]}
            self.metadata["runtimes"][mode] = {"directory": str(runtime), "files": files, "versions": versions}
        if self.metadata["runtimes"]["stock"]["files"]["z3"]["sha256"] != self.metadata["runtimes"]["fork"]["files"]["z3"]["sha256"]:
            raise RuntimeError("Stock and fork Z3 bytes differ")
        self.metadata["tools"] = {}
        rustc = executable(shutil.which("rustc", path=self.env["PATH"]) or "rustc")
        rustup_proxy = executable(shutil.which("rustup", path=self.env["PATH"]) or "rustup")
        for name, path in (("cargo", self.cargo), ("sail", self.sail), ("git", self.git),
                           ("rustc", rustc), ("rustup", rustup_proxy),
                           ("python", executable(sys.executable))):
            output, record = self.tool(name + "-version", [path, "--version"], cwd=self.scratch)
            self.metadata["tools"][name] = dict(identity(path), version=output, version_log=record["stdout"])
        self.metadata["tools"]["sail-z3"] = identity(
            executable(self.sail.resolve(strict=True).with_name("sail-z3")))
        for name in ("cargo", "rustc"):
            output, _ = self.tool("toolchain-" + name, [
                rustup_proxy, "which", "--toolchain", self.env["RUSTUP_TOOLCHAIN"], name],
                cwd=self.scratch)
            self.metadata["tools"]["toolchain-" + name] = identity(executable(output.strip()))
        if platform.system() == "Darwin":
            sysctl = executable("sysctl")
            hardware, _ = self.tool("host-hardware", [sysctl, "-n", "hw.model", "machdep.cpu.brand_string"], cwd=self.scratch)
            self.metadata["host"]["hardware"] = hardware.strip().splitlines()
        root, _ = self.tool("repository-root", [self.git, "-C", project, "rev-parse", "--show-toplevel"], cwd=self.scratch)
        self.repository = Path(root.strip()).resolve(strict=True)
        relative = project.relative_to(self.repository)
        commit, _ = self.tool("source-commit", [self.git, "-C", self.repository, "rev-parse", "--verify", args.revision + "^{commit}"], cwd=self.scratch)
        commit = commit.strip()
        if commit.lower() != args.revision.lower():
            raise ValueError("Revision must be the full source commit")
        self.worktree = self.scratch / "source"
        self.tool("worktree-add", [self.git, "-C", self.repository, "worktree", "add", "--detach", self.worktree, commit], cwd=self.scratch)
        self.project = self.worktree / relative
        self.metadata["source"] = {"repository": str(self.repository), "commit": commit,
                                   "worktree": str(self.worktree), "project": str(self.project)}
        tree, _ = self.tool("source-tree", [self.git, "-C", self.worktree, "rev-parse", "HEAD^{tree}"], cwd=self.scratch)
        self.metadata["source"]["tree"] = tree.strip()
        paths, _ = self.tool("source-files", [self.git, "-C", self.worktree, "ls-files", "-z"], cwd=self.scratch)
        tracked = set(paths.split("\0")) - {""}
        inputs = {}
        for name in sorted(tracked):
            path = self.worktree / name
            if (path.name in ("Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml")
                    or (path.parent.name == ".cargo" and path.name in ("config", "config.toml"))):
                inputs[name] = identity(path)
        self.metadata["source"]["cargo_inputs"] = inputs
        lock = self.project / "Cargo.lock"
        if not lock.is_file():
            # A selected workspace member uses the workspace lock file.
            text, _ = self.tool("workspace-manifest", [self.cargo, "locate-project", "--workspace", "--message-format", "plain"], cwd=self.project)
            lock = Path(text.strip()).parent / "Cargo.lock"
        if not lock.is_file():
            raise ValueError("The pinned project must contain Cargo.lock")
        self.lock = lock
        self.lock_hash = digest(lock)
        self.metadata["source"]["cargo_lock_sha256"] = self.lock_hash
        self.metadata["source"]["cargo_lock"] = str(lock)
        if args.edit_spec:
            spec_path = absolute(args.edit_spec).resolve(strict=True)
            spec_bytes = spec_path.read_bytes()
            spec = json.loads(spec_bytes.decode("utf-8"))
            self.edit = SourceEdit(self.project, self.worktree, tracked, spec)
            self.metadata["edit"] = dict(self.edit.metadata, identity=identity(spec_path),
                                         spec_sha256=bytes_digest(spec_bytes), captured_spec="edit-spec.json")
            write_json(self.out / "edit-spec.json", spec)
        self.env["CARGO_BUILD_JOBS"] = str(args.jobs)
        self.tool("fetch-locked", [self.cargo, "fetch", "--locked", "--manifest-path", self.project / "Cargo.toml"], cwd=self.project)
        if digest(lock) != self.lock_hash:
            raise RuntimeError("Cargo fetch changed the pinned lock file")
        self.env["CARGO_NET_OFFLINE"] = "true"
        text, record = self.tool("dependency-metadata", [self.cargo, "metadata", "--frozen", "--format-version", "1"], cwd=self.project)
        self.metadata["dependency_metadata"] = record["stdout"]
        self.metadata["dependency_metadata_sha256"] = digest(self.out / record["stdout"])
        dependency_metadata = json.loads(text)
        dependency_issues = local_source_problems(dependency_metadata, self.worktree)
        if dependency_issues:
            raise ValueError(". ".join(dependency_issues))
        if any(scenario in WORKFLOWS for scenario in args.scenarios):
            self.select_root(dependency_metadata)
        if self.source_problems("source-after-fetch", "base"):
            raise RuntimeError("Dependency setup changed the pinned source")
        self.socket = self.scratch / "d.sock"
        self.env["SAIL_DAEMON_SOCKET"] = str(self.socket)
        self.metadata["controlled_environment"] = self.env.copy()
        self.metadata["paths"].update({"isolated_home": str(home), "isolated_cargo_home": str(cargo_home),
                                       "engine_build": self.env["SAIL_VERUS_BUILD"], "socket": str(self.socket)})
        self.save_metadata()

    def select_root(self, value):
        manifest = (self.project / "Cargo.toml").resolve(strict=True)
        packages = [package for package in value["packages"]
                    if Path(package["manifest_path"]).resolve() == manifest]
        if len(packages) != 1 or packages[0]["id"] not in value["workspace_members"]:
            raise ValueError("Workflow scenarios need one package. Select a workspace member with --project")
        libraries = [target for target in packages[0]["targets"]
                     if any(kind in LIBRARY_KINDS for kind in target["kind"])]
        if len(libraries) != 1:
            raise ValueError("Workflow scenarios need one selected library target")
        self.metadata["root_package_id"] = packages[0]["id"]
        self.metadata["root_crate"] = libraries[0]["name"].replace("-", "_")
        self.metadata["root_package_name"] = packages[0]["name"]

    def begin_scenario(self, scenario):
        self.stop_daemon()
        self.scenario = scenario
        self.scenario_cache = self.out / "cache" / scenario
        self.scenario_cache.mkdir(parents=True, exist_ok=False)
        self.env["SAIL_CACHE_DIR"] = str(self.scenario_cache)
        self.start_daemon()
        self.save_metadata()

    def start_daemon(self, retained_disk_entries=None):
        if self.daemon is not None:
            raise RuntimeError("An owned daemon already exists")
        generation = sum(record["scenario"] == self.scenario for record in self.metadata["daemons"])
        stem = "daemon-{}-{:03d}".format(self.scenario, generation)
        command = [str(self.sail), "serve", "--socket", str(self.socket),
                   "--cache", str(self.scenario_cache / "results"),
                   "--memory-bytes", str(MEMORY_BYTES), "--idle-timeout", "7200"]
        record = {"scenario": self.scenario, "generation": generation, "command": command,
                  "stdout": "logs/" + stem + ".stdout", "stderr": "logs/" + stem + ".stderr"}
        self.metadata["daemons"].append(record)
        self.daemon_record = record
        self.daemon_streams = [(self.out / record["stdout"]).open("wb"),
                               (self.out / record["stderr"]).open("wb")]
        start = time.perf_counter()
        self.daemon = subprocess.Popen(command, cwd=self.project, env=self.env, stdin=subprocess.DEVNULL,
                                       stdout=self.daemon_streams[0], stderr=self.daemon_streams[1], start_new_session=True)
        record["pid"] = self.daemon.pid
        deadline = time.monotonic() + 20
        attempt = 0
        while time.monotonic() < deadline:
            attempt += 1
            snapshot, problems = self.cache_snapshot(stem + "-ready-" + str(attempt), record=False)
            if not problems:
                if any(number != 0 for key, number in snapshot.items() if key not in ("pid", "state")):
                    raise RuntimeError("The new daemon contains previous requests")
                if snapshot["state"]["memory_entries"] != 0 or snapshot["state"]["pending_writes"] != 0:
                    raise RuntimeError("The new daemon does not have empty memory and write state")
                expected_disk = 0 if retained_disk_entries is None else retained_disk_entries
                if snapshot["state"]["disk_entries"] != expected_disk:
                    raise RuntimeError("The new daemon disk state differs from its declared cache state")
                record["initial_snapshot"] = snapshot
                record["startup_seconds"] = time.perf_counter() - start
                return snapshot, record["startup_seconds"]
            if self.daemon.poll() is not None:
                raise RuntimeError("The owned Sail daemon exited before readiness")
            time.sleep(0.05)
        raise RuntimeError("The owned Sail daemon did not become ready")

    def stop_daemon(self):
        if self.daemon is not None:
            stop_group(self.daemon)
            if self.daemon_record is not None:
                self.daemon_record["exit_code"] = self.daemon.returncode
            self.daemon = None
            if self.socket.exists():
                self.socket.unlink()
        for stream in self.daemon_streams:
            stream.close()
        self.daemon_streams = []

    def cache_snapshot(self, label, record=True):
        problems = []
        command = [str(self.sail), "status", "--socket", str(self.socket), "--json"]
        if record:
            text, result = self.tool(label, command, cwd=self.project, check=False, timeout=10)
        else:
            result = self.capture(command, label, self.env, self.project, 2)
            text = (self.out / result["stdout"]).read_text(encoding="utf-8", errors="replace")
        if result.get("interrupted"):
            raise KeyboardInterrupt
        try:
            value = json.loads(text)
            info, stats = value["info"], value["stats"]
            snapshot = {"pid": info["pid"], "sessions": stats["hellos"]}
            snapshot.update({key: number for key, number in stats.items() if key != "hellos"})
            snapshot["state"] = {"memory_entries": info["memory"]["entries"],
                                 "disk_entries": value["disk"]["entries"],
                                 "pending_writes": info["queue"]["entries"] + info["memory"]["pending"]}
            if result["exit_code"] != 0 or not value["running"]:
                problems.append("Daemon status command failed")
            if self.daemon is None or snapshot["pid"] != self.daemon.pid or self.daemon.poll() is not None:
                problems.append("Daemon PID does not match the owned process")
            if info["memory"]["capacity_bytes"] != MEMORY_BYTES:
                problems.append("Daemon memory capacity differs from 256 MiB")
            if Path(info["cache_root"]).resolve() != (self.scenario_cache / "results").resolve():
                problems.append("Daemon cache is not the private scenario cache")
            if info.get("shared") is not None:
                problems.append("The private daemon shared cache must be off")
            for key in COUNTERS:
                if type(snapshot[key]) is not int or snapshot[key] < 0:
                    problems.append("Invalid daemon counter: " + key)
            for key, number in snapshot["state"].items():
                if type(number) is not int or number < 0:
                    problems.append("Invalid daemon state: " + key)
            return snapshot, problems + result["problems"]
        except (ValueError, TypeError, KeyError) as error:
            return {}, ["Daemon status is not available: " + str(error)] + result["problems"]

    def restart_daemon(self, stem, row):
        deadline = time.monotonic() + min(self.args.timeout, 30)
        attempt = 0
        while True:
            attempt += 1
            old, problems = self.cache_snapshot(stem + "-drain-" + str(attempt))
            if problems:
                raise RuntimeError("Cannot drain the private daemon: " + ". ".join(problems))
            if old["state"]["pending_writes"] == 0:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("The private daemon writes did not drain")
            time.sleep(0.05)
        retained = old["state"]["disk_entries"]
        row["restart"] = {"old_pid": old["pid"], "old_disk_entries": retained, "old_snapshot": old}
        if retained <= 0:
            raise RuntimeError("The restart needs positive retained disk entries")
        self.stop_daemon()
        start = time.perf_counter()
        try:
            before, startup_seconds = self.start_daemon(retained_disk_entries=retained)
        finally:
            row["startup_seconds"] = time.perf_counter() - start
        row["startup_seconds"] = startup_seconds
        row["cache_before"] = before
        if before["pid"] == row["restart"]["old_pid"]:
            raise RuntimeError("The restarted daemon must have a new PID")
        self.save_metadata()
        return before

    def runtime_problems(self):
        entries = list(self.metadata["tools"].values())
        entries.extend(self.metadata["implementation"].values())
        entries.append(self.metadata["stock_version_manifest"]["identity"])
        for runtime in self.metadata["runtimes"].values():
            entries.extend(runtime["files"].values())
        problems = ancestor_config_problems(self.scratch)
        for entry in entries:
            try:
                if (str(Path(entry["path"]).resolve(strict=True)) != entry["resolved_path"]
                        or digest(entry["path"]) != entry["sha256"]):
                    problems.append("Runtime input changed: " + entry["path"])
            except OSError as error:
                problems.append("Runtime input is not available: {}: {}".format(entry["path"], error))
        return problems

    def source_problems(self, label, state):
        problems = []
        if state not in ("base", "edited") or (state == "edited" and self.edit is None):
            return ["The declared source state is not available"]
        text, record = self.tool(label, [self.git, "-C", self.worktree, "status", "--porcelain", "-z",
                                        "--untracked-files=no"], cwd=self.project, check=False, timeout=30)
        if record["exit_code"] != 0 or record["problems"]:
            return ["Cannot establish the pinned source state"]
        expected = " M " + self.edit.relative + "\0" if state == "edited" else ""
        if text != expected:
            problems.append("Tracked source differs from the declared {} state. See {}".format(state, record["stdout"]))
        if self.edit is not None:
            problems.extend(self.edit.problems(state))
        head, head_record = self.tool(label + "-head", [self.git, "-C", self.worktree, "rev-parse", "HEAD"],
                                      cwd=self.project, check=False, timeout=30)
        if (head_record["exit_code"] != 0 or head_record["problems"]
                or head.strip() != self.metadata["source"]["commit"]):
            problems.append("The private worktree no longer has the pinned commit")
        if digest(self.lock) != self.lock_hash:
            problems.append("The pinned Cargo.lock changed")
        return problems

    def set_source_state(self, state):
        if state == self.source_state:
            return
        problems = self.source_problems("edit-transition-before", self.source_state)
        if problems:
            raise RuntimeError("Cannot change source state: " + ". ".join(problems))
        self.edit.transition(state)
        self.source_state = state
        problems = self.source_problems("edit-transition-after", state)
        if problems:
            raise RuntimeError("The source edit state is not valid: " + ". ".join(problems))

    def command(self, mode, target, scenario=None):
        scenario = scenario or self.scenario
        managed = mode == "sail" and scenario in WORKFLOWS
        cargo_args = ["--frozen"]
        if not managed:
            cargo_args.extend(["--target-dir", str(target)])
        # Cargo-Verus consumes unknown options as Cargo arguments. Keep its flags first.
        cargo_args.extend(["--fwd-verus-args-to", "all"])
        if scenario in WORKFLOWS:
            cargo_args.extend(["--package", self.metadata["root_package_name"]])
        if mode != "sail" and scenario in WORKFLOWS:
            cargo_args.append("--message-format=json")
        cargo_args.append("--lib")
        flags = ["--triggers-mode", "silent", "--rlimit", str(self.args.rlimit),
                 "--num-threads", str(self.args.threads)]
        if mode == "sail":
            return [str(self.sail), "verify", "--output", "json"] + cargo_args + ["--"] + flags
        return [str(self.runtimes[mode] / "cargo-verus"), "verus", "verify"] + cargo_args + ["--"] + flags

    def sample_environment(self, mode, target, managed_cache):
        env = self.env.copy()
        if self.scenario in WORKFLOWS:
            env.pop("CARGO_INCREMENTAL", None)
        if mode == "sail" and self.scenario in WORKFLOWS:
            env["SAIL_CACHE_DIR"] = str(managed_cache)
            env.pop("CARGO_TARGET_DIR", None)
            env.pop("CARGO_BUILD_TARGET_DIR", None)
            env.pop("CARGO_BUILD_BUILD_DIR", None)
            for key in ("RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                        "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"):
                env.pop(key, None)
        else:
            env["CARGO_TARGET_DIR"] = str(target)
            env["CARGO_BUILD_BUILD_DIR"] = str(target / "build")
        if mode in self.runtimes:
            runtime = self.runtimes[mode]
            env = {key: value for key, value in env.items() if not key.startswith("SAIL_")}
            env["VERUS_Z3_PATH"] = str(runtime / "z3")
            env["PATH"] = str(runtime) + os.pathsep + env["PATH"]
        return env

    def proof(self, mode, stdout, stderr):
        if self.scenario in WORKFLOWS:
            return parse_workflow_proof(mode, stdout, stderr,
                                        self.metadata["root_package_id"], self.metadata["root_crate"])
        return parse_proof(mode, stdout)

    def baseline_key(self, mode, round_number):
        return (self.scenario, round_number if self.scenario == "proof-edit" else 0, mode)

    def baseline_ok(self, row):
        proof = row["proof"]
        valid = (row["exit_code"] == 0 and not row["problems"] and not proof.get("problems")
                 and proof.get("verified", 0) > 0 and proof.get("errors") == 0)
        if self.scenario in WORKFLOWS:
            valid = valid and proof.get("outcome") == "verified" and proof.get("root_verified", 0) > 0
        return valid

    def sample(self, phase, round_number, mode, target=None, managed_cache=None, artifact_context=None):
        stem = "{}-{}-{}-{}".format(self.scenario, phase, round_number, mode)
        owned_target = target is None and not (mode == "sail" and self.scenario in WORKFLOWS)
        if owned_target:
            target = Path(tempfile.mkdtemp(prefix=stem + "-", dir=str(self.scratch)))
        managed_cache = managed_cache or self.scenario_cache
        command = self.command(mode, target)
        env = self.sample_environment(mode, target, managed_cache)
        problems = []
        interrupted = False
        state = "edited" if self.scenario == "proof-edit" and phase == "measured" else "base"
        context = artifact_context or (managed_cache if target is None else target)
        row = {"scenario": self.scenario, "phase": phase, "round": round_number, "mode": mode,
               "source_state": state, "command": command, "environment": env, "problems": problems,
               "target_dir": str(target) if target is not None else None,
               "artifact_context": str(context),
               "source_checks": {}, "startup_seconds": 0.0}
        if self.edit is not None:
            row["edit_sha256"] = self.edit.hashes[state]
        try:
            source_issues = self.source_problems(stem + "-source-before", state)
            row["source_checks"]["before"] = source_issues
            problems.extend(source_issues)
            runtime_issues = self.runtime_problems()
            problems.extend(runtime_issues)
            before, issues = self.cache_snapshot(stem + "-before")
            problems.extend(issues)
            key = self.baseline_key(mode, round_number)
            baseline_required = phase == "measured" and (mode == "sail" or self.scenario in WORKFLOWS)
            if baseline_required and not self.warmups.get(key, False):
                problems.append("The sample needs a successful baseline")
            refuse = bool(source_issues or runtime_issues or (baseline_required and not self.warmups.get(key, False)))
            if mode == "sail" and problems:
                refuse = True
            if not refuse and self.scenario == "daemon-restart" and phase == "measured" and mode == "sail":
                before = self.restart_daemon(stem, row)
            row["cache_before"] = before
            row["load_before"] = load_average()
            if refuse:
                problems.append("Sample refused because its source, private cache, or baseline is not valid")
                result = {"exit_code": None, "wall_seconds": None, "client_cpu_seconds": None,
                          "stdout": "logs/" + stem + ".stdout", "stderr": "logs/" + stem + ".stderr"}
                (self.out / result["stdout"]).touch()
                (self.out / result["stderr"]).write_text(problems[-1] + "\n", encoding="utf-8")
            else:
                result = self.capture(command, stem, env, self.project, self.args.timeout)
                interrupted = result.pop("interrupted")
                problems.extend(result.pop("problems"))
                result["wall_seconds"] += row["startup_seconds"]
            row.update(result)
            row["load_after"] = load_average()
            after, issues = self.cache_snapshot(stem + "-after")
            problems.extend(issues)
            row["cache_after"] = after
            row["cache_delta"] = {}
            for key in (before.keys() & after.keys()) - {"pid", "state"}:
                if type(before[key]) is int and type(after[key]) is int:
                    row["cache_delta"][key] = after[key] - before[key]
                    if after[key] < before[key]:
                        problems.append("Daemon counter decreased: " + key)
            if before.get("pid") != after.get("pid"):
                problems.append("Daemon PID changed during the sample")
            stdout = (self.out / row["stdout"]).read_text(encoding="utf-8", errors="replace")
            stderr = (self.out / row["stderr"]).read_text(encoding="utf-8", errors="replace")
            row["proof"] = self.proof(mode, stdout, stderr)
            source_issues = self.source_problems(stem + "-source-after", state)
            row["source_checks"]["after"] = source_issues
            problems.extend(source_issues)
            problems.extend(self.runtime_problems())
        except KeyboardInterrupt:
            interrupted = True
            problems.append("Sample interrupted")
        except Exception as error:
            problems.append("Sample error: {}: {}".format(type(error).__name__, error))
        finally:
            row.setdefault("exit_code", None)
            row.setdefault("wall_seconds", None)
            row.setdefault("client_cpu_seconds", None)
            row.setdefault("cache_before", {})
            row.setdefault("cache_after", {})
            row.setdefault("cache_delta", {})
            if "proof" not in row:
                row["proof"] = self.proof(mode, "", "")
            row.setdefault("stdout", "logs/" + stem + ".stdout")
            row.setdefault("stderr", "logs/" + stem + ".stderr")
            for field in ("stdout", "stderr"):
                (self.out / row[field]).touch(exist_ok=True)
            if phase == "warmup":
                self.warmups[self.baseline_key(mode, round_number)] = self.baseline_ok(row)
            self.rows.append(row)
            self.samples.write(json.dumps(row, sort_keys=True, allow_nan=False) + "\n")
            self.samples.flush()
            os.fsync(self.samples.fileno())
            progress = {key: row.get(key) for key in
                        ("scenario", "phase", "round", "mode", "source_state", "exit_code", "wall_seconds")}
            progress["problems"] = problems + row["proof"].get("problems", [])
            print(json.dumps(progress, sort_keys=True, allow_nan=False), flush=True)
            if owned_target:
                shutil.rmtree(target)
        if interrupted:
            raise KeyboardInterrupt
        return row

    def run_scenario(self, scenario):
        self.begin_scenario(scenario)
        if scenario == "proof-edit":
            for round_index in range(self.args.rounds):
                for mode in trial_order(round_index):
                    context = Path(tempfile.mkdtemp(prefix="proof-edit-{}-{}-".format(round_index + 1, mode), dir=str(self.scratch)))
                    target = context / "target" if mode != "sail" else None
                    managed = context / "managed"
                    try:
                        self.set_source_state("base")
                        self.sample("warmup", round_index + 1, mode, target, managed, context)
                        self.set_source_state("edited")
                        self.sample("measured", round_index + 1, mode, target, managed, context)
                    finally:
                        self.set_source_state("base")
                        shutil.rmtree(context)
            return
        targets = {}
        if scenario == "unchanged":
            for mode in MODES:
                if mode != "sail":
                    targets[mode] = Path(tempfile.mkdtemp(prefix="unchanged-" + mode + "-", dir=str(self.scratch)))
        try:
            for mode in trial_order(0):
                self.sample("warmup", 0, mode, target=targets.get(mode))
            for round_index in range(self.args.rounds):
                for mode in trial_order(round_index):
                    self.sample("measured", round_index + 1, mode, target=targets.get(mode))
        finally:
            for target in targets.values():
                shutil.rmtree(target)

    def cleanup(self):
        handlers = {number: signal.signal(number, signal.SIG_IGN) for number in (signal.SIGINT, signal.SIGTERM)}
        try:
            try:
                self.stop_daemon()
            except Exception as error:
                self.metadata["problems"].append("Daemon cleanup error: " + str(error))
            if self.worktree is not None and self.repository is not None:
                try:
                    _, result = self.tool("worktree-remove", [self.git, "-C", self.repository, "worktree", "remove", "--force", self.worktree],
                                          cwd=self.scratch, check=False, timeout=60)
                    if result["exit_code"] != 0:
                        self.metadata["problems"].append("Cannot release the owned worktree. See " + result["stderr"])
                except Exception as error:
                    self.metadata["problems"].append("Worktree cleanup error: " + str(error))
            if self.scratch is not None:
                try:
                    shutil.rmtree(self.scratch)
                except OSError as error:
                    self.metadata["problems"].append("Private directory cleanup error: " + str(error))
        finally:
            for number, handler in handlers.items():
                signal.signal(number, handler)
            self.samples.close()

    def execute(self):
        interrupted = False
        try:
            self.save_metadata()
            self.prepare()
            for scenario in self.args.scenarios:
                try:
                    self.run_scenario(scenario)
                except KeyboardInterrupt:
                    raise
                except Exception as error:
                    self.metadata.setdefault("scenario_problems", {}).setdefault(scenario, []).append(
                        "Scenario error: {}: {}".format(type(error).__name__, error))
                    # A source failure is a safety failure for all later scenarios.
                    issues = self.source_problems("scenario-error-source", "base")
                    if issues:
                        self.metadata["problems"].extend(issues)
                        break
        except KeyboardInterrupt:
            interrupted = True
            self.metadata["problems"].append("Benchmark interrupted")
        except Exception as error:
            self.metadata["problems"].append("Benchmark error: {}: {}".format(type(error).__name__, error))
        finally:
            self.cleanup()
            self.metadata["finished_at"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
            self.save_metadata()
            summary = summarize(self.metadata, self.rows)
            write_json(self.out / "summary.json", summary)
            (self.out / "summary.md").write_text(markdown_report(summary), encoding="utf-8")
        print("Report: " + str(self.out / "summary.md"))
        return 130 if interrupted else (0 if summary["valid"] else 1)


def positive_integer(value):
    number = int(value)
    if number <= 0:
        raise argparse.ArgumentTypeError("Value must be positive")
    return number


def interrupt(signum, frame):
    raise KeyboardInterrupt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", required=True, help="Cargo project in a Git repository")
    parser.add_argument("--revision", required=True, help="Full pinned source commit")
    parser.add_argument("--stock-runtime", required=True, help="Stock Verus runtime directory")
    parser.add_argument("--sail", required=True, help="Sail executable")
    parser.add_argument("--out", required=True, help="New result directory")
    parser.add_argument("--cargo", default="cargo", help="Cargo executable or rustup shim")
    parser.add_argument("--cargo-home", help="Source of registry and Git download caches")
    parser.add_argument("--rustup-home", help="Rustup toolchain directory")
    parser.add_argument("--engine-build", help="Explicit Sail engine build")
    parser.add_argument("--rounds", type=positive_integer, default=3)
    parser.add_argument("--threads", type=positive_integer, default=4)
    parser.add_argument("--jobs", type=positive_integer, default=4)
    parser.add_argument("--rlimit", type=positive_integer, default=20)
    parser.add_argument("--timeout", type=positive_integer, default=1800, help="Per-command timeout in seconds")
    parser.add_argument("--scenarios", nargs="+", choices=SCENARIOS, default=["fresh"])
    parser.add_argument("--edit-spec", help="JSON source edit for the proof-edit scenario")
    args = parser.parse_args()
    if not re.fullmatch(r"(?:[0-9a-fA-F]{40}|[0-9a-fA-F]{64})", args.revision):
        parser.error("--revision requires a full commit hash")
    if len(set(args.scenarios)) != len(args.scenarios):
        parser.error("--scenarios must not contain duplicates")
    if "proof-edit" in args.scenarios and not args.edit_spec:
        parser.error("--edit-spec is required for proof-edit")
    if args.edit_spec and "proof-edit" not in args.scenarios:
        parser.error("--edit-spec requires the proof-edit scenario")
    out = absolute(args.out).resolve()
    try:
        out.mkdir(parents=True, exist_ok=False)
    except FileExistsError:
        parser.error("--out must be a new directory")
    (out / "logs").mkdir()
    signal.signal(signal.SIGTERM, interrupt)
    return Runner(args, out).execute()


if __name__ == "__main__":
    sys.exit(main())
