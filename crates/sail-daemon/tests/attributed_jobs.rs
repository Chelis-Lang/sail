//! Retired attribution entrypoints cannot launch work; historical ordinary
//! job data remains readable without manufacturing an assurance view.
#![cfg(unix)]
use sail_api::attribution::RequirementAttribution;
use sail_api::jobs::{Client, Lifecycle, Mode, Request, Response, Submission};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn completed(client: &Client, id: &str) -> TestResult {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Response::Status(status) = client.call(&Request::Status { id: id.into() })? {
            if status.state == Lifecycle::Completed {
                return Ok(());
            }
            if status.state.terminal() {
                return Err(format!("unexpected terminal {status:?}").into());
            }
        }
        if Instant::now() > deadline {
            return Err("job deadline".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn verification(root: &std::path::Path, revision: &str, mode: Mode) -> Submission {
    Submission {
        mode,
        project_dir: root.join("project"),
        args: vec![],
        force: false,
        input_revision: Some(revision.into()),
        environment: BTreeMap::from([
            ("PATH".into(), "/usr/bin:/bin".into()),
            ("RUNS".into(), root.join("runs").display().to_string()),
            (
                "RESULT".into(),
                root.join("result.json").display().to_string(),
            ),
        ]),
    }
}

#[test]
fn retired_attribution_refuses_work_and_preserves_historical_job_data() -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-attributed-jobs-{}", std::process::id()));
    std::fs::create_dir_all(root.join("project"))?;
    let outcome = (|| -> TestResult {
        let worker = root.join("worker");
        std::fs::write(
            &worker,
            "#!/bin/sh\nprintf ran >> \"$RUNS\"\ncat \"$RESULT\"\nexit 0\n",
        )?;
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755))?;
        std::fs::write(
            root.join("result.json"),
            include_str!(
                "../resources/crates/sail-verus/tests/fixtures/requirement-attribution/positive-requested-target.json"
            ),
        )?;
        let submission = |revision: &str, mode: Mode| verification(&root, revision, mode);
        let claim = |requirement_id: &str| RequirementAttribution {
            requirement_id: requirement_id.into(),
            function: "neutral::candidate".into(),
        };
        let socket = root.join("jobs.sock");
        let store = root.join("jobs");
        let mut server = sail_daemon::jobs::bind(&socket, &store, &worker)?.spawn()?;
        let client = Client::new(&socket);
        assert!(matches!(
            client.call(&Request::Capabilities)?,
            Response::Capabilities {
                attribution_version: None,
                ..
            }
        ));
        for request in [
            Request::SubmitAttributed {
                submission: submission("retired", Mode::Stock),
                requirements: vec![claim("REQ-A")],
            },
            Request::AttributedAssurance {
                id: "../foreign".into(),
            },
        ] {
            let refusal = client
                .call(&request)
                .err()
                .ok_or("retired request unexpectedly succeeded")?;
            assert!(refusal.to_string().starts_with("SAIL3161:"));
        }
        assert!(
            client
                .submit_attributed(submission("retired", Mode::Stock), vec![claim("REQ-A")])
                .is_err()
        );
        assert!(client.attributed_assurance("any-job").is_err());
        assert!(
            matches!(client.call(&Request::List { project_dir: None })?, Response::List(ref jobs) if jobs.is_empty())
        );
        assert!(
            !root.join("runs").exists(),
            "retired calls must not execute a worker"
        );
        let plain = match client.call(&Request::Submit {
            submission: submission("first", Mode::Stock),
        })? {
            Response::Status(status) => status,
            response => return Err(format!("unexpected {response:?}").into()),
        };
        completed(&client, &plain.id)?;
        let original = serde_json::to_value(client.call(&Request::Result {
            id: plain.id.clone(),
        })?)?;
        server.shutdown();
        // Preserve a legitimate historical attribution in a completed ordinary
        // journal. This fixture is a migration input, never new proof evidence.
        let journal = store.join(&plain.id).join("record.json");
        let mut retained: serde_json::Value = serde_json::from_slice(&std::fs::read(&journal)?)?;
        retained["requirements"] = serde_json::to_value(vec![claim("REQ-A")])?;
        let retained = serde_json::to_vec(&retained)?;
        std::fs::write(&journal, &retained)?;
        let mut server = sail_daemon::jobs::bind(&socket, &store, &worker)?.spawn()?;
        let reloaded = serde_json::to_value(client.call(&Request::Result {
            id: plain.id.clone(),
        })?)?;
        assert_eq!(
            reloaded, original,
            "retirement must preserve original ordinary result bytes and evidence"
        );
        assert_eq!(
            std::fs::read(&journal)?,
            retained,
            "historical tags must not be erased"
        );
        let refusal = client
            .call(&Request::AttributedAssurance { id: plain.id })
            .err()
            .ok_or("retired historical assurance read unexpectedly succeeded")?;
        assert!(refusal.to_string().starts_with("SAIL3161:"));
        server.shutdown();
        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    outcome
}
