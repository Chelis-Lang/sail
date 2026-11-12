//! Frozen-response public-dispatch controls, not fresh verification evidence.
#![forbid(unsafe_code)]

use std::os::unix::fs::PermissionsExt as _;
use std::sync::atomic::{AtomicU64, Ordering};

use sail_mcp::tools::{Config, Context};
use serde_json::{Value, json};

static NEXT: AtomicU64 = AtomicU64::new(0);

#[allow(
    clippy::default_trait_access,
    reason = "The public dispatch accepts a cancellation type whose module is private; this external consumer must infer its Default."
)]
fn call(document: &Value, arguments: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "sail-health-view-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root)?;
    let executable = root.join("sail-response");
    std::fs::write(
        &executable,
        format!("#!/bin/sh\ncat <<'SAIL_HEALTH_RESPONSE'\n{document}\nSAIL_HEALTH_RESPONSE\n"),
    )?;
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))?;
    let context = Context::new(Config {
        sail: executable,
        store_dir: None,
        cwd: root.clone(),
    });
    let response = sail_mcp::tools::call(
        &context,
        "proof_health",
        arguments.as_object().ok_or("missing arguments")?,
        &Default::default(),
    )
    .ok_or("missing health tool")?
    .into_json(true);
    std::fs::remove_dir_all(root)?;
    Ok(response)
}

#[test]
fn immutable_baselines_refuse_current_unknown_and_missing_origins_through_public_dispatch()
-> Result<(), Box<dyn std::error::Error>> {
    let mut document = json!({"kind":"sail-proof-health","version":1,"job_id":"job",
        "run_id":"run","workspace":"workspace","context_sha256":"context",
        "input_revision":"revision","currentness":"current","exit_code":0,
        "total_functions":0,"omitted":0,"entries":[],"error":null});
    let mut baseline = document.clone();
    baseline["job_id"] = json!("original-job");
    baseline["currentness"] = json!("historical");
    document["baseline_name"] = json!("saved");
    document["baseline"] = baseline;
    let arguments = json!({"job_id":"job","baseline":"saved"});
    assert_eq!(call(&document, &arguments)?["isError"], false);
    for currentness in ["current", "unknown"] {
        let mut forged = document.clone();
        forged["baseline"]["currentness"] = json!(currentness);
        assert_eq!(call(&forged, &arguments)?["isError"], true);
    }
    for origin in [Value::Null, json!("")] {
        let mut forged = document.clone();
        forged["baseline"]["job_id"] = origin;
        assert_eq!(call(&forged, &arguments)?["isError"], true);
    }
    document
        .as_object_mut()
        .ok_or("document")?
        .remove("baseline_name");
    let arguments = json!({"job_id":"job","before_job":"original-job"});
    for currentness in ["current", "historical", "unknown"] {
        document["baseline"]["currentness"] = json!(currentness);
        assert_eq!(call(&document, &arguments)?["isError"], false);
    }
    Ok(())
}
