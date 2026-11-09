# Your first proof

These examples use a POSIX shell on Linux x86-64 or Apple-silicon macOS. Install
Sail 0.2.0 through Cargo:

```sh
rustup toolchain install 1.99.1 --profile minimal --component rust-src
cargo +1.99.1 install sail-solve --version 0.2.0 --locked
sail engine install
sail engine status
```

The package is called `sail-solve`; the command is `sail`. `sail engine install`
downloads Sail's pinned Verus and Z3. `sail engine status` should report that the
engine is available.

Native release archives are another option; they include the runtime and preverified
vstd. The [installation guide](install.md) covers prerequisites and both routes.

## Start with one file

Use a new directory for the example:

```sh
mkdir sail-proof-demo
cd sail-proof-demo
```

Save this as `proof.rs`:

```rust,ignore
use vstd::prelude::*;

verus! {
    proof fn addition() {
        assert(2 + 2 == 4);
    }
}

fn main() {}
```

The first check may take longer while Sail prepares vstd, Verus's standard
library. Sail handles this; there is no separate setup command. From the example
directory, run:

```sh
sail verify proof.rs
sail verify proof.rs --output json
```

The proof should pass. The second command returns a structured report instead of the
usual verifier output. Sail records a successful full check so a later unchanged
call can reuse its proof. This record is called a proof receipt.

Change the assertion to `assert(2 + 2 == 5);` and verify again. This check should
fail and identify the assertion. Restore `4` and run `sail verify proof.rs --output
json` once more. In a reused result, `targets[0].reused` is `true` and
`targets[0].establishing_run` names the earlier successful check. It verifies again if it cannot confirm that
the earlier inputs still match. The [standalone guide](standalone-verification.md)
explains reuse and file-layout requirements.

## Verify a Cargo project

As a separate exercise, start a Verus library from the example directory:

```sh
sail new --lib demo
cd demo
sail verify
```

In an existing Verus project, use `sail verify` where you would run `cargo verus
verify`. The [workspace guide](workspace-verification.md) covers package and target
selection.

## Use a coding agent

Return to the directory containing `proof.rs` first. If you followed the Cargo
exercise above, run `cd ..`. Register Sail there in Claude Code:

```sh
claude mcp add --transport stdio --scope project sail -- sail mcp
```

Restart Claude Code to load the configuration, and check `claude mcp list` for a
connected Sail server. Other clients can launch `sail mcp` on stdio; see the [MCP
reference](mcp.md). The [agent
skill](https://raw.githubusercontent.com/Chelis-Lang/sail/main/skills/sail-verification/SKILL.md)
teaches the verification and SMT workflows.

Change `4` to `5` again, then ask the agent to verify the file, explain the failure,
and propose a fix that preserves the intended contract. For this standalone example,
the `verify` tool takes the example directory's absolute path as `project_dir` and
`proof.rs` as `source_file`. Have it check the complete file after changing it. A
passing focused check on one function does not mean the whole file passes.

Read the [repair walkthrough](proof-repair-walkthrough.md) when you want a focused
check followed by independent full verification. [Freshness](freshness.md) explains
why a successful result also needs to match the current inputs.
