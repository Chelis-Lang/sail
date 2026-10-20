#!/usr/bin/env python3
"""Generate the four engine/verus/*-inputs.json source policies from the pins.

The policies bind exact package sources and features for the shipped vstd
boundary (ADR 0048) and retained proofs (ADR 0050). Every value comes from
the pins, Cargo's own resolution under the pinned toolchain, crates.io package
archives (checked against the lock's SHA-256) and the Verus fork's Git tree:

- `vstd-boundary-inputs.json`: the `scripts/shipped-deps` closure;
- `retained-public-inputs.json`: the same packages with their crates.io origin;
- `retained-vest-inputs.json`: Vest (corpus pin) with its Verus dependencies
  patched to the fork's source, as `patch_verus_deps` does;
- `retained-public-vest-inputs.json`: Vest with only its vstd requirement and
  lock moved to the pinned crates.io vstd.

`metadata_sha256` hashes the `cargo metadata` output for the patched Vest,
which names absolute paths, so it identifies this run's metadata rather than
a reproducible value. Every other byte is reproducible.

    scripts/source-policies.py --verus-repo FORK_CHECKOUT --work NEW_DIR [--check]
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import subprocess
import sys
import tarfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = 'registry+https://github.com/rust-lang/crates.io-index'
VERUS_GIT_URL = 'https://github.com/verus-lang/verus.git'
NAMES = {
    'boundary': 'vstd-boundary-inputs.json',
    'public': 'retained-public-inputs.json',
    'vest': 'retained-vest-inputs.json',
    'public-vest': 'retained-public-vest-inputs.json',
}
VEST_SCOPE = ('Pinned source/feature authority for the disabled workspace retained-proof '
              'prototype; native provenance and actual compiler observations remain '
              'separately mandatory.')
PUBLIC_VEST_GRAPH = 'actual current-pin Vest metadata; only vstd version/lock updated, no path providers'

# BLAKE3 (https://github.com/BLAKE3-team/BLAKE3-specs), used only when the
# `blake3` module is not installed. Checked against the committed policies.
IV = (0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A, 0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19)
PERMUTATION = (2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8)
CHUNK_START, CHUNK_END, PARENT, ROOT_FLAG = 1, 2, 4, 8
MASK = 0xFFFFFFFF


def compress(cv, m, counter, length, flags):
    s = list(cv) + list(IV[:4]) + [counter & MASK, counter >> 32, length, flags]
    m = list(m)
    for round_ in range(7):
        for a, b, c, d, x, y in ((0, 4, 8, 12, 0, 1), (1, 5, 9, 13, 2, 3), (2, 6, 10, 14, 4, 5),
                                 (3, 7, 11, 15, 6, 7), (0, 5, 10, 15, 8, 9), (1, 6, 11, 12, 10, 11),
                                 (2, 7, 8, 13, 12, 13), (3, 4, 9, 14, 14, 15)):
            sa = (s[a] + s[b] + m[x]) & MASK
            sd = s[d] ^ sa
            sd = ((sd >> 16) | (sd << 16)) & MASK
            sc = (s[c] + sd) & MASK
            sb = s[b] ^ sc
            sb = ((sb >> 12) | (sb << 20)) & MASK
            sa = (sa + sb + m[y]) & MASK
            sd ^= sa
            sd = ((sd >> 8) | (sd << 24)) & MASK
            sc = (sc + sd) & MASK
            sb ^= sc
            s[a], s[b], s[c], s[d] = sa, ((sb >> 7) | (sb << 25)) & MASK, sc, sd
        if round_ < 6:
            m = [m[i] for i in PERMUTATION]
    return [s[i] ^ s[i + 8] for i in range(8)] + [s[i + 8] ^ cv[i] for i in range(8)]


def words(block):
    return list(int.from_bytes(block[i:i + 4], 'little') for i in range(0, 64, 4))


def blake3_python(data):
    def chunk_output(chunk, counter):
        cv, blocks = IV, [chunk[i:i + 64] for i in range(0, max(len(chunk), 1), 64)]
        for index, block in enumerate(blocks[:-1]):
            cv = compress(cv, words(block), counter, 64, CHUNK_START if index == 0 else 0)[:8]
        last = blocks[-1]
        flags = CHUNK_END | (CHUNK_START if len(blocks) == 1 else 0)
        return cv, words(last.ljust(64, b'\0')), counter, len(last), flags

    def parent_output(left, right):
        return IV, list(left) + list(right), 0, 64, PARENT

    chunks = [data[i:i + 1024] for i in range(0, max(len(data), 1), 1024)]
    stack = []
    for counter, chunk in enumerate(chunks[:-1]):
        cv = compress(*chunk_output(chunk, counter))[:8]
        total = counter + 1
        while total & 1 == 0:
            cv = compress(*parent_output(stack.pop(), cv))[:8]
            total >>= 1
        stack.append(cv)
    output = chunk_output(chunks[-1], len(chunks) - 1)
    while stack:
        output = parent_output(stack.pop(), compress(*output)[:8])
    cv, m, _, length, flags = output
    return b''.join(w.to_bytes(4, 'little') for w in compress(cv, m, 0, length, flags | ROOT_FLAG)[:8]).hex()


try:
    import blake3 as _blake3

    def blake3(data):
        return _blake3.blake3(data).hexdigest()
except ImportError:
    blake3 = blake3_python


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def dump(value):
    return (json.dumps(value, indent=2, sort_keys=True) + '\n').encode()


def run(command, cwd, env):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode(errors='replace'))
        raise SystemExit('source-policies: failed: ' + ' '.join(map(str, command)))
    return result.stdout


def tree_entry(files):
    """Per-file SHA-256 (the source authority) and BLAKE3 with size (what rustc's SourceMap records)."""
    return {
        'files': {name: sha256(data) for name, data in files.items()},
        'compiler_files': {name: {'checksum': 'blake3=' + blake3(data), 'size': len(data)}
                           for name, data in files.items()},
    }


class Crates:
    """crates.io package archives, cached under the work directory and checked against the lock."""

    def __init__(self, cache):
        self.cache = cache
        cache.mkdir(parents=True, exist_ok=True)
        self.entries = {}

    def files(self, name, version, checksum):
        key = f'{name}-{version}'
        if key in self.entries:
            return self.entries[key]
        path = self.cache / f'{key}.crate'
        if not path.exists():
            url = f'https://static.crates.io/crates/{name}/{key}.crate'
            with urllib.request.urlopen(url, timeout=120) as response:
                data = response.read()
            path.write_bytes(data)
        data = path.read_bytes()
        if sha256(data) != checksum:
            raise SystemExit(f'source-policies: {key}.crate differs from its locked checksum')
        files = {}
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
            for member in archive.getmembers():
                parts = PurePosixPath(member.name).parts
                if not parts or parts[0] != key or '..' in parts:
                    raise SystemExit(f'source-policies: {key}.crate has a foreign member {member.name}')
                if member.isdir():
                    continue
                if not member.isfile():
                    raise SystemExit(f'source-policies: {key}.crate has a non-file member {member.name}')
                files[str(PurePosixPath(*parts[1:]))] = archive.extractfile(member).read()
        self.entries[key] = (files, checksum)
        return self.entries[key]


def lock_checksums(lock_path):
    lock = tomllib.loads(lock_path.read_text())
    return {(p['name'], p['version']): p['checksum'] for p in lock['package']
            if p.get('source') == REGISTRY}


def closure(metadata, root_manifest):
    """The normal and build dependency closure of the package at `root_manifest`, as `admit` walks it."""
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    root = next(i for i, p in packages.items() if Path(p['manifest_path']) == root_manifest)
    found, pending = {}, [root]
    while pending:
        node = nodes[pending.pop()]
        for dependency in node['deps']:
            if any(kind['kind'] != 'dev' for kind in dependency['dep_kinds']):
                if dependency['pkg'] not in found:
                    found[dependency['pkg']] = (packages[dependency['pkg']], nodes[dependency['pkg']])
                    pending.append(dependency['pkg'])
    return found.values()


def metadata(directory, env):
    return run(['cargo', 'metadata', '--format-version', '1'], directory, env)


def registry_entry(package, node, crates, checksums, source_bytes=False):
    files, archive = crates.files(package['name'], package['version'], checksums[(package['name'], package['version'])])
    entry = tree_entry(files)
    entry.update(name=package['name'], version=package['version'], features=sorted(node['features']),
                 origin={'archive_sha256': archive, 'kind': 'crates-io'})
    if source_bytes:
        entry['source_bytes'] = sum(map(len, files.values()))
    return entry


def boundary_policy(work, env, crates, vstd):
    project = work / 'shipped-deps'
    shutil.copytree(ROOT / 'scripts/shipped-deps', project)
    data = json.loads(run(['cargo', 'metadata', '--locked', '--format-version', '1'], project, env))
    checksums = lock_checksums(project / 'Cargo.lock')
    packages = {}
    for package, node in closure(data, project / 'Cargo.toml'):
        if package['source'] != REGISTRY:
            raise SystemExit(f'source-policies: shipped-deps has a non-registry package {package["id"]}')
        entry = registry_entry(package, node, crates, checksums)
        entry['archive_sha256'] = entry.pop('origin')['archive_sha256']
        if package['name'] == 'vstd':
            entry['compiler_checksums'] = {name: {'blake3': value['checksum'].removeprefix('blake3='), 'size': value['size']}
                                           for name, value in entry['compiler_files'].items()}
        packages[f'{package["name"]}-{package["version"]}'] = entry
    if f'vstd-{vstd}' not in packages:
        raise SystemExit(f'source-policies: scripts/shipped-deps does not lock vstd {vstd}')
    return {'kind': 'sail-vstd-boundary-source-policy', 'packages': packages, 'schema': 2, 'vstd': f'vstd-{vstd}'}


def public_policy(boundary, boundary_bytes, commit):
    packages = {}
    for key, entry in boundary['packages'].items():
        entry = {k: v for k, v in entry.items() if k not in ('archive_sha256', 'compiler_checksums')}
        entry['origin'] = {'archive_sha256': boundary['packages'][key]['archive_sha256'], 'kind': 'crates-io'}
        packages[key] = entry
    return {'authority_source_sha256': sha256(boundary_bytes), 'kind': 'sail-retained-public-registry-inputs',
            'packages': packages, 'schema': 1, 'verus_source_commit': commit}


def clone_vest(work, name, corpus):
    vest = next(p for p in corpus['project'] if p['name'] == 'vest')
    directory = work / name
    env = dict(os.environ, GIT_CONFIG_GLOBAL=os.devnull, GIT_CONFIG_NOSYSTEM='1')
    run(['git', 'clone', '--quiet', '--no-checkout', vest['url'], str(directory)], work, env)
    run(['git', 'checkout', '--quiet', '--detach', vest['commit']], directory, env)
    if not vest.get('patch_verus_deps') or vest.get('crate_roots') != ['vest_lib']:
        raise SystemExit('source-policies: the corpus vest entry no longer matches this generator')
    return directory


def set_vstd_requirement(manifest, version):
    text = manifest.read_text()
    updated, count = re.subn(r'(?m)^(vstd = \{ version = ")=[^"]*(")', rf'\g<1>={version}\g<2>', text)
    if count != 1:
        raise SystemExit(f'source-policies: cannot find the vstd requirement in {manifest}')
    manifest.write_text(updated)


def fork_tree(verus_repo, commit, out):
    """The fork's `source` and `dependencies` trees at `commit`, as Git stores them."""
    archive = run(['git', 'archive', '--format=tar', commit, 'source', 'dependencies'], verus_repo, os.environ)
    files = {}
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        for member in tar.getmembers():
            if member.isfile():
                files[member.name] = tar.extractfile(member).read()
            elif member.issym():
                files[member.name] = None
        tar.extractall(out, filter='data')
    return files


def official_vest_policy(work, env, crates, verus_repo, fork_commit, commit, corpus):
    source = work / 'verus-source'
    tree = fork_tree(verus_repo, fork_commit, source)
    vest = clone_vest(work, 'vest-official', corpus)
    # patch_verus_deps: patch the Verus packages the workspace or vest_lib
    # declare (only vstd) from both sources, and relax the exact pin to it.
    version = tomllib.loads((source / 'source/vstd/Cargo.toml').read_text())['package']['version']
    root = vest / 'Cargo.toml'
    set_vstd_requirement(root, version)
    path = str(source / 'source/vstd')
    with root.open('a') as stream:
        for table in ('crates-io', f'"{VERUS_GIT_URL}"'):
            stream.write(f'\n[patch.{table}]\nvstd = {{ path = "{path}" }}\n')
    raw = metadata(vest, env)
    data = json.loads(raw)
    checksums = lock_checksums(vest / 'Cargo.lock')
    packages = {}
    for package, node in closure(data, vest / 'vest_lib/Cargo.toml'):
        key = f'{package["name"]}-{package["version"]}'
        if package['source'] == REGISTRY:
            packages[key] = registry_entry(package, node, crates, checksums, source_bytes=True)
            continue
        if package['source'] is not None:
            raise SystemExit(f'source-policies: unexpected provider {package["id"]}')
        relative = Path(package['manifest_path']).parent.relative_to(source).as_posix()
        files = {name[len(relative) + 1:]: data for name, data in tree.items()
                 if name.startswith(relative + '/')}
        if any(data is None for data in files.values()):
            raise SystemExit(f'source-policies: {relative} has a symbolic link')
        entry = tree_entry(files)
        entry.update(name=package['name'], version=package['version'], features=sorted(node['features']),
                     origin={'commit': commit, 'kind': 'pinned-verus-source', 'path': relative},
                     source_bytes=sum(map(len, files.values())))
        packages[key] = entry
    return {'kind': 'sail-retained-vest-official-inputs', 'lock_sha256': sha256((vest / 'Cargo.lock').read_bytes()),
            'metadata_sha256': sha256(raw), 'packages': packages, 'schema': 1, 'scope': VEST_SCOPE,
            'verus_source_commit': commit, 'verus_source_fork_commit': fork_commit}


def public_vest_policy(work, env, crates, vstd, boundary, sources, commit, corpus):
    vest = clone_vest(work, 'vest-public', corpus)
    set_vstd_requirement(vest / 'Cargo.toml', vstd)
    data = json.loads(metadata(vest, env))
    checksums = lock_checksums(vest / 'Cargo.lock')
    packages = {}
    for package, node in closure(data, vest / 'vest_lib/Cargo.toml'):
        if package['source'] != REGISTRY:
            raise SystemExit(f'source-policies: unexpected provider {package["id"]}')
        key = f'{package["name"]}-{package["version"]}'
        if key in boundary['packages']:
            # The boundary's archive identity, with Vest's own feature vector.
            entry = {k: v for k, v in boundary['packages'][key].items() if k not in ('archive_sha256', 'compiler_checksums')}
            entry.update(features=sorted(node['features']),
                         origin={'archive_sha256': boundary['packages'][key]['archive_sha256'], 'kind': 'crates-io'})
        else:
            entry = registry_entry(package, node, crates, checksums, source_bytes=True)
        packages[key] = entry
    return {'authority_source_sha256': sources[0], 'authority_sources': sources, 'feature_graph': PUBLIC_VEST_GRAPH,
            'kind': 'sail-retained-public-vest-inputs', 'packages': packages, 'schema': 1, 'verus_source_commit': commit}


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--verus-repo', type=Path, required=True, help="a Git clone holding the fork commit")
    parser.add_argument('--fork-commit', help="default: engine/verus/fork.toml's commit")
    parser.add_argument('--work', type=Path, required=True, help='a new directory for clones and downloads')
    parser.add_argument('--out', type=Path, default=ROOT / 'engine/verus')
    parser.add_argument('--check', action='store_true',
                        help='compare with the files in --out (metadata_sha256 excepted) instead of writing')
    args = parser.parse_args()
    pins = tomllib.loads((ROOT / 'corpus/toolchain.toml').read_text())['verus']
    corpus = tomllib.loads((ROOT / 'corpus/corpus.toml').read_text())
    fork_commit = args.fork_commit or tomllib.loads((ROOT / 'engine/verus/fork.toml').read_text())['fork']['commit']
    fork_commit = run(['git', 'rev-parse', '--verify', fork_commit + '^{commit}'], args.verus_repo, os.environ).decode().strip()
    vstd, commit = pins['vstd'], pins['commit']
    work = args.work.absolute()
    work.mkdir(parents=True)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('CARGO_', 'RUSTC', 'SAIL_')) and k not in ('CARGO', 'RUSTFLAGS', 'RUSTUP_TOOLCHAIN')}
    env['RUSTUP_TOOLCHAIN'] = pins['rust_toolchain']
    crates = Crates(work / 'crates')

    boundary = boundary_policy(work, env, crates, vstd)
    outputs = {'boundary': dump(boundary)}
    outputs['public'] = dump(public_policy(boundary, outputs['boundary'], commit))
    official = official_vest_policy(work, env, crates, args.verus_repo, fork_commit, commit, corpus)
    outputs['vest'] = dump(official)
    sources = [sha256(outputs['boundary']), sha256(outputs['vest'])]
    outputs['public-vest'] = dump(public_vest_policy(work, env, crates, vstd, boundary, sources, commit, corpus))

    differ = []
    for kind, data in outputs.items():
        path = args.out / NAMES[kind]
        if not args.check:
            path.write_bytes(data)
            print(f'wrote {path} ({sha256(data)})')
            continue
        current = path.read_bytes()
        if data != current:
            # The run-specific metadata digest, and the digest of the official
            # Vest policy that carries it, are compared as equal.
            mine, theirs = json.loads(data), json.loads(current)
            if kind == 'vest':
                mine['metadata_sha256'] = theirs['metadata_sha256']
            if kind == 'public-vest':
                mine['authority_sources'][1] = theirs['authority_sources'][1]
            if dump(mine) == current:
                print(f'{path.name}: identical except the run-specific metadata digest')
                continue
            differ.append(path.name)
            (work / path.name).write_bytes(data)
        print(f'{path.name}: {"identical" if data == current else "DIFFERS"}')
    if differ:
        raise SystemExit(f'source-policies: differ: {", ".join(differ)} (generated copies in {work})')

if __name__ == '__main__':
    main()
