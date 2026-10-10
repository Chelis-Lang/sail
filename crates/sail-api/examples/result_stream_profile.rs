//! Offline full-frame transfer/decoding comparison, never a live-reader acceptance.
#![forbid(unsafe_code)]

use std::fmt::Write as _;
use std::io::{self, BufReader, Cursor, Read, Write};
use std::sync::{Arc, Barrier};
use std::time::Instant;

use sail_api::jobs::{MAX_FRAME, Response, VERSION, read_frame};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
struct Envelope<T> {
    version: u32,
    body: T,
}

// Candidate stays in this profiling tool until measured evidence supports a
// production change. Take precedes buffering, so no next-frame bytes are read.
fn streaming<T: serde::de::DeserializeOwned>(reader: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    reader.read_exact(&mut length)?;
    let size = u32::from_be_bytes(length);
    if u64::from(size) > MAX_FRAME as u64 {
        return Err(io::Error::other("SAIL0801: job frame exceeds limit"));
    }
    let mut bounded = BufReader::with_capacity(64 * 1024, reader.take(u64::from(size)));
    let envelope: Envelope<T> = serde_json::from_reader(&mut bounded).map_err(io::Error::other)?;
    if bounded.into_inner().limit() != 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete job frame",
        ));
    }
    if envelope.version != VERSION {
        return Err(io::Error::other("SAIL0802: incompatible job protocol"));
    }
    Ok(envelope.body)
}

#[derive(Clone, Copy)]
enum Decoder {
    Slice,
    Stream,
}
impl Decoder {
    fn read(self, reader: &mut impl Read) -> io::Result<Response> {
        match self {
            Self::Slice => read_frame(reader),
            Self::Stream => streaming(reader),
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::Slice => "current_slice",
            Self::Stream => "candidate_stream",
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [path] = args.as_slice() else {
        return Err("usage: result_stream_profile CAPTURED_FRAME".into());
    };
    let mut frame = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_FRAME as u64 + 5)
        .read_to_end(&mut frame)?;
    let mut cursor = Cursor::new(&frame);
    let expected: Value = read_frame(&mut cursor)?;
    if cursor.position() != frame.len() as u64 {
        return Err("extra bytes after captured frame".into());
    }
    let frame = Arc::new(frame);
    let mut results = serde_json::Map::new();
    for threads in [1, 4] {
        for socket in [false, true] {
            for decoder in [Decoder::Slice, Decoder::Stream] {
                results.insert(
                    format!(
                        "{}_{}_{}",
                        decoder.name(),
                        if socket { "socket" } else { "cursor" },
                        threads
                    ),
                    measure(decoder, socket, threads, &frame, &expected)?,
                );
            }
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "kind":"offline_full_frame_streaming_comparison", "live_acceptance":false,
            "frame_bytes":frame.len(), "frame_sha256":sail_key::sha256(&frame).iter().fold(String::new(), |mut digest, byte| { let _ = write!(digest, "{byte:02x}"); digest }),
            "samples_per_case":8, "complete_graph_equality_checked":true,
            "unchanged_wire_bytes":true, "results":results,
        }))?
    );
    Ok(())
}

fn measure(
    decoder: Decoder,
    socket: bool,
    threads: usize,
    frame: &Arc<Vec<u8>>,
    expected: &Value,
) -> io::Result<Value> {
    let mut samples = Vec::new();
    for _ in 0..(8 / threads) {
        let start = Barrier::new(threads);
        let wave = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        if socket {
                            socket_read(decoder, frame)
                        } else {
                            let begin = Instant::now();
                            let output = decoder.read(&mut Cursor::new(frame.as_slice()))?;
                            Ok((begin.elapsed().as_secs_f64() * 1000.0, output))
                        }
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle
                        .join()
                        .map_err(|_| io::Error::other("profile thread failed"))?
                })
                .collect::<io::Result<Vec<_>>>()
        })?;
        // Validation and graph destruction occur outside every measured wave.
        for (elapsed, output) in wave {
            if serde_json::to_value(output).map_err(io::Error::other)? != *expected {
                return Err(io::Error::other("full decoded graph changed"));
            }
            samples.push(elapsed);
        }
    }
    samples.sort_by(f64::total_cmp);
    Ok(
        json!({"threads":threads,"samples_ms":samples,"median_ms":samples[3].midpoint(samples[4]),"p95_ms":samples[7]}),
    )
}

#[cfg(unix)]
fn socket_read(decoder: Decoder, frame: &[u8]) -> io::Result<(f64, Response)> {
    use std::os::unix::net::UnixStream;
    let (mut reader, mut writer) = UnixStream::pair()?;
    reader.set_read_timeout(Some(std::time::Duration::from_secs(5)))?;
    writer.set_write_timeout(Some(std::time::Duration::from_secs(5)))?;
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        let producer = scope.spawn(|| {
            barrier.wait();
            writer.write_all(frame)
        });
        barrier.wait();
        let begin = Instant::now();
        let output = decoder.read(&mut reader);
        let elapsed = begin.elapsed().as_secs_f64() * 1000.0;
        drop(reader);
        producer
            .join()
            .map_err(|_| io::Error::other("producer failed"))??;
        output.map(|value| (elapsed, value))
    })
}

#[cfg(not(unix))]
fn socket_read(_: Decoder, _: &[u8]) -> io::Result<(f64, Response)> {
    Err(io::Error::other(
        "Unix sockets required for this local profile",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn framed(bytes: &[u8]) -> io::Result<Vec<u8>> {
        let mut frame = u32::try_from(bytes.len())
            .map_err(io::Error::other)?
            .to_be_bytes()
            .to_vec();
        frame.extend_from_slice(bytes);
        Ok(frame)
    }

    #[test]
    fn complete_graph_and_next_frame_are_preserved() -> io::Result<()> {
        let first = framed(
            br#"{ "body": { "value": "SAIL0000: example", "response": "error" }, "version": 1 }"#,
        )?;
        let mut pair = first.clone();
        pair.extend_from_slice(&first);
        let mut cursor = Cursor::new(&pair);
        let value: Value = streaming(&mut cursor)?;
        assert_eq!(cursor.position(), first.len() as u64);
        let expected: Value = read_frame(&mut Cursor::new(&first))?;
        assert_eq!(value, expected);
        assert_eq!(streaming::<Value>(&mut cursor)?, expected);
        Ok(())
    }

    #[test]
    fn bounds_versions_truncation_and_malformed_data_remain_errors() -> io::Result<()> {
        for bytes in [
            b"{}".as_slice(),
            b"{\"version\":2,\"body\":null}",
            b"{\"version\":1,\"body\":null} trailing",
            b"{\"version\":1,\"body\":null,\"version\":1}",
        ] {
            assert!(streaming::<Value>(&mut Cursor::new(framed(bytes)?)).is_err());
        }
        let mut truncated = framed(b"{\"version\":1,\"body\":null} ")?;
        truncated.pop(); // valid JSON still refuses a missing advertised byte
        assert!(streaming::<Value>(&mut Cursor::new(truncated)).is_err());
        let oversized = u32::try_from(MAX_FRAME + 1)
            .map_err(io::Error::other)?
            .to_be_bytes();
        assert!(streaming::<Value>(&mut Cursor::new(oversized)).is_err());
        Ok(())
    }
}
