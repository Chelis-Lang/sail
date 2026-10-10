//! Offline whole-result profiling; this never validates a live proof or freshness.
#![forbid(unsafe_code)]

use std::hint::black_box;
use std::io::{self, Cursor, Read};
use std::sync::Arc;
use std::time::Instant;

use sail_api::jobs::{MAX_FRAME, Response, read_frame, write_frame};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (path, framed) = match args.as_slice() {
        [path] => (path, false),
        [flag, path] if flag == "--frame" => (path, true),
        _ => return Err("usage: result_frame_profile [--frame] RESPONSE_FILE".into()),
    };
    let mut input = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_FRAME as u64) + 5)
        .read_to_end(&mut input)?;
    let (body, expected) = decode_input(&input, framed)?;
    let mut encoded = Vec::new();
    write_frame(&mut encoded, &body)?;
    let captured_equals_reencoded = framed.then(|| input == encoded);
    // Decode costs must use the captured bytes, including their member order.
    // Only an unframed JSON input needs a generated protocol envelope.
    let frame = Arc::new(if framed { input } else { encoded });
    let mut results = serde_json::Map::new();
    for threads in [1, 4] {
        for stage in [
            Stage::Typed,
            Stage::Value,
            Stage::Clone,
            Stage::Encode,
            Stage::Cached,
        ] {
            let measurements = measure(stage, threads, &frame, &body, &expected)?;
            results.insert(format!("{}_{}", stage.name(), threads), measurements);
        }
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "kind": "offline_whole_result_rust_profile",
            "frame_bytes": frame.len(), "samples": 8,
            "captured_frame_used_for_decode": framed,
            "captured_frame_equals_typed_reencoding": captured_equals_reencoded,
            "complete_graph_equality_checked": true, "live_acceptance": false,
            "results": results,
        }))?
    );
    Ok(())
}

fn decode_input(input: &[u8], framed: bool) -> io::Result<(Response, Value)> {
    if input.len() > MAX_FRAME + if framed { 4 } else { 0 } {
        return Err(io::Error::other("profile input exceeds the frame limit"));
    }
    let (body, expected) = if framed {
        let mut cursor = Cursor::new(input);
        let body = read_frame(&mut cursor)?;
        if cursor.position() != input.len() as u64 {
            return Err(io::Error::other("extra bytes after captured frame"));
        }
        (body, read_frame(&mut Cursor::new(input))?)
    } else {
        (
            serde_json::from_slice::<Response>(input).map_err(io::Error::other)?,
            serde_json::from_slice::<Value>(input).map_err(io::Error::other)?,
        )
    };
    if serde_json::to_value(&body).map_err(io::Error::other)? != expected {
        return Err(io::Error::other(
            "typed response did not preserve the original whole graph",
        ));
    }
    Ok((body, expected))
}

#[derive(Clone, Copy)]
enum Stage {
    Typed,
    Value,
    Clone,
    Encode,
    Cached,
}
impl Stage {
    fn name(self) -> &'static str {
        match self {
            Self::Typed => "typed_read_frame",
            Self::Value => "value_read_frame",
            Self::Clone => "response_clone",
            Self::Encode => "write_frame",
            Self::Cached => "cached_arc",
        }
    }
    fn run(self, frame: &Arc<Vec<u8>>, body: &Response) -> io::Result<Output> {
        match self {
            Self::Typed => read_frame(&mut Cursor::new(frame.as_slice())).map(Output::Typed),
            Self::Value => read_frame(&mut Cursor::new(frame.as_slice())).map(Output::Value),
            Self::Clone => Ok(Output::Typed(body.clone())),
            Self::Encode => {
                let mut out = Vec::new();
                write_frame(&mut out, body)?;
                Ok(Output::Frame(out))
            }
            Self::Cached => Ok(Output::Cached(Arc::clone(frame))),
        }
    }
}

enum Output {
    Typed(Response),
    Value(Value),
    Frame(Vec<u8>),
    Cached(Arc<Vec<u8>>),
}
impl Output {
    fn validate(&self, frame: &[u8], expected: &Value) -> io::Result<()> {
        let equal = match self {
            Self::Typed(value) => {
                serde_json::to_value(value).map_err(io::Error::other)? == *expected
            }
            Self::Value(value) => value == expected,
            Self::Frame(value) => {
                let decoded: Value = read_frame(&mut Cursor::new(value))?;
                decoded == *expected
            }
            Self::Cached(value) => value.as_slice() == frame,
        };
        if equal {
            Ok(())
        } else {
            Err(io::Error::other("whole result changed"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_member_order_is_preserved_and_encoding_keeps_the_whole_graph() -> io::Result<()> {
        let payload =
            br#"{ "body": { "value": "SAIL0000: example", "response": "error" }, "version": 1 }"#;
        let mut frame = u32::try_from(payload.len())
            .map_err(io::Error::other)?
            .to_be_bytes()
            .to_vec();
        frame.extend_from_slice(payload);
        let (body, expected) = decode_input(&frame, true)?;
        let captured = Arc::new(frame);
        Stage::Typed
            .run(&captured, &body)?
            .validate(&captured, &expected)?;
        Stage::Encode
            .run(&captured, &body)?
            .validate(&captured, &expected)?;
        let mut encoded = Vec::new();
        write_frame(&mut encoded, &body)?;
        assert_ne!(captured.as_slice(), encoded);
        Ok(())
    }

    #[test]
    fn extra_frames_and_incomplete_or_oversized_input_are_refused() -> io::Result<()> {
        let mut frame = Vec::new();
        write_frame(&mut frame, &Response::Error("SAIL0000: example".into()))?;
        assert!(decode_input(&frame[..frame.len() - 1], true).is_err());
        let mut extra = frame.clone();
        extra.extend_from_slice(&frame);
        assert!(decode_input(&extra, true).is_err());
        extra.truncate(frame.len());
        extra.push(0);
        assert!(decode_input(&extra, true).is_err());
        let oversized = u32::try_from(MAX_FRAME + 1)
            .map_err(io::Error::other)?
            .to_be_bytes();
        assert!(decode_input(&oversized, true).is_err());
        Ok(())
    }
}

fn measure(
    stage: Stage,
    threads: usize,
    frame: &Arc<Vec<u8>>,
    body: &Response,
    expected: &Value,
) -> io::Result<Value> {
    let mut elapsed = Vec::new();
    for _ in 0..(8 / threads) {
        let wave = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|_| {
                    scope.spawn(|| {
                        let start = Instant::now();
                        let output = black_box(stage.run(frame, body))?;
                        Ok::<_, io::Error>((start.elapsed().as_secs_f64() * 1000.0, output))
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
        // Whole-graph checks happen after the measured operation and after every wave.
        for (time, output) in wave {
            output.validate(frame, expected)?;
            elapsed.push(time);
        }
    }
    elapsed.sort_by(f64::total_cmp);
    Ok(json!({"threads":threads, "samples":elapsed.len(),
        "median_ms":elapsed[3].midpoint(elapsed[4]),
        "p95_ms":elapsed[7], "max_ms":elapsed[7]}))
}
