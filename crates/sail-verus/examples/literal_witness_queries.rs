//! Prepare exact opt-in witness queries; execute request.json with Sail's runtime.
use std::path::PathBuf;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let source = PathBuf::from(
        args.next()
            .ok_or("usage: literal_witness_queries SUBMISSION_JSON FRESH_DIRECTORY")?,
    );
    let output = PathBuf::from(args.next().ok_or("missing fresh output directory")?);
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }
    let input: sail_api::contracts::Submission = serde_json::from_slice(&std::fs::read(source)?)?;
    let plan = sail_verus::contracts::literal_witness::prepare(&input, 8)?;
    plan.validate()?;
    std::fs::create_dir(&output)?;
    std::fs::write(output.join("plan.json"), serde_json::to_vec_pretty(&plan)?)?;
    let request = sail_api::runtime::Request {
        version: sail_api::runtime::VERSION,
        input: sail_api::runtime::Input::Smt {
            queries: plan.prepared.iter().map(|p| p.query.clone()).collect(),
        },
    };
    std::fs::write(
        output.join("request.json"),
        serde_json::to_vec_pretty(&request)?,
    )?;
    Ok(())
}
