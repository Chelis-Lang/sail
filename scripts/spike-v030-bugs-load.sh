#!/usr/bin/env bash
# SPIKE #696/#683: discriminator for #683, then the loaded sequential loops.
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$here"
# 1. What does the cache hold after an injected session loss? One fresh
#    project under injection, then a different fresh project without it.
for toggle in die-before-record connect-fail; do
  root="/tmp/k-$toggle"; rm -rf "$root"; mkdir -p "$root"
  rm -f /tmp/sail-spike/die-before-record /tmp/sail-spike/connect-fail
  bash scripts/spike-v030-bugs-env.sh "$root" start >/dev/null
  eval "$(bash scripts/spike-v030-bugs-env.sh "$root" env)"
  touch "/tmp/sail-spike/$toggle"
  echo "== discriminator $toggle: injected"
  python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode cargo --count 1 --work "$root/a"
  rm -f "/tmp/sail-spike/$toggle"
  echo "== discriminator $toggle: next fresh project, no injection"
  python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode cargo --count 1 --start 7 --work "$root/b"
  bash scripts/spike-v030-bugs-env.sh "$root" stop
done
echo DISCRIMINATOR-DONE
# 2. Loaded loops: three services, each fed in sequence, plus stress-ng.
rm -f /tmp/sail-spike/trace.jsonl /tmp/sail-spike/shim.jsonl
for root in /tmp/o /tmp/p /tmp/n; do
  rm -rf "$root"; mkdir -p "$root"
  bash scripts/spike-v030-bugs-env.sh "$root" start >/dev/null
done
nohup stress-ng --cpu 14 --vm 2 --vm-bytes 1G --timeout 0 >/tmp/stress.log 2>&1 &
( while true; do echo "$(date -u +%T) $(cut -d' ' -f1-3 /proc/loadavg)"; sleep 30; done ) >/tmp/loadavg.log &
sampler=$!
sleep 90
(
  eval "$(bash scripts/spike-v030-bugs-env.sh /tmp/o env)"
  python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode cargo --count 100 --work /tmp/o/work/c >/tmp/o/cargo.jsonl 2>/tmp/o/cargo.err
  python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode standalone --count 100 --work /tmp/o/work/s >/tmp/o/standalone.jsonl 2>/tmp/o/standalone.err
) &
(
  eval "$(bash scripts/spike-v030-bugs-env.sh /tmp/p env)"
  python3 scripts/spike-v030-bugs.py --sail "$SAIL" --mode cargo --count 100 --start 100 --work /tmp/p/work/c >/tmp/p/cargo.jsonl 2>/tmp/p/cargo.err
) &
(
  eval "$(bash scripts/spike-v030-bugs-env.sh /tmp/n env)"
  target/release/examples/spike_neutral_loop /tmp/n/d.sock 30 1 0 >/tmp/n/neutral1.jsonl 2>/tmp/n/neutral1.err
  target/release/examples/spike_neutral_loop /tmp/n/d.sock 30 3 1000 >/tmp/n/neutral3.jsonl 2>/tmp/n/neutral3.err
) &
wait %3 %4 %5 2>/dev/null || wait
kill "$sampler" 2>/dev/null
pkill stress-ng
echo LOAD-DONE
