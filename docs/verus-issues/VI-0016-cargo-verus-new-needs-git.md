# VI-0016: `cargo verus new` fails with a bare "No such file or directory" without git

- **Status:** open (tracked here; not filed upstream; see the README)
- **Verus:** `0.2026.10.04.426d8b0` (primary pin), Linux x86-64 release archive
- **Found:** 2026-10-14, following the README install in a fresh
  `ubuntu:24.04` container
- **Impact on sail:** a new user who has not installed `git` cannot create
  the README's first project; the error does not say what is missing.

## Reproduction

In a fresh `ubuntu:24.04` container with `build-essential`, `curl` and
`unzip`, rustup with toolchain `1.98.1`, and the unpacked release on `PATH`
(no `git`):

```sh
cargo verus new --lib demo
# Error: No such file or directory (os error 2)
```

`strace -f -e trace=execve` shows `cargo-verus` running `git init`, which is
not on `PATH`, and reporting the spawn error without naming the program.
With `git` installed the same command succeeds.

## Workaround

The README lists `git` among the prerequisites for installing Verus.
