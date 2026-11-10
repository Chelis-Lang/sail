. ~/.cargo/env
REL=~/.local/opt/sail-0.2.0-x86_64-unknown-linux-gnu
MAIN=~/opt/sail-main
if [ ! -d $MAIN ]; then mkdir -p ~/opt; cp -a $REL $MAIN; cp ~/sail/target/release/sail ~/sail/target/release/sail-z3 $MAIN/bin/; fi
STOCK=~/.cache/sail/verus/0.2026.10.18.7d82994/verus-x86-linux
export FORKDIR=$(dirname $(find $MAIN/engine -name cargo-verus -type f | head -1))
export PATH=$MAIN/bin:$HOME/sail/target/release:$STOCK:$PATH
export SAIL_PERF_MACHINE='c7i.8xlarge, us-east-1, Ubuntu 24.04'
export SAIL_PERF_VSTD=registry SAIL_PERF_NOOPS=0 SAIL_PERF_EDITS=none
