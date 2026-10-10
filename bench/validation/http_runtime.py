#!/usr/bin/env python3
"""Validate the built shared-cache runtime image without printing credentials."""
import argparse
import datetime
import json
import pathlib
import subprocess
import time
import urllib.error
import urllib.request
import uuid


def docker(*args):
    return subprocess.check_output(["docker", *args], text=True).strip()


def status(url, token=None, method="GET"):
    headers = {} if token is None else {"Authorization": "Bearer " + token}
    request = urllib.request.Request(
        url, headers=headers, method=method, data=b"" if method == "PUT" else None
    )
    try:
        with urllib.request.urlopen(request, timeout=3) as reply:
            return reply.status
    except urllib.error.HTTPError as error:
        return error.code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", default="sail-team-cache:live-http")
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args()
    suffix = uuid.uuid4().hex[:12]
    volume = "sail-http-validation-" + suffix
    containers = ["sail-http-first-" + suffix, "sail-http-restart-" + suffix]
    docker("volume", "create", volume)

    def start(name):
        docker(
            "run", "-d", "--rm", "--name", name, "--platform", "linux/amd64",
            "-p", "127.0.0.1::8080", "--mount", "source=" + volume + ",target=/store",
            args.image,
        )
        port = docker("port", name, "8080/tcp").rsplit(":", 1)[1]
        url = "http://127.0.0.1:" + port + "/v2/x86_64-linux/" + "0" * 64 + "/00/" + "0" * 64
        for _ in range(50):
            try:
                if status(url) == 401:
                    return url
            except (OSError, urllib.error.URLError):
                pass
            time.sleep(0.1)
        raise RuntimeError("packaged server did not start")

    def credential(name, role):
        return docker("exec", name, "cat", "/store/.sail-server/" + role + ".token")

    try:
        first, restarted = containers
        url = start(first)
        reader, writer = credential(first, "read"), credential(first, "write")
        result = {
            "schema": 1,
            "recorded_at": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "artifact": "runtime-only HTTP shared-cache container",
            "platform": "linux/amd64",
            "image_id": docker("image", "inspect", args.image, "--format", "{{.Id}}"),
            "runtime_user": int(docker("exec", first, "id", "-u")),
            "glibc": docker("exec", first, "getconf", "GNU_LIBC_VERSION"),
            "binary": {
                "sha256": docker("exec", first, "sha256sum", "/usr/local/bin/sail").split()[0],
                "bytes": int(docker("exec", first, "stat", "-c", "%s", "/usr/local/bin/sail")),
            },
            "credential_modes": docker(
                "exec", first, "stat", "-c", "%a", "/store/.sail-server",
                "/store/.sail-server/read.token", "/store/.sail-server/write.token",
            ).splitlines(),
            "http": {
                "unauthenticated_get": status(url),
                "reader_missing_entry_get": status(url, reader),
                "writer_missing_entry_get": status(url, writer),
                "reader_put_refused": status(url, reader, "PUT"),
            },
        }
        assert result["runtime_user"] == 10001
        assert result["credential_modes"] == ["700", "600", "600"]
        assert result["http"] == {
            "unauthenticated_get": 401, "reader_missing_entry_get": 404,
            "writer_missing_entry_get": 404, "reader_put_refused": 401,
        }
        docker("stop", first)
        url = start(restarted)
        result["credentials_survive_restart"] = (
            reader == credential(restarted, "read") and writer == credential(restarted, "write")
        )
        assert result["credentials_survive_restart"] and status(url, reader) == 404
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, indent=2))
    finally:
        for name in containers:
            # Explicit removal also waits for the --rm container's asynchronous
            # cleanup before removing its private credential volume.
            subprocess.run(["docker", "rm", "-f", name], capture_output=True, check=False)
        deadline = time.monotonic() + 5
        while True:
            removed = subprocess.run(["docker", "volume", "rm", volume],
                                     capture_output=True, check=False)
            if removed.returncode == 0:
                break
            exists = subprocess.run(["docker", "volume", "inspect", volume],
                                    capture_output=True, check=False)
            if exists.returncode != 0:
                break
            if time.monotonic() >= deadline:
                raise RuntimeError("runtime probe could not remove its private test volume")
            time.sleep(.1)


if __name__ == "__main__":
    main()
