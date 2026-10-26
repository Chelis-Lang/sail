//! Exact two-phase declaration fields from the existing unselected producer.
//!
//! These facts describe compiler records, not complete physical input closure.

use sail_acceptance::authority::ObservedAttempt;
use sail_acceptance::inputs::{InputFact, InputValue};
use sail_acceptance::phase_inputs::{
    Inventory, PhaseDocument, PhaseRecord, PhaseRequest, inventory_covers, joined_rows_recorded,
    seal_phase_inputs,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// The existing producer's full final-document bound.
const LIMIT: usize = 8 * 1024 * 1024;
const ENVIRONMENT: u8 = 3;

/// Untrusted decoded producer fields. Duplicate keys never overwrite a phase.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::managed) struct Document {
    pub kind: String,
    pub schema: u32,
    pub binding: String,
    pub pid: u32,
    pub complete: bool,
    #[serde(deserialize_with = "unique_map")]
    pub phases: BTreeMap<String, Phase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::managed) struct Phase {
    #[serde(deserialize_with = "unique_map")]
    pub sources: BTreeMap<String, Source>,
    pub extra_tracked_files: Vec<String>,
    #[serde(deserialize_with = "unique_map")]
    pub tracked_environment: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in crate::managed) struct Source {
    pub checksum: String,
    pub size: u64,
}

fn unique_map<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
    decoder: D,
) -> std::result::Result<BTreeMap<String, V>, D::Error> {
    struct Unique<V>(std::marker::PhantomData<fn() -> V>);
    impl<'de, V: Deserialize<'de>> Visitor<'de> for Unique<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map with unique literal keys")
        }

        fn visit_map<A: MapAccess<'de>>(
            self,
            mut map: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some((name, value)) = map.next_entry::<String, V>()? {
                if result.insert(name, value).is_some() {
                    return Err(serde::de::Error::custom("duplicate compiler phase key"));
                }
            }
            Ok(result)
        }
    }
    decoder.deserialize_map(Unique(std::marker::PhantomData))
}

enum Value<'a> {
    Bytes(&'a [u8]),
    Length([u8; 8]),
    Absent,
}

struct Row<'a> {
    role: u8,
    name: &'a [u8],
    value: Value<'a>,
}

impl Row<'_> {
    fn fact(&self) -> InputFact<'_> {
        InputFact {
            namespace: self.role,
            name: self.name,
            value: match &self.value {
                Value::Bytes(bytes) => InputValue::Present(bytes),
                Value::Length(bytes) => InputValue::Present(bytes),
                Value::Absent => InputValue::Absent,
            },
        }
    }
}

fn rows(phase: &Phase) -> Vec<Row<'_>> {
    let mut rows = Vec::new();
    for (name, source) in &phase.sources {
        rows.push(Row {
            role: 0,
            name: name.as_bytes(),
            value: Value::Bytes(source.checksum.as_bytes()),
        });
        rows.push(Row {
            role: 1,
            name: name.as_bytes(),
            value: Value::Length(source.size.to_le_bytes()),
        });
    }
    for name in &phase.extra_tracked_files {
        // A tracked pathname is a declaration, not an attestation of its bytes.
        rows.push(Row {
            role: 2,
            name: name.as_bytes(),
            value: Value::Bytes(&[]),
        });
    }
    for (name, value) in &phase.tracked_environment {
        rows.push(Row {
            role: ENVIRONMENT,
            name: name.as_bytes(),
            value: value
                .as_ref()
                .map_or(Value::Absent, |value| Value::Bytes(value.as_bytes())),
        });
    }
    rows
}

fn inspect<T>(
    document: &Document,
    inspect: impl FnOnce(&[PhaseRecord<'_>], Inventory<'_>) -> Option<T>,
) -> Option<T> {
    if document.phases.len() != 2 {
        return None;
    }
    let verification = rows(document.phases.get("verification")?);
    let compilation = rows(document.phases.get("compilation")?);
    let proposed: BTreeMap<_, _> = verification
        .iter()
        .chain(&compilation)
        .map(|row| ((row.role, row.name), row))
        .collect();
    let first: Vec<_> = verification.iter().map(Row::fact).collect();
    let second: Vec<_> = compilation.iter().map(Row::fact).collect();
    let joined: Vec<_> = proposed.values().map(|row| row.fact()).collect();
    let (first_order, first_inverse) = crate::parallel::proposed_fact_order(&first);
    let (second_order, second_inverse) = crate::parallel::proposed_fact_order(&second);
    let (joined_order, joined_inverse) = crate::parallel::proposed_fact_order(&joined);
    let phases = [
        PhaseRecord {
            tag: 0,
            fields: Inventory {
                rows: &first,
                order: &first_order,
                inverse: &first_inverse,
            },
        },
        PhaseRecord {
            tag: 1,
            fields: Inventory {
                rows: &second,
                order: &second_order,
                inverse: &second_inverse,
            },
        },
    ];
    inspect(
        &phases,
        Inventory {
            rows: &joined,
            order: &joined_order,
            inverse: &joined_inverse,
        },
    )
}

fn environment_rows(rows: &[InputFact<'_>]) -> Option<BTreeMap<String, Option<String>>> {
    rows.iter()
        .filter(|row| row.namespace == ENVIRONMENT)
        .map(|row| {
            let name = std::str::from_utf8(row.name).ok()?.to_owned();
            let value = match &row.value {
                InputValue::Present(bytes) => Some(std::str::from_utf8(bytes).ok()?.to_owned()),
                InputValue::Absent => None,
                InputValue::Unreadable => return None,
            };
            Some((name, value))
        })
        .collect()
}

/// Exact union used by both existing experimental compiler-read consumers.
/// Their original source/header checks still precede proof admission.
pub(in crate::managed) fn environment(
    document: &Document,
) -> Option<BTreeMap<String, Option<String>>> {
    inspect(document, |phases, joined| {
        if !inventory_covers(&joined, &phases[0].fields)
            || !inventory_covers(&joined, &phases[1].fields)
            || !joined_rows_recorded(&phases[0].fields, &phases[1].fields, &joined)
        {
            return None;
        }
        environment_rows(joined.rows)
    })
}

/// Bound the exact parsed field union to the original request and actual child.
/// This unselected capability supplies no proof-success or compiler-domain token.
pub(in crate::managed) fn admit_observed(
    request: PhaseRequest,
    observed: &ObservedAttempt<'_>,
    raw: &[u8],
) -> Option<BTreeMap<String, Option<String>>> {
    if raw.len() > LIMIT {
        return None;
    }
    let document: Document = serde_json::from_slice(raw).ok()?;
    inspect(&document, |phases, joined| {
        let fields = PhaseDocument {
            kind: document.kind.as_bytes(),
            schema: document.schema,
            pid: document.pid,
            binding: document.binding.as_bytes(),
            complete: document.complete,
            phases,
        };
        let seal = seal_phase_inputs(request, observed, &fields, joined)?;
        environment_rows(seal.facts())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sail_acceptance::authority::{AttemptLedger, CompilerUnit, ProcessDescriptor};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};

    type Failure = Box<dyn std::error::Error>;

    fn directory(name: &str) -> std::io::Result<PathBuf> {
        let path =
            std::env::temp_dir().join(format!("sail-phase-fields-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path)?;
        Ok(path)
    }

    fn document(pid: u32, binding: &str) -> serde_json::Value {
        serde_json::json!({
            "kind":"sail-compiler-read-inputs", "schema":1, "pid":pid, "binding":binding, "complete":true,
            "phases":{
                "verification":{"sources":{"lib.rs":{"checksum":"sha256=source","size":208}},
                    "extra_tracked_files":[], "tracked_environment":{"ORDINARY_PROOF_FLAG":"7", " ABSENT ":null, "EMPTY":""}},
                "compilation":{"sources":{"lib.rs":{"checksum":"sha256=source","size":208}},
                    "extra_tracked_files":[], "tracked_environment":{" ABSENT ":null,"EMPTY":""}}
            }
        })
    }

    // Uses actual owned child/output values. This is a native producer/reader
    // witness; source/OS completeness is not inferred from the scripted payload.
    fn observe<'a>(
        ledger: &mut AttemptLedger<'a>,
        command: &mut Command,
        binding: &[u8],
    ) -> Result<(Vec<PhaseRequest>, ObservedAttempt<'a>, Output), Failure> {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        let allocation = ledger.allocate().ok_or("attempt exhausted")?;
        let requests = (0..6)
            .map(|_| PhaseRequest::for_attempt(&allocation, binding))
            .collect();
        let descriptor = ProcessDescriptor {
            program: command.get_program().as_encoded_bytes().to_vec(),
            arguments: command
                .get_args()
                .map(|arg| arg.as_encoded_bytes().to_vec())
                .collect(),
            directory: command
                .get_current_dir()
                .map(|path| path.as_os_str().as_encoded_bytes().to_vec()),
            overrides: command
                .get_envs()
                .map(|(name, value)| {
                    (
                        name.as_encoded_bytes().to_vec(),
                        value.map(|value| value.as_encoded_bytes().to_vec()),
                    )
                })
                .collect(),
        };
        let reserved = allocation.configure(descriptor);
        let child = command.spawn()?;
        let running = reserved.started(child.id()).ok_or("zero child PID")?;
        let output = child.wait_with_output()?;
        let observed = running
            .stdout_eof(output.stdout.clone())
            .stderr_eof(output.stderr.clone())
            .exited(output.status.code())
            .finish()
            .ok_or("incomplete child")?;
        Ok((requests, observed, output))
    }

    #[test]
    fn exact_phase_fields_keep_raw_names_absence_and_both_read_versions() -> Result<(), Failure> {
        let raw = document(7, "binding");
        let fields: Document = serde_json::from_value(raw.clone())?;
        let union = environment(&fields).ok_or("missing exact union")?;
        assert_eq!(union.get("ORDINARY_PROOF_FLAG"), Some(&Some("7".into())));
        assert_eq!(union.get(" ABSENT "), Some(&None));
        assert_eq!(union.get("EMPTY"), Some(&Some(String::new())));
        assert!(!union.contains_key("ABSENT"));
        let mut conflict = raw.clone();
        conflict["phases"]["compilation"]["tracked_environment"]["ORDINARY_PROOF_FLAG"] =
            serde_json::json!("9");
        assert!(environment(&serde_json::from_value(conflict)?).is_none());
        let mut conflict = raw;
        conflict["phases"]["compilation"]["sources"]["lib.rs"]["checksum"] =
            serde_json::json!("sha256=other");
        assert!(environment(&serde_json::from_value(conflict)?).is_none());
        Ok(())
    }

    #[test]
    fn duplicate_phase_source_and_environment_keys_are_not_overwritten() -> Result<(), Failure> {
        let raw = serde_json::to_string(&document(7, "binding"))?;
        for (needle, replacement) in [
            (
                "\"verification\":",
                "\"verification\":{\"sources\":{},\"extra_tracked_files\":[],\"tracked_environment\":{}},\"verification\":",
            ),
            (
                "\"ORDINARY_PROOF_FLAG\":\"7\"",
                "\"ORDINARY_PROOF_FLAG\":null,\"ORDINARY_PROOF_FLAG\":\"7\"",
            ),
            (
                "\"lib.rs\":{",
                "\"lib.rs\":{\"checksum\":\"different\",\"size\":1},\"lib.rs\":{",
            ),
        ] {
            assert!(raw.contains(needle));
            let duplicated = raw.replacen(needle, replacement, 1);
            assert!(serde_json::from_str::<Document>(&duplicated).is_err());
        }
        Ok(())
    }

    #[test]
    fn real_completed_child_needs_same_attempt_and_both_complete_phases() -> Result<(), Failure> {
        let root = directory("child")?;
        let unit = CompilerUnit {
            manifest: b"manifest".to_vec(),
            name: b"unit".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let mut ledger = AttemptLedger::new(b"phase-run", &unit);
        let path = root.join("inputs.json");
        let mut command = Command::new("python3");
        command.arg("-c").arg("import json,os,sys; d=json.loads(sys.argv[2]); d['pid']=os.getpid(); open(sys.argv[1],'w').write(json.dumps(d)); sys.exit(1)")
            .arg(&path).arg(serde_json::to_string(&document(0, "binding"))?);
        let (mut requests, observed, output) = observe(&mut ledger, &mut command, b"binding")?;
        assert_eq!(output.status.code(), Some(1));
        let raw = std::fs::read(&path)?;
        let union = admit_observed(requests.pop().ok_or("request missing")?, &observed, &raw)
            .ok_or("complete normal-failure accounting refused")?;
        assert_eq!(union.get("ORDINARY_PROOF_FLAG"), Some(&Some("7".into())));
        let later = ledger.allocate().ok_or("attempt exhausted")?;
        let later_request = PhaseRequest::for_attempt(&later, b"binding");
        assert!(
            admit_observed(later_request, &observed, &raw).is_none(),
            "a fresh slot cannot restamp an old child"
        );
        let mut other_run = AttemptLedger::new(b"other-run", &unit);
        let other = other_run.allocate().ok_or("attempt exhausted")?;
        assert!(
            admit_observed(
                PhaseRequest::for_attempt(&other, b"binding"),
                &observed,
                &raw
            )
            .is_none()
        );
        let changed = |change: fn(&mut serde_json::Value)| -> Result<Vec<u8>, Failure> {
            let mut value: serde_json::Value = serde_json::from_slice(&raw)?;
            change(&mut value);
            Ok(serde_json::to_vec(&value)?)
        };
        for raw in [
            changed(|value| {
                let actual = value["pid"].as_u64().unwrap_or_default();
                value["pid"] = serde_json::json!(if actual == 1 { 2 } else { 1 });
            })?,
            changed(|value| value["binding"] = serde_json::json!("other"))?,
            changed(|value| value["complete"] = serde_json::json!(false))?,
            changed(|value| {
                if let Some(phases) = value["phases"].as_object_mut() {
                    phases.remove("verification");
                }
            })?,
            raw[..raw.len() - 1].to_vec(),
        ] {
            assert!(
                admit_observed(requests.pop().ok_or("request missing")?, &observed, &raw).is_none()
            );
        }
        std::fs::remove_dir_all(root)?;
        Ok(())
    }

    #[test]
    #[ignore = "requires the frozen pinned native fork build and an external output directory"]
    fn actual_compiler_phase_record_preserves_ghost_environment() -> Result<(), Failure> {
        let build =
            std::env::var_os("SAIL_ACTUAL_PHASE_BUILD").ok_or("missing SAIL_ACTUAL_PHASE_BUILD")?;
        let out = PathBuf::from(
            std::env::var_os("SAIL_ACTUAL_PHASE_OUT").ok_or("missing SAIL_ACTUAL_PHASE_OUT")?,
        );
        std::fs::create_dir(&out)?;
        let pins = crate::toolchain::Pins::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/toolchain.toml"),
        )?;
        let runtime = crate::engine::Installed::at(Path::new(&build), &pins)?;
        let source = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../corpus/repros/ghost-environment-boundary/src/lib.rs"),
        )?;
        assert_eq!(
            crate::managed::digest(&source),
            "3a1b9d3476f07da80e43a156ce59864bb551a0a29053a21b33275eea2ba04612"
        );
        let file = out.join("lib.rs");
        std::fs::write(&file, source)?;
        let observation = out.join("observation");
        std::fs::create_dir(&observation)?;
        let artifacts = out.join("output");
        std::fs::create_dir(&artifacts)?;
        let unit = CompilerUnit {
            manifest: file.as_os_str().as_encoded_bytes().to_vec(),
            name: b"lib".to_vec(),
            crate_type: b"lib".to_vec(),
            cargo_name: None,
        };
        let mut ledger = AttemptLedger::new(b"actual-phase-witness", &unit);
        let binding = "0".repeat(64);
        let mut command = Command::new(&runtime.verus);
        command.env_clear();
        for name in ["HOME", "USER", "PATH", "TMPDIR", "RUSTUP_HOME"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
            .env("RUSTUP_TOOLCHAIN", &pins.verus.rust_toolchain)
            .env("VERUS_Z3_PATH", runtime.verus.with_file_name("z3"))
            .env("ORDINARY_PROOF_FLAG", "7")
            .env("SAIL_COMPILER_INPUTS", &observation)
            .env("SAIL_COMPILER_INPUTS_BINDING", &binding)
            .arg(&file)
            .args([
                "--crate-type=lib",
                "--compile",
                "--output-json",
                "--time",
                "-Zchecksum-hash-algorithm=sha256",
                "--emit=dep-info,metadata,link",
                "--out-dir",
            ])
            .arg(&artifacts);
        let (mut requests, observed, output) =
            observe(&mut ledger, &mut command, binding.as_bytes())?;
        std::fs::write(out.join("stdout"), &output.stdout)?;
        std::fs::write(out.join("stderr"), &output.stderr)?;
        assert!(output.status.success());
        let raw = std::fs::read(observation.join("inputs.json"))?;
        let union = admit_observed(requests.pop().ok_or("request missing")?, &observed, &raw)
            .ok_or("actual compiler phase union refused")?;
        assert_eq!(union.get("ORDINARY_PROOF_FLAG"), Some(&Some("7".into())));
        let document: Document = serde_json::from_slice(&raw)?;
        assert!(
            !document.phases["compilation"]
                .tracked_environment
                .contains_key("ORDINARY_PROOF_FLAG")
        );
        let erased = std::fs::read_to_string(artifacts.join("lib.d"))?;
        assert!(!erased.contains("ORDINARY_PROOF_FLAG"));
        std::fs::write(
            out.join("accepted-environment.json"),
            serde_json::to_vec_pretty(&union)?,
        )?;
        Ok(())
    }
}
