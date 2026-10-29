#!/usr/bin/env python3
"""Real Git/rsync sync regressions; only the SSH transport is local."""
import importlib.util
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("sail_native_sync", ROOT / "sync-native-checkout.py")
SYNC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SYNC)


class NativeSync(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="sail-sync-tests-")
        self.addCleanup(self.tmp.cleanup)
        self.base = Path(self.tmp.name)
        self.source = self.base / "primary source"
        self.source.mkdir()
        self.remote = self.base / "remote"
        self.remote.mkdir()
        self.calls = self.base / "ssh-calls.jsonl"
        self.ssh = self.base / "ssh"
        self.ssh.write_text('''#!/usr/bin/env python3
import json,os,pathlib,subprocess,sys
args=sys.argv[1:]
while args and args[0].startswith('-'):
 args=args[2:] if args[0] in ['-i','-o','-l'] else args[1:]
destination=args.pop(0)
assert destination in ['fixture-host','ubuntu@fixture-host','ec2-user@fixture-host']
with open(os.environ['SAIL_SYNC_TEST_CALLS'],'a') as f:f.write(json.dumps({'destination':destination,'args':args})+'\\n')
if args and args[0]=='rsync':
 if os.environ.get('SAIL_SYNC_TEST_FAIL_METADATA') and 'sail-git-sync-' in args[-1]:sys.exit(42)
 marker=pathlib.Path(os.environ['SAIL_SYNC_TEST_REMOTE'])/'mutation-done'
 kind=os.environ.get('SAIL_SYNC_TEST_MUTATION')
 if kind and not marker.exists():
  marker.touch();source=pathlib.Path(os.environ['SAIL_SYNC_TEST_SOURCE'])
  if kind=='source':(source/'src/lib.rs').write_text('changed during transfer\\n')
  elif kind=='index':subprocess.run(['git','update-index','--force-remove','README.md'],cwd=source,check=True)
  elif kind=='head':subprocess.run(['git','update-ref','--no-deref','HEAD',os.environ['SAIL_SYNC_TEST_OTHER_HEAD']],cwd=source,check=True)
env=dict(os.environ);env['HOME']=os.environ['SAIL_SYNC_TEST_REMOTE']
command=['bash','-c',args[0]] if len(args)==1 else args
if len(args)==1 and os.environ.get('SAIL_SYNC_TEST_FAIL_PREPARE') and '; rm -rf sail/.git; mkdir sail-git-sync-' in args[0]:
 subprocess.run(['bash','-c',args[0].split('; mkdir sail-git-sync-',1)[0]],cwd=env['HOME'],env=env,check=True)
 sys.exit(43)
if len(args)==1 and os.environ.get('SAIL_SYNC_TEST_FAIL_ACQUIRE_ACK') and "printf '%s'" in args[0]:
 subprocess.run(command,cwd=env['HOME'],env=env,check=True)
 sys.exit(44)
raise SystemExit(subprocess.run(command,cwd=env['HOME'],env=env).returncode)
''')
        self.ssh.chmod(0o755)
        self.env = dict(os.environ)
        for name in list(self.env):
            if name.startswith("GIT_"):
                self.env.pop(name)
        self.env.update(GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
                        GIT_OPTIONAL_LOCKS="0", SAIL_SYNC_TEST_REMOTE=str(self.remote),
                        SAIL_SYNC_TEST_SOURCE=str(self.source), SAIL_SYNC_TEST_CALLS=str(self.calls))
        self.patch = mock.patch.dict(os.environ, self.env, clear=True)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        self.git(self.source, "init", "-q", "-b", "fixture")
        (self.source / "src").mkdir()
        (self.source / "src/lib.rs").write_text("first source\n")
        (self.source / "README.md").write_text("tracked documentation\n")
        (self.source / "delete.txt").write_text("tracked deletion\n")
        (self.source / ".gitignore").write_text("target/\n")
        (self.source / "alias").symlink_to("src/lib.rs")
        self.git(self.source, "add", ".")
        self.commit(self.source, "initial")

    def git(self, path, *args, check=True):
        result = subprocess.run(["git", "-c", "core.fsmonitor=false", "-C", str(path), *args],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        if check:
            self.assertEqual(result.returncode, 0, result.stderr.decode(errors="replace"))
        return result.stdout

    def commit(self, path, message):
        self.git(path, "-c", "user.name=Sync Fixture", "-c", "user.email=sync@example.invalid",
                 "commit", "-q", "-m", message)

    def invoke(self, source=None):
        source = source or self.source
        command = shlex.join([sys.executable, str(self.ssh)])
        SYNC.sync(source, "fixture-host", command)
        return self.remote / "sail"

    def assert_same_git_state(self, source, target):
        for args in [("rev-parse", "HEAD"), ("ls-files", "--stage", "-z"),
                     ("status", "--porcelain=v1", "--untracked-files=all"),
                     ("diff", "--cached", "--binary", "--no-ext-diff", "--no-textconv"),
                     ("diff", "--binary", "--no-ext-diff", "--no-textconv")]:
            self.assertEqual(self.git(source, *args), self.git(target, *args), args)

    def test_primary_standalone_metadata_does_not_copy_local_config_or_hooks(self):
        self.git(self.source, "config", "sail.private", "sensitive-local-only")
        (self.source / ".git/hooks/post-checkout").write_text("private-hook-content\n")
        (self.source / ".git/info/exclude").write_text("hidden.txt\n")
        (self.source / "hidden.txt").write_text("actual untracked input\n")
        target = self.invoke()
        self.assertTrue((target / ".git").is_dir())
        self.assertEqual(self.git(target, "rev-parse", "HEAD"), self.git(self.source, "rev-parse", "HEAD"))
        metadata = b"".join(p.read_bytes() for p in (target / ".git").rglob("*") if p.is_file() and "objects" not in p.parts)
        for secret in [b"sensitive-local-only", b"private-hook-content", os.fsencode(self.source)]:
            self.assertNotIn(secret, metadata)
        self.assertIn(b"?? hidden.txt", self.git(target, "status", "--porcelain=v1"))
        self.assertFalse((target / ".git/hooks").exists())
        self.assertFalse((target / ".git/objects/info/alternates").exists())

    def test_release_proof_evidence_is_preserved_and_tracked_source_still_transfers(self):
        target = self.invoke()
        name = 'release-out-proof-evidence'
        evidence = target / name
        evidence.mkdir()
        (evidence / 'native.log').write_text('retain native proof evidence')
        local = self.source / name
        local.mkdir()
        (local / 'private.log').write_text('do not transfer private evidence')
        # Release staging runs as container root. Sync must not enumerate or
        # delete the resulting inaccessible, untracked evidence directory.
        evidence.chmod(0)
        try:
            self.invoke()
        finally:
            evidence.chmod(0o700)
        self.assertEqual((evidence / 'native.log').read_text(), 'retain native proof evidence')
        self.assertFalse((evidence / 'private.log').exists())
        (local / 'tracked.rs').write_text('tracked source still transfers')
        self.git(self.source, 'add', str(local / 'tracked.rs'))
        self.commit(self.source, 'track a source beneath the output exclusion')
        self.invoke()
        self.assertEqual((evidence / 'tracked.rs').read_text(), 'tracked source still transfers')
        self.assertTrue((evidence / 'native.log').exists())
        self.assertFalse((evidence / 'private.log').exists())

    def test_linked_staged_then_modified_deleted_and_untracked_inputs_survive(self):
        linked = self.base / "linked source"
        self.git(self.source, "worktree", "add", "-q", "-b", "linked", str(linked))
        (linked / "src/lib.rs").write_text("staged version\n")
        (linked / "staged new.txt").write_text("staged new value\n")
        self.git(linked, "add", "src/lib.rs", "staged new.txt")
        (linked / "src/lib.rs").write_text("actual working version\n")
        (linked / "staged new.txt").write_text("actual new working version\n")
        self.git(linked, "rm", "-q", "delete.txt")
        (linked / "README.md").unlink()
        (linked / "untracked.txt").write_text("untracked actual bytes\n")
        pointer = (linked / ".git").read_bytes()
        target = self.invoke(linked)
        self.assert_same_git_state(linked, target)
        self.assertEqual((linked / ".git").read_bytes(), pointer)
        self.assertEqual((target / "staged new.txt").read_bytes(), (linked / "staged new.txt").read_bytes())
        self.assertEqual(os.readlink(target / "alias"), "src/lib.rs")

    def test_repeated_primary_linked_switches_use_contents_and_current_mtimes(self):
        target = self.invoke()
        (target / "target").mkdir()
        artifact = target / "target/compiled"
        artifact.write_text("preserve build cache")
        old = time.time() - 3600
        os.utime(artifact, (old, old))
        linked = self.base / "older linked"
        self.git(self.source, "worktree", "add", "-q", "-b", "second", str(linked))
        (linked / "src/lib.rs").write_text("other source\n")
        self.git(linked, "add", "src/lib.rs")
        self.commit(linked, "second")
        same_time = (target / "src/lib.rs").stat().st_mtime
        os.utime(linked / "src/lib.rs", (same_time, same_time))
        self.invoke(linked)
        self.assert_same_git_state(linked, target)
        self.assertEqual((target / "src/lib.rs").read_text(), "other source\n")
        os.utime(self.source / "src/lib.rs", (1, 1))
        self.invoke(self.source)
        self.assert_same_git_state(self.source, target)
        self.assertGreater((target / "src/lib.rs").stat().st_mtime, artifact.stat().st_mtime)
        unchanged_time = (target / "src/lib.rs").stat().st_mtime_ns
        self.invoke(self.source)
        self.assertEqual((target / "src/lib.rs").stat().st_mtime_ns, unchanged_time)
        self.assertEqual(artifact.read_text(), "preserve build cache")

    def test_split_index_preserves_staged_objects_without_shared_index_pointers(self):
        (self.source / "new.txt").write_text("only in the index, not HEAD\n")
        self.git(self.source, "add", "new.txt")
        self.git(self.source, "update-index", "--split-index")
        before = (self.source / ".git/index").read_bytes()
        target = self.invoke()
        self.assert_same_git_state(self.source, target)
        self.assertEqual((self.source / ".git/index").read_bytes(), before)
        self.assertEqual(list((target / ".git").glob("sharedindex.*")), [])

    def test_opaque_remote_git_pointer_refuses_without_touching_its_old_target(self):
        target = self.remote / "sail"
        target.mkdir()
        source_git = self.source / ".git"
        original = (source_git / "HEAD").read_bytes()
        (target / ".git").symlink_to(source_git)
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertTrue((target / ".git").is_symlink())
        self.assertEqual((source_git / "HEAD").read_bytes(), original)
        self.assertFalse((self.remote / "sail-sync.lock").exists())

    def test_nonempty_checkout_without_prior_ownership_refuses(self):
        target = self.remote / "sail"
        (target / ".claude").mkdir(parents=True)
        old = target / ".claude/unknown-old-source"
        old.write_text("cannot establish prior ownership\n")
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertEqual(old.read_text(), "cannot establish prior ownership\n")
        self.assertFalse((target / ".git").exists())
        self.assertFalse((self.remote / "sail-sync-owned-paths.json").exists())

    def test_prior_git_migration_rejects_links_and_redirection(self):
        target = self.invoke()
        ledger = self.remote / "sail-sync-owned-paths.json"
        ledger.unlink()
        for name, content in [("commondir", "outside"), ("objects/info/alternates", "outside")]:
            with self.subTest(name=name):
                path = target / ".git" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
                with self.assertRaises(subprocess.CalledProcessError):
                    self.invoke()
                self.assertTrue((target / ".git/HEAD").exists())
                path.unlink()
        self.git(target, "config", "core.worktree", str(self.source))
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertTrue((target / ".git/HEAD").exists())
        self.git(target, "config", "--unset", "core.worktree")
        (target / ".git/linked-file").symlink_to(self.source / ".git/HEAD")
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertTrue((target / ".git/linked-file").is_symlink())
        self.assertFalse(ledger.exists())

    def test_readonly_source_directory_can_be_frozen_without_changing_it(self):
        directory = self.source / "readonly"
        directory.mkdir()
        (directory / "actual.txt").write_text("readable source\n")
        directory.chmod(0o555)
        self.addCleanup(directory.chmod, 0o755)
        target = self.invoke()
        self.assertEqual((target / "readonly/actual.txt").read_text(), "readable source\n")
        self.assertEqual(directory.stat().st_mode & 0o777, 0o555)

    def test_tracked_excluded_files_override_filters_without_private_siblings(self):
        agent = self.source / ".claude"
        agent.mkdir()
        settings = agent / "settings.json"
        settings.write_text("tracked project settings\n")
        literal = agent / "[tracked]?.json"
        literal.write_text("literal tracked filename\n")
        self.git(self.source, "add", ".claude/settings.json", ".claude/[tracked]?.json")
        self.commit(self.source, "tracked agent settings")
        (agent / "settings.local.json").write_text("private local settings\n")
        (agent / "worktrees").mkdir()
        (agent / "worktrees/private.txt").write_text("private worktree\n")
        target = self.invoke()
        self.assertEqual((target / ".claude/settings.json").read_bytes(), settings.read_bytes())
        self.assertEqual((target / ".claude/[tracked]?.json").read_bytes(), literal.read_bytes())
        self.assertFalse((target / ".claude/settings.local.json").exists())
        self.assertFalse((target / ".claude/worktrees").exists())
        (target / ".claude/remote-private.txt").write_text("preserve remote excluded data\n")
        settings.write_text("staged project settings\n")
        added = agent / "new*tracked.txt"
        added.write_text("staged added settings\n")
        self.git(self.source, "add", ".claude/settings.json", ".claude/new*tracked.txt")
        settings.write_text("actual dirty settings\n")
        added.write_text("actual new working settings\n")
        literal.unlink()
        self.invoke()
        self.assertEqual(self.git(self.source, "ls-files", "--stage", "-z"), self.git(target, "ls-files", "--stage", "-z"))
        self.assertEqual(self.git(self.source, "diff", "--binary"), self.git(target, "diff", "--binary"))
        self.assertEqual((target / ".claude/settings.json").read_bytes(), settings.read_bytes())
        self.assertEqual((target / ".claude/new*tracked.txt").read_bytes(), added.read_bytes())
        self.assertFalse((target / ".claude/[tracked]?.json").exists())
        self.assertEqual((target / ".claude/remote-private.txt").read_text(), "preserve remote excluded data\n")
        self.assertFalse((target / ".claude/settings.local.json").exists())
        self.assertFalse((target / ".claude/worktrees").exists())

    def test_committed_excluded_deletion_retires_only_previous_owned_source(self):
        agent = self.source / ".claude"
        agent.mkdir()
        obsolete = agent / "obsolete"
        obsolete.write_text("old tracked source\n")
        self.git(self.source, "add", ".claude/obsolete")
        self.commit(self.source, "branch A owns excluded source")
        target = self.invoke()
        private = target / ".claude/private-sibling"
        private.write_text("not owned by sync\n")
        # Migrate a checkout written by the earlier helper, before the ledger.
        (self.remote / "sail-sync-owned-paths.json").unlink()
        linked = self.base / "branch B"
        self.git(self.source, "worktree", "add", "-q", "-b", "branch-b", str(linked))
        self.git(linked, "rm", ".claude/obsolete")
        self.commit(linked, "branch B commits excluded deletion")
        self.invoke(linked)
        self.assertFalse((target / ".claude/obsolete").exists())
        self.assertEqual(private.read_text(), "not owned by sync\n")
        self.assertEqual(self.git(linked, "diff", "HEAD", "--binary"), self.git(target, "diff", "HEAD", "--binary"))
        self.invoke(self.source)
        self.assertEqual((target / ".claude/obsolete").read_bytes(), obsolete.read_bytes())
        self.git(self.source, "rm", ".claude/obsolete")
        self.invoke()
        self.assertFalse((target / ".claude/obsolete").exists())
        self.assertEqual(self.git(self.source, "diff", "--cached", "--binary"), self.git(target, "diff", "--cached", "--binary"))
        self.commit(self.source, "commit previously staged removal")
        self.invoke()
        self.assertFalse((target / ".claude/obsolete").exists())
        self.assertEqual(private.read_text(), "not owned by sync\n")

    def test_failed_transfer_keeps_retired_ownership_for_a_clean_retry(self):
        agent = self.source / ".claude"
        agent.mkdir()
        (agent / "obsolete").write_text("owned source before failure\n")
        self.git(self.source, "add", ".claude/obsolete")
        self.commit(self.source, "own excluded source")
        target = self.invoke()
        with mock.patch.dict(os.environ, {"SAIL_SYNC_TEST_FAIL_PREPARE": "1"}):
            with self.assertRaises(subprocess.CalledProcessError):
                self.invoke()
        self.assertFalse((target / ".git").exists())
        self.assertTrue((target / ".claude/obsolete").exists())
        self.git(self.source, "rm", ".claude/obsolete")
        self.commit(self.source, "remove source after incomplete transfer")
        self.invoke()
        self.assertFalse((target / ".claude/obsolete").exists())
        self.assert_same_git_state(self.source, target)
        ledger = json.loads((self.remote / "sail-sync-owned-paths.json").read_text())
        self.assertNotIn(".claude/obsolete", ledger["paths"])

    def test_invalid_ownership_ledger_refuses_before_git_or_source_mutation(self):
        target = self.invoke()
        before = (target / ".git/HEAD").read_bytes()
        ledger = self.remote / "sail-sync-owned-paths.json"
        ledger.write_text(json.dumps({"version":1, "paths":["../private"]}))
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertEqual((target / ".git/HEAD").read_bytes(), before)
        self.assertFalse((self.remote / "sail-sync.lock").exists())
        self.assertFalse((self.remote / "private").exists())

    def test_unmerged_index_stages_and_original_working_conflict_are_preserved(self):
        self.git(self.source, "checkout", "-q", "-b", "theirs")
        (self.source / "src/lib.rs").write_text("their version\n")
        self.git(self.source, "add", "src/lib.rs")
        self.commit(self.source, "theirs")
        self.git(self.source, "checkout", "-q", "fixture")
        (self.source / "src/lib.rs").write_text("our version\n")
        self.git(self.source, "add", "src/lib.rs")
        self.commit(self.source, "ours")
        self.git(self.source, "-c", "user.name=Fixture", "-c", "user.email=sync@example.invalid", "merge", "theirs", check=False)
        self.assertIn(b" 3\t", self.git(self.source, "ls-files", "--stage"))
        target = self.invoke()
        self.assert_same_git_state(self.source, target)
        self.assertEqual((target / "src/lib.rs").read_bytes(), (self.source / "src/lib.rs").read_bytes())

    def test_assume_unchanged_optimization_cannot_hide_actual_changed_source(self):
        self.git(self.source, "update-index", "--assume-unchanged", "src/lib.rs")
        (self.source / "src/lib.rs").write_text("changed despite optimization\n")
        self.assertEqual(self.git(self.source, "status", "--porcelain"), b"")
        target = self.invoke()
        self.assertIn(b" M src/lib.rs", self.git(target, "status", "--porcelain"))
        self.assertEqual((target / "src/lib.rs").read_bytes(), (self.source / "src/lib.rs").read_bytes())

    def test_unsupported_indexes_refuse_before_any_remote_mutation(self):
        original = self.source / ".git/index"
        before = original.read_bytes()
        for kind in ["intent", "skip", "sparse", "submodule"]:
            with self.subTest(kind=kind):
                original.write_bytes(before)
                self.git(self.source, "config", "core.sparseCheckout", "false")
                if kind == "intent":
                    (self.source / "intent.txt").write_text("intent\n")
                    self.git(self.source, "add", "-N", "intent.txt")
                elif kind == "skip":
                    self.git(self.source, "update-index", "--skip-worktree", "src/lib.rs")
                elif kind == "sparse":
                    self.git(self.source, "config", "core.sparseCheckout", "true")
                else:
                    head = self.git(self.source, "rev-parse", "HEAD").decode().strip()
                    self.git(self.source, "update-index", "--add", "--cacheinfo", "160000," + head + ",nested")
                with self.assertRaisesRegex(RuntimeError, "unsupported"):
                    self.invoke()
                self.assertFalse(self.calls.exists())
                self.assertEqual(list(self.remote.iterdir()), [])

    def test_source_or_index_drift_fails_closed_after_transfer(self):
        for kind in ["source", "index"]:
            with self.subTest(kind=kind):
                os.environ["SAIL_SYNC_TEST_MUTATION"] = kind
                marker = self.remote / "mutation-done"
                marker.unlink(missing_ok=True)
                with self.assertRaisesRegex(RuntimeError, "changed during sync"):
                    self.invoke()
                self.assertFalse((self.remote / "sail/.git").exists())
                self.assertFalse((self.remote / "sail-sync.lock").exists())
                self.assertEqual(list(self.remote.glob("sail-git-sync-*")), [])

    def test_metadata_transport_failure_does_not_leave_usable_git(self):
        target = self.invoke()
        os.environ["SAIL_SYNC_TEST_FAIL_METADATA"] = "1"
        with self.assertRaises(subprocess.CalledProcessError):
            self.invoke()
        self.assertFalse((target / ".git").exists())
        self.assertFalse((self.remote / "sail-sync.lock").exists())

    def test_head_drift_fails_closed_after_transfer(self):
        original = self.git(self.source, "rev-parse", "HEAD").decode().strip()
        self.git(self.source, "-c", "user.name=Fixture", "-c", "user.email=sync@example.invalid",
                 "commit", "--allow-empty", "-q", "-m", "other head")
        other = self.git(self.source, "rev-parse", "HEAD").decode().strip()
        self.git(self.source, "reset", "--hard", original)
        os.environ.update(SAIL_SYNC_TEST_MUTATION="head", SAIL_SYNC_TEST_OTHER_HEAD=other)
        with self.assertRaisesRegex(RuntimeError, "changed during sync"):
            self.invoke()
        self.assertFalse((self.remote / "sail/.git").exists())
        self.assertFalse((self.remote / "sail-sync.lock").exists())

    def test_failure_after_owner_creation_cleans_only_the_owned_lock(self):
        for failure in ["SAIL_SYNC_TEST_FAIL_PREPARE", "SAIL_SYNC_TEST_FAIL_ACQUIRE_ACK"]:
            with self.subTest(failure=failure), mock.patch.dict(os.environ, {failure: "1"}):
                with self.assertRaises(subprocess.CalledProcessError):
                    self.invoke()
                self.assertFalse((self.remote / "sail/.git").exists())
                self.assertFalse((self.remote / "sail-sync.lock").exists())
                self.assertEqual(list(self.remote.glob("sail-git-sync-*")), [])

    def test_foreign_or_missing_lock_owner_is_retained_without_source_mutation(self):
        target = self.invoke()
        before = (target / ".git/HEAD").read_bytes()
        lock = self.remote / "sail-sync.lock"
        lock.mkdir()
        for owner in [None, "different-owner"]:
            with self.subTest(owner=owner):
                if owner is not None:
                    (lock / "owner").write_text(owner)
                with self.assertRaises(subprocess.CalledProcessError):
                    self.invoke()
                self.assertTrue(lock.is_dir())
                self.assertEqual((target / ".git/HEAD").read_bytes(), before)
                if owner is not None:
                    self.assertEqual((lock / "owner").read_text(), owner)

    def test_freeze_drift_refuses_before_remote_source_is_touched(self):
        original = SYNC.copy_sources
        def changing(source, destination, tracked=()):
            original(source, destination, tracked)
            (source / "README.md").write_text("mutated during preparation\n")
        with mock.patch.object(SYNC, "copy_sources", changing), self.assertRaisesRegex(RuntimeError, "changed during sync"):
            self.invoke()
        self.assertFalse(self.calls.exists())

    def test_perf_host_entrypoint_uses_the_real_sync_helper(self):
        state = self.base / "state"
        state.mkdir()
        (state / "fixture.env").write_text("addr=fixture-host\n")
        os.environ.update(SAIL_PERF_STATE_DIR=str(state), PATH=str(self.base) + os.pathsep + os.environ["PATH"])
        result = subprocess.run(["bash", str(ROOT / "perf-host.sh"), "sync", "fixture"],
                                cwd=self.source, text=True, capture_output=True, timeout=120)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_same_git_state(self.source, self.remote / "sail")
        first = json.loads(self.calls.read_text().splitlines()[0])
        self.assertEqual(first["destination"], "ubuntu@fixture-host")

    def test_mac_host_entrypoint_preserves_its_own_transport_user(self):
        home = self.base / "mac-client-home"
        state = home / ".sail-perf"
        state.mkdir(parents=True)
        (state / "fixture.env").write_text("addr=fixture-host\n")
        env = dict(os.environ, HOME=str(home), PATH=str(self.base) + os.pathsep + os.environ["PATH"])
        result = subprocess.run(["bash", str(ROOT / "perf-host-mac.sh"), "sync", "fixture"],
                                cwd=self.source, env=env, text=True, capture_output=True, timeout=120)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assert_same_git_state(self.source, self.remote / "sail")
        first = json.loads(self.calls.read_text().splitlines()[0])
        self.assertEqual(first["destination"], "ec2-user@fixture-host")


if __name__ == "__main__":
    unittest.main()
