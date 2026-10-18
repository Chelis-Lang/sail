//! Explicit native witness using genuine first-Serve producer authority.
//! No compiler, proof, source policy or admitted Plan is synthesized here.

use super::*;

type Failure = Box<dyn std::error::Error>;

struct Saved {
    path: PathBuf,
    bytes: Vec<u8>,
    permissions: std::fs::Permissions,
    modified: std::time::SystemTime,
}

impl Saved {
    fn write(&self, bytes: &[u8]) -> std::io::Result<()> {
        if self.path.exists() {
            let mut permissions = self.permissions.clone();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                permissions.set_mode(permissions.mode() | 0o200);
            }
            std::fs::set_permissions(&self.path, permissions)?;
        }
        std::fs::write(&self.path, bytes)?;
        std::fs::set_permissions(&self.path, self.permissions.clone())
    }

    fn restore(&self) -> std::io::Result<()> {
        self.write(&self.bytes)?;
        std::fs::File::open(&self.path)?
            .set_times(std::fs::FileTimes::new().set_modified(self.modified))
    }
}

impl Drop for Saved {
    fn drop(&mut self) {
        // The explicit restore below reports failures; this also protects an
        // early error. The external owner retains failed raw evidence.
        let _ = self.restore();
    }
}

/// Called only through the existing explicitly ignored native witness entry.
/// The external owner supplies an isolated first-Serve target, exact request
/// argv/environment and an owned mutation list; missing authority is failure.
#[allow(
    clippy::too_many_lines,
    reason = "Keep the positive admission, late mutations, exact restoration and raw outcome together."
)]
pub(crate) fn run(config: &Value) -> Result<(), Failure> {
    let path = |name: &str| -> Result<PathBuf, Failure> {
        Ok(PathBuf::from(
            config[name].as_str().ok_or("missing witness path")?,
        ))
    };
    let root = path("root")?.canonicalize()?;
    let out = path("out")?;
    if !out.starts_with(&root) {
        return Err("output outside owned fixture".into());
    }
    std::fs::create_dir(&out)?;
    let runtime: receipts::Runtime = serde_json::from_slice(&std::fs::read(path("runtime")?)?)?;
    if !out.starts_with(&runtime.sidecars) {
        return Err("witness output is not in this run's excluded sidecars".into());
    }
    let plan = runtime
        .retained
        .as_ref()
        .ok_or("actual retained plan absent")?;
    let identity: receipts::TargetIdentity = serde_json::from_value(config["identity"].clone())?;
    let args = config["args"]
        .as_array()
        .ok_or("missing exact arguments")?
        .iter()
        .map(|arg| {
            arg.as_str()
                .map(OsString::from)
                .ok_or("non-string argument")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if identity.rustc_args_sha256
        != files::digest(
            &args
                .iter()
                .flat_map(|arg| arg.as_encoded_bytes().iter().copied().chain([0]))
                .collect::<Vec<_>>(),
        )
        || !plan.requested(&identity)
    {
        return Err("requested identity does not bind this exact invocation".into());
    }
    let attempt = plan
        .begin(&runtime, &identity, &args, &out)
        .ok_or("genuine full requested Attempt admission failed")?;
    if !attempt.unchanged() || !super::super::producers::has_shipped_producer(plan, &attempt.graph)
    {
        return Err(
            "positive admitted Attempt lacks intact genuine shipped producer authority".into(),
        );
    }
    let mutations = config["mutations"].as_array().ok_or("missing mutations")?;
    let labels = mutations
        .iter()
        .map(|mutation| mutation["label"].as_str().ok_or("missing label"))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if labels
        != BTreeSet::from([
            "producer-document-absent",
            "producer-document-corrupt",
            "release-unit",
            "release-index",
            "dependency-output",
            "compiler",
            "compiler-source",
            "registry-source",
        ])
        || labels.len() != mutations.len()
    {
        return Err("incomplete or duplicated late-authority negative queue".into());
    }
    let mut outcomes = Vec::new();
    for mutation in mutations {
        let label = mutation["label"].as_str().ok_or("missing mutation label")?;
        let file = PathBuf::from(mutation["path"].as_str().ok_or("missing mutation path")?);
        if !file.starts_with(&root) || file.canonicalize()? != file {
            return Err("mutation outside owned canonical fixture".into());
        }
        let held = dependency_snapshot::open(&file).ok_or("non-regular mutation input")?;
        let metadata = held.metadata()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            if metadata.uid() != root.metadata()?.uid() {
                return Err("foreign-owned mutation input".into());
            }
        }
        if metadata.len() == 0 || metadata.len() > 64 * 1024 * 1024 {
            return Err("mutation body outside bounded witness scope".into());
        }
        let saved = Saved {
            path: file.clone(),
            bytes: std::fs::read(&file)?,
            permissions: metadata.permissions(),
            modified: metadata.modified()?,
        };
        match mutation["kind"].as_str() {
            Some("absent") => std::fs::remove_file(&file)?,
            Some("corrupt") => {
                let mut bytes = saved.bytes.clone();
                let last = bytes.last_mut().ok_or("empty mutation")?;
                *last ^= 1;
                saved.write(&bytes)?;
                std::fs::File::open(&file)?
                    .set_times(std::fs::FileTimes::new().set_modified(saved.modified))?;
            }
            _ => return Err("unknown mutation kind".into()),
        }
        // This is the production final guard used by both compiler-finish
        // publication and archive::restore's placement callback, after begin.
        let refused = !attempt.unchanged();
        saved.restore()?;
        let restored = attempt.unchanged();
        outcomes.push(serde_json::json!({"label":label,"refused":refused,"restored":restored}));
        files::json(&out.join("late-guard.json"), &outcomes)?;
        if !refused || !restored {
            return Err("late authority mutation did not refuse and restore exactly".into());
        }
    }
    Ok(())
}
