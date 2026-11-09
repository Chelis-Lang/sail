#!/usr/bin/env bash
# Inside the CI image on a native host (see campaign.py), one arm per container:
#   run.sh PROJECT ARM [TAG]
# PROJECT: ironkv | capybarakv | vest
# ARM: select | naive | sscoped | a3 | camp | camp-unscoped | pipe
# TAG: `fix` uses the spike build in /mut/target-fix (one compiler family for
#      every focus selector); anything else is only a label.
set -euo pipefail
project="$1"; arm="$2"; tag="${3:-}"
py=scripts/spike-mutcost/campaign.py
list="/mut/$project-list.json"
if [ "$tag" = fix ]; then export SPIKE_SAIL=/mut/target-fix/release/sail; fi
name="$project-$arm${tag:+-$tag}"
case "$arm" in
  select)
    mkdir -p /mut/gen
    case "$project" in
      ironkv)
        ./target/release/xtask gen-mutants --project verified-ironkv --name ironkvx --seed 20261109 --per-class 40 --out /mut/gen
        cp scripts/spike-mutcost/hand-ironkv.toml scripts/spike-mutcost/hand-ironkv.meta.json /mut/gen/
        python3 $py select --want scripts/spike-mutcost/want-ironkv.json --out "$list" /mut/gen/ironkvx.toml /mut/gen/hand-ironkv.toml ;;
      capybarakv)
        ./target/release/sail-conformance fetch --tier standard --project verified-storage
        ./target/release/xtask gen-mutants --project verified-storage --unit verified-storage/capybaraKV/capybarakv --name capybarakvx --seed 20261109 --per-class 40 --out /mut/gen
        python3 $py select --per-class 5 --out "$list" /mut/gen/capybarakvx.toml ;;
      vest)
        ./target/release/sail-conformance fetch --tier standard --project vest
        ./target/release/xtask gen-mutants --project vest --name vestx --seed 20261109 --per-class 40 --out /mut/gen
        python3 $py select --per-class 5 --out "$list" /mut/gen/vestx.toml ;;
    esac ;;
  naive|sscoped|a3|camp|pipe)
    rm -rf "/mut/$name" "/mut/out/$name"
    python3 $py "$arm" --list "$list" --root "/mut/$name" --out "/mut/out/$name" ;;
  camp-unscoped)
    rm -rf "/mut/$name" "/mut/out/$name"
    python3 $py camp --strategy unscoped --list "$list" --root "/mut/$name" --out "/mut/out/$name" ;;
esac
