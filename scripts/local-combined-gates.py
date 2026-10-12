#!/usr/bin/env python3
"""Local-only frozen workspace gate, then original solver suite; no image lifecycle."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import tempfile
import time

sys.dont_write_bytecode = True

GATE_LIMIT = 3600
SOLVER_LIMIT = 1800
META_LIMIT = 30
MAX_ROWS = 8192
MAX_DEAD_TASKS = 64
MAX_FILE = 512 * 1024 * 1024
SUPPORT = "scripts/m7-tspec-transcript-run.py"


def sha(path):
    value = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def load(path):
    with path.open("rb") as handle:
        raw = handle.read(32 * 1024 * 1024 + 1)
    if len(raw) > 32 * 1024 * 1024:
        raise ValueError("metadata byte bound exceeded")
    return json.loads(raw)


def phase_command(name):
    if name not in ("gate", "solver"):
        raise ValueError("unsupported phase")
    return ["bash", "ci/run.sh", name]


class Closure:
    """Sticky uncertainty cannot be discharged by leader reap or later success."""
    def __init__(self):
        self.reasons = []

    def uncertain(self, reason):
        if reason not in self.reasons:
            self.reasons.append(reason)

    def require(self):
        if self.reasons:
            raise RuntimeError("closure uncertain; separate owner audit required: " + ", ".join(self.reasons))


def sequence(run, closure):
    """Never launch solver after a failed, timed-out or uncertain gate."""
    for name, limit in (("gate", GATE_LIMIT), ("solver", SOLVER_LIMIT)):
        closure.require()
        if run(name, phase_command(name), limit) != 0:
            return False
    closure.require()
    return True


def bounded_command(command, *, timeout, closure=None, cwd=None, payload=None,
                    output=None, capture_limit=1024 * 1024, check=True):
    """Bound the local transport even when a signal interrupts communicate()."""
    with tempfile.TemporaryFile() as capture:
        process = None
        try:
            process = subprocess.Popen(command, cwd=cwd, stdin=subprocess.PIPE if payload is not None else subprocess.DEVNULL,
                                       stdout=output if output is not None else capture,
                                       stderr=subprocess.STDOUT, start_new_session=True)
            process.communicate(input=payload, timeout=timeout)
            capture.seek(0)
            raw = capture.read(capture_limit + 1) if output is None else b""
            if len(raw) > capture_limit:
                raise ValueError("transport capture bound exceeded")
            if check and process.returncode:
                raise subprocess.CalledProcessError(process.returncode, command, output=raw)
            return subprocess.CompletedProcess(command, process.returncode, raw)
        except BaseException:
            if process is not None and process.poll() is None:
                if closure is not None:
                    closure.uncertain("transport termination; native closure requires separate audit")
                # This only owns the unreaped LOCAL transport group, not native work.
                for number in (signal.SIGTERM, signal.SIGKILL):
                    if process.poll() is not None:
                        break
                    try:
                        os.killpg(process.pid, number)
                    except ProcessLookupError:
                        pass
                    try:
                        process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        pass
            raise


def identity(container, closure=None):
    raw = bounded_command(["docker", "inspect", "--format", "{{json .}}", container],
                          timeout=META_LIMIT, closure=closure).stdout
    value = json.loads(raw)
    if not value["State"]["Running"]:
        raise ValueError("container not running")
    return {"id": value["Id"], "image": value["Image"], "birth": value["State"]["StartedAt"], "created": value["Created"]}


def inventory(archive):
    files, links, directories, seen = {}, {}, {}, set()
    with tarfile.open(archive, "r:") as handle:
        for count, member in enumerate(handle, 1):
            if count > MAX_ROWS:
                raise ValueError("archive member bound exceeded")
            path = Path(member.name)
            if path.is_absolute() or ".." in path.parts or len(member.name.encode()) > 4096:
                raise ValueError("unsafe archive path")
            if member.mode & ~0o777:
                raise ValueError("special archive permission bits")
            name = str(path)
            if name == "." or name != member.name.rstrip("/"):
                raise ValueError("noncanonical archive path")
            if name in seen:
                raise ValueError("duplicate archive member")
            seen.add(name)
            if member.isdir():
                directories[name] = member.mode
                continue
            if member.issym():
                if Path(member.linkname).is_absolute() or ".." in Path(member.linkname).parts:
                    raise ValueError("unsafe archive link")
                links[member.name] = member.linkname
            elif member.isfile() and member.size <= MAX_FILE:
                value = hashlib.sha256()
                with handle.extractfile(member) as stream:
                    for block in iter(lambda: stream.read(1024 * 1024), b""):
                        value.update(block)
                files[member.name] = {"sha256": value.hexdigest(), "bytes": member.size, "mode": member.mode}
            else:
                raise ValueError("unsupported archive member")
    for name in seen:
        if any(str(parent) in links for parent in Path(name).parents):
            raise ValueError("archive member beneath symlink")
    return {"files": files, "symlinks": links, "directories": directories}


def extract_source(archive, source, expected):
    """Use the safe data filter, then restore only admitted ordinary Git modes."""
    if inventory(archive) != expected:
        raise ValueError("archive differs from admitted inventory")
    with tarfile.open(archive, "r:") as handle:
        handle.extractall(source, filter="data")
    restore_modes(source, expected)
    verify_source(source, expected)


def restore_modes(source, expected):
    for name, row in expected["files"].items():
        path = source / name
        if not stat.S_ISREG(path.lstat().st_mode):
            raise ValueError("extracted regular file changed type")
        path.chmod(row["mode"])
    for name, mode in sorted(expected["directories"].items(), key=lambda row: len(Path(row[0]).parts), reverse=True):
        path = source / name
        if not stat.S_ISDIR(path.lstat().st_mode):
            raise ValueError("extracted directory changed type")
        path.chmod(mode)


def verify_source(source, expected):
    actual, directories = set(), set()
    count = 0
    for directory, dirs, names in os.walk(source, followlinks=False):
        for name in list(dirs) + names:
            p = Path(directory) / name
            relative = str(p.relative_to(source))
            count += 1
            if count > MAX_ROWS:
                raise ValueError("source member bound exceeded")
            if relative == "target":
                if not p.is_symlink():
                    raise ValueError("target must remain external symlink")
                continue
            if p.is_symlink() or not p.is_dir():
                actual.add(relative)
            else:
                directories.add(relative)
    expected_directories = {str(parent) for name in set(expected["files"]) | set(expected["symlinks"])
                            for parent in Path(name).parents if str(parent) != "."}
    if "directories" in expected:
        expected_directories = set(expected["directories"])
        for name, mode in expected["directories"].items():
            if stat.S_IMODE((source / name).lstat().st_mode) != mode:
                raise ValueError("source directory mode changed")
    if directories != expected_directories:
        raise ValueError("source directory membership changed")
    if actual != set(expected["files"]) | set(expected["symlinks"]):
        raise ValueError("source file/symlink membership changed")
    for name, row in expected["files"].items():
        p = source / name
        s = p.lstat()
        if not stat.S_ISREG(s.st_mode) or s.st_size != row["bytes"] or stat.S_IMODE(s.st_mode) != row["mode"] or sha(p) != row["sha256"]:
            raise ValueError("source inventory changed: " + name)
    for name, value in expected["symlinks"].items():
        if not (source / name).is_symlink() or os.readlink(source / name) != value:
            raise ValueError("source symlink changed: " + name)


def verify_protected(expected):
    if not isinstance(expected.get("files"), list) or len(expected["files"]) > MAX_ROWS:
        raise ValueError("protected inventory bound exceeded")
    for row in expected["files"]:
        p = Path(row["path"])
        s = p.lstat()
        if not stat.S_ISREG(s.st_mode) or not 0 <= row["bytes"] <= MAX_FILE or s.st_size != row["bytes"] or stat.S_IMODE(s.st_mode) != row["mode"] or sha(p) != row["sha256"]:
            raise ValueError("protected file changed")


def support_module(root):
    config = load(root / "config.json")
    spec = importlib.util.spec_from_file_location("owned_gate_groups", Path(config["source"]) / SUPPORT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def task_members(pid):
    """Bound one actual Linux thread-group membership snapshot."""
    members = []
    with os.scandir("/proc/" + str(pid) + "/task") as entries:
        for entry in entries:
            if not entry.name.isdecimal() or len(members) >= MAX_DEAD_TASKS:
                raise ValueError("task membership unavailable/over bound")
            members.append(int(entry.name))
    return sorted(members)


def dead_threads(module, leader, members=None):
    """A dead leader alone is insufficient: bind a stable all-dead task group."""
    members = task_members if members is None else members
    result = {"status": "unavailable", "complete": False, "task_rows": []}
    try:
        before = module.row(leader["pid"])
        if before != leader or before["state"] not in ("Z", "X"):
            raise ValueError("leader identity/state changed")
        first = members(leader["pid"])
        if not first or len(first) > MAX_DEAD_TASKS or len(set(first)) != len(first) or leader["pid"] not in first:
            raise ValueError("task membership incomplete")
        rows = []
        for tid in first:
            row = module.row(tid)
            if row is None or row["pid"] != tid or row["pgid"] != leader["pgid"] or row["start"] < leader["start"]:
                raise ValueError("task identity unreadable/changed")
            rows.append(row)
        result["task_rows"] = rows
        if any(row["state"] not in ("Z", "X") for row in rows):
            result["status"] = "live" if all(row["state"] in ("Z", "X", "R", "S", "D", "T", "t", "W", "I", "K", "P") for row in rows) else "unavailable"
            result["reason"] = "live or unknown sibling task"
            return result
        if members(leader["pid"]) != first or [module.row(tid) for tid in first] != rows or module.row(leader["pid"]) != before or members(leader["pid"]) != first:
            raise ValueError("task membership/identity/state changed")
        result.update(status="terminated", complete=True)
    except (OSError, ValueError, KeyError, TypeError) as error:
        result["reason"] = str(error)[:256]
    return result


def terminated_row(row):
    if not isinstance(row, dict) or row.get("state") not in ("Z", "X") or any(type(row.get(key)) is not int or row[key] <= 0 for key in ("pid", "pgid", "start")):
        return False
    evidence = row.get("terminated_evidence", {})
    tasks = evidence.get("task_rows", []) if isinstance(evidence, dict) else []
    return (isinstance(evidence, dict) and evidence.get("status") == "terminated"
            and evidence.get("complete") is True and isinstance(tasks, list)
            and 0 < len(tasks) <= MAX_DEAD_TASKS
            and all(isinstance(task, dict) and task.get("state") in ("Z", "X")
                    and type(task.get("pid")) is int and task["pid"] > 0
                    and type(task.get("start")) is int and task["start"] >= row["start"]
                    and task.get("pgid") == row["pgid"] for task in tasks)
            and len({task.get("pid") for task in tasks}) == len(tasks)
            and any(all(task.get(key) == row[key] for key in ("pid", "pgid", "start", "state")) for task in tasks))


def group_closure(module, groups):
    """Fixed known-group observation; never claim arbitrary orphan absence."""
    survivors, unavailable, terminated = [], [], []
    entries = list(Path("/proc").iterdir())
    if len(entries) > module.MAX_SCAN:
        return {"survivors": [], "unavailable": ["process scan bound"], "terminated_unreaped": []}
    for entry in entries:
        if not entry.name.isdecimal():
            continue
        row = module.row(int(entry.name))
        if row is None:
            if entry.exists():
                unavailable.append(int(entry.name))
        elif row["pgid"] in groups.known:
            if row["state"] in ("Z", "X"):
                evidence = dead_threads(module, row)
                if evidence["status"] == "terminated":
                    terminated.append(dict(row, terminated_evidence=evidence))
                elif evidence["status"] == "live":
                    survivors.append(dict(row, thread_group_evidence=evidence))
                else:
                    unavailable.append({"pid": row["pid"], "dead_thread_group": evidence})
            else:
                survivors.append(row)
    return {"survivors": survivors, "unavailable": unavailable, "terminated_unreaped": terminated}


def closed_groups(audit):
    """Dead rows remain recorded; only live or unavailable ownership refuses."""
    return (isinstance(audit, dict) and audit.get("survivors") == []
            and audit.get("unavailable") == []
            and isinstance(audit.get("terminated_unreaped", []), list)
            and all(terminated_row(row) for row in audit.get("terminated_unreaped", [])))


def observe_owned(module, groups):
    """Observe only a live birth-matched parent; normal exit is not scan failure."""
    rows = {}
    with os.scandir("/proc") as entries:
        count = 0
        for entry in entries:
            if not entry.name.isdecimal():
                continue
            count += 1
            if count > module.MAX_SCAN:
                groups.mark_partial('process-scan-budget-exhausted')
                return
            value = module.row(int(entry.name))
            if value is not None:
                rows[value["pid"]] = value
    primary = rows.get(groups.leader["pid"])
    if primary is None or primary["state"] in ("Z", "X"):
        return  # Final known-group scan follows reap; not an orphan-absence claim.
    groups.observe(rows)


def run_owned(child, limit, groups, module, closure, observe):
    """Original command owns its session; outer cleanup never grants continuation."""
    began = time.monotonic()
    try:
        while True:
            if child.poll() is not None:
                return child.returncode
            observe(groups)
            remaining = limit - (time.monotonic() - began)
            if remaining <= 0:
                raise subprocess.TimeoutExpired(child.args, limit)
            try:
                return child.wait(timeout=min(1, remaining))
            except subprocess.TimeoutExpired:
                if time.monotonic() - began >= limit:
                    raise
    except BaseException:
        closure.uncertain("phase timeout/interruption")
        # Signal only this live, unreaped Popen session with exact birth match.
        if child.poll() is None and module.matching(module.row(child.pid), groups.leader):
            try:
                os.killpg(child.pid, signal.SIGINT)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=10)
        except subprocess.TimeoutExpired:
            pass
        # Existing primitive checks every retained descendant leader's birth.
        groups.signal_live()
        if child.poll() is None:
            child.wait(timeout=5)
        raise


class OwnedPhaseRunner:
    """Shared birth-checked phase lifecycle for local gates and product install."""
    def __init__(self, root, source, target, module, closure, record, check, group_budget=None):
        self.root, self.source, self.target = root, source, target
        self.module, self.closure, self.record, self.check = module, closure, record, check
        self.active, self.spawning = None, False
        self.group_budget = group_budget

    def interrupt(self, number, _frame):
        self.closure.uncertain("controller signal " + str(number))
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError("native controller signal " + str(number))

    def require_idle(self):
        self.closure.require()
        if self.active is not None or self.spawning:
            raise RuntimeError("actor ownership unavailable")

    def run(self, name, command, limit):
        self.require_idle()
        self.check()
        row = {"name": name, "command": command, "limit_seconds": limit}
        self.record["phases"].append(row)
        started, groups = time.monotonic(), None
        env = os.environ.copy()
        env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0", NEXTEST_TEST_THREADS="1",
                   CARGO_TARGET_DIR=self.target, PYTHONDONTWRITEBYTECODE="1")
        row["build_environment"] = {key: env[key] for key in
                                    ("CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "NEXTEST_TEST_THREADS",
                                     "CARGO_TARGET_DIR", "PYTHONDONTWRITEBYTECODE")}
        try:
            with (self.root / (name + ".log")).open("wb") as log:
                self.spawning = True
                self.active = subprocess.Popen(command, cwd=self.source, env=env, stdout=log,
                                               stderr=subprocess.STDOUT, start_new_session=True)
                self.spawning = False
                pinned = self.module.row(self.active.pid)
                if pinned is None or pinned["pgid"] != self.active.pid:
                    self.closure.uncertain("phase birth unavailable")
                    raise RuntimeError("phase birth unavailable; owner audit required")
                groups = (self.module.Groups(pinned) if self.group_budget is None
                          else self.module.Groups(pinned, max_groups=self.group_budget))
                row["owner"] = pinned
                # Put potentially failing checkpoint inside the owned wait cleanup.
                def observe(value):
                    save(self.root / "result.json", self.record)
                    observe_owned(self.module, value)
                code = run_owned(self.active, limit, groups, self.module, self.closure, observe)
                row["exit_code"] = code
                self.active = None
                audit = group_closure(self.module, groups)
                row["group_closure"] = audit
                if not closed_groups(audit) or groups.partial:
                    self.closure.uncertain("known descendant-group closure incomplete")
                self.require_idle()
                self.check()
                return code
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            if (self.root / (name + ".log")).is_file():
                row["log_sha256"] = sha(self.root / (name + ".log"))
            if groups is not None:
                row["known_groups"] = list(groups.known.values())
                row["ownership_partial"] = groups.partial
                row["ownership_group_budget"] = groups.max_groups
                row["ownership_first_refusal"] = groups.first_refusal
            save(self.root / "result.json", self.record)


def retained_images(root, paths):
    rows, errors, total = [], [], 0
    destination = root / "retained-images"
    destination.mkdir(exist_ok=True)
    for index, value in enumerate(paths):
        path = Path(value)
        try:
            size = path.lstat().st_size
            if path.is_symlink() or not path.is_file() or not os.access(path, os.X_OK) or total + size > MAX_FILE:
                raise ValueError("explicit executable retention bound/type")
            with path.open("rb") as handle:
                if handle.read(4) != b"\x7fELF":
                    raise ValueError("expected ELF")
            copy = destination / (str(index) + "-" + path.name)
            shutil.copy2(path, copy)
            if sha(path) != sha(copy):
                raise ValueError("retained executable differs")
            total += size
            rows.append({"source": value, "copy": copy.name, "bytes": size,
                         "mode": stat.S_IMODE(copy.stat().st_mode), "sha256": sha(copy),
                         "execution_identity": "operator-selected copy; execution is not inferred from file presence"})
        except (OSError, ValueError) as error:
            errors.append({"index": index, "error": str(error)})
    return {"files": rows, "errors": errors, "all_cargo_artifacts_archived": False}


def native(root):
    config = load(root / "config.json")
    if os.environ.get("CI") == "true":
        raise ValueError("combined controller is local-only")
    module = support_module(root)
    who = module.row(os.getpid())
    if who is None:
        raise ValueError("controller birth unavailable")
    save(root / "owner.json", who)
    source = Path(config["source"])
    expected = load(root / "source-inventory.json")
    protected = load(root / "protected.json")
    record = {"status": "RUNNING", "revision": config["revision"], "archive_sha256": config["archive_sha256"],
              "jobs": 2, "nextest_independent_tests": 1, "phases": [], "scope": "local workspace gate then original solver suite; no installed-product/corpus/performance credit"}
    closure = Closure()
    def check():
        verify_source(source, expected)
        if os.readlink(source / "target") != config["target"] or sha(root / "source.tar") != config["archive_sha256"]:
            raise ValueError("target/archive identity changed")
        verify_protected(protected)
        first = module.row(1)
        if first is None or first["start"] != config["pid1_birth"]:
            raise ValueError("container birth changed")
    runner = OwnedPhaseRunner(root, source, config["target"], module, closure, record, check,
                              group_budget=MAX_ROWS)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, runner.interrupt)
    def run(name, command, limit):
        code = runner.run(name, command, limit)
        if name == "gate":
            record["retention"] = retained_images(root, config["retain_elf"])
            if record["retention"]["errors"]:
                raise ValueError("requested executable retention incomplete")
        return code
    try:
        verify_source(source, expected)
        if os.readlink(source / "target") != config["target"] or sha(root / "source.tar") != config["archive_sha256"]:
            raise ValueError("target/archive identity changed")
        verify_protected(protected)
        record["status"] = "PASS" if sequence(run, closure) else "FAIL"
    except BaseException as error:
        record.update(status="FAIL", error_type=type(error).__name__, error=str(error))
    finally:
        record["uncertainty"] = closure.reasons
        save(root / "primary.json", record)
        try:
            closure.require()
            runner.require_idle()
            verify_source(source, expected)
            if os.readlink(source / "target") != config["target"] or sha(root / "source.tar") != config["archive_sha256"]:
                raise ValueError("target/archive identity changed")
            verify_protected(protected)
            if module.row(1) is None or module.row(1)["start"] != config["pid1_birth"]:
                raise ValueError("container birth changed")
            record["closure"] = "original source/protected bytes and PID1 equal; known phase groups closed, not an arbitrary orphan-absence proof"
            if "retention" not in record:
                record["retention"] = retained_images(root, config["retain_elf"])
        except BaseException as error:
            record.update(status="FAIL", closure="unavailable", closure_error=str(error))
        save(root / "result.json", record)
    return 0 if record["status"] == "PASS" else 1


def cancel_native(root):
    config, pinned = load(root / "config.json"), load(root / "owner.json")
    module = support_module(root)
    current = module.row(pinned["pid"])
    first = module.row(1)
    if first is None or first["start"] != config["pid1_birth"] or not module.matching(current, pinned) or current["state"] in ("Z", "X"):
        raise ValueError("native controller identity changed; no signal")
    os.kill(pinned["pid"], signal.SIGTERM)


def validate_terminal(record, revision, archive_sha):
    """Complete PASS needs both original phases; failed prefixes receive no credit."""
    if record["revision"] != revision or record["archive_sha256"] != archive_sha:
        raise ValueError("native input identity differs")
    phases = record["phases"]
    if len(phases) > 2 or [p["name"] for p in phases] != ["gate", "solver"][:len(phases)]:
        raise ValueError("phase prefix differs")
    for index, phase in enumerate(phases):
        if phase["command"] != phase_command(phase["name"]) or phase["limit_seconds"] != [GATE_LIMIT, SOLVER_LIMIT][index]:
            raise ValueError("original command/local cap differs")
        if index and phases[index - 1].get("exit_code") != 0:
            raise ValueError("phase followed failed/incomplete gate")
    if record["status"] == "PASS":
        if len(phases) != 2 or any(p.get("exit_code") != 0 for p in phases) or record.get("uncertainty") or record.get("closure") in (None, "unavailable"):
            raise ValueError("incomplete combined PASS")
    elif record["status"] != "FAIL":
        raise ValueError("nonterminal native status")


def validate_paths(source, target, output):
    source, target, output = Path(source), Path(target), Path(output)
    if not source.is_absolute() or ".." in source.parts or source == Path("/"):
        raise ValueError("source must be a new explicit absolute native path")
    if source == target or source in target.parents or target in source.parents:
        raise ValueError("source and compiler target must not overlap")
    if source == output or source in output.parents:
        raise ValueError("source must not contain native evidence output")


def retire_closed_gate(args, result, frozen):
    """Retirement is separate from validation, with its own conservative refusal."""
    try:
        directory = Path(__file__).parent
        names = ("retire-completed-run.py", "local-combined-gates.py", "retire-old-functional-two-incremental.py")
        for name in names:
            if sha(directory / name) != frozen["files"]["scripts/" + name]["sha256"]:
                raise ValueError("cleanup helper differs from frozen source")
        spec = importlib.util.spec_from_file_location("completed_gate_retirement", directory / names[0])
        helper = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(helper)
        helper.terminal_closed(result)
        protected = [args.target] + [row["path"] for row in load(args.protected)["files"]]
        required = ["result.json", "config.json", "protected.json", "source-inventory.json"]
        required += [phase["name"] + ".log" for phase in result["phases"]]
        output = args.retirement_output or (Path.home() / ".cache/sail/evidence/completed-runs" / Path(args.native_root).name)
        retained = [args.output / name for name in ("source.tar", "source-inventory.json", "protected.json", "transport.log", "result.json")]
        return helper.retire_run(args.container, args.native_root, output,
                                 dict(result, native_root=args.native_root), protected, required, retained)
    except BaseException as error:
        return {"status": "UNCERTAIN", "removed": False, "original_status": result.get("status"),
                "error_type": type(error).__name__, "error": str(error)[:256]}


def host(args):
    closure = Closure()
    args.output.mkdir(mode=0o700, exist_ok=False)
    report = {"status": "RUNNING", "scope": "local gate then solver; no installed-product/corpus/performance credit"}
    before, remote = None, None
    root = args.native_root
    source = args.source or root + "/source"
    def interrupted(number, _frame):
        closure.uncertain("host interruption")
        for value in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(value, signal.SIG_IGN)
        raise InterruptedError("host signal " + str(number))
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupted)
    def call(command, timeout=META_LIMIT, **kwargs):
        return bounded_command(command, timeout=timeout, closure=closure, **kwargs)
    try:
        if not root.startswith("/tmp/sail-local-gates-") or ".." in Path(root).parts:
            raise ValueError("expected new private /tmp/sail-local-gates-* root")
        if not Path(args.target).is_absolute() or ".." in Path(args.target).parts:
            raise ValueError("target must be an explicit absolute native path")
        validate_paths(source, args.target, root)
        archive = args.output / "source.tar"
        revision = call(["git", "rev-parse", args.revision + "^{commit}"], cwd=args.repo).stdout.decode().strip()
        before = identity(args.container, closure)
        with archive.open("xb") as output:
            call(["git", "archive", "--format=tar", revision], cwd=args.repo, output=output, timeout=60)
        if archive.stat().st_size > MAX_FILE:
            raise ValueError("archive byte bound exceeded")
        frozen = inventory(archive)
        if frozen["files"]["scripts/local-combined-gates.py"]["sha256"] != sha(Path(__file__)):
            raise ValueError("committed controller differs from running helper")
        save(args.output / "source-inventory.json", frozen)
        load(args.protected)
        shutil.copy2(args.protected, args.output / "protected.json")
        report.update(revision=revision, archive_sha256=sha(archive), controller_sha256=sha(Path(__file__)), container_before=before)
        save(args.output / "controller.json", report)
        command = ["docker", "exec", "-i", args.container, "python3", "-"]
        prepare = "from pathlib import Path\nr=Path(%r);r.mkdir(mode=0o700,exist_ok=False);Path(%r).mkdir(mode=0o700,exist_ok=False)\n" % (root, source)
        call(command, payload=prepare.encode())
        for name in ("source.tar", "source-inventory.json", "protected.json"):
            call(["docker", "cp", str(args.output / name), args.container + ":" + root + "/" + name], timeout=60)
        prepare = """import json,tarfile,importlib.util,sys
from pathlib import Path
sys.dont_write_bytecode=True
r=Path(%r);s=Path(%r)
with tarfile.open(r/'source.tar') as h:
 for count,member in enumerate(h,1):
  p=Path(member.name)
  if count>8192 or member.mode & ~0o777 or p.is_absolute() or '..' in p.parts or not (member.isfile() or member.isdir() or member.issym()):raise ValueError('unsafe archive member')
  if member.issym() and (Path(member.linkname).is_absolute() or '..' in Path(member.linkname).parts):raise ValueError('unsafe archive link')
 h.extractall(s,filter='data')
spec=importlib.util.spec_from_file_location('gate_setup',s/'scripts/local-combined-gates.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
expected=m.load(r/'source-inventory.json')
if m.inventory(r/'source.tar') != expected:raise ValueError('archive inventory changed')
m.restore_modes(s,expected);m.verify_source(s,expected)
(s/'target').symlink_to(%r)
print(json.dumps({'pid1_birth':int(Path('/proc/1/stat').read_text().rsplit(')',1)[1].split()[19])}))
""" % (root, source, args.target)
        result = call(command, payload=prepare.encode(), timeout=60).stdout
        config = {"revision": revision, "archive_sha256": sha(archive), "target": args.target, "source": source,
                  "pid1_birth": json.loads(result)["pid1_birth"], "retain_elf": args.retain_elf}
        save(args.output / "config.json", config)
        call(["docker", "cp", str(args.output / "config.json"), args.container + ":" + root + "/config.json"])
        remote = source + "/scripts/local-combined-gates.py"
        if identity(args.container, closure) != before:
            raise RuntimeError("container changed during setup")
        with (args.output / "transport.log").open("wb") as log:
            result = call(["docker", "exec", args.container, "python3", "-B", remote, "--native", root],
                          output=log, timeout=GATE_LIMIT + SOLVER_LIMIT + 300, check=False)
        report["transport_exit_code"] = result.returncode
        after = identity(args.container, closure)
        report["container_after"] = after
        if after != before:
            raise RuntimeError("container identity changed")
        call(["docker", "cp", args.container + ":" + root + "/result.json", str(args.output / "result.json")])
        result_record = load(args.output / "result.json")
        if result.returncode != (0 if result_record["status"] == "PASS" else 1):
            raise RuntimeError("transport and native terminal disagree")
        validate_terminal(result_record, revision, sha(archive))
        for phase in result_record["phases"]:
            name = phase["name"] + ".log"
            call(["docker", "cp", args.container + ":" + root + "/" + name, str(args.output / name)])
            if sha(args.output / name) != phase["log_sha256"]:
                raise ValueError("retained raw phase log differs")
        report["status"] = result_record["status"]
        report["retirement"] = retire_closed_gate(args, result_record, frozen)
    except BaseException as error:
        closure.uncertain("transport/setup/interruption closure unavailable")
        report.update(status="FAIL", error_type=type(error).__name__, error=str(error))
        if before is not None and remote is not None:
            try:
                if identity(args.container, closure) != before:
                    raise RuntimeError("container changed; no cancellation")
                call(["docker", "exec", args.container, "python3", "-B", remote, "--cancel", root])
            except BaseException as cleanup:
                report["cancellation_unavailable"] = str(cleanup)
    finally:
        report["uncertainty"] = closure.reasons
        report["native_root"] = root
        report["native_evidence"] = ("compact evidence verified on host; closed native scratch retired; historical images nonreexecutable"
                                     if report.get("retirement", {}).get("removed") else
                                     "native scratch retained; retirement deferred or uncertain; explicit audit required")
        save(args.output / "controller.json", report)
    return 0 if (report["status"] == "PASS" and not closure.reasons
                 and report.get("retirement", {}).get("status") == "PASS") else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--cancel", type=Path, help=argparse.SUPPRESS)
    parser.add_argument("--repo", type=Path)
    parser.add_argument("--revision")
    parser.add_argument("--container")
    parser.add_argument("--native-root")
    parser.add_argument("--target", default="/work/target")
    parser.add_argument("--source", help="new exact native source path; defaults to native-root/source")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--retirement-output", type=Path, help="new durable host evidence directory outside /tmp; defaults to Sail evidence cache")
    parser.add_argument("--protected", type=Path)
    parser.add_argument("--retain-elf", action="append", default=[])
    args = parser.parse_args()
    if args.native:
        return native(args.native)
    if args.cancel:
        cancel_native(args.cancel)
        return 0
    if not all((args.repo, args.revision, args.container, args.native_root, args.output, args.protected)):
        parser.error("host mode requires repo/revision/container/native-root/output/protected")
    return host(args)


if __name__ == "__main__":
    raise SystemExit(main())
