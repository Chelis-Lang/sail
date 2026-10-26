//! Cancellable readers for the two original Unix Cargo pipes.
//!
//! Closing Cargo does not guarantee EOF: a descendant may still hold a pipe.
//! Nonblocking reads plus bounded poll let an error path stop and join readers
//! without pretending to own or cancel that descendant.

use super::Output;
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;

pub(super) struct Readers {
    stopped: Arc<AtomicBool>,
    handles: Vec<JoinHandle<()>>,
}
impl Readers {
    pub(super) fn start(
        child: &mut impl super::CargoPipes,
        sender: &Sender<io::Result<Output>>,
    ) -> io::Result<Self> {
        let stdout = child
            .take_stdout()
            .ok_or_else(|| io::Error::other("missing Cargo stdout"))?;
        let stderr = child
            .take_stderr()
            .ok_or_else(|| io::Error::other("missing Cargo stderr"))?;
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        let mut readers = Self {
            stopped: Arc::new(AtomicBool::new(false)),
            handles: Vec::new(),
        };
        readers.handles.push(start(
            stdout,
            false,
            sender.clone(),
            Arc::clone(&readers.stopped),
        )?);
        readers.handles.push(start(
            stderr,
            true,
            sender.clone(),
            Arc::clone(&readers.stopped),
        )?);
        Ok(readers)
    }
    fn join(&mut self) -> io::Result<()> {
        self.stopped.store(true, Ordering::Release);
        let mut result = Ok(());
        for reader in self.handles.drain(..) {
            if reader.join().is_err() {
                result = Err(io::Error::other("Cargo output reader failed"));
            }
        }
        result
    }
    pub(super) fn finish(mut self) -> io::Result<()> {
        self.join()
    }
}
impl Drop for Readers {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

fn nonblocking(pipe: &impl AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(pipe).map_err(io::Error::from)?;
    rustix::fs::fcntl_setfl(pipe, flags | rustix::fs::OFlags::NONBLOCK).map_err(io::Error::from)
}

fn start(
    mut pipe: impl Read + AsFd + Send + 'static,
    diagnostics: bool,
    sender: Sender<io::Result<Output>>,
    stopped: Arc<AtomicBool>,
) -> io::Result<JoinHandle<()>> {
    std::thread::Builder::new()
        .name("sail-cargo-output".into())
        .spawn(move || {
            let result = (|| {
                let mut buffer = vec![0; 65536];
                let mut pending = Vec::new();
                while !stopped.load(Ordering::Acquire) {
                    match pipe.read(&mut buffer) {
                        Ok(0) => {
                            if !pending.is_empty() {
                                let _ = sender.send(Ok(Output::Line(pending)));
                            }
                            return Ok(());
                        }
                        Ok(count) if diagnostics => {
                            if sender
                                .send(Ok(Output::Diagnostics(buffer[..count].to_vec())))
                                .is_err()
                            {
                                return Ok(());
                            }
                        }
                        Ok(count) => {
                            let previous = pending.len();
                            pending.extend_from_slice(&buffer[..count]);
                            let mut begin = 0;
                            for (end, byte) in pending.iter().enumerate().skip(previous) {
                                if *byte == b'\n' {
                                    if sender
                                        .send(Ok(Output::Line(pending[begin..=end].to_vec())))
                                        .is_err()
                                    {
                                        return Ok(());
                                    }
                                    begin = end + 1;
                                }
                            }
                            drop(pending.drain(..begin));
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            let mut descriptors = [rustix::event::PollFd::new(
                                &pipe,
                                rustix::event::PollFlags::IN,
                            )];
                            let timeout = rustix::event::Timespec {
                                tv_sec: 0,
                                tv_nsec: 10_000_000,
                            };
                            match rustix::event::poll(&mut descriptors, Some(&timeout)) {
                                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                                Err(error) => return Err(io::Error::from(error)),
                            }
                        }
                        Err(error) => return Err(error),
                    }
                }
                Ok(())
            })();
            if let Err(error) = result {
                let _ = sender.send(Err(error));
            }
            let _ = sender.send(Ok(Output::Closed));
        })
}
