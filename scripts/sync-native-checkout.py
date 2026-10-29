#!/usr/bin/env python3
"""Sync frozen actual sources and standalone Git metadata to an owned host."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import stat
import subprocess
import tempfile
import uuid


EXCLUDED_DIRS = {"target", ".claude", "__pycache__", "conformance-out"}
ROOT_EXCLUDED_DIRS = {"dist", "release-out", "release-out-proof-evidence"}


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def git_environment():
    if any(os.environ.get(name) for name in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE")):
        raise RuntimeError("unset Git repository/index redirection before sync")
    env = {name: value for name, value in os.environ.items() if not name.startswith("GIT_")}
    env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
               GIT_NO_REPLACE_OBJECTS="1", GIT_OPTIONAL_LOCKS="0")
    return env


def git(root, env, *args, payload=None):
    return subprocess.check_output(["git", "-c", "core.fsmonitor=false", "-C", str(root), *args],
                                   env=env, input=payload)


def excluded_directories(parts):
    return [parts[:i + 1] for i, name in enumerate(parts)
            if name in EXCLUDED_DIRS or (i == 0 and name in ROOT_EXCLUDED_DIRS)]


def literal_filter(parts):
    """Quote filename wildcard bytes in an anchored rsync include pattern."""
    return "/" + "".join("\\" + char if char in "\\*?[" else char for char in "/".join(parts))


def source_files(root, tracked=()):
    """Keep actual tracked files even within otherwise excluded output trees."""
    tracked = {Path(name).parts for name in tracked}
    parents = {parts[:i] for parts in tracked for i in range(1, len(parts))}
    def walk(directory, excluded=False):
        for path in sorted(directory.iterdir()):
            if path.name == ".git":
                continue
            mode = path.lstat().st_mode
            parts = path.relative_to(root).parts
            if stat.S_ISDIR(mode):
                child_excluded = excluded or bool(excluded_directories(parts))
                if child_excluded and parts not in parents:
                    continue
                yield path, mode
                yield from walk(path, child_excluded)
            elif excluded and parts not in tracked:
                continue
            elif stat.S_ISREG(mode) or stat.S_ISLNK(mode):
                yield path, mode
            else:
                raise RuntimeError("unsupported special source file: " + str(path.relative_to(root)))
    return walk(root)


def snapshot(root, tracked=()):
    result = {}
    for path, mode in source_files(root, tracked):
        value = ("directory",) if stat.S_ISDIR(mode) else (
            ("symlink", os.readlink(path)) if stat.S_ISLNK(mode) else ("file", digest(path)))
        result[str(path.relative_to(root))] = (stat.S_IMODE(mode), *value)
    return result


def copy_sources(source, destination, tracked=()):
    destination.mkdir()
    directories = []
    for path, mode in source_files(source, tracked):
        target = destination / path.relative_to(source)
        if stat.S_ISDIR(mode):
            target.mkdir()
            directories.append((target, stat.S_IMODE(mode)))
        elif stat.S_ISLNK(mode):
            target.symlink_to(os.readlink(path))
        else:
            shutil.copy2(path, target)
    for target, mode in reversed(directories):
        target.chmod(mode)


def index_observation(root, env):
    sparse = subprocess.run(["git", "-C", str(root), "config", "--bool", "--get", "core.sparseCheckout"],
                            env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if sparse.returncode not in (0, 1) or sparse.stdout.strip() == b"true":
        raise RuntimeError("sparse checkouts are unsupported; use a full worktree")
    tags = git(root, env, "ls-files", "-v", "-z").split(b"\0")
    if any(row[:2] in (b"S ", b"s ") for row in tags):
        raise RuntimeError("skip-worktree entries are unsupported; clear them before sync")
    def intent_paths(option):
        return set(git(root, env, "diff", "--cached", "--no-ext-diff", "--no-textconv",
                       "--name-only", "-z", option).split(b"\0"))
    if intent_paths("--ita-visible-in-index") != intent_paths("--ita-invisible-in-index"):
        raise RuntimeError("intent-to-add entries are unsupported; stage or unstage them before sync")
    entries = git(root, env, "ls-files", "--stage", "-z")
    objects = set()
    for row in entries.split(b"\0"):
        if not row:
            continue
        mode, oid, stage = row.split(b"\t", 1)[0].split()
        if mode not in (b"100644", b"100755", b"120000") or stage not in (b"0", b"1", b"2", b"3"):
            raise RuntimeError("unsupported Git index entry (including submodules)")
        objects.add(oid)
    index = Path(os.fsdecode(git(root, env, "rev-parse", "--git-path", "index").strip()))
    if not index.is_absolute():
        index = root / index
    return entries, objects, digest(index) if index.exists() else None


def export_git(source, directory, env, head, entries, objects):
    """Export only reachable objects and staged blobs, never local config/hooks."""
    bundle = directory / "head.bundle"
    git(source, env, "bundle", "create", str(bundle), "HEAD")
    refs = git(source, env, "bundle", "list-heads", str(bundle)).splitlines()
    if refs != [(head + " HEAD").encode()]:
        raise RuntimeError("HEAD changed while creating the source bundle")
    repo = directory / "metadata"
    repo.mkdir()
    fmt = git(source, env, "rev-parse", "--show-object-format").decode().strip()
    git(repo, env, "init", "--quiet", "--template=", "--object-format=" + fmt)
    for key, value in (("core.logAllRefUpdates", "false"), ("core.ignoreCase", "false"),
                       ("core.fileMode", "true"), ("core.symlinks", "true")):
        git(repo, env, "config", key, value)
    git(repo, env, "bundle", "unbundle", str(bundle))
    git(repo, env, "update-ref", "--no-deref", "HEAD", head)
    if objects:
        requested = b"".join(oid + b"\n" for oid in sorted(objects))
        missing = [line.split()[0] for line in git(repo, env, "cat-file", "--batch-check", payload=requested).splitlines()
                   if line.endswith(b" missing")]
        if missing:
            raw = git(source, env, "cat-file", "--batch", payload=b"".join(oid + b"\n" for oid in missing))
            position = 0
            for expected in missing:
                end = raw.index(b"\n", position)
                header = raw[position:end].split()
                if len(header) != 3 or header[0] != expected or header[1] != b"blob" or not header[2].isdigit():
                    raise RuntimeError("missing staged Git blob")
                oid, _kind, size = header
                position = end + 1
                body = raw[position:position + int(size)]
                position += int(size)
                if raw[position:position + 1] != b"\n":
                    raise RuntimeError("truncated staged Git blob")
                position += 1
                if git(repo, env, "hash-object", "-w", "--stdin", payload=body).strip() != oid:
                    raise RuntimeError("staged Git object identity changed")
            if position != len(raw):
                raise RuntimeError("unexpected staged Git object data")
    git(repo, env, "read-tree", "--empty")
    git(repo, env, "update-index", "-z", "--index-info", payload=entries)
    return repo / ".git"


# Executed through the same SSH transport, under the token-owned sync lock.
# The ledger survives a failed transfer that deliberately removes usable .git.
REMOTE_OWNERSHIP = r"""
import json, os, pathlib, stat, subprocess, sys
state = pathlib.Path('sail-sync-owned-paths.json')
def names(value):
    if not isinstance(value, list) or any(not isinstance(p, str) or not p or '\0' in p or p.startswith('/')
        or any(c in ('', '.', '..', '.git') for c in p.split('/')) for p in value):
        raise RuntimeError('invalid managed source ownership paths')
    return set(value)
if sys.argv[1] == 'read':
    if state.is_symlink():
        raise RuntimeError('source ownership ledger is a symlink')
    if state.exists():
        value = json.loads(state.read_text())
        if not isinstance(value, dict) or set(value) != {'version', 'paths'} or type(value['version']) is not int or value['version'] != 1:
            raise RuntimeError('invalid managed source ownership ledger')
        result = names(value['paths'])
    else:
        result = set()
        metadata = pathlib.Path('sail/.git')
        if metadata.is_symlink() or (metadata.exists() and not metadata.is_dir()):
            raise RuntimeError('opaque prior Git pointer; use a fresh lane or owner-controlled checkout cleanup; existing files preserved')
        if not metadata.exists() and any(pathlib.Path('sail').iterdir()):
            raise RuntimeError('nonempty prior checkout has no ownership metadata; use a fresh lane or owner-controlled checkout cleanup; existing files preserved')
        if metadata.is_dir():
            forbidden = ['commondir', 'gitdir', 'worktrees', 'config.worktree',
                         'objects/info/alternates', 'objects/info/http-alternates']
            if any((metadata / name).exists() or (metadata / name).is_symlink() for name in forbidden):
                raise RuntimeError('prior Git metadata is not standalone')
            for path in metadata.rglob('*'):
                mode = path.lstat().st_mode
                if not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
                    raise RuntimeError('prior Git metadata contains a link or special file')
            env = {k:v for k,v in os.environ.items() if not k.startswith('GIT_')}
            env.update(GIT_CONFIG_NOSYSTEM='1', GIT_CONFIG_GLOBAL=os.devnull,
                       GIT_NO_REPLACE_OBJECTS='1', GIT_OPTIONAL_LOCKS='0')
            configuration = subprocess.check_output(['git', 'config', '--file=sail/.git/config',
                                                      '--no-includes', '--null', '--list'], env=env)
            allowed = {'core.repositoryformatversion', 'core.filemode', 'core.bare',
                       'core.logallrefupdates', 'core.ignorecase', 'core.symlinks', 'core.precomposeunicode',
                       'extensions.objectformat'}
            if any(os.fsdecode(row.split(b'\n', 1)[0]) not in allowed for row in configuration.split(b'\0') if row):
                raise RuntimeError('prior Git configuration may redirect repository inputs')
            command = ['git', '--git-dir=sail/.git', '--work-tree=sail', '-c', 'core.fsmonitor=false']
            def git(*args):
                return subprocess.check_output(command + list(args), env=env)
            before = (git('rev-parse', 'HEAD'), git('ls-files', '--stage', '-z'))
            result = names([os.fsdecode(p) for p in git('ls-tree', '-r', '--name-only', '-z', 'HEAD').split(b'\0') if p])
            result.update(names([os.fsdecode(row.split(b'\t',1)[1]) for row in before[1].split(b'\0') if row]))
            if before != (git('rev-parse', 'HEAD'), git('ls-files', '--stage', '-z')):
                raise RuntimeError('previous remote Git state changed during source capture')
    print(json.dumps(sorted(result)))
elif sys.argv[1] == 'write':
    value = names(json.loads(sys.argv[2]))
    temporary = pathlib.Path('sail-sync.lock/owned-paths')
    with temporary.open('x') as stream:
        json.dump({'version':1, 'paths':sorted(value)}, stream)
    os.replace(temporary, state)
else:
    raise RuntimeError('invalid ownership operation')
"""


def sync(source, destination, ssh_command):
    if not re.fullmatch(r"[A-Za-z0-9_.@:-]+", destination):
        raise RuntimeError("unsupported SSH destination")
    env = git_environment()
    source = Path(os.fsdecode(git(source, env, "rev-parse", "--show-toplevel").strip())).resolve()
    head = git(source, env, "rev-parse", "HEAD").decode().strip()
    if not re.fullmatch(r"[0-9a-f]{40}|[0-9a-f]{64}", head):
        raise RuntimeError("sync requires an existing HEAD commit")
    initial_index = index_observation(source, env)
    tracked = {os.fsdecode(row.split(b"\t", 1)[1]) for row in initial_index[0].split(b"\0") if row}
    tracked.update(os.fsdecode(path) for path in git(source, env, "ls-tree", "-r", "--name-only", "-z", head).split(b"\0") if path)
    initial_sources = snapshot(source, tracked)
    def guard():
        if (git(source, env, "rev-parse", "HEAD").decode().strip() != head
                or index_observation(source, env) != initial_index or snapshot(source, tracked) != initial_sources):
            raise RuntimeError("local HEAD, index or source changed during sync; retry")
    token = uuid.uuid4().hex
    staging = "sail-git-sync-" + token
    ssh = shlex.split(ssh_command)
    if not ssh:
        raise RuntimeError("missing SSH command")
    def remote(command, capture=False):
        return subprocess.run([*ssh, destination, "set -eu; " + command], check=True,
                              stdout=subprocess.PIPE if capture else None).stdout
    def ownership(operation, paths=None):
        args = ["python3", "-c", REMOTE_OWNERSHIP, operation]
        if paths is not None:
            args.append(json.dumps(sorted(paths)))
        return remote(shlex.join(args), capture=operation == "read")
    locked = False
    acquire_attempted = False
    installed = False
    mutation_attempted = False
    with tempfile.TemporaryDirectory(prefix="sail-perf-sync-") as temporary:
        directory = Path(temporary)
        frozen = directory / "source"
        copy_sources(source, frozen, tracked)
        if snapshot(frozen, tracked) != initial_sources:
            raise RuntimeError("source changed while freezing sync inputs")
        metadata = export_git(source, directory, env, head, initial_index[0], initial_index[1])
        guard()
        try:
            acquire_attempted = True
            remote("test ! -L sail; mkdir -p sail; mkdir sail-sync.lock; "
                   + "printf '%s' " + token + " > sail-sync.lock/owner")
            locked = True
            previous = set(json.loads(ownership("read")))
            selected = tracked | previous
            # Owned excluded paths need sender directories so rsync visits and
            # removes missing names even when the whole subtree is gone.
            for name in selected:
                parts = Path(name).parts
                if excluded_directories(parts[:-1]):
                    parent = frozen
                    for component in parts[:-1]:
                        parent /= component
                        if parent.is_symlink() or (parent.exists() and not parent.is_dir()):
                            raise RuntimeError("retired source path crosses a non-directory")
                        parent.mkdir(exist_ok=True)
            ownership("write", selected)
            mutation_attempted = True
            remote("test \"$(cat sail-sync.lock/owner)\" = " + token
                   + "; rm -rf sail/.git; mkdir " + staging)
            common = ["rsync", "-az", "--checksum", "--no-times", "--delete", "-e", ssh_command]
            exclusions = ["--exclude", ".git"]
            included = set()
            protected = set()
            for name in selected:
                parts = Path(name).parts
                excluded = excluded_directories(parts[:-1])
                if not excluded:
                    continue
                included.update(literal_filter(parts[:i]) + "/" for i in range(1, len(parts)))
                included.add(literal_filter(parts))
                protected.update(literal_filter(parts) + "/**" for parts in excluded)
            for pattern in sorted(included):
                exclusions += ["--include", pattern]
            for pattern in sorted(protected):
                exclusions += ["--exclude", pattern]
            for name in sorted(EXCLUDED_DIRS):
                exclusions += ["--exclude", name + "/"]
            for name in sorted(ROOT_EXCLUDED_DIRS):
                exclusions += ["--exclude", "/" + name + "/"]
            subprocess.run([*common, *exclusions, str(frozen) + "/", destination + ":sail/"], check=True)
            subprocess.run([*common, str(metadata) + "/", destination + ":" + staging + "/"], check=True)
            guard()
            remote("test \"$(cat sail-sync.lock/owner)\" = " + token
                   + "; test ! -e sail/.git; test ! -L sail/.git; test ! -L " + staging
                   + "; mv " + staging + " sail/.git; git -C sail cat-file -e 'HEAD^{tree}'; "
                   + "test \"$(git -C sail rev-parse HEAD)\" = " + head)
            guard()
            ownership("write", tracked)
            installed = True
        finally:
            if acquire_attempted:
                invalidate = "rm -rf sail/.git; " if mutation_attempted and not installed else ""
                absent = "false" if locked else "test ! -e sail-sync.lock && test ! -L sail-sync.lock"
                remote("if test ! -L sail-sync.lock && test ! -L sail-sync.lock/owner "
                       + "&& test -f sail-sync.lock/owner && test \"$(cat sail-sync.lock/owner)\" = " + token
                       + "; then " + invalidate + "rm -rf " + staging
                       + "; rm sail-sync.lock/owner; rmdir sail-sync.lock; "
                       + "elif " + absent + "; then :; else "
                       + "echo 'sync lock ownership is unknown; retaining it' >&2; exit 1; fi")
    print("synced " + head + " with actual sources and standalone Git metadata")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--destination", required=True)
    parser.add_argument("--ssh-command", required=True)
    args = parser.parse_args()
    try:
        sync(args.source, args.destination, args.ssh_command)
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        parser.exit(1, "error[SAIL9003]: source sync failed: " + str(error) + "\n")


if __name__ == "__main__":
    main()
