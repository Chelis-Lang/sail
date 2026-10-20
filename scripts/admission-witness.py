#!/usr/bin/env python3
"""Run the five-case actual native admission witness against a staged release.

The witness test (`managed::shipped::native::admission_actual_tests`) checks
the production plan, source policy, argument binding and compiler-read
consumer on one genuine native compiler invocation. This runner supplies it:

1. a private copy of `scripts/shipped-deps` with its own `CARGO_HOME`;
2. one recording `sail verify` of that project, traced with `strace`, which
   yields the genuine `verus_builtin` compiler invocation (driver, arguments
   and environment) that Cargo made;
3. one configuration per case, each run in a fresh root:
   - `official-default`: the unchanged invocation; observation and checked
     native output must succeed;
   - `official`: plus `--force-warn missing_docs`; admitted source, refused
     observation, original output preserved;
   - `official-custom-cfg`: plus custom `--cfg`s; admitted source, refused
     observation, the original compiler failure preserved;
   - `foreign`: the same package identity from a copied, failing source
     directory; refused;
   - `modified`: the official source, which the test changes after planning;
     refused.

Usage (native Linux, outside the CI image; the staged release from
`ci/local.sh release`, the witness test binary built for this host):

    scripts/admission-witness.py --stage STAGE --test-binary BIN --work NEW_DIR

The result is `NEW_DIR/result.json`; each case keeps its full streams under
`NEW_DIR/cases/<case>`.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parent.parent
TEST = 'managed::shipped::native::admission_actual_tests::actual_native_admission_child'
CASES = ('official-default', 'official', 'official-custom-cfg', 'foreign', 'modified')
# The witness plans in the outer process, as Sail does before Cargo runs, and
# its native child inherits that environment. Remove what only Cargo's child
# environment has: the compiler wrapper and dynamic-loader paths, which the
# outer plan refuses, and the driver flag, which the test sets for its child.
NOT_OUTER = ('RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', '__VERUS_DRIVER_VIA_CARGO__')


def run(command, **kwargs):
    result = subprocess.run(command, capture_output=True, check=False, **kwargs)
    if result.returncode != 0:
        sys.stderr.write(result.stderr.decode(errors='replace')[-4000:])
        raise SystemExit('admission-witness: failed: ' + ' '.join(map(str, command)))
    return result.stdout


def hex_strings(text):
    """The strings of one `strace -xx` argument or environment array."""
    return [bytes.fromhex(item.replace('\\x', '')).decode() for item in re.findall(r'"((?:\\x[0-9a-f]{2})*)"', text)]


def builtin_invocation(trace):
    """Cargo's verus_builtin library invocation, as Sail's compiler proxy received it,
    and the Verus launcher the proxy runs it with."""
    found = {}
    lines = [line for path in sorted(trace.parent.glob(trace.name + '.*'))
             for line in path.read_text().splitlines()]
    for line in lines:
        match = re.match(r'^execve\("((?:\\x[0-9a-f]{2})*)", \[(.*?)\], \[(.*)\]\) = 0$', line)
        if not match:
            continue
        argv, envp = hex_strings(match.group(2)), hex_strings(match.group(3))
        if (len(argv) > 2 and Path(argv[1]).name == 'rustc' and '--crate-name' in argv
                and argv[argv.index('--crate-name') + 1:][:1] == ['verus_builtin']
                and 'lib' in argv and '-' not in argv):
            found.setdefault(Path(argv[0]).name, []).append(
                (argv, dict(item.split('=', 1) for item in envp if '=' in item)))
    proxies, launchers = found.get('sail-verus-driver', []), found.get('verus', [])
    if len(proxies) != 1 or len(launchers) != 1:
        raise SystemExit(f'admission-witness: expected one proxy and one launcher invocation, found {found.keys()}')
    (proxy_argv, proxy_env), (launcher_argv, _) = proxies[0], launchers[0]
    return launcher_argv[0], proxy_argv[1:], proxy_env


def argument(args, name):
    for index, value in enumerate(args):
        if value == name:
            return args[index + 1]
        if value.startswith(name + '='):
            return value.split('=', 1)[1]
    raise SystemExit(f'admission-witness: invocation has no {name}')


def outputs(args):
    out_dir = Path(argument(args, '--out-dir'))
    crate = argument(args, '--crate-name')
    extra = next(value.split('=', 1)[1] for value in args if value.startswith('extra-filename='))
    emit = argument(args, '--emit').split(',')
    names = [f'lib{crate}{extra}.rmeta'] if 'metadata' in emit else []
    names += [f'lib{crate}{extra}.rlib'] if 'link' in emit else []
    names += [f'{crate}{extra}.d'] if 'dep-info' in emit else []
    return [str(out_dir/name) for name in names]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--stage', type=Path, required=True)
    parser.add_argument('--test-binary', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    # Ghost code without Verus's tool registration: the official source fails.
    parser.add_argument('--custom-cfg', nargs='+', default=['verus_keep_ghost', 'verus_verify_core'])
    args = parser.parse_args()
    stage, work = args.stage.resolve(strict=True), args.work.absolute()
    work.mkdir(parents=True)
    toolchain = tomllib.loads((ROOT/'corpus/toolchain.toml').read_text())['verus']['rust_toolchain']
    status = json.loads(run([str(stage/'bin/sail'), 'engine', 'status', '--output', 'json'],
                            env=dict(os.environ, SAIL_CACHE_DIR=str(work/'status-cache'))))
    build = Path(status['build'])
    release = build/'verus/source/target-verus/release'
    project, home = work/'project', work/'cargo-home'
    shutil.copytree(ROOT/'scripts/shipped-deps', project)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(('SAIL_', 'CARGO_')) and k not in ('RUSTC_WRAPPER', 'RUSTFLAGS', 'VERUS_Z3_PATH')}
    env.update(CARGO_HOME=str(home), RUSTUP_TOOLCHAIN=toolchain)
    run(['cargo', 'fetch', '--locked'], cwd=project, env=env)
    metadata = work/'metadata.json'
    metadata.write_bytes(run(['cargo', 'metadata', '--locked', '--format-version', '1'], cwd=project, env=env))
    # One genuine recording run supplies the native invocation.
    # As the release recorder does, the cache daemon runs outside the trace.
    socket = work/'daemon.sock'
    record_env = dict(env, SAIL_CACHE_DIR=str(work/'sail-cache'), SAIL_VSTD_BOUNDARY_RESEARCH='1',
                      SAIL_SHIPPED_RECORD=str(work/'capture-bundle'), SAIL_DAEMON_SOCKET=str(socket))
    server = subprocess.Popen([str(stage/'bin/sail'), 'serve', '--socket', str(socket), '--cache',
                               str(work/'sail-cache/results')], env=record_env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(200):
            if socket.exists():
                break
            if server.poll() is not None:
                raise SystemExit('admission-witness: cache daemon exited')
            subprocess.run(['sleep', '0.05'], check=True)
        trace = work/'execve'
        capture = subprocess.run(['strace', '-ff', '-qq', '-xx', '-v', '-s', '1048576', '-e', 'trace=execve',
                                  '-e', 'signal=none', '-o', str(trace), str(stage/'bin/sail'), 'verify',
                                  '--locked', '--output', 'json'], cwd=project, env=record_env,
                                 capture_output=True, check=False, timeout=1200)
    finally:
        server.terminate()
        server.wait(timeout=30)
    (work/'capture.stdout').write_bytes(capture.stdout)
    (work/'capture.stderr').write_bytes(capture.stderr)
    wrapper, compiler_args, driver_env = builtin_invocation(trace)
    target = Path(argument(compiler_args, '--out-dir')).parent.parent
    package = Path(driver_env['CARGO_MANIFEST_DIR'])
    registry = package.parent
    source = next(value for value in compiler_args if value.endswith('src/lib.rs'))
    (work/'invocation.json').write_text(json.dumps({'wrapper': wrapper, 'args': compiler_args,
                                                    'environment': driver_env}, indent=1, sort_keys=True)+'\n')
    foreign = work/'foreign'/package.name
    shutil.copytree(package, foreign)
    (foreign/'src/lib.rs').write_text('compile_error!("owned foreign-source negative");\n')
    results = {}
    for case in CASES:
        root = work/'cases'/case
        root.mkdir(parents=True)
        # Each case compiles into its own empty target: verus_builtin has no
        # dependencies, so its native graph holds only this run's outputs.
        case_target = root/'target'
        (case_target/'debug/deps').mkdir(parents=True)
        case_args = [value.replace(str(target), str(case_target)) for value in compiler_args]
        case_env, case_source = dict(driver_env), source
        if case == 'official':
            case_args += ['--force-warn', 'missing_docs']
        elif case == 'official-custom-cfg':
            case_args += [item for cfg in args.custom_cfg for item in ('--cfg', cfg)]
        elif case == 'foreign':
            case_source = str(foreign/'src/lib.rs')
            case_args = [case_source if value == source else value for value in case_args]
            case_env['CARGO_MANIFEST_DIR'] = str(foreign)
        config = {'case': case, 'root': str(root), 'registry': str(registry), 'target': str(case_target),
                  'wrapper': wrapper, 'metadata': str(metadata), 'solver': str(release/'z3'),
                  'shim': str(stage/'bin/sail-z3'), 'source': case_source, 'args': case_args,
                  'outputs': outputs(case_args)}
        (root/'config.json').write_text(json.dumps(config, indent=1)+'\n')
        test_env = {k: v for k, v in case_env.items()
                    if k not in NOT_OUTER and not k.startswith(('LD_', 'DYLD_'))}
        test_env.update(SAIL_ACTUAL_ADMISSION_CONFIG=str(root/'config.json'), SAIL_VSTD_BOUNDARY_RESEARCH='1',
                        SAIL_SHIPPED_RECORD=str(root/'record'), SAIL_SHIPPED_TRACE=str(root/'trace'))
        child = subprocess.run([str(args.test_binary), '--exact', TEST, '--ignored', '--nocapture',
                                '--test-threads', '1'], cwd=project, env=test_env, capture_output=True,
                               check=False, timeout=900)
        (root/'test.stdout').write_bytes(child.stdout)
        (root/'test.stderr').write_bytes(child.stderr)
        comparison = root/'comparison.json'
        results[case] = {'exit': child.returncode, 'passed': child.returncode == 0 and
                         b'1 passed' in child.stdout,
                         'comparison': json.loads(comparison.read_text()) if comparison.exists() else None}
        print(case, 'PASS' if results[case]['passed'] else 'FAIL', flush=True)
    passed = all(row['passed'] for row in results.values())
    (work/'result.json').write_text(json.dumps({'passed': passed, 'stage_build': str(build),
                                                'cases': results}, indent=1)+'\n')
    if not passed:
        raise SystemExit('admission-witness: FAIL')


if __name__ == '__main__':
    main()
