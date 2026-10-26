// The Cargo toolchain selection (ADR 0058) reaches every process the runner
// starts and the proof context.

type ToolchainResult = std::result::Result<(), Box<dyn std::error::Error>>;

/// A toolchain name no caller has, so the pinned and caller selections differ.
const TEST_PIN: &str = "sail-test-pinned";

fn toolchain_root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("sail-cargo-toolchain-{name}-{}", std::process::id()))
}

/// An executable that appends `RUSTUP_TOOLCHAIN` (or `unset`) and how many
/// `SAIL_MANAGED_*` variables it sees to `log`, and succeeds without output.
fn recording_tool(path: &Path, log: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::write(
        path,
        format!(
            "#!/bin/sh\nprintf '%s %s\\n' \"${{RUSTUP_TOOLCHAIN-unset}}\" \"$(env | grep -c '^SAIL_MANAGED_')\" >> '{}'\n",
            log.display()
        ),
    )?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
}

/// What a child inherits when Sail sets nothing.
fn inherited_toolchain() -> String {
    std::env::var("RUSTUP_TOOLCHAIN").unwrap_or_else(|_| "unset".to_owned())
}

#[test]
fn the_selected_toolchain_joins_the_context_environment_and_hashes() -> ToolchainResult {
    let root = toolchain_root("context");
    let caller = compiler_fixture(&root)?;
    let pinned = Config {
        cargo_toolchain: crate::managed::CargoToolchain::Pinned(TEST_PIN.into()),
        ..compiler_fixture(&root)?
    };
    let unavailable = Config {
        cargo_toolchain: crate::managed::CargoToolchain::Unavailable(TEST_PIN.into()),
        ..compiler_fixture(&root)?
    };
    let key = b"RUSTUP_TOOLCHAIN".to_vec();
    let caller_environment = context_environment(&caller);
    let pinned_environment = context_environment(&pinned);
    assert_eq!(
        pinned_environment.get(&key).map(Vec::as_slice),
        Some(TEST_PIN.as_bytes())
    );
    assert_eq!(
        caller_environment.get(&key),
        std::env::var_os("RUSTUP_TOOLCHAIN")
            .map(std::ffi::OsString::into_encoded_bytes)
            .as_ref(),
        "the caller's selection is recorded as it is"
    );
    assert_eq!(context_environment(&unavailable), caller_environment);
    let mut without = pinned_environment.clone();
    without.remove(&key);
    let mut caller_without = caller_environment.clone();
    caller_without.remove(&key);
    assert_eq!(without, caller_without, "only the toolchain differs");
    let hashes = |environment: BTreeMap<Vec<u8>, Vec<u8>>| {
        let environment = environment.into_iter().collect::<Vec<_>>();
        (
            context_hashes(&[], None, &[], &environment),
            store_family_hash(&[], None, &[], &environment),
        )
    };
    let (caller_hashes, caller_store) = hashes(caller_environment);
    let (pinned_hashes, pinned_store) = hashes(pinned_environment);
    assert_ne!(caller_hashes?, pinned_hashes?);
    assert_ne!(caller_store?, pinned_store?);
    std::fs::remove_dir_all(&root)?;
    Ok(())
}

#[test]
fn the_sysroot_probe_runs_with_the_selected_toolchain() {
    let value = |selection: crate::managed::CargoToolchain| {
        let config = Config {
            cargo_toolchain: selection,
            ..compiler_fixture(&toolchain_root("probe")).unwrap_or_else(|e| unreachable!("{e}"))
        };
        let probe = sysroot_probe(&config, Path::new("rustc"));
        assert_eq!(
            probe.get_args().collect::<Vec<_>>(),
            ["--print", "sysroot"]
        );
        probe
            .get_envs()
            .find(|(name, _)| *name == "RUSTUP_TOOLCHAIN")
            .and_then(|(_, value)| value.map(std::ffi::OsStr::to_owned))
    };
    assert_eq!(
        value(crate::managed::CargoToolchain::Pinned(TEST_PIN.into())),
        Some(TEST_PIN.into())
    );
    assert_eq!(value(crate::managed::CargoToolchain::Caller), None);
    let _ = std::fs::remove_dir_all(toolchain_root("probe"));
}

#[test]
fn clean_and_invocation_run_with_the_selected_toolchain() -> ToolchainResult {
    let root = toolchain_root("spawn");
    let _ = std::fs::remove_dir_all(&root);
    let base = compiler_fixture(&root)?;
    let log = root.join("calls");
    let tool = root.join("tool");
    recording_tool(&tool, &log)?;
    for (selection, expected) in [
        (
            crate::managed::CargoToolchain::Pinned(TEST_PIN.into()),
            TEST_PIN.to_owned(),
        ),
        (crate::managed::CargoToolchain::Caller, inherited_toolchain()),
        (
            crate::managed::CargoToolchain::Unavailable(TEST_PIN.into()),
            inherited_toolchain(),
        ),
    ] {
        let _ = std::fs::remove_file(&log);
        let config = Config {
            cargo: tool.clone(),
            cargo_verus: tool.clone(),
            cargo_toolchain: selection,
            ..base.clone()
        };
        let (paths, owner) = prepare(&config, &root, "context", "family", "run")?;
        cargo_clean(&config, &paths.target, &[])?;
        let mut err = Vec::new();
        assert_eq!(invoke(&config, &paths, &mut None, &mut err, &[])?, Some(0));
        drop(owner);
        let calls = std::fs::read_to_string(&log)?;
        let calls = calls.lines().collect::<Vec<_>>();
        // Neither the clean nor Cargo-Verus sees a private setting (#385).
        assert_eq!(
            calls,
            [format!("{expected} 0"), format!("{expected} 0")],
            "{}",
            String::from_utf8_lossy(&err)
        );
        // The real Cargo the adapter runs, which inherits the same
        // selection, is named beside the aliases instead.
        let transport = paths
            .proxy_alias
            .with_file_name(crate::managed::transport::FILE);
        let transport: serde_json::Value = serde_json::from_slice(&std::fs::read(transport)?)?;
        let cargo: Vec<u8> = serde_json::from_value(
            transport["values"]["SAIL_MANAGED_CARGO"].clone(),
        )?;
        assert_eq!(cargo, tool.as_os_str().as_encoded_bytes());
    }
    std::fs::remove_dir_all(&root)?;
    Ok(())
}
