# A runtime-only image: build Sail separately with the pinned toolchain, then
# copy the Linux x86-64 release binary into this build context as `sail`.
FROM ubuntu:24.04@sha256:534baea6a22c03a63003dbc8dbe78fe34bc0d7e595d9a9dc9834884ff530eb55
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --uid 10001 --create-home sail \
 && mkdir /store && chown sail:sail /store
COPY --chmod=755 sail /usr/local/bin/sail
USER 10001:10001
VOLUME /store
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/sail", "cache", "serve", "--store", "/store", "--listen", "0.0.0.0:8080", "--behind-tls-proxy"]
