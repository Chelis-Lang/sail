# Private source dependencies

Sail's temporal worker links the D.I.V.E. library at the full revision recorded
in `Cargo.toml` and `Cargo.lock`. Both repositories remain private. CI needs
read access to that source independently of the checkout token for Sail.

Create a fine-grained GitHub personal access token restricted to the private
`Chelis-Lang/sail` and `Chelis-Lang/dive` repositories, with **Contents: read**.
Save it as the Actions secret `SAIL_DEPENDENCY_READ_TOKEN` in Sail and Oyster.
The source-based worker build does not require Actions-artifact permissions.
Release or artifact downloading must request only the permissions its actual
endpoint needs. A deploy key covers Git source access but does not authenticate
HTTP release or Actions-artifact downloads.

The compilation gate steps receive this secret at execution time. It is not
available to Docker image builds and is never embedded in an image, URL,
persisted Git configuration or evidence file. The helper is restricted to
HTTPS requests for these two repositories; requests for other paths or hosts
receive no credential. Cargo invokes Git through its command-line fetcher.

For an authenticated local checkout, existing Git credentials work normally.
To select the dedicated credential for a single command, set it in the
environment and run:

```sh
scripts/with-private-dependencies.sh cargo fetch --locked
```

Do not paste a token into a command, issue, log or pull request. Native local
container gates forward an already configured environment credential into the
running container only. The CI image continues to contain public pinned
runtime assets and no private source credential.

With no configured credential, source fetch reports failure. A missing private
dependency never becomes an unavailable backend that the build silently skips.
Compiled worker identity and the exact D.I.V.E. revision are checked again at
execution; successful source download alone is not backend qualification.
