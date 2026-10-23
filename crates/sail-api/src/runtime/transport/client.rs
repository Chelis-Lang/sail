//! Synchronous client with capability negotiation and responsive cancellation.
use super::{
    Deadline, IO_TIMEOUT, MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES, Request, Response, read_frame,
    validate_socket, write_frame,
};
use crate::runtime::{
    Capabilities, Input, Request as RuntimeRequest, Response as RuntimeResponse, VERSION,
};
use socket2::{Domain, SockAddr, Socket, Type};
use std::{
    fmt::Write as _,
    io::{self, Read as _},
    net::Shutdown,
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::mpsc,
    time::Duration,
};

/// Owner-private runtime client; no solver process or fallback is ever started here.
#[derive(Clone, Debug)]
pub struct Client {
    socket: PathBuf,
}

fn unavailable(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!(
        "SAIL6201: runtime endpoint unavailable or incompatible; restart sail serve: {error}"
    ))
}

impl Client {
    /// Construct a side-effect-free client for the independently versioned endpoint.
    #[must_use]
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    fn connect(&self) -> io::Result<UnixStream> {
        validate_socket(&self.socket).map_err(unavailable)?;
        let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
        socket
            .connect_timeout(&SockAddr::unix(&self.socket)?, IO_TIMEOUT)
            .map_err(unavailable)?;
        let fd: std::os::fd::OwnedFd = socket.into();
        Ok(UnixStream::from(fd))
    }

    fn call(&self, request: &Request) -> io::Result<Response> {
        let stream = self.connect()?;
        let mut deadline = Deadline::new(&stream, IO_TIMEOUT);
        write_frame(&mut deadline, request, MAX_REQUEST_BYTES)?;
        read_frame(&mut deadline, MAX_RESPONSE_BYTES)
    }

    /// Negotiate implementation and actual installation readiness without checking a theorem.
    ///
    /// # Errors
    /// Reports absent, incompatible, malformed or failed services explicitly.
    pub fn capabilities(&self) -> io::Result<Capabilities> {
        match self.call(&Request::Capabilities)? {
            Response::Capabilities { capabilities } if capabilities.version == VERSION => {
                Ok(capabilities)
            }
            Response::Error { error } => Err(io::Error::other(error)),
            _ => Err(unavailable("unexpected capability response")),
        }
    }

    /// Cancel an exact active transport identity on a separate control connection.
    ///
    /// # Errors
    /// Returns connection or protocol failures instead of claiming cancellation.
    pub fn cancel(&self, id: &str) -> io::Result<bool> {
        match self.call(&Request::Cancel { id: id.into() })? {
            Response::Cancelled { id: actual, found } if actual == id => Ok(found),
            Response::Error { error } => Err(io::Error::other(error)),
            _ => Err(unavailable("unexpected cancellation response")),
        }
    }

    /// Execute through the service with no local solver fallback.
    ///
    /// # Errors
    /// Returns admission, connection and protocol failures explicitly.
    pub fn execute(&self, request: &RuntimeRequest) -> io::Result<RuntimeResponse> {
        self.execute_cancellable(request, || false)
    }

    /// Execute while polling caller cancellation; all started I/O workers are joined.
    ///
    /// # Errors
    /// Returns admission, cancellation transport or service failures explicitly.
    pub fn execute_cancellable(
        &self,
        request: &RuntimeRequest,
        cancelled: impl Fn() -> bool,
    ) -> io::Result<RuntimeResponse> {
        if cancelled() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "SAIL6201: runtime request cancelled before dispatch",
            ));
        }
        let capabilities = self.capabilities()?;
        if matches!(request.input, Input::Temporal { .. })
            && !capabilities
                .temporal
                .as_ref()
                .is_some_and(|c| c.implemented)
        {
            return Err(unavailable(
                "running service has no native temporal endpoint",
            ));
        }
        let mut random = [0_u8; 16];
        std::fs::File::open("/dev/urandom")?.read_exact(&mut random)?;
        let id = random.iter().fold(String::new(), |mut id, byte| {
            let _ = write!(id, "{byte:02x}");
            id
        });
        let stream = self.connect()?;
        write_frame(
            &mut Deadline::new(&stream, IO_TIMEOUT),
            &Request::Execute {
                id: id.clone(),
                request: request.clone(),
            },
            MAX_REQUEST_BYTES,
        )?;
        let wait = match &request.input {
            Input::Smt { queries } => queries.iter().try_fold(10_000_u64, |sum, query| {
                sum.checked_add(query.limits.wall_time_ms)
            }),
            Input::Temporal { job } => job.wall_time_ms.checked_add(10_000),
        }
        .filter(|millis| *millis <= 20_000_000)
        .ok_or_else(|| io::Error::other("SAIL6201: request execution deadline is excessive"))?;
        let response = std::thread::scope(|scope| {
            let (transmit, receive) = mpsc::channel();
            let read_stream = &stream;
            let worker = std::thread::Builder::new()
                .name("sail-runtime-client".into())
                .spawn_scoped(scope, move || {
                    let result = read_frame(
                        &mut Deadline::new(read_stream, Duration::from_millis(wait)),
                        MAX_RESPONSE_BYTES,
                    );
                    let _ = transmit.send(result);
                })?;
            let mut cancel_sent = false;
            let result = loop {
                if cancelled() && !cancel_sent {
                    match self.cancel(&id) {
                        Ok(found) => cancel_sent = found,
                        Err(error) => {
                            let _ = stream.shutdown(Shutdown::Both);
                            break Err(error);
                        }
                    }
                }
                match receive.recv_timeout(Duration::from_millis(20)) {
                    Ok(response) => break response,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        break Err(unavailable("response worker stopped"));
                    }
                }
            };
            worker
                .join()
                .map_err(|_| unavailable("response worker panicked"))?;
            result
        })?;
        match response {
            Response::Completed {
                id: actual,
                response,
            } if actual == id && response.version == VERSION => {
                validate_payload(request, &response, &capabilities)?;
                Ok(response)
            }
            Response::Error { error } => Err(io::Error::other(error)),
            _ => Err(unavailable(
                "unexpected execution response identity or version",
            )),
        }
    }
}

pub(super) fn validate_payload(
    request: &RuntimeRequest,
    response: &RuntimeResponse,
    capabilities: &Capabilities,
) -> io::Result<()> {
    use crate::runtime::{Answer, Outcome, Output};
    let (queries, results) = match (&request.input, &response.output) {
        (Input::Temporal { job }, Output::Temporal { result }) => {
            return validate_temporal(job, result, capabilities);
        }
        (Input::Smt { queries }, Output::Smt { results }) => (queries, results),
        _ => return Err(unavailable("runtime response family differs")),
    };
    if results.len() != queries.len() {
        return Err(unavailable("runtime query result count changed"));
    }
    for (query, result) in queries.iter().zip(results) {
        let digest = sail_key::sha256(query.script.as_bytes()).iter().fold(
            String::new(),
            |mut text, byte| {
                let _ = write!(text, "{byte:02x}");
                text
            },
        );
        if result.obligation_id != query.obligation_id
            || result.requirement_ids != query.requirement_ids
            || result.backend != query.backend
            || result.script_sha256 != digest
            || result.limits != query.limits
            || result.stdout.len() > query.limits.output_bytes
            || result.stderr.len() > query.limits.output_bytes
        {
            return Err(unavailable(
                "runtime result changed request attribution, content or limits",
            ));
        }
        if result.outcome == Outcome::Completed {
            let identity = capabilities
                .backends
                .iter()
                .find(|backend| backend.backend == query.backend)
                .and_then(|backend| backend.solver.as_ref());
            if result.checks.is_empty()
                || result.stdout.is_empty()
                || !result.stderr.is_empty()
                || result.error.is_some()
                || identity.is_none()
                || result.solver.as_ref() != identity
                || result
                    .checks
                    .windows(2)
                    .any(|pair| pair[0].command_index >= pair[1].command_index)
                || (result.cache_hit
                    && result
                        .checks
                        .iter()
                        .any(|check| check.answer == Answer::Unknown))
            {
                return Err(unavailable(
                    "completed runtime result lacks valid checks or negotiated solver identity",
                ));
            }
            for (index, check) in result.checks.iter().enumerate() {
                let next = result
                    .checks
                    .get(index + 1)
                    .map_or(usize::MAX, |check| check.command_index);
                if check.products.iter().any(|product| {
                    product.command_index <= check.command_index || product.command_index >= next
                }) || check
                    .products
                    .windows(2)
                    .any(|pair| pair[0].command_index >= pair[1].command_index)
                {
                    return Err(unavailable("runtime products changed check ordering"));
                }
            }
        } else if !result.checks.is_empty() || result.cache_hit || result.error.is_none() {
            return Err(unavailable(
                "failed runtime execution carries successful check evidence",
            ));
        }
    }
    Ok(())
}

fn validate_temporal(
    job: &crate::temporal::Job,
    result: &crate::temporal::Result,
    capabilities: &Capabilities,
) -> io::Result<()> {
    use crate::runtime::Outcome;
    let capability = capabilities
        .temporal
        .as_ref()
        .ok_or_else(|| unavailable("temporal capability missing"))?;
    if result.obligation_id != job.obligation_id
        || result.requirement_ids != job.requirement_ids
        || result.request_sha256
            != crate::temporal::request_sha256(&job.request).map_err(unavailable)?
        || result.backend_revision != capability.backend_revision
        || result.wall_time_ms != job.wall_time_ms
        || result.cache_hit
    {
        return Err(unavailable(
            "temporal result attribution, content or limits differ",
        ));
    }
    if result.outcome == Outcome::Completed {
        let response = result
            .response
            .as_ref()
            .ok_or_else(|| unavailable("completed temporal execution lacks native evidence"))?;
        if !capability.installed
            || result.worker_sha256.is_none()
            || result.worker_sha256 != capability.worker_sha256
            || result.error.is_some()
        {
            return Err(unavailable(
                "temporal result lacks negotiated worker identity",
            ));
        }
        crate::temporal::validate_response(&job.request, response).map_err(unavailable)?;
    } else if result.response.is_some() || result.error.is_none() {
        return Err(unavailable(
            "failed temporal execution supplied logical evidence",
        ));
    }
    Ok(())
}
