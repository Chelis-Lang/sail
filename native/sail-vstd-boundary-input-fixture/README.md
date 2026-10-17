# Compiler-input consumption fixture

This branch contains a test-only integration fixture for the unselected vstd
compiler-boundary prototype. It is not a product feature or a qualified runtime.

`sail-verus/sail-boundary-input-fixture` includes private coordination code.
Without that Cargo feature the module is absent. With the feature and
`SAIL_BOUNDARY_INPUT_FIXTURE=<existing-private-absolute-directory>`, native
compiler invocations retain their actual arguments, environment and working
directory in owner-only files. An admitted vstd cache miss records its final
child command after keying, then waits at most 180 seconds for the exact bytes
`continue\n` in `continue-<wrapper-pid>` before spawning the unchanged compiler.
After actual child completion, a second bounded coordination point retains its
status and waits for `continue\n` in `publish-<wrapper-pid>` before publication
rechecks inputs. Successful coordination changes no compiler argument, solver
command, diagnostic or verdict.

The bounded integration control uses two separately retained macro-library
fixtures. The second expands one additional harmless proof function and records
its use. The control distinguishes actual compiler consumption from pathname
hashes and retains before/after library identities, full compiler observations,
complete verification outcomes and cache-publication evidence. No timing or
default-selection claim follows from fixture execution. The official registry
source policy, compiler-input observation and publication checks remain intact.

This is a trusted, isolated test host. Environment snapshots remain private raw
evidence outside the repository. No fixture source or feature is selected for
release. Any observed disagreement blocks the proposed replacement until fixed;
ordinary stock parity and both-platform qualification remain separate gates.
