use std::{io, path::PathBuf};

pub(super) struct Args {
    pub socket: PathBuf,
    pub job: String,
    pub active_job: String,
    pub worker: u32,
    pub environment: String,
    pub out: PathBuf,
}
pub(super) fn parse(values: impl Iterator<Item = String>) -> io::Result<Args> {
    let mut options = std::collections::BTreeMap::new();
    let mut values = values;
    while let Some(name) = values.next() {
        if ![
            "--socket",
            "--job",
            "--active-job",
            "--active-worker",
            "--environment",
            "--out",
        ]
        .contains(&name.as_str())
            || options.contains_key(&name)
        {
            return Err(io::Error::other("unknown or repeated census selector"));
        }
        options.insert(
            name,
            values
                .next()
                .ok_or_else(|| io::Error::other("missing option value"))?,
        );
    }
    let mut get = |name: &str| {
        options
            .remove(name)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| io::Error::other(format!("missing {name}")))
    };
    let args = Args {
        socket: PathBuf::from(get("--socket")?),
        job: get("--job")?,
        active_job: get("--active-job")?,
        worker: get("--active-worker")?.parse().map_err(io::Error::other)?,
        environment: get("--environment")?,
        out: PathBuf::from(get("--out")?),
    };
    if args.worker == 0
        || args.job == args.active_job
        || !cfg!(target_os = "linux")
        || std::env::var_os("CI").is_some()
    {
        return Err(io::Error::other(
            "local Linux census requires distinct completed/active jobs and nonzero worker PID",
        ));
    }
    Ok(args)
}
#[cfg(test)]
mod tests {
    #[test]
    fn selectors_reject_unknown_repeated_missing_and_zero_pid() {
        for values in [
            vec!["--unknown"],
            vec!["--job", "x", "--job", "y"],
            vec!["--socket"],
        ] {
            assert!(super::parse(values.into_iter().map(str::to_owned)).is_err());
        }
        let values = [
            "--socket",
            "s",
            "--job",
            "done",
            "--active-job",
            "active",
            "--active-worker",
            "0",
            "--environment",
            "test",
            "--out",
            "out",
        ];
        assert!(super::parse(values.into_iter().map(str::to_owned)).is_err());
    }
}
