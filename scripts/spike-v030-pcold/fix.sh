#!/bin/bash
# Build the prototype and run CapybaraKV/IronKV with and without the
# members-only lookup, in the alternate-bundle research mode (the release
# index binds the published `sail`, so a rebuilt one cannot use release mode).
#   fix.sh TAG
set -x
. ~/env.sh
tag=$1
cd ~/sail
cargo build --release --locked -p sail-solve 2>&1 | tail -3
INST=~/opt/sail-inst-$tag
rm -rf $INST; cp -a $REL $INST; cp target/release/sail $INST/bin/sail
B=$(ls -d $REL/deps/*)
R="SAIL_VSTD_BOUNDARY_RESEARCH=1 SAIL_SHIPPED_DEPS=$B"
bash ~/keep.sh capy-$tag-off $INST/bin capy $R
bash ~/keep.sh capy-$tag-on $INST/bin capy $R SAIL_SPIKE_OWNED_LOOKUP=1
bash ~/keep.sh iron-$tag-on $INST/bin iron $R SAIL_SPIKE_OWNED_LOOKUP=1
grep WALL ~/sp/capy-$tag-*/log ~/sp/iron-$tag-*/log
echo FIX-DONE
