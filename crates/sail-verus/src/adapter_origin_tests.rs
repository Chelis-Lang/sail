//! Origin-state completion is independent of whole-target proof authority.

use super::*;
use std::collections::BTreeMap;

type TestResult = std::result::Result<(), Box<dyn std::error::Error>>;

struct Fixture {
    root: PathBuf,
    runtime: Runtime,
    identity: TargetIdentity,
}

impl Fixture {
    fn new(label: &str) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let root = std::env::temp_dir().join(format!(
            "sail-origin-observation-{label}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root)?;
        let identity = TargetIdentity {
            manifest: root.join("Cargo.toml"),
            name: "demo".into(),
            crate_type: "lib".into(),
            rustc_args_sha256: "arguments".into(),
            cargo_name: None,
        };
        let runtime = Runtime {
            run_id: "fresh-observation".into(),
            args: Vec::new(),
            z3: root.join("z3"),
            z3_sha256: "solver".into(),
            daemon: false,
            reuse: true,
            sidecars: root.join("sidecars"),
            invocations: root.join("invocations"),
            original_wrapper: root.join("verus"),
            receipts: root.join("receipts"),
            compiler_read_origins: root.join("origins"),
            context_sha256: "context".into(),
            input_revision: "revision".into(),
            packages: BTreeMap::from([(identity.manifest.clone(), "package".into())]),
            target_kinds: BTreeMap::new(),
            engine: None,
            retained: None,
            attribution_logs: false,
        };
        Ok(Self {
            root,
            runtime,
            identity,
        })
    }

    fn marker(&self) -> PathBuf {
        self.runtime
            .compiler_read_origins
            .join("pending")
            .join(format!("{}.json", self.identity.key()))
    }

    fn observe(&self, readable: bool, observed_reusable: bool) -> TestResult {
        crate::managed::compiler_origin::started(&self.runtime, &self.identity)?;
        assert!(self.marker().is_file());
        complete_origin_observation(
            &self.runtime,
            &self.identity.key(),
            readable,
            observed_reusable,
        );
        Ok(())
    }

    fn close(self) -> TestResult {
        std::fs::remove_dir_all(self.root)?;
        Ok(())
    }
}

#[test]
fn qualified_partial_and_no_verify_inputs_clear_only_non_authorizing_state() -> TestResult {
    let fixture = Fixture::new("partial")?;
    let (documents, _) = crate::attribution::json_documents(include_bytes!(
        "../tests/fixtures/engine-terminal/success.json"
    ));
    let attribution = crate::attribution::attribute(
        &documents,
        None,
        &crate::attribution::Provenance::NoCache,
        None,
    );
    for args in [
        vec!["--".into(), "--verify-module".into(), "demo".into()],
        vec!["--".into(), "--no-verify".into()],
    ] {
        let report = crate::report::build(&crate::report::Inputs {
            args: &args,
            exit_code: Some(0),
            solver_path: None,
            solver_sha256: None,
            daemon: false,
            attribution: &attribution,
            diagnostics: &[],
        });
        assert!(
            !receipt_success(&report, &args),
            "origin state must not confer whole-proof success"
        );
        let before = report.clone();
        let environment = BTreeMap::new();
        fixture.observe(
            true,
            crate::managed::compiler_origin::reusable(&environment),
        )?;
        assert!(
            !fixture.marker().exists(),
            "known scoped/compiler-only inputs remained dirty"
        );
        assert_eq!(report, before, "origin completion changed the report");
        assert!(
            !fixture.runtime.receipts.exists(),
            "origin completion published a receipt"
        );
    }
    fixture.close()
}

#[test]
fn unqualified_compiler_observations_keep_origin_state() -> TestResult {
    let mut fixture = Fixture::new("refusals")?;
    // Failed, missing, incomplete and restarted observations never produce the
    // qualified compiler_environment in the caller; all pass false here.
    for label in ["failed", "missing", "incomplete", "restarted"] {
        fixture.observe(true, false)?;
        assert!(
            fixture.marker().is_file(),
            "{label} observation cleared origin state"
        );
    }
    fixture.observe(false, true)?;
    assert!(
        fixture.marker().is_file(),
        "vanished verifier cleared origin state"
    );
    for name in [
        verus_channel::COMMON,
        "SAIL_PROVENANCE_DIR",
        "SAIL_COMPILER_INPUTS",
        "SAIL_COMPILER_INPUTS_BINDING",
        "SAIL_ENGINE",
        "SAIL_ENGINE_EVENTS",
        "SAIL_ENGINE_CONTEXT",
        "SAIL_ENGINE_FOCUS_REPORT",
        "SAIL_ENGINE_FUNCTION",
        "SAIL_RUSTC_DEV_SOURCE_BASE",
        "SAIL_VIR_SPAN_REMAP",
    ] {
        let environment = BTreeMap::from([(name.into(), None)]);
        fixture.observe(
            true,
            crate::managed::compiler_origin::reusable(&environment),
        )?;
        assert!(
            fixture.marker().is_file(),
            "source-sensitive read {name} cleared origin state"
        );
    }
    fixture.runtime.reuse = false;
    fixture.observe(true, true)?;
    assert!(
        fixture.marker().is_file(),
        "no-reuse observation cleared origin state"
    );
    fixture.close()
}
