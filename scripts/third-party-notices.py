#!/usr/bin/env python3
"""Generate THIRD-PARTY-NOTICES for what a Sail release archive redistributes.

A release archive (scripts/release.sh) holds:

- `bin/sail` and `bin/sail-z3`, which statically link their crates.io
  dependencies and the Rust standard library;
- `engine/<variant>/`, Sail's Verus: the fork's `verus`, `cargo-verus`,
  `rust_verify` (which also compiles in Verus's copies of rustc crates),
  `verus_builtin`, the two macro libraries and `vstd` with their crates.io
  dependencies, and the Z3 binary from the Verus release;
- `deps/<variant>/`, the shipped vstd proof, compiled from crates.io `vstd`
  and its closure (scripts/shipped-deps).

`rust_verify` loads `librustc_driver` from the user's own Rust toolchain;
that library is not redistributed.

The crates come from `cargo metadata` under each lock file, for Linux x86-64
and macOS arm64: Sail's workspace (Cargo.lock) for `sail-cli` and
`sail-shim`, and the pinned fork commit (engine/verus/fork.toml, built with
engine/verus/Cargo.lock) for the runtime packages. Each crate's notice is the
licence files its package contains, copied verbatim.

Z3's licence is vendored per version as scripts/notices/z3-<version>-LICENSE.txt,
for the version corpus/toolchain.toml pins; a pin without it fails. Without
--verus-repo, the pinned fork commit is fetched from engine/verus/fork.toml's
repository.

    scripts/third-party-notices.py [--verus-repo FORK_CLONE] --work NEW_DIR [--check]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'THIRD-PARTY-NOTICES'
TARGETS = ('x86_64-unknown-linux-gnu', 'aarch64-apple-darwin')
SAIL_ROOTS = ('sail-cli', 'sail-shim')
VERUS_ROOTS = ('rust_verify', 'verus', 'cargo-verus', 'verus_builtin', 'verus_builtin_macros',
               'verus_state_machines_macros')
LICENSE_FILE = re.compile(r'^(licen[cs]e|copying|notice|copyright|unlicense|authors)([-._].*)?$', re.I)
def run(command, cwd, env=None, data=None):
    result = subprocess.run(command, cwd=cwd, env=env, input=data, capture_output=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode(errors='replace')[-4000:])
        raise SystemExit('third-party-notices: failed: ' + ' '.join(map(str, command)))
    return result.stdout


def closure(directory, roots, env):
    """Registry packages in the normal-dependency closure of `roots`, and the path packages."""
    registry, local = {}, {}
    for target in TARGETS:
        data = json.loads(run(['cargo', 'metadata', '--locked', '--format-version', '1',
                               '--filter-platform', target], directory, env))
        packages = {p['id']: p for p in data['packages']}
        nodes = {n['id']: n for n in data['resolve']['nodes']}
        pending = [i for i, p in packages.items() if p['name'] in roots and p['source'] is None]
        if len({packages[i]['name'] for i in pending}) != len(roots):
            raise SystemExit(f'third-party-notices: missing roots in {directory}')
        seen = set()
        while pending:
            node = pending.pop()
            if node in seen:
                continue
            seen.add(node)
            package = packages[node]
            key = (package['name'], package['version'])
            (local if package['source'] is None else registry)[key] = package
            for dependency in nodes[node]['deps']:
                if any(kind['kind'] is None for kind in dependency['dep_kinds']):
                    pending.append(dependency['pkg'])
    return registry, local


def license_files(directory):
    files = []
    for path in sorted(directory.iterdir()):
        if path.is_file() and LICENSE_FILE.match(path.name):
            files.append(path)
        elif path.is_dir() and path.name.lower() in ('license', 'licenses'):
            files += sorted(p for p in path.rglob('*') if p.is_file())
    return files


MIT_TEXT = """\
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""


def notices(packages, standard):
    """`(name version, licence expression, [(file name, text)], repository)`, sorted.

    A package without licence files gets the standard text of its licence
    (MIT when offered, else Apache-2.0) under the authors its manifest names;
    Verus's own crates get the Verus licence.
    """
    rows = []
    for (name, version), package in sorted(packages.items()):
        directory = Path(package['manifest_path']).parent
        expression = package.get('license') or package.get('license_file') or 'unspecified'
        texts = [(str(path.relative_to(directory)), path.read_text(errors='replace'))
                 for path in license_files(directory)]
        if not texts:
            if (package.get('repository') or '').rstrip('/') == 'https://github.com/verus-lang/verus':
                texts = [('Verus LICENSE (the package has no licence file)', standard['verus'])]
            else:
                authors = ', '.join(package.get('authors') or ['the package authors'])
                kind = 'MIT' if 'MIT' in expression else 'Apache-2.0'
                texts = [(f'standard {kind} text, copyright {authors} (the package has no licence file)',
                          standard[kind])]
        source = package['source'] or ''
        origin = ('Git source ' + source.removeprefix('git+')) if source.startswith('git+') \
            else (package.get('repository') or '')
        rows.append((f'{name} {version}', expression, texts, origin))
    return rows


def render(sections):
    lines = ['Third-party notices for Sail release archives',
             '=============================================', '',
             'Sail itself is MIT-licensed (LICENSE). A Sail release archive also',
             'redistributes the components below. Each entry gives the component, its',
             'licence expression and the licence files its source package contains,',
             'reproduced verbatim. Identical licence texts are printed once and referred',
             'to by number. Generated by scripts/third-party-notices.py.', '',
             'Every crate listed under "Crates compiled into" is published on',
             'crates.io, where its source code is available at the listed version,',
             'except those marked "Git source", which come from the named commit.']
    mpl = sorted({name for _, rows in sections for name, expression, _, _ in rows if 'MPL' in expression})
    if mpl:
        lines += ['The MPL-2.0 crates (' + ', '.join(mpl) + ') are distributed',
                  'in executable form under MPL-2.0 section 3.2; their Source Code Form is',
                  'available from crates.io at the listed version.']
    lines.append('')
    texts, order = {}, []
    for title, rows in sections:
        for _, _, files, _ in rows:
            for _, text in files:
                digest = hashlib.sha256(text.encode()).hexdigest()
                if digest not in texts:
                    texts[digest] = (len(texts) + 1, text)
                    order.append(digest)
    for title, rows in sections:
        lines += [title, '-' * len(title), '']
        for name, expression, files, repository in rows:
            refs = ', '.join(f'{file} [text {texts[hashlib.sha256(text.encode()).hexdigest()][0]}]'
                             for file, text in files) or 'no licence file in the package'
            lines.append(f'* {name}: {expression}' + (f' ({repository})' if repository else ''))
            lines.append(f'  {refs}')
        lines.append('')
    lines += ['Licence texts', '-------------', '']
    for digest in order:
        number, text = texts[digest]
        lines += [f'[text {number}]', '', text.rstrip('\n'), '', '-' * 72, '']
    return '\n'.join(lines).rstrip('\n') + '\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--verus-repo', type=Path,
                        help="a Git clone holding the pinned fork commit (default: fetch it from fork.toml's repository)")
    parser.add_argument('--work', type=Path, required=True, help='a new directory')
    parser.add_argument('--check', action='store_true', help='compare with THIRD-PARTY-NOTICES instead of writing')
    args = parser.parse_args()
    work = args.work.absolute()
    work.mkdir(parents=True)
    env = {k: v for k, v in os.environ.items() if not k.startswith(('CARGO_TARGET', 'SAIL_'))}
    # Run every `cargo metadata` with Sail's own toolchain. In the fork's
    # checkout, its rust-toolchain.toml would otherwise make rustup install
    # rustc-dev, llvm-tools and rustfmt into the verifier toolchain, and
    # release staging records the shipped vstd proof against that toolchain
    # next, under a compiler identity that README installations never have.
    # A missing toolchain fails the check instead of being installed.
    env['RUSTUP_AUTO_INSTALL'] = '0'
    env['RUSTUP_TOOLCHAIN'] = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    pin = tomllib.loads((ROOT / 'engine/verus/fork.toml').read_text())['fork']
    fork = pin['commit']
    z3_version = tomllib.loads((ROOT / 'corpus/toolchain.toml').read_text())['z3']['version']
    z3_license = ROOT / f'scripts/notices/z3-{z3_version}-LICENSE.txt'
    if not z3_license.is_file():
        raise SystemExit(f'third-party-notices: no vendored licence for the pinned Z3 {z3_version} '
                         f'({z3_license.relative_to(ROOT)}); add it from Z3Prover/z3 at z3-{z3_version}')
    repo = args.verus_repo
    if repo is None:
        repo = work / 'fork.git'
        git_env = dict(os.environ, GIT_TERMINAL_PROMPT='0')
        run(['git', 'init', '--quiet', '--bare', str(repo)], work, git_env)
        run(['git', 'fetch', '--quiet', '--depth', '1', pin['repo'], fork], repo, git_env)
    verus = work / 'verus'
    verus.mkdir()
    archive = run(['git', 'archive', '--format=tar', fork], repo)
    run(['tar', '-x', '-C', str(verus)], work, data=archive)
    shutil.copyfile(ROOT / 'engine/verus/Cargo.lock', verus / 'source/Cargo.lock')

    sail_registry, _ = closure(ROOT, SAIL_ROOTS, env)
    verus_registry, verus_local = closure(verus / 'source', VERUS_ROOTS, env)
    # The shipped proof is compiled from crates.io vstd and its closure.
    shipped = work / 'shipped-deps'
    shutil.copytree(ROOT / 'scripts/shipped-deps', shipped)
    shipped_registry, _ = closure(shipped, ('sail-shipped-dependencies',), env)

    fork_rows = [('Sail\'s Verus fork (Chelis-Lang/verus at ' + fork[:8] + ')', 'MIT',
                  [('LICENSE', (verus / 'LICENSE').read_text())], 'https://github.com/Chelis-Lang/verus')]
    for (name, version), package in sorted(verus_local.items()):
        if name.startswith('rustc_'):
            continue
        directory = Path(package['manifest_path']).parent
        files = [(str(p.relative_to(directory)), p.read_text(errors='replace')) for p in license_files(directory)]
        if files:
            fork_rows.append((f'{name} {version} (in the fork)', package.get('license') or 'see files', files, ''))
    rustc = verus / 'source/rustc_mir_build'
    standard = {'MIT': MIT_TEXT, 'Apache-2.0': (rustc / 'LICENSE-APACHE').read_text(),
                'verus': (verus / 'LICENSE').read_text()}
    fork_rows.append(('rustc_hir_analysis, rustc_hir_typeck and rustc_mir_build: Verus\'s copies of '
                      'Rust compiler crates, compiled into rust_verify', 'MIT OR Apache-2.0',
                      [(name, (rustc / name).read_text()) for name in ('LICENSE-APACHE', 'LICENSE-MIT')],
                      'https://github.com/rust-lang/rust'))
    sections = [
        ('Z3 (the solver binary in engine/<variant>/, from the Verus release)',
         [(f'Z3 {z3_version}', 'MIT', [('LICENSE.txt', z3_license.read_text())], 'https://github.com/Z3Prover/z3')]),
        ('Sail\'s Verus (engine/<variant>/ and the vstd proof in deps/<variant>/)', fork_rows),
        ('Crates compiled into Sail\'s Verus', notices(verus_registry, standard)),
        ('Crates compiled into the shipped vstd proof (deps/<variant>/)', notices(shipped_registry, standard)),
        ('Crates compiled into bin/sail and bin/sail-z3', notices(sail_registry, standard)),
        ('The Rust standard library, compiled into every Rust binary above',
         [('Rust standard library (rust-lang/rust)', 'MIT OR Apache-2.0',
           [(name, (rustc / name).read_text()) for name in ('LICENSE-APACHE', 'LICENSE-MIT')],
           'https://github.com/rust-lang/rust')]),
    ]
    text = render(sections)
    if args.check:
        if OUT.read_text() != text:
            (work / 'THIRD-PARTY-NOTICES').write_text(text)
            raise SystemExit(f'third-party-notices: THIRD-PARTY-NOTICES is stale; generated copy in {work}')
        print('THIRD-PARTY-NOTICES is current')
        return
    OUT.write_text(text)
    print(f'wrote {OUT}: {len(sail_registry)} Sail crates, {len(verus_registry)} Verus crates, '
          f'{len(shipped_registry)} shipped-proof crates')


if __name__ == '__main__':
    main()
