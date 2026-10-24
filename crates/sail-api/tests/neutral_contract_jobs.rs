//! Neutral inputs are source-only, and old services cannot silently accept them.
use sail_api::contracts::{MAX_SOURCE_BYTES, Submission};
use sail_api::jobs::{Request, Response};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn input() -> Submission {
    let source = "fn candidate(input: u32) -> u32 { input }".to_owned();
    Submission {
        profile: 1,
        source_sha256: sail_key::QueryKey(sail_key::sha256(source.as_bytes())).to_string(),
        source,
        crate_name: "neutral".into(),
    }
}
#[test]
fn actual_tagged_request_rejects_every_extra_execution_control() -> TestResult {
    let request = serde_json::to_value(Request::SubmitContracts(input()))?;
    assert_eq!(request["operation"], "submit_contracts");
    let decoded: Request = serde_json::from_value(request.clone())?;
    assert!(matches!(decoded, Request::SubmitContracts(source) if source == input()));
    for field in [
        "argv",
        "args",
        "environment",
        "project_dir",
        "tool",
        "source_path",
        "timeout",
    ] {
        let mut value = request.clone();
        value[field] = serde_json::json!("caller control");
        assert!(serde_json::from_value::<Request>(value).is_err(), "{field}");
    }
    let legacy = serde_json::json!({"operation":"submit","ignored_legacy_field":true,
        "submission":{"project_dir":"/tmp/example","args":[],"force":false,"environment":{},"input_revision":null}});
    assert!(matches!(
        serde_json::from_value::<Request>(legacy)?,
        Request::Submit { .. }
    ));
    Ok(())
}
#[test]
fn source_admission_binds_exact_utf8_size_digest_and_crate_identity() {
    let original = input();
    assert!(original.validate().is_ok());
    for source in [
        String::new(),
        "x".repeat(MAX_SOURCE_BYTES + 1),
        format!("{}\n", original.source),
    ] {
        assert!(
            Submission {
                source,
                ..original.clone()
            }
            .validate()
            .is_err()
        );
    }
    for crate_name in ["/tmp/source", "../escape", "9invalid", "two-crates", ""] {
        assert!(
            Submission {
                crate_name: crate_name.into(),
                ..original.clone()
            }
            .validate()
            .is_err()
        );
    }
    assert!(
        Submission {
            source_sha256: original.source_sha256.to_uppercase(),
            ..original
        }
        .validate()
        .is_err()
    );
}
#[test]
fn old_capabilities_and_ordinary_results_keep_their_exact_wire_shape() -> TestResult {
    let legacy = r#"{"response":"capabilities","value":{"proof_version":1,"repair":false}}"#;
    let capabilities: Response = serde_json::from_str(legacy)?;
    assert!(matches!(
        &capabilities,
        Response::Capabilities {
            neutral_contracts_version: None,
            ..
        }
    ));
    assert_eq!(serde_json::to_string(&capabilities)?, legacy);
    let result = serde_json::json!({"job":{"version":1,"id":"job","project_dir":"/tmp/p","args":[],"state":"queued","submitted_at_ms":0,"started_at_ms":null,"finished_at_ms":null,"exit_code":null,"input_revision":null,"currentness":"unknown","error":null},"result":null});
    let decoded: sail_api::jobs::ResultDocument = serde_json::from_value(result.clone())?;
    assert_eq!(decoded.contracts, None);
    assert_eq!(serde_json::to_value(decoded)?, result);
    Ok(())
}
#[cfg(unix)]
#[test]
fn absent_or_future_capability_refuses_before_any_submission() -> TestResult {
    use std::os::unix::net::UnixListener;
    for version in [None, Some(2)] {
        let socket = std::env::temp_dir().join(format!(
            "sn-cap-{}-{}.sock",
            std::process::id(),
            version.unwrap_or(0)
        ));
        let listener = UnixListener::bind(&socket)?;
        let worker = std::thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            assert!(matches!(
                sail_api::jobs::read_frame::<Request>(&mut stream)?,
                Request::Capabilities
            ));
            sail_api::jobs::write_frame(
                &mut stream,
                &Response::Capabilities {
                    proof_version: 1,
                    repair: false,
                    neutral_contracts_version: version,
                    neutral_contract_profiles: vec![1, 2],
                    assurance_version: None,
                    mutation_version: None,
                },
            )?;
            listener.set_nonblocking(true)?;
            std::thread::sleep(std::time::Duration::from_millis(50));
            assert!(
                matches!(listener.accept(), Err(issue) if issue.kind() == std::io::ErrorKind::WouldBlock)
            );
            Ok(())
        });
        assert!(
            sail_api::jobs::Client::new(&socket)
                .submit_contracts(input())
                .is_err()
        );
        worker.join().map_err(|_| "service thread failed")??;
        std::fs::remove_file(socket)?;
    }
    Ok(())
}

#[test]
fn explicit_profiles_preserve_legacy_exact_bytes_and_reject_unknown_profiles() -> TestResult {
    let legacy = serde_json::to_string(&input())?;
    assert!(!legacy.contains("profile"));
    let retained: Submission = serde_json::from_str(&legacy)?;
    assert_eq!(retained.profile, 1);
    assert_eq!(serde_json::to_string(&retained)?, legacy);
    let mut selected = retained;
    selected.profile = 2;
    let bytes = serde_json::to_string(&selected)?;
    assert!(bytes.contains("\"profile\":2"));
    assert_eq!(serde_json::from_str::<Submission>(&bytes)?, selected);
    assert!(selected.validate().is_ok());
    for profile in [0, 3, u32::MAX] {
        selected.profile = profile;
        assert!(selected.validate().is_err());
    }
    Ok(())
}

#[test]
#[cfg(unix)]
fn profile_two_refuses_endpoint_without_explicit_profile_support() -> TestResult {
    use std::os::unix::net::UnixListener;
    let socket = std::env::temp_dir().join(format!("sn-profile-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket)?;
    let worker = std::thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        assert!(matches!(
            sail_api::jobs::read_frame::<Request>(&mut stream)?,
            Request::Capabilities
        ));
        sail_api::jobs::write_frame(
            &mut stream,
            &Response::Capabilities {
                proof_version: 1,
                neutral_contracts_version: Some(1),
                neutral_contract_profiles: vec![],
                assurance_version: None,
                mutation_version: None,
                repair: false,
            },
        )?;
        listener.set_nonblocking(true)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(matches!(listener.accept(),Err(e) if e.kind()==std::io::ErrorKind::WouldBlock));
        Ok(())
    });
    let mut selected = input();
    selected.profile = 2;
    assert!(
        sail_api::jobs::Client::new(&socket)
            .submit_contracts(selected)
            .is_err()
    );
    worker.join().map_err(|_| "service thread failed")??;
    std::fs::remove_file(socket)?;
    Ok(())
}
