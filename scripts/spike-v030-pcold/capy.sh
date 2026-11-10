#!/bin/bash
# CapybaraKV shipped-vstd refusal with the published 0.2.0 archive, IronKV as
# the served control, then the same pair with main's binaries over that layout.
set -x
. ~/env.sh
cd ~/sail
export RUSTUP_TOOLCHAIN=1.99.1 SAIL_PERF_VERUS_TIMES=0 SAIL_PERF_DIAG=1 SAIL_PERF_MODES="sail"
(
  export PATH=$REL/bin:$PATH
  which sail; sail --version
  SAIL_PERF_OUT=~/sp/capy-rel scripts/perf-real.sh run capybarakv 1
  SAIL_PERF_OUT=~/sp/iron-rel scripts/perf-real.sh run ironkv 1
)
which sail; sail --version
SAIL_PERF_OUT=~/sp/capy-main scripts/perf-real.sh run capybarakv 1
SAIL_PERF_OUT=~/sp/iron-main scripts/perf-real.sh run ironkv 1
echo CAPY-DONE
