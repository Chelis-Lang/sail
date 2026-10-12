# Reading job results from Rust

`sail_api::jobs::Client` reads job Status and Results from the local jobs
service, the same service the CLI and `sail mcp` use. Every call opens a
connection and reads a complete fresh reply; nothing is cached on the client.

## Connect to the jobs service

`Client::new` expects the jobs socket, not the solver-cache socket. Resolve it
with `jobs::configured_socket(&cwd)` to keep the CLI's `SAIL_CACHE_DIR` and
`SAIL_DAEMON_SOCKET` semantics, including relative settings belonging to that
project directory. If an application already has the cache socket, derive its
job endpoint with `jobs::socket_path(&cache_socket)`.

The SDK does not start a daemon. Submit through the CLI or MCP first, for
example `sail verify --background --output json` from the project directory.

## Read a Result

Within a function returning `std::io::Result<()>`, where `job_id` is the
durable ID returned by submission:

```rust
use sail_api::jobs::{Client, Request, Response};

let project_dir = std::env::current_dir()?;
let client = Client::new(sail_api::jobs::configured_socket(&project_dir)?);
match client.call(&Request::Result { id: job_id.to_owned() })? {
    Response::Result(document) if document.job.id == job_id => {
        println!("{:?}: {:?}", document.job.state, document.job.currentness);
    }
    _ => eprintln!("unexpected response to a result request"),
}
```

The lifecycle and currentness are separate from the proof outcome. Inspect the
complete report before treating a function as verified; a repair also needs its
independent full evidence and `repair.accepted`. A Historical result applies
only to its earlier input revision. `Request::ProofEvents` returns a bounded
proof-journal page and cursor; `Request::Events` reads the target-lifecycle log
by byte offset.

## Errors and limits

Connection and I/O failures return `io::Error`, and a service `Response::Error`
is returned as an `io::Error` carrying its `SAIL` code. Framing rejects bodies
over 32 MiB with `SAIL0801` and an incompatible envelope version with
`SAIL0802`. Each read and write has a five-second timeout.

`Client::call_with` also hands the caller a clone of the connected stream.
Shutting that clone down from another thread abandons the call; `sail mcp` uses
this to cancel a direct read at once.
