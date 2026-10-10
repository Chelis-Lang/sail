//! Offline whole-result profiling; this never validates a live proof or freshness.

use std::hint::black_box;
use std::io::{self, Cursor};
use std::sync::Arc;
use std::time::Instant;

use sail_api::jobs::{Response, read_frame, write_frame};
use serde_json::{Value, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let (input, framed) = match args.as_slice() {
        [path] => (std::fs::read(path)?, false),
        [flag, path] if flag == "--frame" => (std::fs::read(path)?, true),
        _ => return Err("usage: result_frame_profile [--frame] RESPONSE_FILE".into()),
    };
    let (body, expected) = if framed {
        (
            read_frame(&mut Cursor::new(&input))?,
            read_frame(&mut Cursor::new(&input))?,
        )
    } else {
        (
            serde_json::from_slice::<Response>(&input)?,
            serde_json::from_slice::<Value>(&input)?,
        )
    };
    if serde_json::to_value(&body)? != expected {
        return Err("typed response did not preserve the original whole graph".into());
    }
    let mut encoded = Vec::new();
    write_frame(&mut encoded, &body)?;
    let frame = Arc::new(encoded);
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
            "complete_graph_equality_checked": true, "live_acceptance": false,
            "results": results,
        }))?
    );
    Ok(())
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
            Self::Frame(value) => value == frame,
            Self::Cached(value) => value.as_slice() == frame,
        };
        if equal {
            Ok(())
        } else {
            Err(io::Error::other("whole result changed"))
        }
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
