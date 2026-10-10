use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    io::Read as _,
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct Process {
    pub pid: u32,
    pub parent: u32,
    pub name: String,
    pub state: String,
    pub started_ticks: u64,
    pub crate_name: Option<String>,
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
        crate_name: None,
    })
}
pub(super) fn crate_name(command: &[u8]) -> Option<String> {
    let mut args = command.split(|v| *v == 0);
    let mut name = None;
    while let Some(arg) = args.next() {
        if arg == b"--" {
            break;
        }
        let value = if arg == b"--crate-name" {
            Some(args.next()?)
        } else {
            arg.strip_prefix(b"--crate-name=")
        };
        if let Some(value) = value {
            if name.is_some() {
                return None;
            }
            let value = std::str::from_utf8(value).ok()?;
            let mut bytes = value.bytes();
            if !bytes
                .next()
                .is_some_and(|v| v.is_ascii_alphabetic() || v == b'_')
                || !bytes.all(|v| v.is_ascii_alphanumeric() || v == b'_')
            {
                return None;
            }
            name = Some(value.to_owned());
        }
    }
    name
}
fn selector(root: &Path, process: &Process) -> Option<String> {
    let mut command = Vec::new();
    std::fs::File::open(root.join("cmdline"))
        .ok()?
        .take(65_537)
        .read_to_end(&mut command)
        .ok()?;
    let after = parse(
        process.pid,
        &std::fs::read_to_string(root.join("stat")).ok()?,
    )?;
    if command.len() > 65_536
        || after.started_ticks != process.started_ticks
        || after.parent != process.parent
        || after.name != "rust_verify"
        || !["R", "S", "D"].contains(&after.state.as_str())
    {
        return None;
    }
    crate_name(&command)
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
            && let Some(mut process) = parse(pid, &raw)
        {
            if process.name == "rust_verify" {
                process.crate_name = selector(&entry.path(), &process);
            }
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
    if !["R", "S", "D"].contains(&process.state.as_str())
        || command.split(|v| *v == 0).nth(1) != Some(b"_repair-worker".as_slice())
        || std::fs::read_link(root.join("cwd"))?.canonicalize()? != project.canonicalize()?
    {
        return Err(io::Error::other(
            "active PID is not the selected project's repair worker",
        ));
    }
    Ok(process)
}
pub(super) fn verifiers(
    table: &BTreeMap<u32, Process>,
    owner: &Process,
    crate_name: &str,
) -> Vec<Process> {
    if !table.get(&owner.pid).is_some_and(|v| {
        v.started_ticks == owner.started_ticks && ["R", "S", "D"].contains(&v.state.as_str())
    }) {
        return Vec::new();
    }
    table
        .values()
        .filter(|v| {
            v.name == "rust_verify"
                && ["R", "S", "D"].contains(&v.state.as_str())
                && v.crate_name.as_deref() == Some(crate_name)
        })
        .filter(|v| {
            let mut parent = v.parent;
            let mut child_started = v.started_ticks;
            let mut seen = BTreeSet::new();
            while seen.insert(parent) {
                let Some(process) = table.get(&parent) else {
                    break;
                };
                if process.started_ticks > child_started {
                    break;
                }
                if parent == owner.pid {
                    return true;
                }
                child_started = process.started_ticks;
                parent = process.parent;
            }
            false
        })
        .cloned()
        .collect()
}
pub(super) fn continuing(before: &[Process], after: &[Process]) -> bool {
    before.iter().any(|first| {
        after.iter().any(|last| {
            first.pid == last.pid
                && first.started_ticks == last.started_ticks
                && first.parent == last.parent
                && first.crate_name == last.crate_name
        })
    })
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
            crate_name: None,
        };
        let child = Process {
            pid: 11,
            parent: 10,
            name: "rust_verify".into(),
            state: "S".into(),
            started_ticks: 200,
            crate_name: Some("human_eval_verus".into()),
        };
        let mut t = std::collections::BTreeMap::from([(10, owner.clone()), (11, child)]);
        assert_eq!(verifiers(&t, &owner, "human_eval_verus").len(), 1);
        for name in [
            None,
            Some("verus_builtin_macros"),
            Some("vstd"),
            Some("other"),
        ] {
            if let Some(v) = t.get_mut(&11) {
                v.crate_name = name.map(str::to_owned);
            }
            assert_eq!(
                verifiers(&t, &owner, "human_eval_verus"),
                Vec::<Process>::new()
            );
        }
        if let Some(v) = t.get_mut(&11) {
            v.crate_name = Some("human_eval_verus".into());
        }
        for state in ["Z", "T", "t"] {
            if let Some(v) = t.get_mut(&11) {
                v.state = state.into();
            }
            assert_eq!(
                verifiers(&t, &owner, "human_eval_verus"),
                Vec::<Process>::new()
            );
        }
        if let Some(v) = t.get_mut(&11) {
            v.state = "S".into();
            v.parent = 20;
        }
        assert_eq!(
            verifiers(&t, &owner, "human_eval_verus"),
            Vec::<Process>::new()
        );
        if let Some(v) = t.get_mut(&11) {
            v.parent = 10;
        }
        if let Some(v) = t.get_mut(&10) {
            v.started_ticks = 101;
        }
        assert_eq!(
            verifiers(&t, &owner, "human_eval_verus"),
            Vec::<Process>::new()
        );
    }
    #[test]
    fn continuing_compiler_requires_exact_identity_without_freezing_state() {
        let first = Process {
            pid: 22,
            parent: 21,
            name: "rust_verify".into(),
            state: "R".into(),
            started_ticks: 50,
            crate_name: Some("human_eval_verus".into()),
        };
        let mut last = first.clone();
        last.state = "S".into();
        assert!(super::continuing(
            std::slice::from_ref(&first),
            std::slice::from_ref(&last)
        ));
        last.pid = 23;
        assert!(!super::continuing(
            std::slice::from_ref(&first),
            std::slice::from_ref(&last)
        ));
        last.pid = 22;
        last.started_ticks = 51;
        assert!(!super::continuing(
            std::slice::from_ref(&first),
            std::slice::from_ref(&last)
        ));
        assert!(!super::continuing(std::slice::from_ref(&first), &[]));
    }
    #[test]
    fn recycled_ancestor_newer_than_compiler_never_qualifies() {
        let owner = Process {
            pid: 20,
            parent: 1,
            name: "sail".into(),
            state: "S".into(),
            started_ticks: 40,
            crate_name: None,
        };
        let parent = Process {
            pid: 21,
            parent: 20,
            name: "cargo".into(),
            state: "S".into(),
            started_ticks: 60,
            crate_name: None,
        };
        let child = Process {
            pid: 22,
            parent: 21,
            name: "rust_verify".into(),
            state: "R".into(),
            started_ticks: 50,
            crate_name: Some("human_eval_verus".into()),
        };
        let table =
            std::collections::BTreeMap::from([(20, owner.clone()), (21, parent), (22, child)]);
        assert_eq!(verifiers(&table, &owner, "human_eval_verus"), []);
    }
    #[test]
    fn exact_crate_selector_rejects_missing_duplicate_and_malformed_values() {
        assert_eq!(
            super::crate_name(b"rust_verify\0--crate-name\0human_eval_verus\0"),
            Some("human_eval_verus".into())
        );
        assert_eq!(
            super::crate_name(b"--crate-name=vest_lib\0"),
            Some("vest_lib".into())
        );
        assert_eq!(
            super::crate_name(b"--crate-name=vest_lib\0--\0--crate-name=other\0"),
            Some("vest_lib".into())
        );
        for command in [
            b"--crate-name\0".as_slice(),
            b"",
            b"--crate-name=\0",
            b"--crate-name\0one\0--crate-name=two\0",
            b"--crate-name\0bad-name\0",
            b"--crate-name\0\xff\0",
            b"--\0--crate-name=human_eval_verus\0",
        ] {
            assert!(super::crate_name(command).is_none());
        }
    }
}
