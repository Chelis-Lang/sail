#!/usr/bin/env bash
# Host: retention wall overhead. bench.sh PROJECT PKG NOOP_FILE ROUNDS
# Arms, interleaved per round, every run with an empty cache and a fresh daemon:
#   cached         sail-z3 + daemon (the product's solver path), nothing retained
#   cached-retain  the same with SAIL_SESSION_RETAIN_DIR (prototype: every session, raw)
#   pass           sail-z3 pass-through, no daemon, no tap
#   pass-tap       the same with SAIL_TRACE_DIR (the existing tap: input, output, jsonl)
set -uo pipefail
. ~/.cargo/env
name=$1 pkg=$2 noop=$3 rounds=${4:-5}
S=~/spike; sail_root=~/sail
V=$(echo ~/.cache/sail/verus/*/verus-x86-linux)
export PATH=$sail_root/target/release:$V:$PATH
B=$S/bench-$name; rm -rf $B; mkdir -p $B
out=$B/results.tsv
printf 'project\tround\tarm\twall_s\texit\tretained_files\tretained_bytes\tverified_line\n' >$out
arms="cached cached-retain pass pass-tap"
run() { # arm dir tag -> wall exit
  local arm=$1 dir=$2 tag=$3 cache=$B/c ret=$B/ret
  rm -rf $cache $ret; mkdir -p $cache $ret
  local envs=(VERUS_Z3_PATH=$(command -v sail-z3) SAIL_Z3_PATH=$V/z3)
  local pid=
  case $arm in
    cached*)
      export SAIL_CACHE_DIR=$cache SAIL_DAEMON_SOCKET=$cache/d.sock
      sail serve --socket $SAIL_DAEMON_SOCKET >$B/daemon.log 2>&1 & pid=$!
      for _ in $(seq 100); do sail status --json 2>/dev/null | jq -e .running >/dev/null && break; sleep 0.05; done
      envs+=(SAIL_CACHE_DIR=$cache SAIL_DAEMON_SOCKET=$SAIL_DAEMON_SOCKET) ;;
  esac
  case $arm in
    cached-retain) envs+=(SAIL_SESSION_RETAIN_DIR=$ret) ;;
    pass-tap) envs+=(SAIL_TRACE_DIR=$ret) ;;
  esac
  echo "// $tag $(date +%s%N)" >>$dir/$noop
  sync
  local t0 t1 code
  t0=$(date +%s.%N)
  ( cd $dir/$pkg && env -u SAIL_CACHE_DIR -u SAIL_DAEMON_SOCKET "${envs[@]}" cargo verus verify >$B/$tag.log 2>&1 ); code=$?
  t1=$(date +%s.%N)
  local stats=""
  if [ -n "$pid" ]; then stats=$(sail status --json 2>/dev/null | jq -c '{hits:(.hits//.cache.hits//null),puts:(.puts//.cache.puts//null)}' 2>/dev/null); kill $pid 2>/dev/null; wait $pid 2>/dev/null; fi
  unset SAIL_CACHE_DIR SAIL_DAEMON_SOCKET
  local files bytes
  files=$(find $ret -type f | wc -l); bytes=$(du -sb $ret | cut -f1)
  printf '%s\t%s\t%s\t%.2f\t%s\t%s\t%s\t%s %s\n' "$name" "${tag%%-*}" "$arm" "$(echo "$t1 - $t0" | bc)" "$code" "$files" "$bytes" \
    "$(grep -h 'verification results' $B/$tag.log | tail -1 | tr '\t' ' ')" "$stats" >>$out
}
for arm in $arms; do
  rm -rf $B/p-$arm; cp -r $S/pristine-$name $B/p-$arm
  # Untimed: dependencies and vstd, then one full run of the arm.
  ( cd $B/p-$arm/$pkg && cargo verus verify >$B/warm-$arm.log 2>&1 ) || echo "warm $arm exit $?"
done
for r in $(seq $rounds); do
  order=$arms; [ $((r % 2)) = 0 ] && order="pass-tap pass cached-retain cached"
  for arm in $order; do run $arm $B/p-$arm r$r-$arm; done
done
cat $out
echo BENCH-DONE
