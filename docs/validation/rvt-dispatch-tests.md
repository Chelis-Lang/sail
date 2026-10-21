# Deep source-suite dispatch regressions

**Status:** duplicate progress-option correction built; native qualification
pending. The independent random SMT-log-path comparison is unresolved.

On native c7i.8xlarge in us-east-1, the first full fork pass-through run tested
4,901 cases: 4,899 Same, 2 Different, 0 Unstable, 0 infrastructure errors,
63 ignored. Plain libtests passed 4,837 cases and failed one. The enclosing
command stopped before stock or cache arms. Runtime f8589ed4 on upstream
7d82994; fork test variant `7d829946231d6413a398f6e4160cdfe951261dad-5c434c1cc6390ee4`.

The dispatch start-note test supplies `--no-report-long-running` itself. The
runner's duplicate caused both sides to fail before the original assertion.
Omit only that redundant default for the exact case, binary and full source
hash. The original fixture and every note-order assertion remain unchanged.
Unknown or changed source and unrelated tests retain the default. Explicit
thread overrides remain intact.

The dispatch query test passes its query-byte assertions on both sides but
prints independently randomized tempfile log paths in its rerun commands.
Eleven plain retries reproduce the changing output. That stderr comparison
remains failed. Progress-order approval does not permit changing these paths.

Failed attempt: 2026-10-21, composed source `36cbfdec`, first full fork
pass-through: 4,899/4,901 Same, duplicate-option and random-log-path failures;
remaining arms not run.
