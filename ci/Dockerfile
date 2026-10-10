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
# Only the primary Verus pin's toolchain, the first `rust_toolchain` line:
# secondary pins are selected locally with SAIL_VERUS_RELEASE.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain none \
 && sail_tc="$(sed -n 's/^channel = "\(.*\)"/\1/p' /tmp/pins/rust-toolchain.toml)" \
 && verus_tc="$(sed -n '/^rust_toolchain = /{s/^rust_toolchain = "\(.*\)"/\1/p;q;}' /tmp/pins/toolchain.toml)" \
 && rustup toolchain install "$sail_tc" --profile minimal -c clippy,rustfmt \
 && rustup toolchain install "$verus_tc" --profile minimal -t x86_64-unknown-none \
 && rustup default "$sail_tc"

# Prebuilt, digest-checked binaries: compiling them made image rebuilds slow.
RUN set -eu; cd /tmp \
 && curl -sSfLo nextest.tgz https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-0.9.143/cargo-nextest-0.9.143-x86_64-unknown-linux-gnu.tar.gz \
 && echo "66786b9abe23920d022a182d1416b1bbc8130dd4872a9553d76985a1708dcd1e  nextest.tgz" | sha256sum -c - \
 && tar -xzf nextest.tgz -C /opt/cargo/bin \
 && curl -sSfLo deny.tgz https://github.com/EmbarkStudios/cargo-deny/releases/download/0.20.2/cargo-deny-0.20.2-x86_64-unknown-linux-musl.tar.gz \
 && echo "9f12ed4c49936e09b48bf862b595cde2fe64fcbd9d74dfacac6131ca824c8d5f  deny.tgz" | sha256sum -c - \
 && tar -xzf deny.tgz \
 && install -m 0755 cargo-deny-0.20.2-x86_64-unknown-linux-musl/cargo-deny /opt/cargo/bin/cargo-deny \
 && rm -rf nextest.tgz deny.tgz cargo-deny-0.20.2-x86_64-unknown-linux-musl \
 && cargo nextest --version && cargo deny --version

# The pinned Verus release, checked by SHA-256 (archive and bundled z3).
COPY ci/fetch-verus.py /tmp/
RUN python3 /tmp/fetch-verus.py /tmp/pins/toolchain.toml /opt/verus
ENV SAIL_VERUS_HOME=/opt/verus \
    SAIL_TEST_Z3=/opt/verus/verus-x86-linux/z3 \
    PATH=/opt/verus/verus-x86-linux:/opt/cargo/bin:$PATH

# Optional interactive engine: built and validated locally, never from source
# in CI. Both the archive and its complete runtime/patch/lock provenance bind
# to committed inputs. The private asset credential exists only in this mount.
COPY ci/engine-artifact.py /tmp/ci/
COPY engine/verus/interactive.patch engine/verus/Cargo.lock /tmp/engine/verus/
COPY engine/verus/artifact-linux.json /tmp/pins/
RUN --mount=type=secret,id=sail_engine_token,required=true \
    SAIL_ENGINE_ARTIFACT_TOKEN="$(cat /run/secrets/sail_engine_token)" \
      python3 /tmp/ci/engine-artifact.py fetch \
        --pin /tmp/pins/artifact-linux.json --cache /opt/sail-engine-cache \
 && python3 -c 'import json,pathlib; p=json.loads(pathlib.Path("/tmp/pins/artifact-linux.json").read_text()); pathlib.Path("/opt/sail-engine").symlink_to(pathlib.Path("/opt/sail-engine-cache/verus-src-build") / p["variant"])'
ENV SAIL_ENGINE_BUILD=/opt/sail-engine
