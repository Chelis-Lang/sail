#![forbid(unsafe_code)]
//! Old endpoints must reject standalone submissions before any worker starts.

#[cfg(unix)]
#[test]
fn standalone_submission_requires_explicit_capability() -> std::io::Result<()> {
    use sail_api::jobs::{Client, Mode, Request, Response, Submission};
    use std::os::unix::net::UnixListener;
    let socket =
        std::env::temp_dir().join(format!("sail-standalone-cap-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&socket)?;
    let worker = std::thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        assert!(matches!(
            sail_api::jobs::read_frame::<Request>(&mut stream)?,
            Request::Capabilities
        ));
        let old: Response = serde_json::from_str(
            r#"{"response":"capabilities","value":{"proof_version":1,"repair":true}}"#,
        )?;
        sail_api::jobs::write_frame(&mut stream, &old)?;
        listener.set_nonblocking(true)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
        Ok(())
    });
    let result = Client::new(&socket).call(&Request::Submit {
        submission: Submission {
            mode: Mode::Standalone,
            project_dir: std::env::temp_dir(),
            args: vec!["proof.rs".into()],
            force: false,
            environment: std::collections::BTreeMap::new(),
            input_revision: None,
        },
    });
    assert!(result.is_err_and(|error| error.to_string().contains("does not support standalone")));
    worker
        .join()
        .map_err(|_| std::io::Error::other("capability fixture failed"))??;
    std::fs::remove_file(socket)?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn standalone_repair_refuses_a_service_without_the_exact_capability()
-> Result<(), Box<dyn std::error::Error>> {
    use sail_api::jobs::{Client, Mode, Request, Response, Submission};
    use std::collections::BTreeMap;
    use std::os::unix::net::UnixListener;
    let root = std::env::temp_dir().join(format!("sail-file-repair-cap-{}", std::process::id()));
    std::fs::create_dir_all(&root)?;
    let socket = root.join("jobs.sock");
    let listener = UnixListener::bind(&socket)?;
    let service = std::thread::spawn(move || -> std::io::Result<()> {
        let (mut stream, _) = listener.accept()?;
        assert!(matches!(
            sail_api::jobs::read_frame::<Request>(&mut stream)?,
            Request::Capabilities
        ));
        sail_api::jobs::write_frame(
            &mut stream,
            &Response::Capabilities {
                capabilities_version: sail_api::jobs::CAPABILITIES_VERSION,
                proof_version: sail_api::proof::VERSION,
                repair: true,
                standalone: true,
                standalone_repair: false,
                attribution_version: None,
            },
        )?;
        listener.set_nonblocking(true)?;
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert!(
            matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
        );
        Ok(())
    });
    let result = Client::new(&socket).call(&Request::Submit {
        submission: Submission {
            mode: Mode::StandaloneRepair {
                function: "proof::lemma".into(),
            },
            project_dir: root.clone(),
            args: vec!["proof.rs".into()],
            force: false,
            environment: BTreeMap::new(),
            input_revision: None,
        },
    });
    assert!(result.is_err());
    service.join().map_err(|_| "service thread failed")??;
    std::fs::remove_dir_all(root)?;
    Ok(())
}

/// A 0.2 service sends no capability-set version and reads as set 1, the set
/// that still advertised the neutral operations; 0.3 advertises set 2.
#[test]
fn capability_set_version_defaults_to_the_0_2_set() -> serde_json::Result<()> {
    use sail_api::jobs::{CAPABILITIES_VERSION, Response};
    let old: Response = serde_json::from_str(
        r#"{"response":"capabilities","value":{"proof_version":1,"repair":true,"neutral_contracts_version":1}}"#,
    )?;
    assert!(matches!(
        old,
        Response::Capabilities {
            capabilities_version: 1,
            ..
        }
    ));
    assert_eq!(CAPABILITIES_VERSION, 2);
    Ok(())
}
