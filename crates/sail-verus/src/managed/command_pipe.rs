//! Private actual child-pipe readers and immutable EOF readbacks.
//!
//! Only the owned command module can construct a reader. Compiler callers
//! cannot obtain its raw reader/descriptor or supply a replacement byte vector.

use super::{LaunchLease, NativeOwnerIdentity};
use sail_acceptance::stream_capture::{
    CompilerPipeReadHost, PipeCapture, PipeReadFailure, PipeReadback, begin_pipe_capture,
    capture_pipe_read, finish_pipe_capture,
};
use std::io::{self, Read};
use std::sync::Arc;

pub(crate) struct PipeOutput {
    readback: PipeReadback<LaunchLease>,
}

pub(crate) struct PipeReader<Reader> {
    host: NativePipeHost<Reader>,
    capture: PipeCapture<LaunchLease>,
}

struct NativePipeHost<Reader> {
    reader: Reader,
    owner: LaunchLease,
    kind: u8,
}

impl<Reader: Read> CompilerPipeReadHost<LaunchLease, io::Error> for NativePipeHost<Reader> {
    fn owner(&self) -> &LaunchLease {
        &self.owner
    }
    fn kind(&self) -> u8 {
        self.kind
    }
    fn fault_kind(&self, error: &io::Error) -> u8 {
        match error.kind() {
            io::ErrorKind::Interrupted => 1,
            io::ErrorKind::WouldBlock => 2,
            _ => 0,
        }
    }
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buffer)
    }
}

impl PipeOutput {
    pub(super) fn into_readback(self) -> PipeReadback<LaunchLease> {
        self.readback
    }

    fn missing() -> Self {
        // No endpoint means no observed EOF. Empty fresh output is explicitly
        // unavailable and can never satisfy the shared stream admission rule.
        Self {
            readback: PipeReadback::Unavailable(Vec::new()),
        }
    }
}

impl<Reader: Read> PipeReader<Reader> {
    pub(super) fn new(reader: Reader, owner: Arc<()>, kind: u8) -> Self {
        Self {
            host: NativePipeHost {
                reader,
                owner: LaunchLease {
                    owner: Arc::clone(&owner),
                },
                kind,
            },
            capture: begin_pipe_capture(LaunchLease { owner }, kind),
        }
    }

    pub(crate) fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        capture_pipe_read(
            &NativeOwnerIdentity,
            &mut self.host,
            &mut self.capture,
            buffer,
        )
        .map_err(|failure| match failure {
            PipeReadFailure::Read(error) => error,
            PipeReadFailure::Capacity => {
                io::Error::other("compiler pipe capture exceeds addressable storage")
            }
        })
    }

    pub(crate) fn finish(self) -> PipeOutput {
        PipeOutput {
            readback: finish_pipe_capture(self.capture),
        }
    }

    pub(crate) fn read_all(mut self) -> io::Result<PipeOutput> {
        let mut buffer = vec![0; 65536];
        loop {
            match self.read(&mut buffer) {
                Ok(0) => return Ok(self.finish()),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
    }
}

#[cfg(unix)]
impl<Reader: Read + std::os::fd::AsFd> PipeReader<Reader> {
    fn nonblocking(&self) -> io::Result<()> {
        let flags = rustix::fs::fcntl_getfl(&self.host.reader).map_err(io::Error::from)?;
        rustix::fs::fcntl_setfl(&self.host.reader, flags | rustix::fs::OFlags::NONBLOCK)
            .map_err(io::Error::from)
    }
}

/// Drain both actual endpoints concurrently without treating process exit or
/// poll readiness/hangup as EOF. Normal operation has no cancellation timeout.
#[cfg(unix)]
pub(super) fn read_pair(
    mut stdout: Option<PipeReader<std::process::ChildStdout>>,
    mut stderr: Option<PipeReader<std::process::ChildStderr>>,
) -> io::Result<(PipeOutput, PipeOutput)> {
    if let Some(pipe) = &stdout {
        pipe.nonblocking()?;
    }
    if let Some(pipe) = &stderr {
        pipe.nonblocking()?;
    }
    let mut stdout_eof = stdout.is_none();
    let mut stderr_eof = stderr.is_none();
    let mut stdout_buffer = vec![0; 65536];
    let mut stderr_buffer = vec![0; 65536];
    while !stdout_eof || !stderr_eof {
        let mut progressed = false;
        if !stdout_eof && let Some(pipe) = &mut stdout {
            match pipe.read(&mut stdout_buffer) {
                Ok(0) => {
                    stdout_eof = true;
                    progressed = true;
                }
                Ok(_) => progressed = true,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => progressed = true,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        if !stderr_eof && let Some(pipe) = &mut stderr {
            match pipe.read(&mut stderr_buffer) {
                Ok(0) => {
                    stderr_eof = true;
                    progressed = true;
                }
                Ok(_) => progressed = true,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => progressed = true,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
        }
        if !progressed {
            let mut descriptors = Vec::new();
            if !stdout_eof && let Some(pipe) = &stdout {
                descriptors.push(rustix::event::PollFd::new(
                    &pipe.host.reader,
                    rustix::event::PollFlags::IN,
                ));
            }
            if !stderr_eof && let Some(pipe) = &stderr {
                descriptors.push(rustix::event::PollFd::new(
                    &pipe.host.reader,
                    rustix::event::PollFlags::IN,
                ));
            }
            if descriptors.is_empty() {
                return Err(io::Error::other(
                    "compiler pipe drain lost an active endpoint",
                ));
            }
            match rustix::event::poll(&mut descriptors, None) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(io::Error::from(error)),
            }
        }
    }
    Ok((
        stdout.map_or_else(PipeOutput::missing, PipeReader::finish),
        stderr.map_or_else(PipeOutput::missing, PipeReader::finish),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_acceptance::stream_capture::pipe_readback_matches;
    use std::io::Cursor;

    fn matches(output: &PipeOutput, owner: &Arc<()>, kind: u8) -> bool {
        pipe_readback_matches(
            &NativeOwnerIdentity,
            &output.readback,
            &LaunchLease {
                owner: Arc::clone(owner),
            },
            kind,
        )
    }

    #[test]
    fn actual_prefixes_and_eof_keep_byte_order() -> io::Result<()> {
        let owner = Arc::new(());
        let mut reader = PipeReader::new(Cursor::new(b"AB"), Arc::clone(&owner), 0);
        let mut byte = [0u8];
        assert_eq!(reader.read(&mut byte)?, 1);
        assert_eq!(byte, [b'A']);
        assert_eq!(reader.read(&mut byte)?, 1);
        assert_eq!(byte, [b'B']);
        assert_eq!(reader.read(&mut byte)?, 0);
        let output = reader.finish();
        assert_eq!(output.readback.bytes(), b"AB");
        assert!(matches(&output, &owner, 0));
        Ok(())
    }

    #[test]
    fn missing_eof_keeps_actual_prefix_without_authority() -> io::Result<()> {
        let owner = Arc::new(());
        let mut reader = PipeReader::new(Cursor::new(b"AB"), Arc::clone(&owner), 0);
        let mut byte = [0u8];
        assert_eq!(reader.read(&mut byte)?, 1);
        let output = reader.finish();
        assert_eq!(output.readback.bytes(), b"A");
        assert!(!matches(&output, &owner, 0));
        Ok(())
    }

    #[test]
    fn zero_capacity_read_cannot_supply_eof() -> io::Result<()> {
        let owner = Arc::new(());
        let mut reader = PipeReader::new(Cursor::new(b""), Arc::clone(&owner), 1);
        assert_eq!(reader.read(&mut [])?, 0);
        let output = reader.finish();
        assert!(!matches(&output, &owner, 1));
        Ok(())
    }

    #[test]
    fn equal_bytes_from_other_owner_or_role_refuse() -> io::Result<()> {
        let owner = Arc::new(());
        let other = Arc::new(());
        let foreign = PipeReader::new(Cursor::new(b"A"), other, 0).read_all()?;
        let stderr = PipeReader::new(Cursor::new(b"A"), Arc::clone(&owner), 1).read_all()?;
        assert_eq!(foreign.readback.bytes(), stderr.readback.bytes());
        assert!(!matches(&foreign, &owner, 0));
        assert!(!matches(&stderr, &owner, 0));
        assert!(matches(&stderr, &owner, 1));
        Ok(())
    }
}
