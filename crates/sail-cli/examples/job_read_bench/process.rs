use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Process {
    pub pid: u32,
    pub parent: u32,
    pub name: String,
    pub state: String,
    pub started_ticks: u64,
}
fn parse(pid: u32, raw: &str) -> Option<Process> {
    let closing = raw.rfind(')')?;
    let fields: Vec<_> = raw.get(closing + 2..)?.split_whitespace().collect();
    Some(Process {
        pid,
        parent: fields.get(1)?.parse().ok()?,
        name: raw.get(raw.find('(')? + 1..closing)?.to_owned(),
        state: fields.first()?.to_string(),
        started_ticks: fields.get(19)?.parse().ok()?,
    })
}
pub(super) fn table() -> io::Result<BTreeMap<u32, Process>> {
    let mut table = BTreeMap::new();
    for entry in std::fs::read_dir("/proc")? {
        let entry = entry?;
        let Some(pid) = entry.file_name().to_str().and_then(|v| v.parse().ok()) else {
            continue;
        };
        // Exiting processes are not evidence of an active verifier.
        if let Ok(raw) = std::fs::read_to_string(entry.path().join("stat"))
            && let Some(process) = parse(pid, &raw)
        {
            table.insert(pid, process);
        }
    }
    Ok(table)
}
pub(super) fn owner(worker: u32, project: &Path) -> io::Result<Process> {
    let process = table()?
        .remove(&worker)
        .ok_or_else(|| io::Error::other("active worker absent"))?;
    let root = std::path::PathBuf::from(format!("/proc/{worker}"));
    let command = std::fs::read(root.join("cmdline"))?;
    if process.state == "Z"
        || command.split(|v| *v == 0).nth(1) != Some(b"_repair-worker".as_slice())
        || std::fs::read_link(root.join("cwd"))?.canonicalize()? != project.canonicalize()?
    {
        return Err(io::Error::other(
            "active PID is not the selected project's repair worker",
        ));
    }
    Ok(process)
}
pub(super) fn verifiers(table: &BTreeMap<u32, Process>, owner: &Process) -> Vec<Process> {
    if !table
        .get(&owner.pid)
        .is_some_and(|v| v.started_ticks == owner.started_ticks && v.state != "Z")
    {
        return Vec::new();
    }
    table
        .values()
        .filter(|v| v.name == "rust_verify" && v.state != "Z")
        .filter(|v| {
            let mut parent = v.parent;
            let mut seen = BTreeSet::new();
            while seen.insert(parent) {
                if parent == owner.pid {
                    return true;
                }
                let Some(process) = table.get(&parent) else {
                    break;
                };
                parent = process.parent;
            }
            false
        })
        .cloned()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::{Process, verifiers};
    #[test]
    fn proc_name_parentheses_and_start_time_are_exact() {
        let raw = "9 (name ) with spaces) S 7 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 4242";
        let p = super::parse(9, raw).unwrap_or_else(|| unreachable!("fixture"));
        assert_eq!(p.name, "name ) with spaces");
        assert_eq!(p.parent, 7);
        assert_eq!(p.started_ticks, 4242);
        assert!(super::parse(9, "9 (bad) S").is_none());
    }
    #[test]
    fn zombies_pid_reuse_and_unrelated_workers_cannot_qualify() {
        let owner = Process {
            pid: 10,
            parent: 1,
            name: "sail".into(),
            state: "S".into(),
            started_ticks: 100,
        };
        let child = Process {
            pid: 11,
            parent: 10,
            name: "rust_verify".into(),
            state: "S".into(),
            started_ticks: 200,
        };
        let mut t = std::collections::BTreeMap::from([(10, owner.clone()), (11, child)]);
        assert_eq!(verifiers(&t, &owner).len(), 1);
        if let Some(v) = t.get_mut(&11) {
            v.state = "Z".into();
        }
        assert_eq!(verifiers(&t, &owner), Vec::<Process>::new());
        if let Some(v) = t.get_mut(&11) {
            v.state = "S".into();
            v.parent = 20;
        }
        assert_eq!(verifiers(&t, &owner), Vec::<Process>::new());
        if let Some(v) = t.get_mut(&11) {
            v.parent = 10;
        }
        if let Some(v) = t.get_mut(&10) {
            v.started_ticks = 101;
        }
        assert_eq!(verifiers(&t, &owner), Vec::<Process>::new());
    }
}
