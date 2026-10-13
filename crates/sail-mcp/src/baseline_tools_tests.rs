//! Baseline tools preserve the original CLI authority and full evidence.

use super::*;
use sail_api::evidence::Baseline;
use sail_api::jobs::Response;
use sail_api::output::VerdictName;
use sail_api::project::{Currentness, Execution, ProjectResult, TargetResult};

fn baseline(name: &str) -> Baseline {
    Baseline {
        version: sail_api::evidence::VERSION,
        name: name.into(),
        job_id: "recorded-job".into(),
        result: ProjectResult {
            kind: "sail-project-result".into(),
            schema_version: 1,
            run_id: "recorded-run".into(),
            workspace: "recorded-workspace".into(),
            context_sha256: "recorded-context".into(),
            input_revision: "recorded-revision".into(),
            currentness: Currentness::Historical,
            exit_code: Some(1),
            targets: vec![TargetResult {
                package_id: "demo@1".into(),
                target: "demo".into(),
                kind: vec!["lib".into()],
                establishing_run: "recorded-run".into(),
                reused: false,
                requested: true,
                complete: false,
                report: crate::store::tests::report(vec![crate::store::tests::function(
                    "demo::pending",
                    VerdictName::Provisional,
                )]),
            }],
            execution: Execution::default(),
            notes: vec!["Recorded evidence only.".into()],
            error: None,
            engine: None,
        },
    }
}

fn document(baseline: Baseline) -> Value {
    serde_json::to_value(Response::Baseline(Box::new(baseline)))
        .unwrap_or_else(|error| unreachable!("{error}"))
}

#[test]
fn baseline_arguments_are_exact_literal_cli_requests_without_service_override() {
    let args = json!({"name":"--json","job_id":"recorded-job"});
    let parsed = arguments(
        "save_baseline",
        args.as_object().unwrap_or_else(|| unreachable!()),
    )
    .unwrap_or_else(|error| unreachable!("{error}"));
    assert_eq!(parsed.1, None);
    assert_eq!(
        parsed.0,
        [
            "baseline",
            "save",
            "--output",
            "json",
            "--",
            "--json",
            "--job",
            "recorded-job"
        ]
    );
    let args = json!({"name":"--json"});
    assert_eq!(
        arguments(
            "get_baseline",
            args.as_object().unwrap_or_else(|| unreachable!())
        )
        .unwrap_or_else(|error| unreachable!("{error}"))
        .0,
        ["baseline", "get", "--output", "json", "--", "--json"]
    );
    assert_eq!(
        arguments("list_baselines", &Map::new())
            .unwrap_or_else(|error| unreachable!("{error}"))
            .0,
        ["baseline", "list", "--output", "json"]
    );
    let args = json!({"after_job":"after","baseline":"--json"});
    assert_eq!(
        arguments(
            "compare_verification",
            args.as_object().unwrap_or_else(|| unreachable!())
        )
        .unwrap_or_else(|error| unreachable!("{error}"))
        .0,
        [
            "compare",
            "--output",
            "json",
            "--",
            "after",
            "--baseline",
            "--json"
        ]
    );
    for name in ["", ".", "..", "../escape", "a/b", "non-ascii-λ"] {
        let args = json!({"name":name});
        assert!(
            arguments(
                "get_baseline",
                args.as_object().unwrap_or_else(|| unreachable!())
            )
            .is_err()
        );
    }
    let args = json!({"name":"x".repeat(81)});
    assert!(
        arguments(
            "get_baseline",
            args.as_object().unwrap_or_else(|| unreachable!())
        )
        .is_err()
    );
    for (tool, args) in [
        ("save_baseline", json!({"name":"release"})),
        (
            "save_baseline",
            json!({"name":"release","job_id":"--output"}),
        ),
        (
            "get_baseline",
            json!({"name":"release","project_dir":"other"}),
        ),
        ("list_baselines", json!({"project_dir":"other"})),
        ("list_baselines", json!({"name":"release"})),
    ] {
        assert!(arguments(tool, args.as_object().unwrap_or_else(|| unreachable!())).is_err());
    }
}

#[test]
fn baseline_response_cannot_claim_currentness_or_substitute_identity() {
    let args = json!({"name":"release","job_id":"recorded-job"});
    let args = args.as_object().unwrap_or_else(|| unreachable!());
    let expected = document(baseline("release"));
    assert!(valid("save_baseline", &expected));
    assert!(valid("get_baseline", &expected));
    assert!(requested_baseline_matches("save_baseline", args, &expected));
    for (path, value) in [
        ("version", json!(2)),
        ("name", json!("../escape")),
        ("job_id", json!("")),
    ] {
        let mut corrupt = expected.clone();
        corrupt["value"][path] = value;
        assert!(!valid("get_baseline", &corrupt));
    }
    for currentness in ["current", "unknown"] {
        let mut corrupt = expected.clone();
        corrupt["value"]["result"]["currentness"] = json!(currentness);
        assert!(!valid("get_baseline", &corrupt));
    }
    for (path, value) in [("kind", json!("wrong")), ("schema_version", json!(2))] {
        let mut corrupt = expected.clone();
        corrupt["value"]["result"][path] = value;
        assert!(!valid("get_baseline", &corrupt));
    }
    let mut wrong = expected.clone();
    wrong["value"]["name"] = json!("another");
    assert!(valid("get_baseline", &wrong));
    assert!(!requested_baseline_matches("get_baseline", args, &wrong));
    wrong = expected.clone();
    wrong["value"]["job_id"] = json!("another-job");
    assert!(!requested_baseline_matches("save_baseline", args, &wrong));
    assert!(!valid(
        "get_baseline",
        &json!({"response":"baselines","value":[]})
    ));
    for names in [json!([]), json!(["a", "release"])] {
        assert!(valid(
            "list_baselines",
            &json!({"response":"baselines","value":names})
        ));
    }
    for names in [
        json!(["release", "a"]),
        json!(["a", "a"]),
        json!(["../escape"]),
    ] {
        assert!(!valid(
            "list_baselines",
            &json!({"response":"baselines","value":names})
        ));
    }
    assert_eq!(
        expected["value"]["result"]["targets"][0]["report"]["crates"][0]["functions"][0]["verdict"],
        "provisional"
    );
    assert_eq!(expected["value"]["result"]["targets"][0]["complete"], false);
}

#[test]
fn baseline_definitions_expose_mutation_and_read_only_semantics() {
    let definitions = definitions();
    for (name, read_only, required) in [
        ("save_baseline", false, vec!["job_id", "name"]),
        ("list_baselines", true, vec![]),
        ("get_baseline", true, vec!["name"]),
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition["name"] == name)
            .unwrap_or_else(|| unreachable!("tool missing"));
        assert_eq!(definition["annotations"]["readOnlyHint"], read_only);
        assert_eq!(definition["annotations"]["destructiveHint"], false);
        assert_eq!(definition["inputSchema"]["additionalProperties"], false);
        let actual = definition["inputSchema"]
            .get("required")
            .cloned()
            .unwrap_or_else(|| json!([]));
        assert_eq!(actual, json!(required));
        if name != "list_baselines" {
            assert_eq!(
                definition["inputSchema"]["properties"]["name"],
                baseline_schema()
            );
            assert_eq!(
                definition["inputSchema"]["properties"]["name"]["not"],
                json!({"enum":[".",".."]})
            );
        }
    }
    let compare = definitions
        .iter()
        .find(|definition| definition["name"] == "compare_verification")
        .unwrap_or_else(|| unreachable!("comparison missing"));
    assert_eq!(
        compare["inputSchema"]["properties"]["baseline"],
        baseline_schema()
    );
}

#[cfg(unix)]
#[test]
fn actual_baseline_tool_dispatch_retains_full_graph_and_refuses_bad_cli_reply()
-> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt as _;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "sail-mcp-baseline-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&fixture.0)?;
    let root = fixture.0.canonicalize()?;
    let script = root.join("sail");
    std::fs::write(
        &script,
        "#!/bin/sh\nd=${0%/*}\nprintf '%s\\n' \"$@\" > \"$d/arguments\"\nIFS= read -r response < \"$d/response.json\"\nprintf '%s' \"$response\"\n",
    )?;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))?;
    let context = Context::new(crate::tools::Config {
        sail: script,
        store_dir: None,
        cwd: root.clone(),
    });
    let expected = document(baseline("--json"));
    for tool in ["save_baseline", "get_baseline"] {
        std::fs::write(root.join("response.json"), serde_json::to_vec(&expected)?)?;
        let args = if tool == "save_baseline" {
            json!({"name":"--json","job_id":"recorded-job"})
        } else {
            json!({"name":"--json"})
        };
        let args = args.as_object().ok_or("arguments")?;
        let reply =
            crate::tools::call(&context, tool, args, &Cancel::default()).ok_or("unknown tool")?;
        assert!(!reply.is_error);
        assert_eq!(reply.structured, expected);
        let actual = std::fs::read_to_string(root.join("arguments"))?;
        let wanted = arguments(tool, args)?.0.join("\n") + "\n";
        assert_eq!(actual, wanted);
    }
    let list = json!({"response":"baselines","value":["--json","release"]});
    std::fs::write(root.join("response.json"), serde_json::to_vec(&list)?)?;
    let reply = crate::tools::call(&context, "list_baselines", &Map::new(), &Cancel::default())
        .ok_or("unknown tool")?;
    assert!(!reply.is_error);
    assert_eq!(reply.structured, list);
    let args = json!({"name":"--json"});
    let args = args.as_object().ok_or("arguments")?;
    for wrong in [
        document(baseline("another")),
        json!({"response":"error","value":"SAIL0905: missing"}),
    ] {
        std::fs::write(root.join("response.json"), serde_json::to_vec(&wrong)?)?;
        let reply = crate::tools::call(&context, "get_baseline", args, &Cancel::default())
            .ok_or("unknown tool")?;
        assert!(reply.is_error);
        assert_eq!(reply.structured["error"], "invalid_response");
    }
    Ok(())
}
