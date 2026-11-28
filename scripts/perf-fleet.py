#!/usr/bin/env python3
"""Fleets of native EC2 perf hosts with detached, resumable jobs (ADR 0191).

Lanes are ordinary `scripts/perf-host.sh` lanes named FLEET-1..FLEET-N, so
they keep its tags, TTL self-termination, recorded region and verified
teardown. This tool adds what long throughput runs need:

  up FLEET COUNT [--type T] [--market spot-fallback|spot|on-demand] [--vcpu-limit N]
                        launch COUNT lanes at once (spot with on-demand fallback
                        by default) after a vCPU precheck
  list [FLEET]          running instances of the fleet plus recorded lanes
  down FLEET            tear every lane down, then confirm none is listed
  extend FLEET MINUTES  heartbeat: every lane now self-terminates MINUTES from now
  helpers LANE          write lane-private ssh/scp wrappers and print their directory
  ship FLEET PATH...    copy one build to every lane, verified by sha256
  submit LANE NAME [options] -- CMD...
                        start CMD as a detached systemd unit with its own cgroup
  status LANE [NAME...] state of detached jobs (tolerates jobs that died silently)
  fetch LANE NAME DEST  copy one job's directory off the host
  run FLEET JOBS --results DIR [options]
                        run a JSONL job list across the fleet: shard by longest
                        prior time, collect results as they finish, re-run only
                        unfinished items after a lost node or a restart
  quiet LANE [--dry-run] configure a bare-metal lane as a quiet timing host
  precheck TYPE COUNT [--market M] [--vcpu-limit N]
                        the vCPU check alone

Every detached command runs in its own transient unit with MemoryMax and no
swap, under `timeout --kill-after` and `/usr/bin/time -v`, and always leaves
`rc`, `result` and `DONE` unless the host itself is lost. AWS_REGION must be
set for `up` and `precheck`; other subcommands use each lane's recorded region.
"""
import argparse
import base64
import concurrent.futures
import hashlib
import io
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile
import tempfile
import time

HERE = Path(__file__).resolve().parent
NAME = re.compile(r"^[a-z0-9][a-z0-9-]{0,40}$")
ITEM = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$")
REMOTE_ROOT = "fleet"  # under the host user's home

# Standard (A, C, D, H, I, M, R, T, Z) families share one EC2 vCPU quota per
# market; other families are not prechecked.
STANDARD_FAMILY = re.compile(r"^[acdhimrtz][0-9]")
QUOTA_CODES = {"on-demand": "L-1216C47A", "spot": "L-34B43A08"}
# A spot request refused for capacity, price or limit falls back to on-demand.
SPOT_REFUSALS = ("InsufficientInstanceCapacity", "SpotMaxPriceTooLow", "MaxSpotInstanceCountExceeded",
                 "InsufficientCapacity", "UnfulfillableCapacity", "SpotCapacityNotAvailable",
                 "VcpuLimitExceeded", "Unsupported", "AuthFailure.ServiceLinkedRoleCreationNotPermitted")


def fail(message, status=1):
    print(f"perf-fleet: {message}", file=sys.stderr)
    sys.exit(status)


def state_dir():
    return Path(os.environ.get("SAIL_PERF_STATE_DIR", Path.home() / ".sail-perf"))


def perf_host():
    return os.environ.get("SAIL_FLEET_PERF_HOST", str(HERE / "perf-host.sh"))


def check_name(kind, value):
    if not NAME.match(value):
        fail(f"{kind} must match [a-z0-9][a-z0-9-]*: {value!r}", 2)
    return value


def read_lane(lane):
    path = state_dir() / f"{lane}.env"
    if not path.is_file():
        fail(f"no recorded host for lane {lane}")
    info = {}
    for line in path.read_text().splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            info[key] = value
    return info


def fleet_lanes(fleet):
    pattern = re.compile(rf"^{re.escape(fleet)}-([0-9]+)\.env$")
    found = []
    for path in state_dir().glob(f"{fleet}-*.env"):
        match = pattern.match(path.name)
        if match:
            found.append((int(match.group(1)), path.name[:-4]))
    return [lane for _, lane in sorted(found)]


def host_env():
    env = dict(os.environ)
    env.setdefault("SAIL_PERF_SSH_CONFIG", "/dev/null")
    return env


def ssh_options(lane):
    directory = state_dir()
    return ["-F", os.environ.get("SAIL_PERF_SSH_CONFIG", "/dev/null"), "-i", str(directory / f"{lane}.pem"),
            "-o", "StrictHostKeyChecking=accept-new", "-o", f"UserKnownHostsFile={directory / f'{lane}.known_hosts'}",
            "-o", "ServerAliveInterval=30", "-o", "ConnectTimeout=10", "-o", "BatchMode=yes"]


def remote(lane, script, *args, data=None, timeout=600, check=True):
    """Run a bash script on the lane's host with ARGS, script and data on stdin."""
    info = read_lane(lane)
    if not info.get("addr"):
        fail(f"lane {lane} has no reachable host")
    # Bash reads a script from a pipe one byte at a time, so a script built by
    # script_with_payload leaves DATA on stdin for the commands it runs.
    payload = script.encode() + b"\n" + (data if data is not None else b"")
    command = "bash -s -- " + " ".join(shlex.quote(str(arg)) for arg in args)
    argv = ["ssh", *ssh_options(lane), f"ubuntu@{info['addr']}", command]
    try:
        result = subprocess.run(argv, input=payload, capture_output=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        result = subprocess.CompletedProcess(argv, 255, b"", b"ssh timed out\n")
    if check and result.returncode != 0:
        fail(f"{lane}: remote step failed ({result.returncode}): {result.stderr.decode(errors='replace').strip()}")
    return result


def aws(region, *args):
    return subprocess.run(["aws", "--region", region, *args], capture_output=True, text=True)


def instance_state(lane):
    info = read_lane(lane)
    if not info.get("id"):
        return "absent"
    result = aws(info["region"], "ec2", "describe-instances", "--instance-ids", info["id"],
                 "--query", "Reservations[0].Instances[0].State.Name", "--output", "text")
    if result.returncode != 0:
        return "absent" if "InvalidInstanceID.NotFound" in result.stderr else "unknown"
    return result.stdout.strip() or "unknown"


def parallel(lanes, function):
    with concurrent.futures.ThreadPoolExecutor(max_workers=max(1, len(lanes))) as pool:
        return dict(zip(lanes, pool.map(function, lanes)))


# ---------------------------------------------------------------------------
# Remote scripts. They run on Ubuntu 24.04 hosts as `ubuntu`, which has sudo.

JOB_SH = r'''#!/bin/bash
# job.sh DIR MEM TIMEOUT GRACE QUIET CPUS
# Runs DIR/cmd once in its own transient unit (MemoryMax=MEM, no swap) under
# timeout --kill-after=GRACE TIMEOUT and /usr/bin/time -v. It runs as root so the
# command's unit, not this wrapper, is what an out-of-memory kill or timeout
# ends: DIR/rc, DIR/result and DIR/DONE are always written afterwards.
set -u
dir=$1 mem=$2 limit=$3 grace=$4 quiet=${5:-0} cpus=${6:-}
cd "$dir" || exit 1
owner=$(stat -c %U "$dir")
rm -f rc result DONE time stdout stderr unit.log finished
date -u +%Y-%m-%dT%H:%M:%SZ >started
finish() {
  printf '%s\n' "$1" >rc.tmp && mv rc.tmp rc
  printf '%s\n' "$2" >result
  date -u +%Y-%m-%dT%H:%M:%SZ >finished
  chown -R "$owner:" "$dir" 2>/dev/null
  touch DONE
  exit 0
}
prefix=()
if [ "$quiet" = 1 ]; then
  # Quiet pool: one job at a time, only on a host `quiet` configured, and only
  # when no CPU time was stolen and no other work is runnable.
  [ -f /run/sail-fleet-quiet ] || finish 125 not-quiet
  exec 9>/run/sail-fleet-quiet.lock
  flock 9
  s1=$(awk '/^cpu /{print $9}' /proc/stat); sleep 1; s2=$(awk '/^cpu /{print $9}' /proc/stat)
  [ $((s2 - s1)) -le "${SAIL_FLEET_MAX_STEAL:-0}" ] || finish 125 steal
  running=$(awk '/^procs_running/{print $2}' /proc/stat)
  [ "$running" -le 2 ] || finish 125 busy
  prefix=(taskset -c "$cpus" numactl --membind=0)
fi
timer=()
[ -x /usr/bin/time ] && timer=(/usr/bin/time -v -o "$dir/time")
unit="sail-fleet-item-$(basename "$dir" | tr -c 'A-Za-z0-9_\n-' _)-$(date +%s%N)"
# The item unit stops with the job or runner unit that started it, and any
# process left after timeout(1) exits is killed GRACE later, not after
# systemd's default 90 s stop timeout.
parent=$(sed -n 's|^0::.*/||p' /proc/self/cgroup)
bind=()
case "$parent" in *.service) bind=(-p BindsTo="$parent" -p After="$parent") ;; esac
systemd-run --wait --collect --uid="$owner" --gid="$owner" --unit="$unit" \
  -p MemoryMax="$mem" -p MemorySwapMax=0 -p TimeoutStopSec="$grace" ${bind[@]+"${bind[@]}"} \
  -p StandardOutput="file:$dir/stdout" -p StandardError="file:$dir/stderr" \
  --working-directory="$dir" -- \
  ${timer[@]+"${timer[@]}"} ${prefix[@]+"${prefix[@]}"} \
  timeout --kill-after="$grace" "$limit" /bin/bash "$dir/cmd" >unit.log 2>&1
rc=$?
result=$(sed -n 's/^Finished with result: //p' unit.log | tail -n 1)
finish "$rc" "${result:-unknown}"
'''

RUNNER_SH = r'''#!/bin/bash
# runner.sh RUN ROUND SLOTS SETUP_TIMEOUT MEM TIMEOUT GRACE QUIET CPUS
# Runs the lane's setup once, then every item of shards/ROUND.keys that has no
# DONE marker, SLOTS at a time. Finished items are never run again.
set -u
run=$1 round=$2 slots=$3 setup_timeout=$4; shift 4
lib=$(cd "$(dirname "$0")" && pwd)
cd "$run" || exit 1
if [ -f setup/cmd ] && [ ! -f setup/DONE ]; then
  bash "$lib/job.sh" "$run/setup" "$1" "$setup_timeout" "$3" 0 ""
fi
if [ ! -f setup/cmd ] || [ "$(cat setup/rc 2>/dev/null)" = 0 ]; then
  while read -r key; do
    [ -n "$key" ] && [ ! -f "items/$key/DONE" ] && printf '%s\n' "$key"
  done <"shards/$round.keys" |
    xargs -r -P "$slots" -I{} bash "$lib/job.sh" "$run/items/{}" "$@"
fi
touch "shards/$round.DONE"
'''

INSTALL_LIB = r'''set -eu
lib="$HOME/fleet/lib"; mkdir -p "$lib"
printf '%s' "$1" | base64 -d >"$lib/job.sh.tmp" && mv "$lib/job.sh.tmp" "$lib/job.sh"
printf '%s' "$2" | base64 -d >"$lib/runner.sh.tmp" && mv "$lib/runner.sh.tmp" "$lib/runner.sh"
chmod 755 "$lib/job.sh" "$lib/runner.sh"
'''


def install_lib(lane):
    remote(lane, INSTALL_LIB, base64.b64encode(JOB_SH.encode()).decode(),
           base64.b64encode(RUNNER_SH.encode()).decode())


def quiet_script(freq_khz):
    return f'''set -eu
# Quiet timing host (ADR 0191): SMT and turbo off, fixed frequency, background
# services stopped. Jobs submitted with --quiet-pool run one at a time behind
# a lock, bound to fixed CPUs and NUMA node 0, after a steal and busy check.
echo off >/sys/devices/system/cpu/smt/control
[ ! -f /sys/devices/system/cpu/intel_pstate/no_turbo ] || echo 1 >/sys/devices/system/cpu/intel_pstate/no_turbo
for c in /sys/devices/system/cpu/cpu[0-9]*/cpufreq; do
  [ -d "$c" ] || continue
  echo performance >"$c/scaling_governor"
  echo {freq_khz} >"$c/scaling_max_freq"
  echo {freq_khz} >"$c/scaling_min_freq"
done
systemctl stop snapd unattended-upgrades apt-daily.timer apt-daily-upgrade.timer fwupd-refresh.timer 2>/dev/null || true
[ "$(cat /sys/devices/system/cpu/smt/active)" = 0 ] || {{ echo "SMT is still active" >&2; exit 1; }}
turbo=$(cat /sys/devices/system/cpu/intel_pstate/no_turbo 2>/dev/null || echo unknown)
freqs=$(cat /sys/devices/system/cpu/cpu[0-9]*/cpufreq/scaling_max_freq 2>/dev/null | sort -u | tr '\\n' ' ')
[ "$freqs" = "{freq_khz} " ] || {{ echo "frequency not fixed: $freqs" >&2; exit 1; }}
touch /run/sail-fleet-quiet
echo "quiet: smt=off no_turbo=$turbo max_freq_khz=$freqs cpus=$(nproc)"
'''


# ---------------------------------------------------------------------------
# Launch, list, teardown.

def instance_vcpus(region, instance_type):
    result = aws(region, "ec2", "describe-instance-types", "--instance-types", instance_type,
                 "--query", "InstanceTypes[0].VCpuInfo.DefaultVCpus", "--output", "text")
    if result.returncode != 0 or not result.stdout.strip().isdigit():
        fail(f"cannot read the vCPU count of {instance_type}: {result.stderr.strip()}")
    return int(result.stdout.strip())


def vcpus_in_use(region, market):
    """Standard-family vCPUs of running or pending instances in the region."""
    result = aws(region, "ec2", "describe-instances",
                 "--filters", "Name=instance-state-name,Values=pending,running",
                 "--query", "Reservations[].Instances[].[InstanceType,InstanceLifecycle,"
                 "CpuOptions.CoreCount,CpuOptions.ThreadsPerCore]", "--output", "json")
    if result.returncode != 0:
        return None
    total = 0
    for row in json.loads(result.stdout or "[]"):
        kind, lifecycle, cores, threads = row
        if not STANDARD_FAMILY.match(kind or ""):
            continue
        if (lifecycle == "spot") != (market == "spot"):
            continue
        total += (cores or 0) * (threads or 1)
    return total


def precheck(region, instance_type, count, market, limit=None):
    """Return True when launching fits the known quota. A missing
    servicequotas permission degrades to SAIL_FLEET_VCPU_LIMIT or a warning."""
    need = instance_vcpus(region, instance_type) * count
    if not STANDARD_FAMILY.match(instance_type):
        print(f"precheck: {instance_type} is not a standard family; quota not checked", file=sys.stderr)
        return True
    quota, source = None, None
    if limit is not None:
        quota, source = limit, "--vcpu-limit"
    else:
        result = aws(region, "service-quotas", "get-service-quota", "--service-code", "ec2",
                     "--quota-code", QUOTA_CODES[market], "--query", "Quota.Value", "--output", "text")
        if result.returncode == 0:
            try:
                quota, source = int(float(result.stdout.strip())), "service quotas"
            except ValueError:
                quota = None
        elif "AccessDenied" in result.stderr:
            print("precheck: no servicequotas permission; quota unknown", file=sys.stderr)
        else:
            print(f"precheck: quota lookup failed: {result.stderr.strip()}", file=sys.stderr)
        if quota is None and os.environ.get("SAIL_FLEET_VCPU_LIMIT", "").isdigit():
            quota, source = int(os.environ["SAIL_FLEET_VCPU_LIMIT"]), "SAIL_FLEET_VCPU_LIMIT"
    used = vcpus_in_use(region, market)
    print(f"precheck: {region} {market}: need {need} vCPU, in use {used if used is not None else 'unknown'}, "
          f"quota {quota if quota is not None else 'unknown'}" + (f" ({source})" if source else ""),
          file=sys.stderr)
    if quota is None or used is None:
        return True
    return used + need <= quota


def up_lane(lane, instance_type, market):
    """Launch one lane; a refused spot request falls back to on-demand."""
    markets = {"spot-fallback": ["spot", "on-demand"], "spot": ["spot"], "on-demand": ["on-demand"]}[market]
    log = []
    for attempt in markets:
        env = host_env()
        env["SAIL_PERF_MARKET"] = attempt
        result = subprocess.run(["bash", perf_host(), "up", lane, instance_type], env=env,
                                capture_output=True, text=True)
        log.append(f"[{lane} {attempt}] {result.stdout}{result.stderr}".rstrip())
        if result.returncode == 0:
            return True, attempt, "\n".join(log)
        recorded = (state_dir() / f"{lane}.env").exists()
        refused = any(code in result.stderr for code in SPOT_REFUSALS)
        if recorded or attempt != "spot" or not refused:
            # An instance may exist (state retained for `down`), or the
            # failure has nothing to do with the spot market.
            return False, attempt, "\n".join(log)
    return False, markets[-1], "\n".join(log)


def cmd_up(args):
    fleet = check_name("fleet", args.fleet)
    region = os.environ.get("AWS_REGION")
    if not region:
        fail("set AWS_REGION explicitly for a fleet (prefer a region other than us-east-1)", 2)
    lanes = [f"{fleet}-{i}" for i in range(args.first, args.first + args.count)]
    todo = [lane for lane in lanes if not (state_dir() / f"{lane}.env").exists()]
    for lane in sorted(set(lanes) - set(todo)):
        print(f"{lane} already up", file=sys.stderr)
    if not todo:
        return 0
    check_market = "spot" if args.market != "on-demand" else "on-demand"
    if not precheck(region, args.type, len(todo), check_market, args.vcpu_limit):
        if args.market != "spot-fallback" or not precheck(region, args.type, len(todo), "on-demand",
                                                          args.vcpu_limit):
            fail("the fleet does not fit the vCPU quota; reduce COUNT or the instance size")
        print("precheck: spot quota short; launching on-demand", file=sys.stderr)
        args.market = "on-demand"
    results = parallel(todo, lambda lane: up_lane(lane, args.type, args.market))
    failed = 0
    for lane in todo:
        ok, market, log = results[lane]
        print(log)
        print(f"{lane}: {'up' if ok else 'FAILED'} ({market})")
        failed += 0 if ok else 1
    return 1 if failed else 0


def running_rows(fleet=None):
    result = subprocess.run(["bash", perf_host(), "list"], env=host_env(), capture_output=True, text=True)
    if result.returncode != 0:
        fail(f"list failed: {result.stderr.strip()}")
    rows = [line.split("\t") for line in result.stdout.splitlines() if line.strip()]
    if fleet:
        rows = [row for row in rows if re.match(rf"^{re.escape(fleet)}-[0-9]+$", row[0])]
    return rows


def cmd_list(args):
    if args.fleet:
        check_name("fleet", args.fleet)
    rows = running_rows(args.fleet)
    for row in rows:
        print("\t".join(row))
    listed = {row[0] for row in rows}
    lanes = fleet_lanes(args.fleet) if args.fleet else []
    for lane in lanes:
        if lane not in listed:
            print(f"{lane}\trecorded, no running instance (run: down {args.fleet})")
    return 0


def cmd_down(args):
    fleet = check_name("fleet", args.fleet)
    lanes = fleet_lanes(fleet)
    results = parallel(lanes, lambda lane: subprocess.run(
        ["bash", perf_host(), "down", lane], env=host_env(), capture_output=True, text=True))
    failed = 0
    for lane in lanes:
        result = results[lane]
        print((result.stdout + result.stderr).strip())
        failed += result.returncode != 0
    left = running_rows(fleet)
    for row in left:
        print("still running: " + "\t".join(row), file=sys.stderr)
    if failed or left or fleet_lanes(fleet):
        fail(f"fleet {fleet} is not fully down; retry down {fleet}")
    print(f"fleet {fleet} down: {len(lanes)} lane(s), none listed")
    return 0


def extend_lane(lane, minutes):
    return remote(lane, f"sudo shutdown -c 2>/dev/null; sudo shutdown -h +{int(minutes)} 2>&1",
                  check=False, timeout=60).returncode == 0


def cmd_extend(args):
    lanes = fleet_lanes(check_name("fleet", args.fleet))
    results = parallel(lanes, lambda lane: extend_lane(lane, args.minutes))
    for lane in lanes:
        print(f"{lane}: {'extended to +' + str(args.minutes) + ' min' if results[lane] else 'unreachable'}")
    return 0 if all(results.values()) else 1


def cmd_helpers(args):
    lane = check_name("lane", args.lane)
    info = read_lane(lane)
    directory = state_dir() / f"{lane}.bin"
    directory.mkdir(mode=0o700, exist_ok=True)
    options = " ".join(shlex.quote(option) for option in ssh_options(lane))
    host = f"ubuntu@{info.get('addr', '')}"
    scripts = {
        "ssh": f'#!/bin/sh\n# ssh to lane {lane}\nexec ssh {options} {host} "$@"\n',
        "scp": f'#!/bin/sh\n# scp with lane {lane} options; the host is {host}\nexec scp {options} "$@"\n',
        "host": f"{host}\n",
    }
    for name, text in scripts.items():
        path = directory / name
        path.write_text(text)
        path.chmod(0o755 if name != "host" else 0o644)
    print(directory)
    return 0


# ---------------------------------------------------------------------------
# One build everywhere.

SHIP_SH = r'''set -eu
name=$1 sha=$2
build="$HOME/fleet/build"; mkdir -p "$build/.incoming"
tmp=$(mktemp "$build/.incoming/XXXXXX")
cat >"$tmp"
got=$(sha256sum "$tmp" | cut -d' ' -f1)
[ "$got" = "$sha" ] || { echo "archive sha256 $got, expected $sha" >&2; rm -f "$tmp"; exit 1; }
rm -rf "$build/$name.new"; mkdir "$build/$name.new"
tar -xf "$tmp" -C "$build/$name.new"; rm -f "$tmp"
(cd "$build/$name.new" && sha256sum --quiet -c .sail-fleet-manifest)
rm -rf "$build/$name"; mv "$build/$name.new" "$build/$name"
echo "$sha" >"$build/$name.sha256"
echo "verified $sha"
'''


def build_archive(paths):
    """Tar PATHS (each under its own basename) with a per-file sha256 manifest."""
    manifest, buffer = [], io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for source in paths:
            source = Path(source)
            if not source.exists():
                fail(f"no such path: {source}")
            for path in sorted([source] + (sorted(source.rglob("*")) if source.is_dir() else [])):
                arcname = str(Path(source.name) / path.relative_to(source)) if path != source else source.name
                info = archive.gettarinfo(str(path), arcname)
                info.mtime, info.uid, info.gid, info.uname, info.gname = 0, 0, 0, "", ""
                if info.isfile():
                    data = path.read_bytes()
                    manifest.append(f"{hashlib.sha256(data).hexdigest()}  {arcname}")
                    archive.addfile(info, io.BytesIO(data))
                elif info.isdir() or info.issym():
                    archive.addfile(info)
        text = ("\n".join(manifest) + "\n").encode()
        info = tarfile.TarInfo(".sail-fleet-manifest")
        info.size = len(text)
        archive.addfile(info, io.BytesIO(text))
    return buffer.getvalue(), len(manifest)


def cmd_ship(args):
    fleet = check_name("fleet", args.fleet)
    name = check_name("build name", args.name or Path(args.paths[0]).name.lower().replace("_", "-").replace(".", "-"))
    data, files = build_archive(args.paths)
    sha = hashlib.sha256(data).hexdigest()
    lanes = fleet_lanes(fleet)
    if not lanes:
        fail(f"fleet {fleet} has no lanes")
    print(f"ship {name}: {files} files, {len(data)} bytes, sha256 {sha}")
    results = parallel(lanes, lambda lane: remote(lane, script_with_payload(SHIP_SH), name, sha,
                                                  data=data, check=False))
    bad = 0
    for lane in lanes:
        result = results[lane]
        line = (result.stdout + result.stderr).decode(errors="replace").strip()
        ok = result.returncode == 0 and f"verified {sha}" in line
        print(f"{lane}: {line if line else 'no output'}")
        bad += not ok
    return 1 if bad else 0


def script_with_payload(script):
    """Wrap SCRIPT so the bytes after it on stdin are its input, not script."""
    return "main() {\n" + script + "\n}\nmain \"$@\"; exit $?"


# ---------------------------------------------------------------------------
# Detached single jobs.

SUBMIT_SH = r'''set -eu
name=$1; shift
dir="$HOME/fleet/jobs/$name"
if [ -e "$dir" ]; then echo "job $name exists on this host" >&2; exit 3; fi
mkdir -p "$dir"
printf '%s' "$1" | base64 -d >"$dir/cmd"; shift
sudo systemd-run --unit="sail-fleet-job-$name" --collect --quiet \
  /bin/bash "$HOME/fleet/lib/job.sh" "$dir" "$@"
echo "submitted $name"
'''

STATUS_SH = r'''set -u
cd "$HOME/fleet/jobs" 2>/dev/null || exit 0
for name in "$@"; do
  if [ ! -d "$name" ]; then echo "$name absent"; continue; fi
  if [ -f "$name/DONE" ]; then
    echo "$name done $(cat "$name/rc" 2>/dev/null || echo none) $(cat "$name/result" 2>/dev/null || echo unknown)"
  elif systemctl is-active --quiet "sail-fleet-job-$name"; then echo "$name running"
  else echo "$name died"
  fi
done
'''


def classify(rc, result):
    """Map a job's exit status and unit result to a reported status. Only an
    exit status of 0 with a successful unit is ever `ok`."""
    if result == "oom-kill":
        return "oom"
    if rc == "125" and result in ("steal", "busy", "not-quiet"):
        return "refused"
    if rc == "124":
        return "timeout"
    if rc == "137":
        return "killed"
    if rc == "0" and result in ("success", "unknown"):
        return "ok"
    return "failed"


def job_options(parser):
    parser.add_argument("--mem", default="12G", help="MemoryMax for the job's unit (default 12G)")
    parser.add_argument("--timeout", default="3600", help="timeout(1) duration (default 3600 s)")
    parser.add_argument("--grace", default="30", help="timeout --kill-after (default 30 s)")
    parser.add_argument("--quiet-pool", action="store_true", help="one at a time on a quiet host")
    parser.add_argument("--cpus", default="0-15", help="CPUs for --quiet-pool jobs (default 0-15)")


def job_args(args):
    return [args.mem, args.timeout, args.grace, "1" if args.quiet_pool else "0",
            args.cpus if args.quiet_pool else ""]


def cmd_submit(args):
    lane = check_name("lane", args.lane)
    if not ITEM.match(args.name):
        fail("job name must match [A-Za-z0-9][A-Za-z0-9._-]*", 2)
    command = args.command
    if not command:
        fail("give the command after --: submit LANE NAME [options] -- CMD...", 2)
    text = "#!/bin/bash\n" + " ".join(shlex.quote(word) for word in command) + "\n"
    install_lib(lane)
    result = remote(lane, SUBMIT_SH, args.name, base64.b64encode(text.encode()).decode(), *job_args(args))
    print(result.stdout.decode().strip())
    return 0


def job_status_lines(lane, names):
    result = remote(lane, STATUS_SH, *names, check=False, timeout=60)
    if result.returncode != 0:
        return [f"{name} unreachable" for name in names]
    return result.stdout.decode().splitlines()


def cmd_status(args):
    lane = check_name("lane", args.lane)
    names = args.names
    if not names:
        listing = remote(lane, 'ls "$HOME/fleet/jobs" 2>/dev/null || true', check=False, timeout=60)
        names = listing.stdout.decode().split()
    for line in job_status_lines(lane, names) if names else []:
        parts = line.split()
        if len(parts) >= 4 and parts[1] == "done":
            print(f"{parts[0]}\t{classify(parts[2], parts[3])}\trc={parts[2]}\tresult={parts[3]}")
        elif len(parts) >= 2:
            print(f"{parts[0]}\t{parts[1]}")
    return 0


def cmd_fetch(args):
    lane = check_name("lane", args.lane)
    if not ITEM.match(args.name):
        fail("job name must match [A-Za-z0-9][A-Za-z0-9._-]*", 2)
    result = remote(lane, 'cd "$HOME/fleet/jobs" && tar -cf - -- "$1"', args.name, check=False)
    if result.returncode != 0:
        fail(f"{lane}: cannot read job {args.name}: {result.stderr.decode(errors='replace').strip()}")
    dest = Path(args.dest)
    dest.mkdir(parents=True, exist_ok=True)
    safe_extract(result.stdout, dest)
    done = (dest / args.name / "DONE").exists()
    print(f"fetched {args.name} to {dest / args.name}" + ("" if done else " (no DONE marker: job unfinished or died)"))
    return 0


def safe_extract(data, dest):
    with tarfile.open(fileobj=io.BytesIO(data)) as archive:
        for member in archive.getmembers():
            target = (dest / member.name).resolve()
            if not (member.isfile() or member.isdir()) or not str(target).startswith(str(dest.resolve())):
                continue
            archive.extract(member, dest)


# ---------------------------------------------------------------------------
# Resumable fleet runs.

def item_key(item_id, command):
    """Idempotent result key: the item's id plus a digest of its command, so an
    edited command never reuses an old result."""
    return f"{item_id}.{hashlib.sha256(command.encode()).hexdigest()[:12]}"


def load_jobs(path):
    items, seen = [], set()
    for number, line in enumerate(Path(path).read_text().splitlines(), 1):
        if not line.strip():
            continue
        row = json.loads(line)
        if not isinstance(row, dict) or not isinstance(row.get("id"), str) or not isinstance(row.get("cmd"), str):
            fail(f"{path}:{number}: each line needs string id and cmd", 2)
        if not ITEM.match(row["id"]) or row["id"] in seen:
            fail(f"{path}:{number}: id must be unique and match [A-Za-z0-9][A-Za-z0-9._-]*", 2)
        seen.add(row["id"])
        items.append({"id": row["id"], "cmd": row["cmd"], "key": item_key(row["id"], row["cmd"]),
                      "est": row.get("est")})
    return items


def shard(items, lanes, estimates):
    """Longest processing time first: each item, longest estimate first, goes
    to the lane with the least estimated work so far."""
    load = {lane: 0.0 for lane in lanes}
    shards = {lane: [] for lane in lanes}
    for item in sorted(items, key=lambda item: (-estimates.get(item["id"], 1.0), item["id"])):
        lane = min(lanes, key=lambda lane: (load[lane], lanes.index(lane)))
        shards[lane].append(item)
        load[lane] += estimates.get(item["id"], 1.0)
    return shards


def wall_seconds(time_text):
    match = re.search(r"Elapsed \(wall clock\) time \(h:mm:ss or m:ss\): ([0-9:.]+)", time_text or "")
    if not match:
        return None
    seconds = 0.0
    for part in match.group(1).split(":"):
        seconds = seconds * 60 + float(part)
    return seconds


def max_rss_kib(time_text):
    match = re.search(r"Maximum resident set size \(kbytes\): ([0-9]+)", time_text or "")
    return int(match.group(1)) if match else None


POLL_SH = r'''set -u
run="$HOME/fleet/runs/$1"
[ -d "$run" ] || { echo "NORUN"; exit 0; }
cd "$run"
echo "SETUP $(cat setup/rc 2>/dev/null || { [ -f setup/cmd ] && echo pending || echo none; })"
for keys in shards/*.keys; do
  [ -f "$keys" ] || continue
  round=$(basename "$keys" .keys)
  [ -f "shards/$round.DONE" ] && continue
  if systemctl is-active --quiet "sail-fleet-run-$1-$round"; then
    echo "ACTIVE $round $(tr '\n' ' ' <"$keys")"
  else
    echo "STOPPED $round"
  fi
done
for d in items/*/; do
  [ ! -f "$d/DONE" ] || echo "DONE $(basename "$d")"
done
# Only a failed connection, never an unfinished item, may end this nonzero.
exit 0
'''

COLLECT_SH = r'''set -eu
cd "$HOME/fleet/runs/$1/items"; shift
tar -cf - -- "$@"
'''

ROUND_SH = r'''set -eu
name=$1 slots=$2 setup_timeout=$3; shift 3
run="$HOME/fleet/runs/$name"
mkdir -p "$run/shards" "$run/items"
cd "$run"
tmp=$(mktemp -d)
cat | tar -xf - -C "$tmp"
if [ -f "$tmp/setup/cmd" ] && [ ! -f setup/cmd ]; then mkdir -p setup; cp "$tmp/setup/cmd" setup/cmd; fi
for d in "$tmp"/items/*/; do
  key=$(basename "$d"); mkdir -p "items/$key"
  [ -f "items/$key/DONE" ] || cp "$d/cmd" "items/$key/cmd"
done
round=r$(( $(ls shards/*.keys 2>/dev/null | wc -l) + 1 ))
cp "$tmp/keys" "shards/$round.keys"; rm -rf "$tmp"
sudo systemd-run --unit="sail-fleet-run-$name-$round" --collect --quiet \
  /bin/bash "$HOME/fleet/lib/runner.sh" "$run" "$round" "$slots" "$setup_timeout" "$@"
echo "ROUND $round"
'''


class Coordinator:
    """Drives one run. All durable state is the local results directory plus
    what each live host reports, so killing and restarting it is safe."""

    def __init__(self, args):
        self.args = args
        self.fleet = check_name("fleet", args.fleet)
        self.run_name = check_name("run name", args.run or self.fleet)
        self.results = Path(args.results)
        self.items = load_jobs(args.jobs)
        self.by_key = {item["key"]: item for item in self.items}
        (self.results / "items").mkdir(parents=True, exist_ok=True)
        self.events = self.results / "events.jsonl"
        self.lost = set()
        self.last_active = {}
        self.unreachable_since = {}
        self.attempts = self.read_json("attempts.json", {})
        self.setup_failed = set()
        self.estimates = self.prior_estimates()
        self.installed = set()
        meta = {"run": self.run_name, "fleet": self.fleet, "items": len(self.items)}
        old = self.read_json("run.json", None)
        if old and old.get("run") != self.run_name:
            fail(f"{self.results} belongs to run {old.get('run')}, not {self.run_name}")
        (self.results / "run.json").write_text(json.dumps(meta, indent=1) + "\n")

    def read_json(self, name, default):
        path = self.results / name
        return json.loads(path.read_text()) if path.exists() else default

    def event(self, kind, **fields):
        row = {"t": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "event": kind, **fields}
        with self.events.open("a") as handle:
            handle.write(json.dumps(row) + "\n")
        print(json.dumps(row), flush=True)

    def prior_estimates(self):
        estimates = {}
        if self.args.times:
            estimates.update(json.loads(Path(self.args.times).read_text()))
        for item in self.items:
            if item.get("est") is not None:
                estimates.setdefault(item["id"], float(item["est"]))
            seconds = wall_seconds(self.read_text(item["key"], "time"))
            if seconds is not None:
                estimates.setdefault(item["id"], seconds)
        return estimates

    def read_text(self, key, name):
        path = self.results / "items" / key / name
        return path.read_text() if path.exists() else None

    def done_local(self):
        return {key for key in self.by_key if (self.results / "items" / key / "DONE").exists()}

    def given_up(self):
        return {key for key, count in self.attempts.items() if count > self.args.retries and key in self.by_key}

    def probe(self, lane):
        result = remote(lane, POLL_SH, self.run_name, check=False, timeout=60)
        if result.returncode == 0:
            self.unreachable_since.pop(lane, None)
            return result.stdout.decode().splitlines()
        state = instance_state(lane)
        first = self.unreachable_since.setdefault(lane, time.monotonic())
        if state in ("shutting-down", "terminated", "stopping", "stopped", "absent") or \
                time.monotonic() - first > self.args.dead_after:
            self.lost.add(lane)
            self.event("lane-lost", lane=lane, state=state)
        return None

    def collect(self, lane, keys):
        """Copy finished items off LANE; a key already held locally from
        another lane or start time is recorded as a duplicate."""
        new = []
        for key in keys:
            if key not in self.by_key:
                continue
            if (self.results / "items" / key / "DONE").exists():
                continue
            new.append(key)
        for start in range(0, len(new), 200):
            batch = new[start:start + 200]
            result = remote(lane, COLLECT_SH, self.run_name, *batch, check=False, timeout=600)
            if result.returncode != 0:
                return
            (self.results / ".incoming").mkdir(exist_ok=True)
            with tempfile.TemporaryDirectory(dir=self.results / ".incoming") as tmp:
                safe_extract(result.stdout, Path(tmp))
                for key in batch:
                    source = Path(tmp) / key
                    if not (source / "DONE").exists():
                        continue
                    (source / "lane").write_text(lane + "\n")
                    (source / "instance").write_text(read_lane(lane).get("id", "") + "\n")
                    target = self.results / "items" / key
                    if target.exists():
                        self.duplicate(key, target, source, lane)
                        continue
                    os.rename(source, target)
                    rc, unit = (self.read_text(key, "rc") or "").strip(), (self.read_text(key, "result") or "").strip()
                    self.event("collected", lane=lane, id=self.by_key[key]["id"], status=classify(rc, unit),
                               seconds=wall_seconds(self.read_text(key, "time")))

    def duplicate(self, key, target, source, lane):
        same = ((target / "lane").read_text().strip() == lane and
                (target / "started").read_text() == (source / "started").read_text())
        if not same:
            self.event("duplicate", id=self.by_key[key]["id"], lane=lane)
            with (self.results / "duplicates.txt").open("a") as handle:
                handle.write(f"{key}\t{lane}\n")

    def submit(self, lane, items):
        if lane not in self.installed:
            install_lib(lane)
            self.installed.add(lane)
        buffer = io.BytesIO()
        with tarfile.open(fileobj=buffer, mode="w") as archive:
            def add(name, text):
                data = text.encode()
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
            add("keys", "".join(item["key"] + "\n" for item in items))
            for item in items:
                add(f"items/{item['key']}/cmd", "#!/bin/bash\n" + item["cmd"] + "\n")
            if self.args.setup:
                add("setup/cmd", "#!/bin/bash\n" + self.args.setup + "\n")
        slots = "1" if self.args.quiet_pool else str(self.args.slots)
        result = remote(lane, script_with_payload(ROUND_SH), self.run_name, slots, self.args.setup_timeout,
                        *job_args(self.args), data=buffer.getvalue(), check=False)
        if result.returncode != 0:
            self.event("submit-failed", lane=lane, error=result.stderr.decode(errors="replace").strip()[-400:])
            return False
        round_name = result.stdout.decode().split()[-1]
        for item in items:
            self.attempts[item["key"]] = self.attempts.get(item["key"], 0) + 1
        (self.results / "attempts.json").write_text(json.dumps(self.attempts, indent=1, sort_keys=True) + "\n")
        self.event("submitted", lane=lane, round=round_name, items=len(items),
                   ids=[item["id"] for item in items])
        return True

    def heartbeat(self, lanes):
        for lane in lanes:
            extend_lane(lane, self.args.heartbeat)

    def run(self):
        lanes = fleet_lanes(self.fleet)
        if not lanes:
            fail(f"fleet {self.fleet} has no lanes")
        done = self.done_local()
        self.event("start", run=self.run_name, items=len(self.items), done=len(done), lanes=lanes)
        last_beat = 0.0
        while True:
            live, active, busy = [], set(), set()
            for lane in lanes:
                if lane in self.lost or lane in self.setup_failed:
                    continue
                lines = self.probe(lane)
                if lines is None:
                    # Until an unreachable lane counts as lost, its items stay
                    # assigned to it so they are never run twice.
                    if lane not in self.lost:
                        active.update(self.last_active.get(lane, ()))
                        busy.add(lane)
                    continue
                live.append(lane)
                lane_active = set()
                finished = []
                for line in lines:
                    parts = line.split()
                    if parts[:1] == ["DONE"]:
                        finished.append(parts[1])
                    elif parts[:1] == ["ACTIVE"]:
                        busy.add(lane)
                        lane_active.update(parts[2:])
                    elif parts[:1] == ["SETUP"] and parts[1] not in ("0", "none", "pending"):
                        self.setup_failed.add(lane)
                        self.event("setup-failed", lane=lane, rc=parts[1])
                self.collect(lane, finished)
                self.last_active[lane] = lane_active
                active |= lane_active
            if time.monotonic() - last_beat > 300 and self.args.heartbeat:
                self.heartbeat(live)
                last_beat = time.monotonic()
            done = self.done_local()
            given_up = self.given_up() - done
            pending = [item for item in self.items
                       if item["key"] not in done and item["key"] not in active and item["key"] not in given_up]
            if not pending and not (active - done):
                break
            live = [lane for lane in live if lane not in self.setup_failed]
            if all(lane in self.lost or lane in self.setup_failed for lane in lanes):
                self.event("no-live-lanes", unfinished=len(self.items) - len(done))
                break
            idle = [lane for lane in live if lane not in busy]
            if pending and idle:
                shards = shard(pending, idle, self.estimates)
                for lane in idle:
                    if shards[lane]:
                        self.submit(lane, shards[lane])
            time.sleep(self.args.poll)
        return self.summary()

    def summary(self):
        statuses, per_lane = {}, {}
        for item in self.items:
            key = item["key"]
            if (self.results / "items" / key / "DONE").exists():
                status = classify((self.read_text(key, "rc") or "").strip(), (self.read_text(key, "result") or "").strip())
                lane = (self.read_text(key, "lane") or "").strip()
                per_lane[lane] = per_lane.get(lane, 0) + 1
            elif key in self.given_up():
                status = "died"
            else:
                status = "unfinished"
            statuses[status] = statuses.get(status, 0) + 1
        duplicates = (self.results / "duplicates.txt")
        times = {item["id"]: wall_seconds(self.read_text(item["key"], "time")) for item in self.items}
        (self.results / "times.json").write_text(
            json.dumps({k: v for k, v in times.items() if v is not None}, indent=1, sort_keys=True) + "\n")
        summary = {"run": self.run_name, "items": len(self.items), "statuses": statuses, "per_lane": per_lane,
                   "lost_lanes": sorted(self.lost), "setup_failed": sorted(self.setup_failed),
                   "attempts_over_one": sum(1 for count in self.attempts.values() if count > 1),
                   "duplicates": len(duplicates.read_text().splitlines()) if duplicates.exists() else 0}
        (self.results / "summary.json").write_text(json.dumps(summary, indent=1, sort_keys=True) + "\n")
        print(json.dumps(summary, sort_keys=True))
        unfinished = statuses.get("unfinished", 0) + statuses.get("died", 0)
        return 1 if unfinished or summary["duplicates"] else 0


def cmd_run(args):
    return Coordinator(args).run()


def cmd_quiet(args):
    lane = check_name("lane", args.lane)
    script = quiet_script(args.freq_khz)
    if args.dry_run:
        print(script)
        return 0
    info = read_lane(lane)
    if ".metal" not in info.get("type", "") and not args.allow_virtual:
        fail(f"{lane} is {info.get('type')}: the quiet pool needs a bare-metal type such as c7i.metal-24xl")
    result = remote(lane, "sudo bash -s <<'SAIL_QUIET'\n" + script + "\nSAIL_QUIET\n", check=False)
    print((result.stdout + result.stderr).decode(errors="replace").strip())
    return result.returncode


def cmd_precheck(args):
    region = os.environ.get("AWS_REGION")
    if not region:
        fail("set AWS_REGION", 2)
    return 0 if precheck(region, args.type, args.count, args.market, args.vcpu_limit) else 1


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0],
                                     formatter_class=argparse.RawDescriptionHelpFormatter, epilog=__doc__)
    sub = parser.add_subparsers(dest="sub", required=True)
    p = sub.add_parser("up")
    p.add_argument("fleet")
    p.add_argument("count", type=int)
    p.add_argument("--type", default="c7i.4xlarge")
    p.add_argument("--market", choices=["spot-fallback", "spot", "on-demand"], default="spot-fallback")
    p.add_argument("--first", type=int, default=1)
    p.add_argument("--vcpu-limit", type=int)
    p.set_defaults(func=cmd_up)
    p = sub.add_parser("list")
    p.add_argument("fleet", nargs="?")
    p.set_defaults(func=cmd_list)
    p = sub.add_parser("down")
    p.add_argument("fleet")
    p.set_defaults(func=cmd_down)
    p = sub.add_parser("extend")
    p.add_argument("fleet")
    p.add_argument("minutes", type=int)
    p.set_defaults(func=cmd_extend)
    p = sub.add_parser("helpers")
    p.add_argument("lane")
    p.set_defaults(func=cmd_helpers)
    p = sub.add_parser("ship")
    p.add_argument("fleet")
    p.add_argument("paths", nargs="+")
    p.add_argument("--name")
    p.set_defaults(func=cmd_ship)
    p = sub.add_parser("submit")
    p.add_argument("lane")
    p.add_argument("name")
    job_options(p)
    p.set_defaults(func=cmd_submit)
    p = sub.add_parser("status")
    p.add_argument("lane")
    p.add_argument("names", nargs="*")
    p.set_defaults(func=cmd_status)
    p = sub.add_parser("fetch")
    p.add_argument("lane")
    p.add_argument("name")
    p.add_argument("dest")
    p.set_defaults(func=cmd_fetch)
    p = sub.add_parser("run")
    p.add_argument("fleet")
    p.add_argument("jobs")
    p.add_argument("--results", required=True)
    p.add_argument("--run", help="run name (default: the fleet name)")
    p.add_argument("--setup", help="command each lane runs once before its items")
    p.add_argument("--setup-timeout", default="3600")
    p.add_argument("--slots", type=int, default=1, help="items at once per lane (default 1)")
    p.add_argument("--times", help="JSON {id: seconds} from an earlier run, for sharding")
    p.add_argument("--poll", type=float, default=20.0)
    p.add_argument("--heartbeat", type=int, default=120, help="keep lanes alive this many minutes past "
                                                             "the last poll (0 disables)")
    p.add_argument("--retries", type=int, default=2, help="re-runs of an item that died without output")
    p.add_argument("--dead-after", type=float, default=600.0,
                   help="seconds a running but unreachable lane is waited for before it counts as lost")
    job_options(p)
    p.set_defaults(func=cmd_run)
    p = sub.add_parser("quiet")
    p.add_argument("lane")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--freq-khz", type=int, default=2400000)
    p.add_argument("--allow-virtual", action="store_true")
    p.set_defaults(func=cmd_quiet)
    p = sub.add_parser("precheck")
    p.add_argument("type")
    p.add_argument("count", type=int)
    p.add_argument("--market", choices=["spot", "on-demand"], default="on-demand")
    p.add_argument("--vcpu-limit", type=int)
    p.set_defaults(func=cmd_precheck)
    argv = list(sys.argv[1:] if argv is None else argv)
    command = []
    if "--" in argv:
        split = argv.index("--")
        argv, command = argv[:split], argv[split + 1:]
    args = parser.parse_args(argv)
    args.command = command
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
