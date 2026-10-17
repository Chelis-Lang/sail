# An unsupported command can exit successfully

Z3 4.16.0 distinguishes an unsupported command from a parse error. With the
pinned solver, run each file separately and retain both streams and the status:

```sh
z3 -smt2 -in < unsupported.smt2 > unsupported.stdout 2> unsupported.stderr
printf 'exit=%s\n' "$?"
z3 -smt2 -in < parse-error.smt2 > parse-error.stdout 2> parse-error.stderr
printf 'exit=%s\n' "$?"
```

`unsupported.smt2` exits zero, writes `unsupported` plus a newline to stdout,
and writes `; definitely-not-an-smt-command line: 1 position: 31` plus a
newline to stderr. It produces no satisfiability verdict. A successful process
exit alone does not establish a verified result.

The original kernel-owner research control assumed this command would exit
nonzero and failed strictly: ten of eleven mandatory native controls passed;
IronKV and Anvil did not run. This is a fixture error, not evidence of a wrong
solver answer or of trace eligibility. A resource-completion gate is separate
from verification and exact response admission. The original failed attempt
remains failed.

Native observation: Linux x86-64 `7.0.0-1014-aws`, pinned Z3 SHA-256
`e583c4186a45e72411fa2cb2048401eed03f0f8e5f24694676a8f6271a50b765`.
The retained native control report has SHA-256
`7bd1d67758419227a123d9c37d65c946a8c2092374a9c2349221f63288dda1e6`.
The original final archive has SHA-256
`bb9e1a650af99190fa7b80e47517dfde0db7739dff5e182692af9299771c4ed3`.

The pinned macOS arm64 Z3 was also run directly against both files. The
unsupported command produces identical streams and exit zero. The single
unexpected closing parenthesis in `parse-error.smt2` genuinely exits one,
with empty stderr and this stdout:

```text
(error "line 1 column 1: invalid command, '(' expected")
```

That probe binds the actual macOS solver SHA-256
`edae32f9e37ea4b5bb35310d72f0e352d0dc07626cac4e9e30bc1ea9a5bc8efb`.
Its complete five-input API probe report has SHA-256 `41338122f738cf803bc7238a5555d8d2e39a0cf8059b5e9a3884857d8b4e4efb`.
This is an API behavior check, not Linux qualification or a performance sample.
The later corrected Linux twelve-case gate at `10b96ed1` demonstrates both
this genuine parse-error exit one with exact retained diagnostics and the
separate unsupported exit-zero behavior before either trace runs. Its native
controls archive SHA-256 is
`9698995ac3605b8ed9c60c3a42b41fd2386da4c1df09a64503736b31b23bf77a`.
The original failed eleven-case attempt remains failed.
