#!/usr/bin/env python3
"""Exercise scripts/perf-fleet.py against fake hosts.

Each fake host is a local directory used as HOME. A fake `ssh` runs the remote
command there with bash, and fake `sudo`, `systemd-run`, `systemctl`,
`shutdown` and `aws` stand in for the host and EC2. The real job.sh and
runner.sh scripts run unchanged, so detached jobs, DONE markers, sharding,
collection and resume after a lost host are tested end to end.
"""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest

SCRIPT = Path(__file__).with_name("perf-fleet.py")
spec = importlib.util.spec_from_file_location("perf_fleet", SCRIPT)
fleet = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fleet)

FAKE_SSH = r'''#!/bin/bash
# fake ssh OPTIONS... ubuntu@ADDR COMMAND
while [ $# -gt 0 ]; do case "$1" in ubuntu@*) addr=${1#ubuntu@}; shift; break ;; *) shift ;; esac; done
[ -e "$SAIL_FAKE_ROOT/dead/$addr" ] && { echo "ssh: connect to host $addr port 22: Connection refused" >&2; exit 255; }
export HOME="$SAIL_FAKE_ROOT/hosts/$addr"
mkdir -p "$HOME"; cd "$HOME"
echo "$addr" >>"$SAIL_FAKE_ROOT/ssh.log"
exec bash -c "$*"
'''

FAKE_SYSTEMD_RUN = r'''#!/usr/bin/env python3
import os, subprocess, sys
args = sys.argv[1:]
unit = wait = None; props = {}; cwd = None
while args and args[0] != "--" and args[0].startswith("-"):
    a = args.pop(0)
    if a == "--wait": wait = True
    elif a.startswith("--unit="): unit = a.split("=", 1)[1]
    elif a.startswith("--working-directory="): cwd = a.split("=", 1)[1]
    elif a == "-p": k, v = args.pop(0).split("=", 1); props[k] = v
if args and args[0] == "--": args.pop(0)
units = os.path.join(os.environ["HOME"], ".units"); os.makedirs(units, exist_ok=True)
with open(os.path.join(units, unit + ".props"), "w") as f:
    f.write(repr(props))
def opened(name, default):
    v = props.get(name)
    return open(v[5:], "w") if v and v.startswith("file:") else default
if wait:
    status = subprocess.call(args, cwd=cwd, stdin=subprocess.DEVNULL,
                             stdout=opened("StandardOutput", sys.stdout), stderr=opened("StandardError", sys.stderr))
    print("Running as unit: " + unit)
    print("Finished with result: " + ("success" if status == 0 else "exit-code"))
    sys.exit(status)
devnull = open(os.devnull, "w")
process = subprocess.Popen(args, cwd=cwd, stdin=subprocess.DEVNULL, stdout=devnull, stderr=devnull,
                           start_new_session=True)
with open(os.path.join(units, unit + ".pid"), "w") as f:
    f.write(str(process.pid))
'''

FAKE_SYSTEMCTL = r'''#!/bin/bash
# is-active [--quiet] UNIT | show -p Result --value UNIT
unit=${@: -1}
pidfile="$HOME/.units/$unit.pid"
case "$1" in
  is-active)
    if [ -f "$pidfile" ] && kill -0 "$(cat "$pidfile")" 2>/dev/null \
       && ! grep -q '^Z' <(ps -o stat= -p "$(cat "$pidfile")"); then
      [ "$2" = --quiet ] || echo active; exit 0
    fi
    [ "$2" = --quiet ] || echo inactive; exit 3 ;;
  show) echo signal ;;
esac
'''

FAKE_AWS = r'''#!/usr/bin/env python3
import json, os, sys
args = sys.argv[1:]
root = os.environ["SAIL_FAKE_ROOT"]
with open(os.path.join(root, "aws.log"), "a") as f:
    f.write(json.dumps(args) + "\n")
if "describe-instances" in args and "--instance-ids" in args:
    iid = args[args.index("--instance-ids") + 1]
    print("terminated" if os.path.exists(os.path.join(root, "dead-ids", iid)) else "running")
elif "describe-instance-types" in args:
    print(os.environ.get("SAIL_FAKE_VCPUS", "8"))
elif "get-service-quota" in args:
    print(os.environ.get("SAIL_FAKE_QUOTA_ERR", "An error occurred (AccessDeniedException) when calling "
          "the GetServiceQuota operation: not authorized"), file=sys.stderr)
    sys.exit(254)
elif "describe-instances" in args:
    print(os.environ.get("SAIL_FAKE_INSTANCES", "[]"))
else:
    print("unexpected fake aws call: %s" % args, file=sys.stderr); sys.exit(99)
'''

# A fake perf-host.sh: `up` honours SAIL_PERF_MARKET and can refuse spot.
FAKE_PERF_HOST = r'''#!/bin/bash
sub=$1; lane=$2; dir=$SAIL_PERF_STATE_DIR
case "$sub" in
  up)
    echo "$lane $SAIL_PERF_MARKET" >>"$SAIL_FAKE_ROOT/up.log"
    if [ "$SAIL_PERF_MARKET" = spot ] && [ -n "${SAIL_FAKE_REFUSE_SPOT:-}" ]; then
      echo "An error occurred (InsufficientInstanceCapacity) when calling the RunInstances operation: none" >&2; exit 254
    fi
    printf 'region=us-east-2\ntype=t3.small\narch=amd64\nmarket=%s\nid=i-%s\naddr=%s\n' "$SAIL_PERF_MARKET" "$lane" "$lane" >"$dir/$lane.env"
    echo "$lane ready" ;;
  down) rm -rf "$dir/$lane".{env,pem,known_hosts,bin}; echo "$lane down" ;;
  list) for f in "$dir"/*.env; do [ -f "$f" ] && printf '%s\ti-x\tt3.small\trunning\n' "$(basename "$f" .env)"; done; true ;;
esac
'''


class Fleet:
    def __init__(self, root):
        self.root = Path(root)
        self.state = self.root / "state"
        self.state.mkdir()
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for name in ("hosts", "dead", "dead-ids"):
            (self.root / name).mkdir()
        tools = {"ssh": FAKE_SSH, "systemd-run": FAKE_SYSTEMD_RUN, "systemctl": FAKE_SYSTEMCTL,
                 "aws": FAKE_AWS, "sudo": '#!/bin/sh\nexec "$@"\n', "perf-host.sh": FAKE_PERF_HOST,
                 "shutdown": '#!/bin/sh\necho "$@" >>"$HOME/shutdown.log"\n'}
        for name, text in tools.items():
            path = self.bin / name
            path.write_text(text)
            path.chmod(0o755)
        self.env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", SAIL_PERF_STATE_DIR=str(self.state),
                        SAIL_FAKE_ROOT=str(self.root), SAIL_FLEET_PERF_HOST=str(self.bin / "perf-host.sh"),
                        AWS_REGION="us-east-2")

    def lane(self, name):
        (self.state / f"{name}.env").write_text(
            f"region=us-east-2\ntype=t3.small\narch=amd64\nmarket=on-demand\nid=i-{name}\naddr={name}\n")
        (self.state / f"{name}.pem").write_text("key\n")

    def host(self, name):
        return self.root / "hosts" / name

    def kill(self, name):
        """Lose a host: unreachable, reported terminated, its processes gone."""
        (self.root / "dead" / name).touch()
        (self.root / "dead-ids" / f"i-{name}").touch()
        for pid in (self.host(name) / ".units").glob("*.pid"):
            try:
                os.killpg(int(pid.read_text()), signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass

    def tool(self, *args, check=True, timeout=120):
        result = subprocess.run([sys.executable, str(SCRIPT), *args], env=self.env, text=True,
                                capture_output=True, timeout=timeout)
        if check:
            assert result.returncode == 0, result.stdout + result.stderr
        return result

    def start(self, *args):
        return subprocess.Popen([sys.executable, str(SCRIPT), *args], env=self.env, text=True,
                                stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)


def events(results):
    path = Path(results) / "events.jsonl"
    return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []


def wait_for(predicate, timeout=60):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return True
        time.sleep(0.05)
    return False


class Pure(unittest.TestCase):
    def test_longest_first_sharding_balances_estimates(self):
        items = [{"id": name} for name in "abcdef"]
        estimates = {"a": 10, "b": 7, "c": 6, "d": 5, "e": 2, "f": 1}
        shards = fleet.shard(items, ["x-1", "x-2"], estimates)
        loads = {lane: sum(estimates[i["id"]] for i in group) for lane, group in shards.items()}
        self.assertEqual(sorted(loads.values()), [15, 16])
        self.assertEqual([i["id"] for i in shards["x-1"]][0], "a")
        self.assertEqual(sorted(i["id"] for group in shards.values() for i in group), list("abcdef"))

    def test_key_binds_the_command(self):
        self.assertEqual(fleet.item_key("c1", "true"), fleet.item_key("c1", "true"))
        self.assertNotEqual(fleet.item_key("c1", "true"), fleet.item_key("c1", "false"))
        self.assertRegex(fleet.item_key("c1", "true"), fleet.ITEM)

    def test_only_a_clean_exit_is_ok(self):
        self.assertEqual(fleet.classify("0", "success"), "ok")
        self.assertEqual(fleet.classify("0", "oom-kill"), "oom")
        self.assertEqual(fleet.classify("137", "oom-kill"), "oom")
        self.assertEqual(fleet.classify("124", "exit-code"), "timeout")
        self.assertEqual(fleet.classify("137", "exit-code"), "killed")
        self.assertEqual(fleet.classify("125", "steal"), "refused")
        self.assertEqual(fleet.classify("1", "exit-code"), "failed")
        self.assertEqual(fleet.classify("0", "signal"), "failed")

    def test_time_parsing(self):
        text = ("\tElapsed (wall clock) time (h:mm:ss or m:ss): 1:02.50\n"
                "\tMaximum resident set size (kbytes): 2048\n")
        self.assertEqual(fleet.wall_seconds(text), 62.5)
        self.assertEqual(fleet.max_rss_kib(text), 2048)
        self.assertIsNone(fleet.wall_seconds("no data"))

    def test_quiet_script_turns_off_smt_and_turbo_and_fixes_frequency(self):
        script = fleet.quiet_script(2400000)
        for line in ("echo off >/sys/devices/system/cpu/smt/control",
                     "echo 1 >/sys/devices/system/cpu/intel_pstate/no_turbo",
                     'echo 2400000 >"$c/scaling_max_freq"', 'echo 2400000 >"$c/scaling_min_freq"',
                     "touch /run/sail-fleet-quiet"):
            self.assertIn(line, script)
        self.assertIn("flock 9", fleet.JOB_SH)
        self.assertIn("taskset -c", fleet.JOB_SH)
        self.assertIn("numactl --membind=0", fleet.JOB_SH)
        self.assertIn("finish 125 steal", fleet.JOB_SH)

    def test_job_unit_has_memory_limit_no_swap_and_kill_after(self):
        for text in ('-p MemoryMax="$mem"', "-p MemorySwapMax=0", 'timeout --kill-after="$grace"',
                     "/usr/bin/time -v", '-p TimeoutStopSec="$grace"', '-p BindsTo="$parent"'):
            self.assertIn(text, fleet.JOB_SH)


@unittest.skipUnless(sys.platform.startswith("linux"), "the remote scripts target Linux hosts")
class Hosts(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="sail-fleet-")
        self.addCleanup(tmp.cleanup)
        self.fleet = Fleet(tmp.name)

    def jobs(self, commands):
        path = self.fleet.root / "jobs.jsonl"
        path.write_text("".join(json.dumps({"id": i, "cmd": c}) + "\n" for i, c in commands))
        return str(path)

    def test_detached_job_status_and_fetch(self):
        self.fleet.lane("f-1")
        self.fleet.tool("submit", "f-1", "hello", "--timeout", "30", "--", "sh", "-c", "echo out; echo err >&2")
        self.assertTrue(wait_for(lambda: (self.fleet.host("f-1") / "fleet/jobs/hello/DONE").exists()))
        status = self.fleet.tool("status", "f-1", "hello").stdout
        self.assertIn("hello\tok\trc=0\tresult=success", status)
        dest = self.fleet.root / "fetched"
        self.fleet.tool("fetch", "f-1", "hello", str(dest))
        self.assertEqual((dest / "hello/stdout").read_text(), "out\n")
        self.assertEqual((dest / "hello/stderr").read_text(), "err\n")
        self.assertIn("Elapsed (wall clock)", (dest / "hello/time").read_text())
        props = next((self.fleet.host("f-1") / ".units").glob("sail-fleet-item-*.props")).read_text()
        self.assertIn("'MemoryMax': '12G'", props)
        self.assertIn("'MemorySwapMax': '0'", props)
        again = self.fleet.tool("submit", "f-1", "hello", "--", "true", check=False)
        self.assertNotEqual(again.returncode, 0)

    def test_timeout_is_reported_as_timeout_never_ok(self):
        self.fleet.lane("f-1")
        self.fleet.tool("submit", "f-1", "slow", "--timeout", "0.3", "--grace", "1", "--", "sleep", "20")
        self.assertTrue(wait_for(lambda: (self.fleet.host("f-1") / "fleet/jobs/slow/DONE").exists()))
        self.assertIn("slow\ttimeout\trc=124", self.fleet.tool("status", "f-1", "slow").stdout)

    def test_job_that_died_without_output_is_reported_died(self):
        self.fleet.lane("f-1")
        self.fleet.tool("submit", "f-1", "gone", "--", "sleep", "30")
        units = self.fleet.host("f-1") / ".units"
        self.assertTrue(wait_for(lambda: (units / "sail-fleet-job-gone.pid").exists()))
        self.assertIn("gone\trunning", self.fleet.tool("status", "f-1", "gone").stdout)
        os.killpg(int((units / "sail-fleet-job-gone.pid").read_text()), signal.SIGKILL)
        self.assertTrue(wait_for(lambda: "gone\tdied" in self.fleet.tool("status", "f-1", "gone").stdout))
        self.assertIn("absent", self.fleet.tool("status", "f-1", "missing").stdout)
        dest = self.fleet.root / "fetched"
        self.assertIn("no DONE marker", self.fleet.tool("fetch", "f-1", "gone", str(dest)).stdout)

    def test_ship_verifies_one_build_on_every_lane(self):
        for lane in ("f-1", "f-2"):
            self.fleet.lane(lane)
        build = self.fleet.root / "build"
        (build / "sub").mkdir(parents=True)
        (build / "tool").write_bytes(b"\x7fELF binary")
        (build / "sub/data").write_text("data\n")
        out = self.fleet.tool("ship", "f", str(build), "--name", "b").stdout
        sha = out.split("sha256 ")[1].split()[0]
        self.assertEqual(out.count(f"verified {sha}"), 2)
        for lane in ("f-1", "f-2"):
            self.assertEqual((self.fleet.host(lane) / "fleet/build/b/build/tool").read_bytes(), b"\x7fELF binary")
            self.assertEqual((self.fleet.host(lane) / "fleet/build/b.sha256").read_text().strip(), sha)

    def test_helpers_are_private_to_the_lane(self):
        for lane in ("f-1", "f-2"):
            self.fleet.lane(lane)
        one = Path(self.fleet.tool("helpers", "f-1").stdout.strip())
        two = Path(self.fleet.tool("helpers", "f-2").stdout.strip())
        self.assertNotEqual(one, two)
        self.assertEqual(one, self.fleet.state / "f-1.bin")
        self.assertIn("-F /dev/null", (one / "ssh").read_text())
        self.assertIn("ubuntu@f-1", (one / "ssh").read_text())
        self.assertIn("f-1.pem", (one / "scp").read_text())
        result = subprocess.run([str(one / "ssh"), "echo", "hi"], env=self.fleet.env, capture_output=True, text=True)
        self.assertEqual(result.stdout, "hi\n")

    def test_run_resumes_only_unfinished_items_after_a_lost_host(self):
        for lane in ("f-1", "f-2", "f-3"):
            self.fleet.lane(lane)
        # Items block on f-2 until it is lost; elsewhere they finish at once.
        block = 'if [ "$(basename "$HOME")" = f-2 ]; then sleep 60; fi; '
        jobs = self.jobs([(f"c{i:02}", f"{block}echo {i}") for i in range(12)])
        results = self.fleet.root / "results"
        process = self.fleet.start("run", "f", jobs, "--results", str(results), "--poll", "0.1",
                                   "--setup", "echo setup >>$HOME/setup.log")
        collected = lambda: sum(1 for e in events(results) if e["event"] == "collected")
        self.assertTrue(wait_for(lambda: collected() >= 8))
        self.fleet.kill("f-2")
        output, _ = process.communicate(timeout=120)
        self.assertEqual(process.returncode, 0, output)
        summary = json.loads((results / "summary.json").read_text())
        self.assertEqual(summary["statuses"], {"ok": 12})
        self.assertEqual(summary["lost_lanes"], ["f-2"], "\n".join(map(json.dumps, events(results))))
        self.assertEqual(summary["duplicates"], 0)
        rows = events(results)
        collected_ids = [e["id"] for e in rows if e["event"] == "collected"]
        self.assertEqual(sorted(collected_ids), [f"c{i:02}" for i in range(12)])
        first = {i for e in rows if e["event"] == "submitted" for i in e["ids"]}
        resubmitted = [i for e in rows if e["event"] == "submitted" and e["round"] != "r1" for i in e["ids"]]
        before_loss = {e["id"] for e in rows[:next(n for n, e in enumerate(rows) if e["event"] == "lane-lost")]
                       if e["event"] == "collected"}
        on_lost = {i for e in rows if e["event"] == "submitted" and e["lane"] == "f-2" for i in e["ids"]}
        self.assertEqual(len(before_loss), 8, rows)
        self.assertEqual(sorted(resubmitted), sorted(on_lost), rows)
        self.assertFalse(set(resubmitted) & before_loss, rows)
        self.assertEqual(len(first), 12)
        for lane in ("f-1", "f-3"):
            self.assertEqual((self.fleet.host(lane) / "setup.log").read_text(), "setup\n")

    def test_restarted_coordinator_reruns_nothing_finished(self):
        self.fleet.lane("f-1")
        jobs = self.jobs([(f"c{i}", f"sleep 0.3; echo {i}") for i in range(6)])
        results = self.fleet.root / "results"
        process = self.fleet.start("run", "f", jobs, "--results", str(results), "--poll", "0.1")
        self.assertTrue(wait_for(lambda: sum(1 for e in events(results) if e["event"] == "collected") >= 2))
        os.killpg(process.pid, signal.SIGKILL)
        process.wait()
        process.stdout.close()
        first = [e for e in events(results) if e["event"] == "submitted"]
        self.fleet.tool("run", "f", jobs, "--results", str(results), "--poll", "0.1", timeout=120)
        rows = events(results)
        self.assertEqual(sum(1 for e in rows if e["event"] == "submitted"), len(first),
                         "the restart must wait for the running round, not resubmit it")
        done = sorted(path.name.split(".")[0] for path in (results / "items").glob("*/DONE") for path in [path.parent])
        self.assertEqual(done, [f"c{i}" for i in range(6)])
        self.assertEqual(json.loads((results / "summary.json").read_text())["duplicates"], 0)
        # A third invocation finds everything done and submits nothing.
        self.fleet.tool("run", "f", jobs, "--results", str(results), "--poll", "0.1")
        self.assertEqual(sum(1 for e in events(results) if e["event"] == "submitted"), len(first))

    def test_failed_items_are_final_and_the_run_reports_them(self):
        self.fleet.lane("f-1")
        jobs = self.jobs([("good", "true"), ("bad", "exit 3"), ("slow", "sleep 20")])
        results = self.fleet.root / "results"
        self.fleet.tool("run", "f", jobs, "--results", str(results), "--poll", "0.1", "--timeout", "0.5",
                        "--grace", "1")
        summary = json.loads((results / "summary.json").read_text())
        self.assertEqual(summary["statuses"], {"ok": 1, "failed": 1, "timeout": 1})

    def test_up_falls_back_to_on_demand_when_spot_is_refused(self):
        self.fleet.env["SAIL_FAKE_REFUSE_SPOT"] = "1"
        out = self.fleet.tool("up", "g", "2", "--type", "t3.small").stdout
        self.assertIn("g-1: up (on-demand)", out)
        self.assertIn("g-2: up (on-demand)", out)
        log = (self.fleet.root / "up.log").read_text().split("\n")
        self.assertEqual(sorted(line for line in log if line),
                         ["g-1 on-demand", "g-1 spot", "g-2 on-demand", "g-2 spot"])
        down = self.fleet.tool("down", "g").stdout
        self.assertIn("fleet g down: 2 lane(s), none listed", down)

    def test_spot_only_does_not_fall_back(self):
        self.fleet.env["SAIL_FAKE_REFUSE_SPOT"] = "1"
        result = self.fleet.tool("up", "g", "1", "--type", "t3.small", "--market", "spot", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.fleet.root / "up.log").read_text(), "g-1 spot\n")

    def test_precheck_degrades_without_servicequotas_permission(self):
        result = self.fleet.tool("precheck", "c7i.2xlarge", "3")
        self.assertIn("no servicequotas permission; quota unknown", result.stderr)
        self.fleet.env["SAIL_FAKE_INSTANCES"] = json.dumps([["c7i.4xlarge", None, 8, 2]])
        result = self.fleet.tool("precheck", "c7i.2xlarge", "3", "--vcpu-limit", "32", check=False)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("need 24 vCPU, in use 16, quota 32 (--vcpu-limit)", result.stderr)
        self.fleet.env["SAIL_FLEET_VCPU_LIMIT"] = "64"
        result = self.fleet.tool("precheck", "c7i.2xlarge", "3")
        self.assertIn("quota 64 (SAIL_FLEET_VCPU_LIMIT)", result.stderr)

    def test_up_requires_an_explicit_region(self):
        del self.fleet.env["AWS_REGION"]
        result = self.fleet.tool("up", "g", "1", check=False)
        self.assertEqual(result.returncode, 2)
        self.assertFalse((self.fleet.root / "up.log").exists())

    def test_quiet_refuses_virtual_types_and_dry_run_prints(self):
        self.fleet.lane("f-1")
        result = self.fleet.tool("quiet", "f-1", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("bare-metal", result.stderr)
        self.assertIn("smt/control", self.fleet.tool("quiet", "f-1", "--dry-run").stdout)

    def test_quiet_pool_job_refuses_an_unconfigured_host(self):
        self.fleet.lane("f-1")
        self.fleet.tool("submit", "f-1", "timed", "--quiet-pool", "--", "true")
        self.assertTrue(wait_for(lambda: (self.fleet.host("f-1") / "fleet/jobs/timed/DONE").exists()))
        self.assertIn("timed\trefused\trc=125\tresult=not-quiet", self.fleet.tool("status", "f-1", "timed").stdout)


if __name__ == "__main__":
    unittest.main()
