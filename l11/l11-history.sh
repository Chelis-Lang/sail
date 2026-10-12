#!/usr/bin/env bash
# Temporary L11 history-experiment driver (not committed).
cd ~/sail
Z3=~/.cache/sail/verus/0.2026.10.04.426d8b0/verus-x86-linux/z3
target/release/sail-bench history ~/l11/edit/r0/*/*/fresh --z3 "$Z3" --jobs 15 \
  --all-checks --json ~/l11/history-edit.json > ~/l11/history-edit.txt 2>&1
target/release/sail-bench history ~/l11/fast/r0/*/*/fresh --z3 "$Z3" --jobs 15 \
  --all-checks --json ~/l11/history-fast.json > ~/l11/history-fast.txt 2>&1
echo finished >> ~/l11/history-fast.txt
