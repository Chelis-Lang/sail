# Stale solver identity after restored mtime (#213)

Run against a native Sail executable:

```sh
python3 corpus/repros/daemon-solver-identity/repro.py --sail target/release/sail
```

The reproduction starts one private daemon and uses two separate protocol-v4
connections. Between them it changes one comment byte of its private executable
script, preserving path, length and nanosecond modification time. Both scripts
report the same version. Exit 1 means the second identity does not match the new
bytes; exit 0 means it does. Processes and temporary files are removed on either
outcome. Installed solvers and shared caches are untouched.

On the retained private macOS arm64 CLI built from `af339f8a`, the second HELLO
returned the first executable hash despite changed bytes. The daemon source is
byte-identical on main `79e26818`. This establishes an identity-cache defect,
not an observed wrong proof or solver verdict. The initial observation and its
limits are in the [validation record](../../../docs/validation/solver-identity.md).
