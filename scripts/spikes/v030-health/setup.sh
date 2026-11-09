#!/usr/bin/env bash
# Host: patched pristine copies of vest and ironkv, then fragility capture.
set -euo pipefail
. ~/.cargo/env
S=~/spike; sail_root=~/sail
V=$(echo ~/.cache/sail/verus/*/verus-x86-linux)
export PATH=$sail_root/target/release:$V:$PATH
corpus=~/.cache/sail/corpus; verus_src=$corpus/verus-7d829946231d
tc=$(sed -n 's/^rust_toolchain = "\(.*\)"/\1/p' $sail_root/corpus/toolchain.toml | head -1)
patcher=$S/patcher
if [ ! -x $patcher/target/release/sail-perf-patcher ]; then
  mkdir -p $patcher/src
  cat >$patcher/Cargo.toml <<EOF
[package]
name = "sail-perf-patcher"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
sail-verus = { path = "$sail_root/crates/sail-verus" }

[workspace]
EOF
  cat >$patcher/src/main.rs <<'EOF'
fn main() -> std::process::ExitCode {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [package, root, verus, toolchain] = a.as_slice() else { return std::process::ExitCode::from(2) };
    match sail_verus::patch::patch_verus_workspace(package.as_ref(), root.as_ref(), verus.as_ref(), Some(toolchain)) {
        Ok(p) => { for p in p { println!("patched {:?}: {}", p.manifest, p.crates.join(", ")); } std::process::ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); std::process::ExitCode::FAILURE }
    }
}
EOF
  (cd $sail_root && CARGO_TARGET_DIR=$patcher/target cargo build --release -q --manifest-path $patcher/Cargo.toml)
fi
prep() { # name src pkg
  local p=$S/pristine-$1
  [ -f $p/.ready ] && return 0
  rm -rf $p; cp -r $corpus/$2 $p; rm -rf $p/.git
  $patcher/target/release/sail-perf-patcher $p/$3 $p $verus_src $tc
  touch $p/.ready
}
prep vest vest-11af01ddaef7 vest_lib
prep ironkv verified-ironkv-7e35ed097c40 ironsht
capture() { # name pkg
  local d=$S/cap-$1
  rm -rf $d $S/traces-$1; cp -r $S/pristine-$1 $d
  ( cd $d/$2 && /usr/bin/time -v sail fragility --keep-traces $S/traces-$1 --seeds 1 --jobs 14 --json >$S/fragility-$1.json 2>$S/fragility-$1.err ) || echo "capture $1 exit $?"
  du -sh $S/traces-$1; ls $S/traces-$1 | wc -l
}
capture ironkv ironsht
capture vest vest_lib
echo SETUP-DONE
