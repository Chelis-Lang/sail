//! Durable records written by the neutral source jobs of Sail 0.2 (removed in
//! 0.3.0) stay listable and read as the coded refusal `SAIL3161`. They are
//! never read as ordinary verification evidence, and never panic.
#![cfg(unix)]
use sail_api::evidence::Reference;
use sail_api::jobs::{Client, Lifecycle, Mode, Request, Response, Submission};
use std::collections::BTreeMap;
use std::path::Path;
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

/// The 0.2.0 on-disk shape: `contracts` held the retained neutral submission.
fn retire(record: &Path) -> TestResult {
    let mut value: serde_json::Value = serde_json::from_slice(&std::fs::read(record)?)?;
    value["contracts"] = serde_json::json!({
        "source": "pub fn add_one(x: u64) -> u64 { x + 1 }\n",
        "source_sha256": "a".repeat(64),
        "crate_name": "neutral",
    });
    std::fs::write(record, serde_json::to_vec(&value)?)?;
    Ok(())
}

fn refused(response: std::io::Result<Response>) -> bool {
    response.is_err_and(|error| error.to_string().starts_with("SAIL3161:"))
}

#[test]
fn retired_neutral_records_list_and_refuse_with_a_stable_code() -> TestResult {
    use std::os::unix::fs::PermissionsExt as _;
    let root = std::env::temp_dir().join(format!("sail-retired-neutral-{}", std::process::id()));
    std::fs::create_dir_all(root.join("project"))?;
    // List filters use canonical project identities, as submission does.
    let root = root.canonicalize()?;
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
        let submission = |revision: &str| Submission {
            mode: Mode::Stock,
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
        let (socket, store) = (root.join("jobs.sock"), root.join("store"));
        let mut server = sail_daemon::jobs::bind(&socket, &store, &worker)?.spawn()?;
        let client = Client::new(&socket);
        let submit = |revision: &str| match client.call(&Request::Submit {
            submission: submission(revision),
        }) {
            Ok(Response::Status(status)) => Ok(status),
            other => Err(format!("unexpected submit response {other:?}")),
        };
        let (old, ordinary) = (submit("old")?, submit("ordinary")?);
        completed(&client, &old.id)?;
        completed(&client, &ordinary.id)?;
        let Response::Baseline(_) = client.call(&Request::SaveBaseline {
            id: ordinary.id.clone(),
            name: "old-neutral".into(),
        })?
        else {
            return Err("baseline was not saved".into());
        };
        server.shutdown();

        // Rewrite one record and the baseline into their 0.2.0 neutral shapes.
        retire(&store.join(&old.id).join("record.json"))?;
        retire(&store.join("baselines").join("old-neutral.json"))?;

        let mut server = sail_daemon::jobs::bind(&socket, &store, &worker)?.spawn()?;
        // Status and listing stay readable and name the refusal.
        let Response::Status(status) = client.call(&Request::Status { id: old.id.clone() })? else {
            return Err("retired status was not readable".into());
        };
        assert!(
            status
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with("SAIL3161:"))
        );
        assert_eq!(status.currentness, sail_api::jobs::Currentness::Unknown);
        let Response::List(list) = client.call(&Request::List {
            project_dir: Some(root.join("project")),
        })?
        else {
            return Err("list was not readable".into());
        };
        assert_eq!(list.len(), 2);
        // Every evidence read of the retired record is refused with the code.
        assert!(refused(
            client.call(&Request::Result { id: old.id.clone() })
        ));
        assert!(refused(client.call(&Request::SaveBaseline {
            id: old.id.clone(),
            name: "again".into(),
        })));
        assert!(refused(client.call(&Request::Compare {
            before: Reference::Job(old.id.clone()),
            after: ordinary.id.clone(),
        })));
        assert!(refused(client.call(&Request::GetBaseline {
            name: "old-neutral".into(),
        })));
        assert!(refused(client.call(&Request::Compare {
            before: Reference::Baseline("old-neutral".into()),
            after: ordinary.id.clone(),
        })));
        assert!(refused(
            client.call(&Request::AttributedAssurance { id: old.id.clone() })
        ));
        // The ordinary job beside it is unaffected.
        assert!(matches!(
            client.call(&Request::Result {
                id: ordinary.id.clone()
            })?,
            Response::Result(document) if document.result.is_some()
        ));
        server.shutdown();
        Ok(())
    })();
    std::fs::remove_dir_all(&root)?;
    outcome
}
