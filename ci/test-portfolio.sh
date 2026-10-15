#!/usr/bin/env bash
# Small real-Verus complete-run portfolio, cold then warm. No long corpus runs.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${SAIL_TEST_Z3:?SAIL_TEST_Z3 must name the pinned solver}"
sail_portfolio_root="$PWD"
sail_portfolio_work="$(mktemp -d)"
sail_portfolio_daemon=""
cleanup() {
  if [ -n "$sail_portfolio_daemon" ]; then
    kill "$sail_portfolio_daemon" 2>/dev/null || true
    wait "$sail_portfolio_daemon" 2>/dev/null || true
  fi
  rm -rf "$sail_portfolio_work"
}
trap cleanup EXIT
export SAIL_CACHE_DIR="$sail_portfolio_work/cache"
export SAIL_DAEMON_SOCKET="$sail_portfolio_work/daemon.sock"
export VERUS_Z3_PATH="$SAIL_TEST_Z3"
"$sail_portfolio_root/target/release/sail" serve --socket "$SAIL_DAEMON_SOCKET" \
  --cache "$SAIL_CACHE_DIR" --idle-timeout 0 >"$sail_portfolio_work/daemon.log" 2>&1 &
sail_portfolio_daemon=$!
bash "$sail_portfolio_root/ci/wait-daemon.sh" "$SAIL_DAEMON_SOCKET" "$sail_portfolio_daemon" || {
  cat "$sail_portfolio_work/daemon.log" >&2
  exit 1
}
for sail_portfolio_round in cold warm; do
  "$sail_portfolio_root/target/release/sail" verify \
    --portfolio accept-first-proof --portfolio-seeds 4 --portfolio-timeout-secs 60 \
    --output json --verus-source "$sail_portfolio_root/corpus/repros/z3-nla-seed-portfolio/input.rs" -- \
    --crate-type lib --rlimit 0.01 --no-report-long-running \
    >"$sail_portfolio_work/$sail_portfolio_round.json"
done
python3 - "$sail_portfolio_work" <<'PY'
import hashlib
import json
import os
from pathlib import Path
import sys

root = Path(sys.argv[1])
solver_hash = hashlib.sha256(Path(os.environ['SAIL_TEST_Z3']).read_bytes()).hexdigest()
for label in ('cold', 'warm'):
    report = json.loads((root / f'{label}.json').read_text())
    assert report['exit_code'] == 0 and report['error'] is None, report
    assert report['mode'] == 'accept-first-proof', report
    assert report['solver']['sha256'] == solver_hash, report
    portfolio = report['portfolio']
    assert portfolio['seeds'] == [0, 4] and portfolio['winner_seed'] == 4, portfolio
    attempts = portfolio['attempts']
    assert [a['configuration']['seed'] for a in attempts] == [0, 4], attempts
    assert [a['verified'] for a in attempts] == [False, True], attempts
    assert [a['exit_code'] for a in attempts] == [1, 0], attempts
    assert all(a['error'] is None for a in attempts), attempts
    assert all(a['report']['solver']['sha256'] == solver_hash for a in attempts), attempts
    assert attempts[0]['report']['summary']['unknown'] == 1, attempts[0]
    baseline_functions = [f for c in attempts[0]['report']['crates'] for f in c['functions']]
    assert sum(f['provenance']['checks_solved'] for f in baseline_functions) > 0, baseline_functions
    assert all(c['verus']['errors'] == 0 for c in report['crates']), report
    assert report['summary']['functions'] == report['summary']['verified'] == 1, report
    assert report['summary']['failed'] == report['summary']['unknown'] == report['summary']['provisional'] == 0
    functions = [f for c in report['crates'] for f in c['functions']]
    assert len(functions) == 1 and functions[0]['function'] == 'input::square', functions
    function = functions[0]
    assert function['verdict'] == 'verified' and function['establishment'] == 'confirmed' and function['evidence_tier'] == 'smt_verified', function
    provenance = function['provenance']
    assert provenance['mode'] == 'accept-first-proof' and provenance['configuration']['seed'] == 4, provenance
    if label == 'warm':
        assert provenance['checks_served'] > 0, provenance
    else:
        assert provenance['checks_solved'] > 0, provenance
    print(f'portfolio {label}: baseline refused; one complete proof from seed 4')
PY
