# syntax=docker/dockerfile:1
#
# The sail test environment: everything CI needs to run every check, on the
# reference platform (Linux x86-64, ADR 0010). CI jobs run inside this image,
# and `ci/local.sh` runs the same steps locally. Its tag is a hash of its
# inputs (see ci/image-tag.sh), so it is rebuilt only when a pin changes.
FROM ubuntu:24.04@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55

ENV DEBIAN_FRONTEND=noninteractive
# build-essential/pkg-config/libssl-dev/cmake/ninja: corpus build
# requirements (anvil needs OpenSSL; verismo needs cmake and ninja).
# time: GNU time for the harness's CPU and memory figures.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential pkg-config libssl-dev cmake ninja-build \
      git curl ca-certificates unzip python3 time \
 && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH
COPY rust-toolchain.toml corpus/toolchain.toml /tmp/pins/
# sail's own toolchain (rust-toolchain.toml) and Verus's
# (corpus/toolchain.toml), with the bare-metal target verismo builds for.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain none \
 && sail_tc="$(sed -n 's/^channel = "\(.*\)"/\1/p' /tmp/pins/rust-toolchain.toml)" \
 && verus_tc="$(sed -n 's/^rust_toolchain = "\(.*\)"/\1/p' /tmp/pins/toolchain.toml)" \
 && rustup toolchain install "$sail_tc" --profile minimal -c clippy,rustfmt \
 && rustup toolchain install "$verus_tc" --profile minimal -t x86_64-unknown-none \
 && rustup default "$sail_tc"

RUN cargo install --locked cargo-nextest@0.9.143 cargo-deny@0.20.2 \
 && rm -rf /opt/cargo/registry /opt/cargo/git

# The pinned Verus release, checked by SHA-256 (archive and bundled z3).
COPY ci/fetch-verus.py /tmp/
RUN python3 /tmp/fetch-verus.py /tmp/pins/toolchain.toml /opt/verus
ENV SAIL_VERUS_HOME=/opt/verus \
    SAIL_TEST_Z3=/opt/verus/verus-x86-linux/z3 \
    PATH=/opt/verus/verus-x86-linux:/opt/cargo/bin:$PATH
