#!/usr/bin/env python3
"""Install a selected Sail archive without changing the default Rust toolchain."""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import urllib.error

REPOSITORY = "Chelis-Lang/sail"


def target():
    system, machine = platform.system(), platform.machine()
    if system == "Darwin" and machine in ("arm64", "aarch64"):
        return "aarch64-apple-darwin"
    if system == "Linux" and machine == "x86_64":
        libc, version = platform.libc_ver()
        if libc != "glibc" or tuple(map(int, version.split(".")[:2])) < (2, 39):
            raise ValueError("Sail requires Linux glibc 2.39 or newer (for example Ubuntu 24.04)")
        return "x86_64-unknown-linux-gnu"
    raise ValueError("supported platforms: Linux x86-64 and macOS on Apple silicon")


def download(version, name, directory):
    if shutil.which("gh") and subprocess.run(
        ["gh", "auth", "status"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL
    ).returncode == 0:
        subprocess.run(["gh", "release", "download", "v" + version, "--repo", REPOSITORY,
                        "--pattern", name, "--pattern", name + ".sha256",
                        "--dir", str(directory)], check=True)
    else:
        base = "https://github.com/" + REPOSITORY + "/releases/download/v" + version + "/"
        for filename in (name, name + ".sha256"):
            try:
                with urllib.request.urlopen(base + filename, timeout=60) as response:
                    with (directory / filename).open("wb") as output:
                        shutil.copyfileobj(response, output)
            except urllib.error.HTTPError as error:
                if error.code in (401, 403, 404):
                    raise ValueError("cannot download the selected version; check that it exists. "
                                     "Private drafts require authenticated GitHub CLI (`gh auth login`), "
                                     "or use --archive with a downloaded archive and checksum") from error
                raise
    return directory / name


def verify(archive):
    checksum = Path(str(archive) + ".sha256").read_text().strip()
    match = re.fullmatch(r"([0-9a-fA-F]{64}) [ *](.+)", checksum)
    if not match or match[2] != archive.name:
        raise ValueError("checksum must contain exactly the selected archive's SHA-256 and filename")
    digest = hashlib.sha256()
    with archive.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    if digest.hexdigest() != match[1].lower():
        raise ValueError("archive checksum does not match; nothing installed")


def unpack(archive, destination, name):
    # Factory archives contain only directories and regular files. Refuse
    # links and special files before extracting anything, including a link
    # followed by an otherwise harmless-looking member beneath that link.
    with tarfile.open(archive, "r:gz") as source:
        members = source.getmembers()
        if not members:
            raise ValueError("empty archive")
        seen = set()
        for member in members:
            path = PurePosixPath(member.name)
            if (path.is_absolute() or ".." in path.parts or not path.parts
                    or path.parts[0] != name or str(path) in seen
                    or not (member.isdir() or member.isfile())):
                raise ValueError("archive has an unsupported or unsafe member: " + member.name)
            seen.add(str(path))
        for member in members:
            path = destination.joinpath(*PurePosixPath(member.name).parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                path.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as content, path.open("xb") as output:
                    shutil.copyfileobj(content, output)
                path.chmod(member.mode & 0o777)
    installed = destination / name
    for binary in ("sail", "sail-z3"):
        if not os.access(installed / "bin" / binary, os.X_OK):
            raise ValueError("archive is missing executable bin/" + binary)
    if not (installed / "engine").is_dir():
        raise ValueError("archive is missing its bundled Verus engine")
    return installed


def install(archive, prefix, host_target, setup_toolchain=False):
    match = re.fullmatch(r"sail-([0-9][A-Za-z0-9.+-]*)-" + re.escape(host_target)
                         + r"\.tar\.gz", archive.name)
    if not match:
        raise ValueError("archive filename must select this platform: " + host_target)
    name, version = archive.name[:-7], match[1]
    verify(archive)
    prefix.mkdir(parents=True, exist_ok=True)
    final = prefix / name
    if final.exists() or final.is_symlink():
        raise ValueError("installation already exists; choose another --prefix or remove it: " + str(final))
    with tempfile.TemporaryDirectory(prefix=".sail-install-", dir=prefix) as temporary:
        staged = unpack(archive, Path(temporary), name)
        executable = staged / "bin" / "sail"
        result = subprocess.run([str(executable), "--version"], check=True, text=True,
                                stdout=subprocess.PIPE)
        if result.stdout.strip() != "sail " + version:
            raise ValueError("archive version does not match its filename")
        with tempfile.TemporaryDirectory(prefix="sail-install-check-") as cache:
            environment = {key: value for key, value in os.environ.items()
                           if key not in ("SAIL_VERUS_BUILD", "SAIL_ENGINE_BUILD")}
            environment["SAIL_CACHE_DIR"] = cache
            status = json.loads(subprocess.check_output(
                [str(executable), "engine", "status"], env=environment, text=True))
            if Path(status["build"]).resolve().parent != (staged / "engine").resolve():
                raise ValueError("Sail did not select the archive's bundled engine")
        if setup_toolchain:
            subprocess.run(["rustup", "toolchain", "install", status["rust_toolchain"],
                            "--profile", "minimal", "--component", "rust-src"], check=True)
        staged.rename(final)
    return final


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--version", help="explicit release version, e.g. 0.1.0")
    choice.add_argument("--archive", type=Path, help="local archive with adjacent .sha256 file")
    parser.add_argument("--prefix", type=Path, default=Path.home() / ".local" / "opt")
    parser.add_argument("--install-toolchain", action="store_true",
                        help="install the verifier's Rust pin and rust-src using existing rustup")
    args = parser.parse_args()
    try:
        host_target = target()
        if args.version and not re.fullmatch(r"[0-9][A-Za-z0-9.+-]*", args.version):
            raise ValueError("invalid release version")
        if args.install_toolchain and not shutil.which("rustup"):
            raise ValueError("install rustup first: https://rustup.rs")
        with tempfile.TemporaryDirectory(prefix="sail-download-") as temporary:
            archive = args.archive.expanduser().resolve() if args.archive else download(
                args.version, "sail-" + args.version + "-" + host_target + ".tar.gz", Path(temporary))
            installed = install(archive, args.prefix.expanduser().resolve(), host_target,
                                args.install_toolchain)
        import shlex
        print("Installed " + str(installed))
        print("Add this to your shell profile:")
        print("export PATH=" + shlex.quote(str(installed / "bin")) + ':"$PATH"')
        if not args.install_toolchain:
            print("Install the verifier's pinned Rust toolchain with rust-src before first use; see the install guide.")
    except (OSError, ValueError, KeyError, tarfile.TarError, subprocess.CalledProcessError) as error:
        print("error[SAIL4215]: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
