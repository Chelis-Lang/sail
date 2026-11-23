//! Candidate selection only. Current-source validation owns every verdict.

use super::{command, external_dive, periodic, validated};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

const MAX_DONORS: usize = 8;
const MAX_ATTEMPTS: usize = 16;
const MAX_WORD: u64 = 8 << 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u32,
    donors: Vec<Entry>,
    #[serde(default)]
    layout: Layout,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    module: PathBuf,
    config: PathBuf,
    word: PathBuf,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Layout {
    integer_carrier: Option<String>,
    #[serde(default)]
    renames: BTreeMap<String, String>,
    #[serde(default)]
    fields: Vec<Field>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Field {
    path: Vec<String>,
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Integer,
    IntegerOrModel,
    Model,
}

struct Donor {
    module: PathBuf,
    cfg: PathBuf,
    provenance: Value,
    word: Value,
    config: Value,
}

pub(super) struct Search {
    pub(super) report: Option<Value>,
    pub(super) detail: Value,
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> std::io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "sail-witness-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            builder.mode(0o700);
        }
        builder.create(&dir)?;
        Ok(Self(dir))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    let file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}

fn configured(cfg: &Value, property: &str) -> bool {
    cfg["properties"]
        .as_array()
        .is_some_and(|properties| properties.iter().any(|p| p["text"] == property))
}

fn claimed(word: &Value, property: &str) -> bool {
    word["properties"]
        .as_array()
        .is_some_and(|properties| properties.iter().any(|p| p == property))
}

fn load(entry: &Entry, base: &Path, module: &Path, property: &str) -> Option<Donor> {
    if module.file_name()? != entry.module.file_name()? {
        return None;
    }
    let module = fs::canonicalize(base.join(&entry.module)).ok()?;
    let cfg_path = fs::canonicalize(base.join(&entry.config)).ok()?;
    let source = read(&module, 1 << 20)?;
    let cfg = read(&cfg_path, 128 << 10)?;
    let config = sail_dive::witness_configuration(std::str::from_utf8(&cfg).ok()?)?;
    if !configured(&config, property) {
        return None;
    }
    let path = base.join(&entry.word);
    let bytes = read(&path, MAX_WORD)?;
    let word: Value = serde_json::from_slice(&bytes).ok()?;
    if !claimed(&word, property)
        || !matches!(word["length"].as_u64(), Some(1..=4096))
        || !periodic(&word)
    {
        return None;
    }
    let hashed = sail_api::temporal::sha256;
    let provenance = json!({"word":path,"word_sha256":hashed(&bytes),"module_sha256":hashed(&source),"cfg_sha256":hashed(&cfg)});
    Some(Donor {
        module,
        cfg: cfg_path,
        provenance,
        word,
        config,
    })
}

fn bound(cfg: &Value, name: &str) -> Option<u64> {
    let mut matching = cfg["constants"]
        .as_array()?
        .iter()
        .filter(|c| c["name"]["text"] == name && c["module"].is_null());
    let entry = matching.next()?;
    if matching.next().is_some()
        || entry["kind"] != "assignment"
        || !entry["args"].as_array()?.is_empty()
    {
        return None;
    }
    entry["value"]["int"]
        .as_u64()
        .filter(|n| (1..=128).contains(n))
}

fn valid_layout(layout: &Layout) -> bool {
    layout.fields.len() <= 24
        && layout.renames.len() <= 128
        && layout.renames.values().collect::<BTreeSet<_>>().len() == layout.renames.len()
        && layout.fields.iter().enumerate().all(|(i, f)| {
            !f.path.is_empty()
                && f.path.len() <= 12
                && f.path[0] != "*"
                && f.path.iter().all(|segment| {
                    segment
                        .parse::<usize>()
                        .ok()
                        .is_none_or(|index| index.to_string() == *segment)
                })
                && layout.fields[..i].iter().all(|g| {
                    f.path
                        .iter()
                        .zip(&g.path)
                        .any(|(a, b)| a != b && a != "*" && b != "*")
                })
                && match f.kind {
                    Kind::Model => !layout.renames.is_empty(),
                    _ => layout.integer_carrier.is_some(),
                }
        })
}

fn embeddings(old: u64, new: u64) -> Vec<Vec<u64>> {
    if new <= old {
        return vec![];
    }
    // Keep the first coordinate fixed. Try putting the new interval after
    // successive existing coordinates, then try contiguous translations.
    let insert = new - old;
    let mut candidates = (1..old.min(9))
        .map(|gap| {
            (1..=old)
                .map(|i| if i <= gap { i } else { i + insert })
                .collect()
        })
        .collect::<Vec<_>>();
    for shift in 1..=insert.min(4) {
        candidates.push((1 + shift..=old + shift).collect());
    }
    candidates
}

fn rewrite(
    value: &mut Value,
    path: &[String],
    kind: Kind,
    integers: &[u64],
    renames: &BTreeMap<String, String>,
    images: &mut BTreeMap<String, String>,
) -> Option<()> {
    if let Some((part, tail)) = path.split_first() {
        if part == "*" {
            for element in value.as_array_mut()? {
                rewrite(element, tail, kind, integers, renames, images)?;
            }
        } else {
            let child = if value.is_array() {
                value.get_mut(part.parse::<usize>().ok()?)?
            } else {
                value.get_mut(part)?
            };
            rewrite(child, tail, kind, integers, renames, images)?;
        }
        return Some(());
    }
    match kind {
        Kind::IntegerOrModel if value.get("model").and_then(Value::as_str).is_some() => (),
        Kind::Integer | Kind::IntegerOrModel => {
            let index = usize::try_from(value.as_u64()?.checked_sub(1)?).ok()?;
            *value = json!(integers.get(index)?);
        }
        Kind::Model => {
            let name = value.get_mut("model")?;
            let old = name.as_str()?;
            let new = renames.get(old).map_or(old, String::as_str);
            match images.get(new) {
                Some(previous) if previous != old => return None,
                Some(_) => (),
                None => {
                    images.insert(new.to_owned(), old.to_owned());
                }
            }
            if new != old {
                *name = json!(new);
            }
        }
    }
    Some(())
}

fn mapped(donor: &Donor, layout: &Layout, integers: &[u64]) -> Option<Value> {
    let mut word = donor.word.clone();
    let mut images = BTreeMap::new();
    for state in word.get_mut("states")?.as_array_mut()? {
        for field in &layout.fields {
            rewrite(
                state.get_mut("values")?,
                &field.path,
                field.kind,
                integers,
                &layout.renames,
                &mut images,
            )?;
        }
        state["action"] = Value::Null;
    }
    Some(word)
}

pub(super) struct Session<'a> {
    dive: &'a Path,
    property: &'a str,
    started: Instant,
    deadline: Duration,
    cancel: &'a AtomicBool,
    attempts: Vec<Value>,
    donor_replays: Vec<Value>,
    mapping_ms: f64,
}

impl Session<'_> {
    fn available(&self) -> bool {
        self.attempts.len() < MAX_ATTEMPTS
            && self.started.elapsed() < self.deadline
            && !self.cancel.load(Ordering::Relaxed)
    }

    fn invoke(&self, cmd: Command) -> Option<Value> {
        let deadline = self
            .deadline
            .min(self.started.elapsed() + Duration::from_millis(900));
        let (code, bytes) =
            external_dive::run(cmd, self.started, deadline, self.cancel, "witness donor").ok()?;
        (code == 0)
            .then(|| serde_json::from_slice(&bytes).ok())
            .flatten()
    }

    fn precheck(&mut self, trace: &Path, id: usize, donor: &Donor) -> bool {
        if !self.available() {
            return false;
        }
        let mut cmd = command(self.dive, "validate-trace", &donor.module, &donor.cfg);
        cmd.arg("--trace").arg(trace);
        let started = Instant::now();
        let accepted =
            self.invoke(cmd).is_some_and(|report| {
                report["command"] == "validate-trace"
                    && report["schema_version"] == 4
                    && report["result"] == "valid"
                    && report["length"] == donor.word["length"]
            }) && matches_artifact(&donor.module, 1 << 20, &donor.provenance["module_sha256"])
                && matches_artifact(&donor.cfg, 128 << 10, &donor.provenance["cfg_sha256"]);
        self.donor_replays
            .push(json!({"donor":id,"wall_ms":ms(started),"accepted":accepted}));
        accepted
    }

    fn check(
        &mut self,
        mut cmd: Command,
        trace: &Path,
        donor: usize,
        mapping: &Value,
    ) -> Option<Value> {
        if !self.available() {
            return None;
        }
        cmd.arg("--trace").arg(trace).arg("--repair");
        let started = Instant::now();
        let result = self.invoke(cmd).filter(|report| {
            validated(report) && claimed(&report["repair"]["trace"], self.property)
        });
        self.attempts.push(json!({"donor":donor,"mapping":mapping,"wall_ms":ms(started),"accepted":result.is_some()}));
        result
    }

    fn search(
        &mut self,
        donors: &[Donor],
        cfg: &Value,
        layout: &Layout,
        current: &dyn Fn() -> Command,
    ) -> Option<Value> {
        if donors.is_empty() {
            return None;
        }
        let scratch = Scratch::new().ok()?;
        let path = scratch.0.join("candidate.json");
        let mut verified = Vec::new();
        for (id, donor) in donors.iter().enumerate() {
            if !self.available() {
                return None;
            }
            if write_candidate(&path, &donor.word) && self.precheck(&path, id, donor) {
                verified.push((id, donor));
                if let Some(report) = self.check(current(), &path, id, &Value::Null) {
                    return Some(report);
                }
            }
        }
        if !self.available() || !valid_layout(layout) || layout.fields.is_empty() {
            return None;
        }
        let maps = verified
            .iter()
            .map(|(_, d)| candidate_maps(&d.config, cfg, layout))
            .collect::<Vec<_>>();
        for round in 0..maps.iter().map(Vec::len).max().unwrap_or_default() {
            for (&(id, donor), choices) in verified.iter().zip(&maps) {
                if !self.available() {
                    return None;
                }
                let Some(integers) = choices.get(round) else {
                    continue;
                };
                let mapping_started = Instant::now();
                let mapped = mapped(donor, layout, integers)
                    .is_some_and(|word| write_candidate(&path, &word));
                self.mapping_ms += ms(mapping_started);
                if !mapped {
                    continue;
                }
                let mapping = json!({"integer_carrier":layout.integer_carrier,"integer_image":integers,"model_renames":layout.renames});
                if let Some(report) = self.check(current(), &path, id, &mapping) {
                    return Some(report);
                }
            }
        }
        None
    }
}

fn matches_artifact(path: &Path, max: u64, digest: &Value) -> bool {
    read(path, max).is_some_and(|data| digest == &sail_api::temporal::sha256(&data))
}

fn write_candidate(path: &Path, word: &Value) -> bool {
    serde_json::to_vec(word)
        .is_ok_and(|bytes| bytes.len() as u64 <= MAX_WORD && fs::write(path, bytes).is_ok())
}

fn candidate_maps(old: &Value, new: &Value, layout: &Layout) -> Vec<Vec<u64>> {
    if let Some(carrier) = &layout.integer_carrier {
        let Some((old, new)) = bound(old, carrier).zip(bound(new, carrier)) else {
            return vec![];
        };
        let mut candidates = embeddings(old, new);
        if !layout.renames.is_empty() && old <= new {
            candidates.insert(0, (1..=old).collect());
        }
        candidates
    } else {
        vec![Vec::new()]
    }
}

fn ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.
}

pub(super) fn select(
    index_path: &Path,
    module: &Path,
    config: &Path,
    mut session: Session<'_>,
) -> Search {
    let lookup = Instant::now();
    let index = read(index_path, 64 << 10)
        .and_then(|bytes| serde_json::from_slice::<Index>(&bytes).ok())
        .filter(|i| i.version == 1 && i.donors.len() <= MAX_DONORS);
    let cfg = read(config, 128 << 10)
        .and_then(|bytes| sail_dive::witness_configuration(std::str::from_utf8(&bytes).ok()?));
    let mut reason = match (index.as_ref(), cfg.as_ref()) {
        (None, _) => "missing-or-invalid-donor-index",
        (_, None) => "invalid-current-configuration",
        _ => "property-not-configured",
    };
    let mut donors = Vec::new();
    let mut report = None;
    let mut lookup_ms = ms(lookup);
    if let (Some(index), Some(cfg)) = (
        index,
        cfg.as_ref().filter(|cfg| configured(cfg, session.property)),
    ) {
        let base = index_path.parent().unwrap_or_else(|| Path::new("."));
        donors = index
            .donors
            .iter()
            .take_while(|_| session.available())
            .filter_map(|entry| load(entry, base, module, session.property))
            .collect();
        lookup_ms = ms(lookup);
        reason = if donors.is_empty() {
            "no-matching-donor"
        } else if !valid_layout(&index.layout) {
            "invalid-or-ambiguous-layout"
        } else {
            "donor-attempts-exhausted"
        };
        let dive = session.dive;
        report = session.search(&donors, cfg, &index.layout, &|| {
            command(dive, "validate-trace", module, config)
        });
        if report.is_none()
            && !session.donor_replays.is_empty()
            && session.donor_replays.iter().all(|r| r["accepted"] == false)
        {
            reason = "no-donor-replayed-on-its-source";
        }
    }
    let chosen = session
        .attempts
        .last()
        .filter(|attempt| attempt["accepted"] == true)
        .map(|attempt| json!({"donor":attempt["donor"],"mapping":attempt["mapping"]}));
    Search {
        detail: json!({"lookup_ms":lookup_ms,"donor_phase_ms":ms(lookup),"mapping_ms":session.mapping_ms,"chosen":chosen,"donors":donors.iter().map(|d| &d.provenance).collect::<Vec<_>>(),"attempts":session.attempts,"donor_replays":session.donor_replays,"fallback_reason":report.is_none().then_some(if session.available() { reason } else { "donor-budget-exhausted" })}),
        report,
    }
}

pub(super) fn session<'a>(
    dive: &'a Path,
    property: &'a str,
    started: Instant,
    budget: Duration,
    cancel: &'a AtomicBool,
) -> Session<'a> {
    Session {
        dive,
        property,
        started,
        deadline: (budget / 2).min(Duration::from_secs(5)),
        cancel,
        attempts: vec![],
        donor_replays: vec![],
        mapping_ms: 0.,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeddings_touch_only_declared_typed_coordinates() -> Result<(), String> {
        let layout: Layout = serde_json::from_value(json!({
            "integer_carrier":"N", "renames":{"p_x":"p_y"}, "fields":[
                {"path":["positions","fcn","*","1","record","coordinate"],"kind":"integer_or_model"},
                {"path":["positions","fcn","*","0"],"kind":"model"}
            ]
        })).map_err(|error| error.to_string())?;
        assert!(valid_layout(&layout));
        let donor = Donor {
            module: PathBuf::new(),
            cfg: PathBuf::new(),
            provenance: Value::Null,
            config: Value::Null,
            word: json!({"states":[{"values":{
                "positions":{"fcn":[[{"model":"p_x"},{"record":{"coordinate":2,"value":2}}]]},
                "noise":2,
                "copy":{"model":"p_x"}
            }}]}),
        };
        let maps = embeddings(3, 4);
        assert_eq!(maps[0], vec![1, 3, 4]);
        let candidate = mapped(&donor, &layout, &maps[0]).ok_or("no mapped candidate")?;
        let row = &candidate["states"][0]["values"];
        assert_eq!(row["positions"]["fcn"][0][0]["model"], "p_y");
        assert_eq!(row["positions"]["fcn"][0][1]["record"]["coordinate"], 3);
        assert_eq!(row["positions"]["fcn"][0][1]["record"]["value"], 2);
        assert_eq!(row["noise"], 2);
        assert_eq!(row["copy"]["model"], "p_x");
        assert!(mapped(&donor, &layout, &[1]).is_none());
        let mut collision = donor;
        collision.word["states"][0]["values"]["positions"]["fcn"]
            .as_array_mut()
            .ok_or("positions")?
            .push(json!([{"model":"p_y"},{"record":{"coordinate":2,"value":2}}]));
        assert!(mapped(&collision, &layout, &maps[0]).is_none());
        Ok(())
    }

    #[test]
    fn refuses_overlapping_coordinates_and_nonunique_cfg_bindings() -> Result<(), String> {
        let layout: Layout = serde_json::from_value(json!({"integer_carrier":"N","fields":[
            {"path":["rows","fcn","*","1"],"kind":"integer"},
            {"path":["rows","fcn","2","1"],"kind":"integer"}
        ]}))
        .map_err(|error| error.to_string())?;
        assert!(!valid_layout(&layout));
        let config =
            sail_dive::witness_configuration("CONSTANT N = 3\nPROPERTY Live\n").ok_or("CFG")?;
        assert_eq!(bound(&config, "N"), Some(3));
        assert!(configured(&config, "Live"));
        assert!(!configured(&config, "Other"));
        let config = sail_dive::witness_configuration("CONSTANTS N = 3 N = 4\nPROPERTY Live\n");
        assert!(config.is_none_or(|value| bound(&value, "N").is_none()));
        Ok(())
    }
}
