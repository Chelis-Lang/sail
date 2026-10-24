//! Source admission, arithmetic definedness and actual native lowering oracles.
use sail_verus::contracts::lower;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const ADD_ONE: &str = include_str!("fixtures/neutral-contracts/add_one.rs");
const SHORT_CIRCUIT: &str = include_str!("fixtures/neutral-contracts/short_circuit.rs");
const NEGATED: &str = include_str!("fixtures/neutral-contracts/negated_overflow.rs");
const OLD: &str = include_str!("fixtures/neutral-contracts/old_overflow.rs");
const SIGNED: &str = include_str!("fixtures/neutral-contracts/signed_min.rs");
const WALLET: &str = include_str!("fixtures/neutral-contracts/wallet.rs");
const COMPOUND: &str = include_str!("fixtures/neutral-contracts/compound_field.rs");
const GUARDED_COMPOUND: &str = include_str!("fixtures/neutral-contracts/compound_field_guarded.rs");
mod runtime_compound {
    include!("fixtures/neutral-contracts/compound_field.rs");
}
mod runtime_guarded_compound {
    include!("fixtures/neutral-contracts/compound_field_guarded.rs");
}

const NATIVE_CASES: [(&str, &str, bool); 9] = [
    ("add-one", ADD_ONE, true),
    ("short-circuit", SHORT_CIRCUIT, true),
    ("signed-min", SIGNED, true),
    ("wallet", WALLET, true),
    ("negated-overflow", NEGATED, false),
    ("old-overflow", OLD, false),
    ("compound-field-overflow", COMPOUND, false),
    ("compound-field-guarded", GUARDED_COMPOUND, true),
    (
        "order-profile-two",
        include_str!("fixtures/neutral-contracts/order_service_profile2.rs"),
        true,
    ),
];

mod runtime_add_one {
    include!("fixtures/neutral-contracts/add_one.rs");
}
mod runtime_short {
    include!("fixtures/neutral-contracts/short_circuit.rs");
}
mod runtime_negated {
    include!("fixtures/neutral-contracts/negated_overflow.rs");
}
mod runtime_old {
    include!("fixtures/neutral-contracts/old_overflow.rs");
}
mod runtime_signed {
    include!("fixtures/neutral-contracts/signed_min.rs");
}
mod runtime_wallet {
    include!("fixtures/neutral-contracts/wallet.rs");
}

mod runtime_body_edit {
    include!("fixtures/neutral-contracts/body_edit.rs");
}

#[test]
fn retained_version_one_manifests_are_checked_against_corrected_semantics() -> TestResult {
    let original: sail_verus::contracts::LoweringManifest = serde_json::from_str(include_str!(
        "fixtures/neutral-contracts/add_one.v1-manifest.json"
    ))?;
    sail_verus::contracts::validate_manifest(ADD_ONE, &original)?;
    assert_eq!(original.version, 1);
    assert_eq!(lower(ADD_ONE, "neutral")?.manifest.version, 2);
    for (source, frozen) in [
        (
            COMPOUND,
            include_str!("fixtures/neutral-contracts/compound_field.v1-manifest.json"),
        ),
        (
            GUARDED_COMPOUND,
            include_str!("fixtures/neutral-contracts/compound_field_guarded.v1-manifest.json"),
        ),
    ] {
        let original = serde_json::from_str(frozen)?;
        assert!(sail_verus::contracts::validate_manifest(source, &original).is_err());
        let corrected = lower(source, "neutral")?;
        assert_eq!(corrected.manifest.version, 2);
        sail_verus::contracts::validate_manifest(source, &corrected.manifest)?;
    }
    let mut changed = original.clone();
    changed.version = 99;
    assert!(sail_verus::contracts::validate_manifest(ADD_ONE, &changed).is_err());
    changed = original.clone();
    changed.lowered_sha256 = "0".repeat(64);
    assert!(sail_verus::contracts::validate_manifest(ADD_ONE, &changed).is_err());
    assert!(sail_verus::contracts::validate_manifest(COMPOUND, &original).is_err());
    Ok(())
}

#[test]
fn identical_source_lowers_identically_and_binds_actual_requirement() -> TestResult {
    let first = lower(ADD_ONE, "neutral")?;
    assert_eq!(first, lower(ADD_ONE, "neutral")?);
    assert_eq!(first.manifest.bindings.len(), 1);
    assert_eq!(first.manifest.bindings[0].requirement_id, "ADD-ONE");
    assert_eq!(
        first.manifest.bindings[0].lowered_function,
        "neutral::candidate"
    );
    assert_ne!(
        first.manifest.original_sha256,
        first.manifest.lowered_sha256
    );
    let changed = lower(&ADD_ONE.replace("input + 1\n", "input\n"), "neutral")?;
    assert_ne!(
        first.manifest.original_sha256,
        changed.manifest.original_sha256
    );
    assert_ne!(
        first.manifest.lowered_sha256,
        changed.manifest.lowered_sha256
    );
    assert_eq!(first.manifest.bindings, changed.manifest.bindings);
    Ok(())
}

#[test]
fn checked_predicate_definedness_survives_negation_and_old_does_not_narrow_entry() -> TestResult {
    let negated = lower(NEGATED, "neutral")?;
    assert!(negated.source.contains("__sail_defined_overflowing"));
    let old = lower(OLD, "neutral")?;
    assert!(!old.source.contains("requires"));
    assert!(old.source.contains("assert"));
    let wallet = lower(WALLET, "neutral")?;
    assert!(wallet.source.contains("old (self)"));
    assert!(wallet.source.contains("final (self)"));
    assert_eq!(wallet.manifest.bindings.len(), 2);
    Ok(())
}

#[test]
fn runtime_of_the_exact_lowered_fixtures_matches_bounded_expectations() {
    for input in 0..100u8 {
        assert_eq!(runtime_add_one::candidate(input), input + 1);
        assert!(runtime_add_one::next_value(input, input + 1));
    }
    for input in u8::MIN..=u8::MAX {
        assert_eq!(runtime_short::candidate(input), input);
    }
    for input in i8::MIN..=i8::MAX {
        assert_eq!(runtime_signed::candidate(input), input);
    }
    assert!(std::panic::catch_unwind(|| runtime_body_edit::candidate(1)).is_err());
    assert!(std::panic::catch_unwind(runtime_negated::candidate).is_err());
    assert!(std::panic::catch_unwind(|| runtime_negated::overflowing(255)).is_err());
    assert!(std::panic::catch_unwind(|| runtime_old::candidate(255)).is_err());
    for balance in 0..8u64 {
        for amount in 0..=balance {
            let mut wallet = runtime_wallet::Wallet::new(balance);
            assert_eq!(wallet.withdraw(amount), balance - amount);
            assert_eq!(wallet.calls, 1);
        }
    }
}

#[test]
fn compound_field_runtime_preserves_overflow_and_entry_guards() -> TestResult {
    for flag in [false, true] {
        for x in 0..255u8 {
            assert_eq!(
                runtime_compound::candidate(
                    x,
                    runtime_compound::Row { flag },
                    runtime_compound::Row { flag: !flag }
                ),
                flag
            );
            assert_eq!(
                runtime_guarded_compound::candidate(
                    x,
                    runtime_guarded_compound::Row { flag },
                    runtime_guarded_compound::Row { flag: !flag }
                ),
                flag
            );
        }
    }
    let overflow = std::panic::catch_unwind(|| {
        runtime_compound::candidate(
            255,
            runtime_compound::Row { flag: true },
            runtime_compound::Row { flag: false },
        )
    })
    .err()
    .ok_or("compound field failed to evaluate its guard")?;
    assert_eq!(
        panic_message(&overflow),
        Some("SAIL0703: specification arithmetic overflow or invalid division")
    );
    let rejected = std::panic::catch_unwind(|| {
        runtime_guarded_compound::candidate(
            255,
            runtime_guarded_compound::Row { flag: true },
            runtime_guarded_compound::Row { flag: false },
        )
    })
    .err()
    .ok_or("entry guard admitted overflowing input")?;
    assert!(panic_message(&rejected).is_some_and(
        |message| message == "SAIL0702: precondition failed for requirement FIELD-DEFINED"
    ));
    Ok(())
}

fn panic_message(value: &Box<dyn std::any::Any + Send>) -> Option<&str> {
    value
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| value.downcast_ref::<&str>().copied())
}

#[test]
fn whole_source_refuses_hidden_resolution_effects_and_unsupported_arithmetic() {
    for prefix in [
        "trait Hidden {}",
        "type Alias = u64;",
        "const HIDDEN:u64=0;",
        "use other::thing;",
        "mod external;",
        "fn helper() {}",
        "impl core::ops::Deref for Hidden {}",
    ] {
        assert!(
            lower(&format!("{prefix}\n{SHORT_CIRCUIT}"), "neutral").is_err(),
            "{prefix}"
        );
    }
    for predicate in [
        "#[derive(Clone,Copy)] struct Hidden { value: Custom }",
        "#[derive(Clone,Copy)] struct Hidden { value: u64 } impl core::ops::Deref for Hidden {}",
    ] {
        assert!(lower(&format!("{predicate}\n{SHORT_CIRCUIT}"), "neutral").is_err());
    }
    let division = "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| ret / 2 > 0)] fn f(x:i8)->i8{x}";
    assert!(lower(division, "neutral").is_err());
    let integer = "#[sail_contracts::satisfies(\"R\")] #[sail_contracts::ensures(|ret| true)] fn f(x: Int)->Int{x}";
    assert!(lower(integer, "neutral").is_err());
}

#[test]
fn raw_spelling_cannot_shadow_reserved_translation_or_scalar_names() {
    for declaration in [
        "#[sail_contracts::requirement(\"RESERVED\")] fn r#old(value:bool)->bool{value}",
        "#[sail_contracts::requirement(\"RESERVED\")] fn r#__sail_defined_fake(value:bool)->bool{value}",
        "#[derive(Clone,Copy)] struct r#u8 { value: bool }",
        "#[derive(Clone,Copy)] struct r#Int { value: bool }",
    ] {
        assert!(
            lower(&format!("{declaration}\n{SHORT_CIRCUIT}"), "neutral").is_err(),
            "{declaration}"
        );
    }
}

#[test]
fn passive_attributes_cannot_hide_compile_time_macro_expansion() {
    for attribute in [
        "#[doc = include_str!(\"unreviewed.txt\")]",
        "#[must_use = concat!(\"unreviewed\")]",
        "#[inline(unresolved!())]",
    ] {
        assert!(lower(&format!("{attribute}\n{SHORT_CIRCUIT}"), "neutral").is_err());
    }
    assert!(
        lower(
            &format!("#![doc = include_str!(\"unreviewed.txt\")]\n{SHORT_CIRCUIT}"),
            "neutral"
        )
        .is_err()
    );
    assert!(
        lower(
            &format!("#[doc = \"plain documentation\"]\n#[inline(always)]\n{SHORT_CIRCUIT}"),
            "neutral"
        )
        .is_ok()
    );
    let parameter = "#[sail_contracts::satisfies(\"P\")] #[sail_contracts::ensures(|ret| ret == input)] fn f(#[cfg(any())] input: u8)->u8 { input }";
    assert!(lower(parameter, "neutral").is_err());
}

#[test]
fn invalid_rust_literals_cannot_turn_into_vacuous_verified_preconditions() {
    for condition in [
        "input == 256",
        "input == 256u8",
        "input == -0u8",
        "input > -1",
        "input == 340282366920938463463374607431768211456",
    ] {
        let source = format!(
            "#[sail_contracts::requires({condition})] #[sail_contracts::ensures(|result| result == input)] #[sail_contracts::satisfies(\"LITERAL\")] fn candidate(input:u8)->u8 {{input}}"
        );
        assert!(lower(&source, "neutral").is_err(), "{condition}");
    }
    assert!(lower(SIGNED, "neutral").is_ok());
}

#[test]
#[ignore = "requires actual pinned fork and stock runtimes; run explicitly in native acceptance"]
fn native_fork_and_stock_agree_on_exact_lowered_source() -> TestResult {
    use std::process::Command;
    let pins = sail_verus::toolchain::Pins::embedded()?;
    // The CI image installs the qualified engine outside the mutable Sail cache.
    let fork = match std::env::var_os("SAIL_VERUS_BUILD")
        .or_else(|| std::env::var_os("SAIL_ENGINE_BUILD"))
    {
        Some(build) => sail_verus::engine::Installed::at(std::path::Path::new(&build), &pins)?,
        None => sail_verus::engine::Installed::locate_verified(&sail_verus::cache_dir()?, &pins)?,
    };
    let stock = sail_verus::toolchain::Installed::locate_release(pins)?;
    let root = oracle_directory()?;
    for (label, source, expected) in NATIVE_CASES {
        let lowered = sail_verus::contracts::lower_profile(
            source,
            "neutral",
            if label == "order-profile-two" { 2 } else { 1 },
        )?;
        let path = root.join(format!("{label}.rs"));
        std::fs::write(&path, &lowered.source)?;
        std::fs::write(
            root.join(format!("{label}.mapping.json")),
            serde_json::to_vec_pretty(&lowered.manifest)?,
        )?;
        let mut observed = Vec::new();
        for (backend, executable) in [("fork", &fork.verus), ("stock", &stock.verus)] {
            let output = Command::new(executable)
                .env(
                    "VERUS_Z3_PATH",
                    std::env::var_os("SAIL_TEST_Z3")
                        .map_or_else(|| stock.z3.clone(), std::path::PathBuf::from),
                )
                .arg(&path)
                .args([
                    "--crate-type",
                    "lib",
                    "--crate-name",
                    "neutral",
                    "--output-json",
                    "--time",
                    "--no-report-long-running",
                ])
                .output()?;
            std::fs::write(
                root.join(format!("{label}.{backend}.stdout")),
                &output.stdout,
            )?;
            std::fs::write(
                root.join(format!("{label}.{backend}.stderr")),
                &output.stderr,
            )?;
            assert_eq!(
                output.status.success(),
                expected,
                "{label}/{backend}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let document = serde_json::Deserializer::from_slice(&output.stdout)
                .into_iter::<serde_json::Value>()
                .next()
                .ok_or("no actual verifier JSON document")??;
            let result = document
                .get("verification-results")
                .ok_or("missing actual verification result")?
                .clone();
            assert_eq!(
                result
                    .get("encountered-vir-error")
                    .and_then(serde_json::Value::as_bool),
                Some(false),
                "{label}/{backend}: source/compiler error is not a property refutation"
            );
            assert_eq!(
                result.get("success").and_then(serde_json::Value::as_bool),
                Some(expected)
            );
            assert_eq!(
                result
                    .get("is-verifying-entire-crate")
                    .and_then(serde_json::Value::as_bool),
                Some(true),
                "{label}/{backend}: partial verification does not qualify a control"
            );
            let completed = result
                .get(if expected { "verified" } else { "errors" })
                .and_then(serde_json::Value::as_u64)
                .ok_or("missing actual verification count")?;
            assert!(
                completed > 0,
                "{label}/{backend}: empty verification is not evidence"
            );
            observed.push(result);
        }
        assert_eq!(
            observed[0], observed[1],
            "{label}: fork and stock verdict/count mismatch"
        );
    }
    Ok(())
}

fn oracle_directory() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = std::env::var_os("SAIL_NEUTRAL_ORACLE_OUT") {
        let path = std::path::PathBuf::from(path);
        std::fs::create_dir(&path)?;
        return Ok(path);
    }
    for suffix in 0..1000 {
        let path = std::env::temp_dir().join(format!(
            "sail-neutral-oracle-{}-{suffix}",
            std::process::id()
        ));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err("cannot allocate a fresh native oracle evidence directory".into())
}
