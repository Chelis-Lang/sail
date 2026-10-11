#!/usr/bin/env python3
"""Pure/fake-proc controls; no test, solver, daemon or observer process launch."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location("observer", Path(__file__).with_name("m7-faults-live-observer.py"))
OBS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OBS)


def raw_stat(pid, parent=1, start=10, state="S", comm="a ) ( b"):
    fields = [state, str(parent)] + ["0"] * 18
    fields[11], fields[12], fields[19] = "12", "13", str(start)
    return f"{pid} ({comm}) {' '.join(fields)}\n".encode()


def row(pid, parent=1, start=10, state="S"):
    return OBS.parse_stat(raw_stat(pid, parent, start, state), pid)


def anchor(pid, start):
    return {"identity": (pid, start), "root": (pid, start)}


def frozen(path):
    inode = path.stat()
    return {"path": str(path), "dev": inode.st_dev, "ino": inode.st_ino,
            "bytes": inode.st_size, "uid": inode.st_uid,
            "mtime_ns": inode.st_mtime_ns, "ctime_ns": inode.st_ctime_ns}


def fixture_process(proc, pid, parent, start, executable):
    base = proc / str(pid)
    (base / "task" / str(pid)).mkdir(parents=True)
    (base / "fd").mkdir()
    (base / "fdinfo").mkdir()
    data = raw_stat(pid, parent, start)
    (base / "stat").write_bytes(data)
    task = base / "task" / str(pid)
    (task / "stat").write_bytes(data)
    (task / "comm").write_bytes(b"sail-z3-out\n")
    (task / "wchan").write_bytes(b"anon_pipe_write\n")
    (base / "exe").symlink_to(executable)
    (base / "fd" / "1").symlink_to("pipe:[765]")
    (base / "fdinfo" / "1").write_bytes(b"pos:\t0\nflags:\t01\nmnt_id:\t17\nino:\t765\nsecret:\tignored\n")


class ObserverControls(unittest.TestCase):
    def test_kernel_comm_parentheses_and_cpu_fields(self):
        self.assertEqual(row(7), {"pid": 7, "ppid": 1, "start_ticks": 10,
                                  "state": "S", "utime_ticks": 12, "stime_ticks": 13})
        for data in (raw_stat(8), b"7 (short) S 1", b"7 missing parentheses"):
            with self.assertRaises(ValueError):
                OBS.parse_stat(data, 7)

    def test_live_birth_ordered_descendant_chain(self):
        rows = {10: row(10, start=10), 11: row(11, 10, 11), 12: row(12, 11, 12), 99: row(99)}
        selected = OBS.descendents(rows, {10: anchor(10, 10)}, 128)
        self.assertEqual(set(selected), {10, 11, 12})
        self.assertEqual(selected[12]["parent_identity"], (11, 11))

    def test_child_predating_recycled_intermediate_parent_refused(self):
        rows = {10: row(10, start=10), 11: row(11, 10, 30), 12: row(12, 11, 20)}
        self.assertEqual(set(OBS.descendents(rows, {10: anchor(10, 10)}, 128)), {10, 11})

    def test_zombie_parent_and_recycled_anchor_refused(self):
        rows = {10: row(10, start=11), 11: row(11, 10, 12)}
        self.assertEqual(OBS.descendents(rows, {10: anchor(10, 10)}, 128), {})
        rows[10] = row(10, start=10, state="Z")
        self.assertEqual(OBS.descendents(rows, {10: anchor(10, 10)}, 128), {})

    def test_anchor_and_descendant_limits_apply_before_growth(self):
        rows = {pid: row(pid, start=pid) for pid in range(10, 20)}
        anchors = {pid: anchor(pid, pid) for pid in rows}
        self.assertEqual(len(OBS.descendents(rows, anchors, 3)), 3)
        rows = {10: row(10), **{pid: row(pid, 10, pid) for pid in range(11, 20)}}
        self.assertEqual(len(OBS.descendents(rows, {10: anchor(10, 10)}, 3)), 3)

    def test_chain_rechecks_actual_parent_birth(self):
        rows = {10: row(10), 11: row(11, 10, 11)}
        selected = OBS.descendents(rows, {10: anchor(10, 10)}, 128)
        cycle = OBS.Cycle("unused")
        with patch.object(cycle, "process", side_effect=lambda pid: row(pid, 10, 11) if pid == 11 else row(10, start=99)):
            self.assertFalse(OBS.still_chain(cycle, 11, selected, rows))

    def test_fdinfo_fixed_metadata_only(self):
        value = OBS.fd_metadata(b"pos: 0\nflags: 02000001\nmnt_id: 12\nino: 765\ncommand: secret\nflags: secret value\n")
        self.assertEqual(value, {"pos": "0", "flags": "02000001", "mnt_id": "12", "ino": "765"})

    def test_read_and_directory_bounds_record_partial(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            (path / "large").write_bytes(b"12345")
            cycle = OBS.Cycle(path)
            with self.assertRaises(OBS.Partial):
                cycle.read(path / "large", 4)
            for number in range(5):
                (path / str(number)).touch()
            self.assertEqual(len(cycle.numeric(path, 2)), 2)
            self.assertEqual(cycle.notes[0]["reason"], "directory-count-limit")

    def test_actual_fake_proc_snapshot_retains_thread_pipe_mapping(self):
        with tempfile.TemporaryDirectory() as temp:
            base = Path(temp)
            elf_path = base / "frozen-elf"
            elf_path.write_bytes(b"fixture")
            proc = base / "proc"
            proc.mkdir()
            fixture_process(proc, 100, 1, 10, elf_path)
            fixture_process(proc, 101, 100, 11, elf_path)
            child_elf = base / "child-elf"
            child_elf.write_bytes(b"child")
            (proc / "101" / "exe").unlink()
            (proc / "101" / "exe").symlink_to(child_elf)
            limits = {**OBS.LIMITS, "cycle_seconds": 10}
            known = {}
            result = OBS.snapshot(proc, frozen(elf_path), known, limits)
            self.assertFalse(result["partial"], result["unavailable"])
            self.assertEqual([entry["pid"] for entry in result["processes"]], [100, 101])
            child = result["processes"][1]
            self.assertEqual(child["root_identity"], [100, 10])
            self.assertEqual(child["threads"][0]["wchan"], "anon_pipe_write")
            self.assertEqual(child["fds"][0]["target"], "pipe:[765]")
            self.assertEqual(child["fds"][0]["metadata"]["ino"], "765")
            # Previously birth-bound child remains observable after root exit.
            (proc / "100" / "stat").write_bytes(raw_stat(100, 1, 10, "Z"))
            result = OBS.snapshot(proc, frozen(elf_path), known, limits)
            self.assertEqual([entry["pid"] for entry in result["processes"]], [101])
            self.assertTrue(result["processes"][0]["previously_bound"])

    def test_recycled_thread_and_process_metadata_discarded(self):
        with tempfile.TemporaryDirectory() as temp:
            proc = Path(temp)
            elf_path = proc / "elf"
            elf_path.touch()
            fixture_process(proc, 100, 1, 10, elf_path)
            cycle = OBS.Cycle(proc)
            original = cycle.read
            calls = 0

            def read(path, limit):
                nonlocal calls
                if Path(path).parts[-3:] == ("task", "100", "stat"):
                    calls += 1
                    if calls == 2:
                        return raw_stat(100, 1, 99)
                return original(path, limit)

            with patch.object(cycle, "read", side_effect=read):
                value = OBS.details(cycle, row(100))
            self.assertEqual(value["threads"], [])
            self.assertEqual(cycle.notes[0]["reason"], "thread-recycled")
            with patch.object(cycle, "process", return_value=row(100, start=99)), self.assertRaises(OBS.Partial):
                OBS.details(cycle, row(100))

    def test_fd_replacement_discarded_and_target_never_opened(self):
        with tempfile.TemporaryDirectory() as temp:
            proc = Path(temp)
            elf = proc / "elf"
            elf.touch()
            fixture_process(proc, 100, 1, 10, elf)
            cycle = OBS.Cycle(proc)
            original = cycle.link
            calls = 0

            def link(path):
                nonlocal calls
                if Path(path).parts[-2:] == ("fd", "1"):
                    calls += 1
                    if calls == 2:
                        return "pipe:[other]"
                return original(path)

            with patch.object(cycle, "link", side_effect=link):
                value = OBS.details(cycle, row(100))
            self.assertEqual(value["fds"], [])
            self.assertIn("fd-target-changed", [note["reason"] for note in cycle.notes])

    def test_private_targets_aliases_and_unhashed_globs_rejected(self):
        for path in ("/tmp/private/target/debug/deps/faults-123", "/work/target/debug/deps/faults-*",
                     "/work/target/debug/deps/not-faults-123", "/work/target/debug/deps/../deps/faults-123"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                OBS.elf_identity(path, "0" * 64)

    def test_frozen_executable_replacement_refuses_all_records(self):
        with tempfile.TemporaryDirectory() as temp:
            proc = Path(temp) / "proc"
            proc.mkdir()
            elf = Path(temp) / "elf"
            elf.write_bytes(b"old")
            before = frozen(elf)
            elf.write_bytes(b"new")
            result = OBS.snapshot(proc, before, {})
            self.assertEqual(result["processes"], [])
            self.assertTrue(result["partial"])
            self.assertEqual(result["unavailable"][0]["reason"], "frozen-elf-changed")

    def test_storage_bound_retains_terminal_marker(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "observations.jsonl"
            writer = OBS.Writer(path, {**OBS.LIMITS, "record_bytes": 256, "output_bytes": 4352})
            self.assertTrue(writer.write({"kind": "header"}))
            self.assertFalse(writer.write({"oversize": "x" * 300}))
            writer.finish("record-or-output-limit")
            rows = [json.loads(line) for line in path.read_bytes().splitlines()]
            self.assertEqual(rows[-1]["reason"], "record-or-output-limit")
            self.assertFalse(rows[-1]["closure_certified"])
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)

    def test_phase_labels_never_inferred_from_processes(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "phase"
            self.assertEqual(OBS.phase(None), "unattributed")
            self.assertEqual(OBS.phase(path), "unavailable")
            path.write_bytes(b"solver\n")
            self.assertEqual(OBS.phase(path), "solver")
            path.write_bytes(b"solver completed\n")
            self.assertEqual(OBS.phase(path), "unavailable")


if __name__ == "__main__":
    unittest.main()
