//! SPIKE (v0.3.0 Track D, #696/#683): submit distinct one-function neutral
//! source jobs in sequence to a running `sail serve` and print one JSON line
//! per job with the backend currentness, notes and check counts.
//!
//! Usage: `spike_neutral_loop JOBS_SOCKET COUNT [PROFILE] [START]`
use sail_api::contracts::Submission;
use sail_api::jobs::{Client, Request, Response};
use std::time::{Duration, Instant};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn sha(bytes: &[u8]) -> String {
    sail_key::QueryKey(sail_key::sha256(bytes)).to_string()
}

fn source(index: usize) -> String {
    let bound = 1 + index % 250;
    format!(
        "#[sail_contracts::requirement(\"ADD-ONE-{index}\")]\n\
pub fn next_value(before: u8, after: u8) -> bool {{ after == before + 1 }}\n\n\
#[sail_contracts::requires(input < {bound})]\n\
#[sail_contracts::ensures(|result| next_value(input, result))]\n\
#[sail_contracts::satisfies(\"ADD-ONE-{index}\")]\n\
pub fn candidate(input: u8) -> u8 {{\n    if input == 0 {{ return 1; }}\n    input + 1\n}}\n"
    )
}

fn main() -> TestResult {
    let args: Vec<String> = std::env::args().collect();
    let socket = args.get(1).ok_or("JOBS_SOCKET")?;
    let count: usize = args.get(2).ok_or("COUNT")?.parse()?;
    let profile: u32 = args.get(3).map_or(Ok(1), |p| p.parse())?;
    let start: usize = args.get(4).map_or(Ok(0), |p| p.parse())?;
    // The daemon socket, as given to `sail serve --socket`.
    let client = Client::new(sail_api::jobs::socket_path(std::path::Path::new(socket)));
    for index in start..start + count {
        let text = source(index);
        let began = Instant::now();
        let job = client.submit_contracts(Submission {
            profile,
            source: text.clone(),
            source_sha256: sha(text.as_bytes()),
            crate_name: format!("neutral_{index}"),
            modules: Vec::new(),
        })?;
        let deadline = Instant::now() + Duration::from_secs(900);
        let state = loop {
            let Response::Status(status) = client.call(&Request::Status { id: job.id.clone() })?
            else {
                return Err("wrong status response".into());
            };
            if status.state.terminal() {
                break status;
            }
            if Instant::now() > deadline {
                return Err("deadline".into());
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let document = match client.call(&Request::Result { id: job.id.clone() }) {
            Ok(Response::Result(document)) => Some(document),
            _ => None,
        };
        let backend = document.as_ref().and_then(|d| d.result.clone());
        let proof = document
            .as_ref()
            .and_then(|d| d.contracts.as_deref())
            .map(|e| serde_json::to_value(&e.proof))
            .transpose()?;
        let targets: Vec<serde_json::Value> = backend
            .as_ref()
            .and_then(|b| b["targets"].as_array())
            .into_iter()
            .flatten()
            .map(|t| {
                serde_json::json!({
                    "package_id": t["package_id"],
                    "reused": t["reused"],
                    "complete": t["complete"],
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::json!({
                "index": index,
                "job": job.id,
                "wall_ms": began.elapsed().as_millis().to_string(),
                "state": format!("{:?}", state.state),
                "error": state.error,
                "backend_currentness": backend.as_ref().map(|b| b["currentness"].clone()),
                "notes": backend.as_ref().map(|b| b["notes"].clone()),
                "proof_state": proof.as_ref().map(|p| p["state"].clone()),
                "checks": proof.as_ref().map(|p| p["checks"].clone()),
                "targets": targets,
            })
        );
    }
    Ok(())
}
