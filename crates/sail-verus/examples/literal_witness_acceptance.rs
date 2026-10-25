//! Actual pinned-runtime scalar witness acceptance with exact original evidence.
use sail_api::{
    contracts::Submission,
    literal_witness::{Observation, model_query, observe},
    runtime::{Input, Output, Request, Response},
};
use sail_verus::contracts::literal_witness::{prepare, validate};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
#[allow(
    clippy::too_many_lines,
    reason = "Keep the bounded actual-runtime acceptance matrix and its negative controls together."
)]
fn main() -> TestResult {
    let output = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("usage: literal_witness_acceptance FRESH_DIRECTORY")?,
    );
    std::fs::create_dir(&output)?;
    let binary =
        std::env::var_os("SAIL_BIN").ok_or("set SAIL_BIN to the actual built Sail binary")?;
    let jobs = std::env::var_os("SAIL_WITNESS_SOCKET")
        .map(PathBuf::from)
        .map(|socket| sail_api::jobs::Client::new(sail_api::jobs::socket_path(&socket)));
    let mut summary = Vec::new();
    let bool_source = "#[sail_contracts::satisfies(\"FLAG\")] #[sail_contracts::ensures(|ret| ret == value)] pub fn candidate(value:bool)->bool { if value {true} else {false} }";
    let scalar_source = "#[sail_contracts::satisfies(\"ORD-SCALAR-001\")] #[sail_contracts::ensures(|ret| (!(amount < 255)) || ret == amount + 1)] pub fn increment_amount(amount:u8)->u8 { if amount < 255 { amount + 1 } else { amount } }";
    let cases = [
        ("bool", bool_source, 1, 2, 0),
        ("scalar", scalar_source, 1, 2, 0),
        (
            "signed",
            "#[sail_contracts::satisfies(\"SIGNED\")] #[sail_contracts::requires(x < 0)] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:i128)->i128 {x}",
            1,
            2,
            0,
        ),
        (
            "wide",
            "#[sail_contracts::satisfies(\"WIDE\")] #[sail_contracts::requires(x > 1)] #[sail_contracts::ensures(|ret| ret == x)] fn candidate(x:u128)->u128 {x}",
            1,
            2,
            0,
        ),
        (
            "multiply",
            "#[sail_contracts::satisfies(\"MUL\")] #[sail_contracts::requires(x < 10)] #[sail_contracts::ensures(|ret| ret == x * x)] fn candidate(x:u8)->u8 {x * x}",
            1,
            2,
            0,
        ),
        (
            "conditional",
            "#[sail_contracts::satisfies(\"COND\")] #[sail_contracts::ensures(|ret| ret == (if x < 10 {x + 1} else {x}))] fn candidate(x:u8)->u8 {if x < 10 {x + 1} else {x}}",
            1,
            2,
            0,
        ),
        (
            "old",
            "#[sail_contracts::satisfies(\"OLD\")] #[sail_contracts::requires(x < 255)] #[sail_contracts::ensures(|ret| ret == old(x + 1))] fn candidate(x:u8)->u8 {x + 1}",
            1,
            2,
            0,
        ),
        (
            "order-service",
            include_str!("../tests/fixtures/neutral-contracts/order_service_profile2_consumer.rs"),
            2,
            2,
            0,
        ),
        (
            "tautology",
            "#[sail_contracts::satisfies(\"TRUE\")] #[sail_contracts::ensures(|ret| true)] fn candidate(value:bool)->bool {value}",
            1,
            0,
            2,
        ),
        (
            "infeasible",
            "#[sail_contracts::satisfies(\"FALSE\")] #[sail_contracts::requires(false)] #[sail_contracts::ensures(|ret| ret)] fn candidate(value:bool)->bool {value}",
            1,
            0,
            2,
        ),
        (
            "undefined-post",
            "#[sail_contracts::satisfies(\"OVERFLOW\")] #[sail_contracts::requires(x == 255)] #[sail_contracts::ensures(|ret| ret == x + 1)] fn candidate(x:u8)->u8 {x}",
            1,
            0,
            2,
        ),
        (
            "undefined-old",
            "#[sail_contracts::satisfies(\"OLD-OVERFLOW\")] #[sail_contracts::requires(x == 255)] #[sail_contracts::ensures(|ret| ret == old(x + 1))] fn candidate(x:u8)->u8 {x}",
            1,
            0,
            2,
        ),
    ];
    for (name, source, profile, expected_sat, expected_unsat) in cases {
        let dir = output.join(name);
        std::fs::create_dir(&dir)?;
        let input = Submission {
            profile,
            source: source.into(),
            source_sha256: sail_api::mutation::digest(source.as_bytes()),
            crate_name: "neutral".into(),
        };
        let plan = if let Some(jobs) = &jobs {
            jobs.literal_witnesses(&input, 8)?
        } else {
            prepare(&input, 8)?
        };
        plan.validate()?;
        write(&dir.join("plan.json"), &plan)?;
        let queries = plan.prepared.iter().map(|p| p.query.clone()).collect();
        let response = execute(Path::new(&binary), &dir, "check", queries)?;
        let Output::Smt { results } = response.output else {
            return Err("unexpected runtime response".into());
        };
        if results.len() != plan.prepared.len() {
            return Err("missing original query results".into());
        }
        let mut models = Vec::new();
        let mut sat = 0;
        let mut unsat = 0;
        for (prepared, result) in plan.prepared.iter().zip(&results) {
            match observe(prepared, result) {
                Observation::SatisfiableContractQuery => {
                    sat += 1;
                    models.push(model_query(prepared));
                }
                Observation::NoWitness => unsat += 1,
                Observation::Inconclusive => {
                    return Err(format!("unexpected incomplete query: {name}").into());
                }
            }
        }
        if sat != expected_sat || unsat != expected_unsat {
            return Err(format!("unexpected outcomes {name}: {sat} SAT/{unsat} UNSAT").into());
        }
        let mut concrete = Vec::new();
        if !models.is_empty() {
            let response = execute(Path::new(&binary), &dir, "model", models)?;
            let Output::Smt { results } = response.output else {
                return Err("unexpected model response".into());
            };
            if results.len() != sat {
                return Err("missing model results".into());
            }
            for result in results {
                let prepared = plan
                    .prepared
                    .iter()
                    .find(|p| model_query(p).obligation_id == result.obligation_id)
                    .ok_or("unknown model identity")?;
                concrete.push(if let Some(jobs) = &jobs {
                    jobs.validate_literal_witness(
                        plan.clone(),
                        prepared.candidate.id.clone(),
                        &result,
                    )?
                } else {
                    validate(&plan, &prepared.candidate.id, &result)?
                });
                let mut unknown = result.clone();
                unknown.checks[0].answer = sail_api::runtime::Answer::Unknown;
                if validate(&plan, &prepared.candidate.id, &unknown).is_ok() {
                    return Err("UNKNOWN model was accepted".into());
                }
                let mut changed = plan.clone();
                changed.prepared[0].candidate.source.source.push(' ');
                if validate(&changed, &prepared.candidate.id, &result).is_ok() {
                    return Err("changed source was accepted".into());
                }
            }
        }
        write(&dir.join("validated.json"), &concrete)?;
        summary.push(serde_json::json!({"case":name,"profile":profile,"source_sha256":input.source_sha256,"sat":sat,"unsat":unsat,"concrete_validated":concrete.len(),"unsupported":plan.unsupported,"omitted":plan.omitted}));
    }
    write(
        &output.join("summary.json"),
        &serde_json::json!({"scope":"actual exact-source scalar witness queries and independent concrete model evaluation; no native Verus kill or proof-tier promotion","cases":summary}),
    )?;
    Ok(())
}
fn write<T: serde::Serialize>(path: &Path, value: &T) -> TestResult {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn execute(
    binary: &Path,
    directory: &Path,
    label: &str,
    queries: Vec<sail_api::runtime::Query>,
) -> TestResult<Response> {
    let request = Request {
        version: sail_api::runtime::VERSION,
        input: Input::Smt { queries },
    };
    let path = directory.join(format!("{label}-request.json"));
    write(&path, &request)?;
    let mut command = Command::new(binary);
    command.args(["runtime", "execute", "--file"]).arg(&path);
    if let Some(socket) = std::env::var_os("SAIL_WITNESS_SOCKET") {
        command.arg("--socket").arg(socket);
    } else {
        command.arg("--cache").arg(directory.join("runtime-cache"));
    }
    let result = command.output()?;
    std::fs::write(
        directory.join(format!("{label}-stdout.json")),
        &result.stdout,
    )?;
    std::fs::write(
        directory.join(format!("{label}-stderr.txt")),
        &result.stderr,
    )?;
    if !result.status.success() {
        return Err(format!(
            "Sail runtime execution failed: {label}, {:?}",
            result.status.code()
        )
        .into());
    }
    Ok(serde_json::from_slice(&result.stdout)?)
}
