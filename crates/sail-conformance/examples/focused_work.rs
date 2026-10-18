//! Untimed, explicitly partial Anvil module fixture observation.
//!
//! This reads an independently frozen controlled-fixture authority. It never
//! creates a `ProjectResult`, declares Current, publishes proof, or runs a child.
//! Usage: `--fixture DIR --authority-sha256 SHA256`.
#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use sail_verus::attribution::work::{self, Counts};
use sail_verus::attribution::{SessionRecord, json_documents};
use sail_verus::run::RunRecord;
use serde::Deserialize;
use serde_json::Value;

const MODULE: &str = "vdeployment_controller::proof::liveness::resource_match";
const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_FILES: usize = 16_384;

fn invalid(message: impl std::fmt::Display) -> io::Error {
    io::Error::other(message.to_string())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    path: PathBuf,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Invocation {
    pid: u32,
    argv: Vec<String>,
    exit_code: i32,
    complete_io: bool,
    stdout: Binding,
    stderr: Binding,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    pid: u32,
    transcript: Binding,
    air: Binding,
    provenance: Option<Binding>,
    direct_tap: Option<Binding>,
    raw_input: Option<Binding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authority {
    kind: String,
    schema: u32,
    run_id: String,
    case: String,
    module: String,
    /// Full frozen source/provider/lock/runtime/body inventory, not endpoints
    /// promoted into a compiler-consumption certificate.
    inputs: Vec<Binding>,
    /// The exact census from this owned invocation, independently established
    /// by the controlled caller. Session records alone cannot establish it.
    session_pids: Vec<u32>,
    solver: Binding,
    invocation: Invocation,
    oracle: Invocation,
    /// Complete names/modes from the genuine stock baseline, preserving repeats.
    expected_functions: Vec<(String, String, Option<String>)>,
    sessions: Vec<Session>,
}

fn read(root: &Path, relative: &Path) -> io::Result<Vec<u8>> {
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(invalid("nonliteral fixture path"));
    }
    let path = root.join(relative);
    if path.canonicalize()? != path {
        return Err(invalid("fixture alias"));
    }
    let before = std::fs::symlink_metadata(&path)?;
    if !before.is_file() || before.len() > MAX_FILE {
        return Err(invalid("unsupported fixture file"));
    }
    let file = std::fs::File::open(&path)?;
    let held = file.metadata()?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1).read_to_end(&mut bytes)?;
    let after = std::fs::symlink_metadata(path)?;
    if u64::try_from(bytes.len()).map_err(invalid)? != before.len()
        || held.len() != before.len()
        || after.len() != before.len()
        || held.modified()? != before.modified()?
        || after.modified()? != before.modified()?
    {
        return Err(invalid("fixture file changed while read"));
    }
    Ok(bytes)
}

fn bound(root: &Path, binding: &Binding) -> io::Result<Vec<u8>> {
    let bytes = read(root, &binding.path)?;
    if work::digest(&bytes) != binding.sha256 {
        return Err(invalid("fixture body digest differs"));
    }
    Ok(bytes)
}

fn invocation(root: &Path, item: &Invocation) -> io::Result<RunRecord> {
    if item.pid == 0 || !item.complete_io {
        return Err(invalid("incomplete owned invocation"));
    }
    let selectors: Vec<_> = item
        .argv
        .windows(2)
        .filter(|pair| pair[0] == "--verify-only-module")
        .map(|pair| pair[1].as_str())
        .collect();
    if selectors != [MODULE]
        || item
            .argv
            .iter()
            .filter(|arg| arg.as_str() == "--verify-only-module")
            .count()
            != 1
        || item.argv.iter().any(|arg| {
            let name = arg.split_once('=').map_or(arg.as_str(), |(name, _)| name);
            matches!(
                name,
                "--verify-module" | "--verify-function" | "--verify-root" | "--no-verify"
            ) || arg.starts_with("--verify-only-module=")
        })
    {
        return Err(invalid("focused selector differs"));
    }
    let (outputs, rest) = json_documents(&bound(root, &item.stdout)?);
    if !rest.iter().all(u8::is_ascii_whitespace) || outputs.len() != 1 {
        return Err(invalid("missing/unknown focused stdout"));
    }
    // VI-0028: the pinned stock/fork emitter omits verify_only_module from
    // this flag. Bind its actual literal value, never use it as scope authority.
    // The exact argv and full module/function catalogue establish partial scope.
    if outputs[0].pointer("/verification-results/is-verifying-entire-crate")
        != Some(&Value::Bool(true))
    {
        return Err(invalid("pinned focused emitter metadata differs"));
    }
    Ok(RunRecord {
        exit_code: Some(item.exit_code),
        stderr: String::from_utf8(bound(root, &item.stderr)?).map_err(invalid)?,
        outputs,
        stdout_rest: String::new(),
        wall_ms: 0.0,
        user_s: None,
        sys_s: None,
        max_rss_bytes: None,
    })
}

type Function = (String, String, Option<String>, Option<bool>);

fn functions(run: &RunRecord) -> io::Result<Vec<Function>> {
    let modules = run.outputs[0]
        .pointer("/times-ms/smt/smt-run-module-times")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("no focused function breakdown"))?;
    let mut result = Vec::new();
    for module in modules {
        if module["module"].as_str() != Some(MODULE) {
            return Err(invalid("unexpected function module"));
        }
        for function in module["function-breakdown"]
            .as_array()
            .ok_or_else(|| invalid("missing function list"))?
        {
            let name = function["function"]
                .as_str()
                .ok_or_else(|| invalid("missing function"))?;
            let prefix = format!("verifiable_controllers::{MODULE}::");
            if !name.starts_with(&prefix) {
                return Err(invalid("function outside exact module"));
            }
            let success = match function.get("success") {
                Some(Value::Bool(value)) => Some(*value),
                Some(Value::Null) => None,
                _ => return Err(invalid("unknown function verdict")),
            };
            let mode = match function.get("mode:") {
                Some(Value::String(value)) => Some(value.clone()),
                Some(Value::Null) | None => None,
                _ => return Err(invalid("unknown function mode")),
            };
            result.push((MODULE.to_owned(), name.to_owned(), mode, success));
        }
    }
    if result.is_empty() {
        return Err(invalid("empty focused function multiset"));
    }
    result.sort();
    Ok(result)
}

fn catalogue_outcome(case: &str, run: &RunRecord, functions: &[Function]) -> io::Result<()> {
    let results = &run.outputs[0]["verification-results"];
    let errors = results["errors"]
        .as_u64()
        .ok_or_else(|| invalid("missing error count"))?;
    let verified = results["verified"]
        .as_u64()
        .ok_or_else(|| invalid("missing verified count"))?;
    let accepted = if case == "fail" {
        run.exit_code.is_some_and(|code| code != 0)
            && results["success"] == false
            && errors > 0
            && functions.iter().any(|function| function.3 == Some(false))
    } else {
        run.exit_code == Some(0)
            && results["success"] == true
            && errors == 0
            && verified > 0
            && results["encountered-error"] == false
            && results["encountered-vir-error"] == false
            && functions.iter().all(|function| function.3 != Some(false))
    };
    if !accepted {
        return Err(invalid("catalogue case outcome differs"));
    }
    Ok(())
}

fn session(
    root: &Path,
    item: &Session,
    solver: &Binding,
) -> io::Result<(String, Counts, SessionRecord)> {
    let raw = bound(root, &item.transcript)?;
    let air = String::from_utf8(bound(root, &item.air)?).map_err(invalid)?;
    let stem = item
        .transcript
        .path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| invalid("nonliteral transcript name"))?;
    let (hash, counts) = work::transcript(&air, &raw, stem)?;
    let record = match (&item.provenance, &item.direct_tap, &item.raw_input) {
        (Some(provenance), None, None) => {
            let name = provenance
                .path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| invalid("unknown provenance name"))?;
            let prefix = format!("{}-", item.pid);
            if !name.starts_with(&prefix)
                || Path::new(name).extension() != Some(std::ffi::OsStr::new("json"))
            {
                return Err(invalid("session PID differs from provenance origin"));
            }
            work::session(&bound(root, provenance)?)?
        }
        (None, Some(tap), Some(input)) => {
            let raw = bound(root, tap)?;
            let rows: Vec<Value> = raw
                .split(|b| *b == b'\n')
                .filter(|line| !line.is_empty())
                .map(serde_json::from_slice)
                .collect::<Result<_, _>>()
                .map_err(invalid)?;
            let first = rows.first().ok_or_else(|| invalid("empty direct tap"))?;
            let last = rows.last().ok_or_else(|| invalid("empty direct tap"))?;
            if first["type"] != "start"
                || first["trace_version"] != 2
                || first["pid"].as_u64() != Some(u64::from(item.pid))
                || first.pointer("/solver/sha256").and_then(Value::as_str)
                    != Some(solver.sha256.as_str())
                || first
                    .pointer("/solver/path")
                    .and_then(Value::as_str)
                    .map(Path::new)
                    != Some(root.join(&solver.path).as_path())
                || last["type"] != "end"
                || last["complete"] != true
                || last["paired"] != true
                || last["exit_code"] != 0
                || rows.iter().any(|row| row["type"] == "solver-error")
                || work::digest(&bound(root, input)?) != hash
            {
                return Err(invalid("incomplete/unbound direct solver capture"));
            }
            SessionRecord {
                version: 1,
                input_sha256: hash.clone(),
                replies_served: 0,
                solver_started: true,
            }
        }
        _ => return Err(invalid("ambiguous session evidence")),
    };
    if record.input_sha256 != hash {
        return Err(invalid("literal input digest differs"));
    }
    Ok((hash, counts, record))
}

fn observe(root: &Path, expected_sha256: &str) -> io::Result<Value> {
    let bytes = read(root, Path::new("authority.json"))?;
    if work::digest(&bytes) != expected_sha256 {
        return Err(invalid("frozen authority differs"));
    }
    let authority: Authority = serde_json::from_slice(&bytes).map_err(invalid)?;
    if authority.kind != "sail-focused-module-fixture"
        || authority.schema != 1
        || authority.run_id.is_empty()
        || authority.module != MODULE
        || !["A", "body-early", "body-late", "fail", "fix"].contains(&authority.case.as_str())
        || authority.inputs.is_empty()
        || authority.inputs.len() > MAX_FILES
        || authority.sessions.is_empty()
        || authority.sessions.len() > MAX_FILES
    {
        return Err(invalid("unsupported focused fixture authority"));
    }
    for binding in &authority.inputs {
        bound(root, binding)?;
    }
    bound(root, &authority.solver)?;
    let actual = invocation(root, &authority.invocation)?;
    let oracle = invocation(root, &authority.oracle)?;
    let actual_functions = functions(&actual)?;
    let oracle_functions = functions(&oracle)?;
    catalogue_outcome(&authority.case, &actual, &actual_functions)?;
    catalogue_outcome(&authority.case, &oracle, &oracle_functions)?;
    let mut expected = authority.expected_functions;
    expected.sort();
    let names = |items: &[Function]| {
        items
            .iter()
            .map(|(m, n, k, _)| (m.clone(), n.clone(), k.clone()))
            .collect::<Vec<_>>()
    };
    if expected.is_empty()
        || names(&oracle_functions) != expected
        || actual_functions != oracle_functions
    {
        return Err(invalid("full genuine-stock function multiset differs"));
    }
    let comparison = sail_conformance::compare(&oracle, &actual);
    if comparison.verdict() != sail_conformance::Verdict::Same {
        return Err(invalid(format!(
            "genuine-stock mismatch: {:?}",
            comparison.differences
        )));
    }
    let census: BTreeSet<_> = authority.session_pids.iter().copied().collect();
    if census.len() != authority.session_pids.len() || census.contains(&0) {
        return Err(invalid("invalid independent session census"));
    }
    let mut observed = BTreeSet::new();
    let mut logs: BTreeMap<String, Vec<Counts>> = BTreeMap::new();
    let mut records = Vec::new();
    for item in &authority.sessions {
        if !observed.insert(item.pid) {
            return Err(invalid("duplicate session PID"));
        }
        let (hash, counts, record) = session(root, item, &authority.solver)?;
        logs.entry(hash).or_default().push(counts);
        records.push(record);
    }
    if observed != census {
        return Err(invalid("session census coverage differs"));
    }
    let work = work::join(logs, records)?;
    for binding in &authority.inputs {
        bound(root, binding)?;
    }
    Ok(
        serde_json::json!({"schema":1,"run_id":authority.run_id,"case":authority.case,
        "scope":"partial owned Anvil module fixture; not whole-target Current, proof reuse or default qualification",
        "partial":true,"fixture_authority_sha256":expected_sha256,"module":MODULE,
        "work":work,"functions":actual_functions,"rlimit_drift":comparison.rlimit_drift}),
    )
}

fn run() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [fixture_flag, fixture, hash_flag, hash] = args.as_slice() else {
        return Err(invalid("usage: --fixture DIR --authority-sha256 SHA256"));
    };
    if fixture_flag != "--fixture" || hash_flag != "--authority-sha256" {
        return Err(invalid("usage: --fixture DIR --authority-sha256 SHA256"));
    }
    let root = Path::new(fixture).canonicalize()?;
    let result = observe(&root, hash)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&result).map_err(invalid)?
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error[SAIL0931]: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf, Value);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn write(root: &Path, name: &str, bytes: &[u8]) -> io::Result<Value> {
        std::fs::write(root.join(name), bytes)?;
        Ok(serde_json::json!({"path":name,"sha256":work::digest(bytes)}))
    }

    fn document(success: bool) -> Value {
        serde_json::json!({"verification-results":{"success":success,"verified":u64::from(success),
            "errors":u64::from(!success),"encountered-error":false,"encountered-vir-error":false,
            "is-verifying-entire-crate":true},"func-details":{},
            "times-ms":{"smt":{"smt-run-module-times":[{"module":MODULE,"function-breakdown":[{
                "function":format!("verifiable_controllers::{MODULE}::controlled"),"mode:":"proof",
                "success":success,"rlimit-count":1}]}]}}})
    }

    fn fixture(case: &str) -> io::Result<Fixture> {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let serial = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("sail-focused-{}-{serial}", std::process::id()));
        std::fs::create_dir(&root)?;
        let root = root.canonicalize()?;
        let success = case != "fail";
        let raw = serde_json::to_vec(&document(success)).map_err(invalid)?;
        let stdout = write(&root, "stdout.json", &raw)?;
        let stderr = write(
            &root,
            "stderr",
            if success {
                b""
            } else {
                b"error: controlled proof failure\n"
            },
        )?;
        let solver = write(
            &root,
            "solver",
            b"explicit fixture solver identity; never executed",
        )?;
        let input = write(
            &root,
            "frozen-input",
            b"controlled source/runtime/lock inventory",
        )?;
        let transcript =
            b";;;>>> QUERY\n(push)\n(check-sat)\n(pop)\n;;;<<<\n;;;>>> RESPONSE\nunsat\n;;;<<<\n";
        let log = write(&root, "module.smt_transcript", transcript)?;
        let air = write(
            &root,
            "module.air",
            b";; Function-Def controlled\n(check-valid dummy)\n",
        )?;
        let pipe = sail_verus::attribution::Transcript::parse(transcript).pipe_input();
        let raw_input = write(&root, "input.smt2", &pipe)?;
        let tap = serde_json::json!([{"type":"start","trace_version":2,"pid":241,
            "solver":{"path":root.join("solver"),"sha256":solver["sha256"]}},
            {"type":"end","complete":true,"paired":true,"exit_code":0}]);
        let lines = tap
            .as_array()
            .ok_or_else(|| invalid("fixture tap array"))?
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()
            .map_err(invalid)?
            .join("\n");
        let direct = write(&root, "direct.jsonl", lines.as_bytes())?;
        let invocation = serde_json::json!({"pid":420,"argv":["verus","--verify-only-module",MODULE,"--output-json","--time"],
            "exit_code":if success {0}else{1},"complete_io":true,"stdout":stdout,"stderr":stderr});
        Ok(Fixture(
            root,
            serde_json::json!({"kind":"sail-focused-module-fixture","schema":1,
            "run_id":"explicit-controlled-fixture","case":case,"module":MODULE,"inputs":[input],
            "session_pids":[241],"solver":solver,"invocation":invocation,"oracle":invocation,
            "expected_functions":[[MODULE,format!("verifiable_controllers::{MODULE}::controlled"),"proof"]],
            "sessions":[{"pid":241,"transcript":log,"air":air,"provenance":null,
                "direct_tap":direct,"raw_input":raw_input}]}),
        ))
    }

    fn observe_fixture(fixture: &Fixture) -> io::Result<Value> {
        let raw = serde_json::to_vec(&fixture.1).map_err(invalid)?;
        std::fs::write(fixture.0.join("authority.json"), &raw)?;
        observe(&fixture.0, &work::digest(&raw))
    }

    #[test]
    fn canonical_catalogue_remains_partial_and_counts_real_session_work() -> io::Result<()> {
        // All compiler/Z3/ownership facts here are deliberate doubles. No actual
        // project, SMT answer, source edit or native qualification is inferred.
        for case in ["A", "body-early", "body-late", "fail", "fix"] {
            let result = observe_fixture(&fixture(case)?)?;
            assert_eq!(result["partial"], true);
            assert_eq!(result["work"]["sessions"], 1);
            assert_eq!(result["work"]["actual_solver_checks"], 1);
            assert!(result.get("currentness").is_none());
        }
        Ok(())
    }

    #[test]
    fn full_oracle_function_verdict_and_diagnostic_mismatches_refuse() -> io::Result<()> {
        let mut f = fixture("A")?;
        f.1["expected_functions"] = serde_json::json!([]);
        assert!(observe_fixture(&f).is_err());
        let mut f = fixture("A")?;
        let raw = serde_json::to_vec(&document(false)).map_err(invalid)?;
        f.1["invocation"]["stdout"] = write(&f.0, "changed.json", &raw)?;
        assert!(observe_fixture(&f).is_err());
        let mut f = fixture("A")?;
        f.1["invocation"]["stderr"] =
            write(&f.0, "changed.stderr", b"unknown literal diagnostic\n")?;
        assert!(observe_fixture(&f).is_err());
        Ok(())
    }

    #[test]
    fn independent_census_multiplicity_and_selector_are_required() -> io::Result<()> {
        for census in [
            serde_json::json!([]),
            serde_json::json!([241, 241]),
            serde_json::json!([242]),
        ] {
            let mut f = fixture("A")?;
            f.1["session_pids"] = census;
            assert!(observe_fixture(&f).is_err());
        }
        let mut f = fixture("A")?;
        f.1["invocation"]["argv"] = serde_json::json!(["verus", "--verify-only-module", "foreign"]);
        assert!(observe_fixture(&f).is_err());
        Ok(())
    }

    #[test]
    fn pinned_emitter_metadata_is_literal_and_never_scope_authority() -> io::Result<()> {
        for value in [
            Value::Bool(false),
            Value::Null,
            Value::String("true".to_owned()),
        ] {
            for which in ["invocation", "oracle"] {
                let mut f = fixture("A")?;
                let mut doc = document(true);
                doc["verification-results"]["is-verifying-entire-crate"] = value.clone();
                let raw = serde_json::to_vec(&doc).map_err(invalid)?;
                f.1[which]["stdout"] = write(&f.0, "changed-metadata.json", &raw)?;
                assert!(observe_fixture(&f).is_err());
            }
        }
        // Stock's misleading true flag cannot authorize an absent selector.
        for which in ["invocation", "oracle"] {
            let mut f = fixture("A")?;
            f.1[which]["argv"] = serde_json::json!(["verus"]);
            assert!(observe_fixture(&f).is_err());
        }
        Ok(())
    }

    #[test]
    fn equals_and_duplicate_selector_variants_refuse() -> io::Result<()> {
        for argument in [
            "--verify-module",
            "--verify-module=foreign",
            "--verify-function",
            "--verify-function=foreign",
            "--verify-root",
            "--verify-root=foreign",
            "--no-verify",
            "--no-verify=true",
            "--verify-only-module=foreign",
            "--verify-only-module",
        ] {
            for which in ["invocation", "oracle"] {
                let mut f = fixture("A")?;
                f.1[which]["argv"]
                    .as_array_mut()
                    .ok_or_else(|| invalid("fixture argv"))?
                    .push(Value::String(argument.to_owned()));
                assert!(observe_fixture(&f).is_err(), "selector variant {argument}");
            }
        }
        Ok(())
    }

    #[test]
    fn matching_wrong_catalogue_outcomes_never_authorize_observation() -> io::Result<()> {
        let mut f = fixture("A")?;
        f.1["case"] = Value::String("fail".to_owned());
        assert!(observe_fixture(&f).is_err());
        for case in ["A", "body-early", "body-late", "fix"] {
            let mut f = fixture("fail")?;
            f.1["case"] = Value::String(case.to_owned());
            assert!(observe_fixture(&f).is_err());
        }
        let mut f = fixture("fail")?;
        f.1["invocation"]["exit_code"] = Value::from(0);
        f.1["oracle"]["exit_code"] = Value::from(0);
        assert!(observe_fixture(&f).is_err());
        // Matching nonzero exit alone is not a genuine failed function.
        let mut f = fixture("fail")?;
        let raw = serde_json::to_vec(&document(true)).map_err(invalid)?;
        let output = write(&f.0, "matching-success.json", &raw)?;
        f.1["invocation"]["stdout"] = output.clone();
        f.1["oracle"]["stdout"] = output;
        assert!(observe_fixture(&f).is_err());
        let mut f = fixture("fail")?;
        let mut doc = document(false);
        doc["verification-results"]["errors"] = Value::from(0);
        let raw = serde_json::to_vec(&doc).map_err(invalid)?;
        let output = write(&f.0, "matching-no-errors.json", &raw)?;
        f.1["invocation"]["stdout"] = output.clone();
        f.1["oracle"]["stdout"] = output;
        assert!(observe_fixture(&f).is_err());
        Ok(())
    }

    #[test]
    fn incomplete_direct_capture_and_changed_frozen_inputs_refuse() -> io::Result<()> {
        let mut f = fixture("A")?;
        f.1["sessions"][0]["direct_tap"] =
            write(&f.0, "incomplete.jsonl", b"{\"type\":\"start\"}\n")?;
        assert!(observe_fixture(&f).is_err());
        let f = fixture("A")?;
        std::fs::write(f.0.join("frozen-input"), b"different bytes")?;
        assert!(observe_fixture(&f).is_err());
        Ok(())
    }
}
