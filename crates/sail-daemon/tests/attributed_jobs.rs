//! Submitter requirement attributions are retained with ordinary jobs, survive
//! restart and are reported apart from neutral-contract assurance.
#![cfg(unix)]
use sail_api::assurance::ProofState;
use sail_api::attribution::{Relationship, RequirementAttribution};
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

#[test]
fn attributed_job_round_trip_restart_and_refusals() -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-attributed-jobs-{}", std::process::id()));
    std::fs::create_dir_all(root.join("project"))?;
    let outcome = (|| -> TestResult {
        let worker = root.join("worker");
        std::fs::write(&worker, "#!/bin/sh\ncat \"$RESULT\"\nexit 0\n")?;
        std::fs::set_permissions(&worker, std::fs::Permissions::from_mode(0o755))?;
        std::fs::write(
            root.join("result.json"),
            include_str!(
                "../resources/crates/sail-verus/tests/fixtures/requirement-attribution/positive-requested-target.json"
            ),
        )?;
        let submission = |revision: &str, mode: Mode| Submission {
            mode,
            project_dir: root.join("project"),
            args: vec![],
            force: false,
            input_revision: Some(revision.into()),
            environment: BTreeMap::from([
                ("PATH".into(), "/usr/bin:/bin".into()),
                (
                    "RESULT".into(),
                    root.join("result.json").display().to_string(),
                ),
            ]),
        };
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
                capabilities_version: sail_api::jobs::CAPABILITIES_VERSION,
                attribution_version: Some(sail_api::attribution::VERSION),
                ..
            }
        ));
        let attributed = client.submit_attributed(
            submission("first", Mode::Stock),
            vec![claim("REQ-B"), claim("REQ-A")],
        )?;
        let plain = match client.call(&Request::Submit {
            submission: submission("first", Mode::Stock),
        })? {
            Response::Status(status) => status,
            response => return Err(format!("unexpected {response:?}").into()),
        };
        assert_ne!(plain.id, attributed.id, "attributions never coalesce away");
        completed(&client, &attributed.id)?;
        completed(&client, &plain.id)?;
        let view = client.attributed_assurance(&attributed.id)?;
        let ids: Vec<_> = view
            .requirements
            .iter()
            .map(|r| r.requirement_id.as_str())
            .collect();
        assert_eq!(ids, ["REQ-A", "REQ-B"]);
        let row = &view.requirements[0].evidence[0];
        assert_eq!(row.relationship, Relationship::VerusSpecificationToCode);
        assert!(row.function_reported);
        // The fixture service cannot confirm live inputs, so no tier is established.
        assert_eq!(row.outcome, ProofState::NotProved);
        assert_eq!(row.tier, None);
        assert!(
            row.qualifiers
                .iter()
                .any(|q| q.contains("not confirmed current"))
        );
        for refused in [
            client.attributed_assurance(&plain.id).map(|_| ()),
            client
                .submit_attributed(submission("second", Mode::Interactive), vec![claim("R")])
                .map(|_| ()),
            client
                .submit_attributed(submission("second", Mode::Stock), vec![])
                .map(|_| ()),
        ] {
            assert!(refused.is_err());
        }
        server.shutdown();
        let mut server = sail_daemon::jobs::bind(&socket, &store, &worker)?.spawn()?;
        assert_eq!(client.attributed_assurance(&attributed.id)?, view);
        server.shutdown();
        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    outcome
}
