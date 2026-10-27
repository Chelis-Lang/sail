// Included in runner's native test module so the real command constructor is used.

#[test]
fn request_capture_transport_is_not_compiler_environment()
-> std::result::Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt as _;
    const TEST: &str = "managed::runner::tests::request_capture_transport_is_not_compiler_environment";
    const NAMES: [&str; 2] = ["SAIL_JOB_REQUEST_CAPTURE", "SAIL_JOB_REQUEST_ID"];
    if std::env::var_os("SAIL_TEST_REQUEST_ENV").is_none() {
        for supplied in [false, true] {
            let mut child = Command::new(std::env::current_exe()?);
            child.arg("--exact").arg(TEST).arg("--nocapture")
                .env("SAIL_TEST_REQUEST_ENV", "child");
            for name in NAMES {
                if supplied { child.env(name, "original-submitted-value"); }
                else { child.env_remove(name); }
            }
            let output = child.output()?;
            assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        return Ok(());
    }
    let root = std::env::temp_dir().join(format!("sail-request-env-{}", std::process::id()));
    let mut config = compiler_fixture(&root)?;
    let probe = root.join("environment-probe");
    std::fs::write(&probe, b"#!/bin/sh\nexec /usr/bin/env\n")?;
    std::fs::set_permissions(&probe, std::fs::Permissions::from_mode(0o700))?;
    config.cargo_verus = probe;
    config.request_capture = Some(super::super::RequestCaptureDestination {
        path: root.join("private-journal"), worker_id: "private-worker-id".into(),
    });
    let (paths, ownership) = prepare(&config, &root, "context", "compiler", "run")?;
    let observed = invocation_command(&config, &paths, false, None, None)?.output()?;
    assert!(observed.environment().is_some());
    let output = observed.into_output();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout)?;
    let names = NAMES.into_iter().map(str::to_owned).collect();
    let declared = super::super::environment::values(&names);
    for name in NAMES {
        let prefix = format!("{name}=");
        let observed = text.lines().find_map(|line| line.strip_prefix(&prefix))
            .map(|value| value.as_bytes().to_vec());
        assert_eq!(declared.get(name), Some(&observed));
        assert!(input_environment_key(std::ffi::OsStr::new(name)), "original user values stay inputs");
    }
    assert!(!text.contains("private-worker-id"));
    assert!(!text.contains("private-journal"));
    drop(ownership);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
