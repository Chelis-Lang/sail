//! The *warm, no change* workload (charter "How to prove it's useful";
//! ADR 0012).
//!
//! For each unit, a fresh `sail serve` with an empty cache root is started.
//! Round 0 is the *fill*: Verus through `sail-z3` with the cache, starting
//! empty. Rounds 1 to N are *warm*: the same, with the cache the fill left.
//! Every round also runs plain Verus, alternating which side goes first, and
//! compares the two (a speedup that coincides with any verdict difference is
//! a bug, not a result). The daemon's counters are read around each sail run.

use std::path::Path;

use sail_verus::corpus::Unit;
use sail_verus::run::{RunRecord, Solver, run_unit};
use sail_verus::toolchain::Installed;

use crate::cold::{Judgement, Samples, UnitResult, judge};
use crate::daemon::{CacheRun, Daemon, dir_bytes};

/// Where the warm workload finds its binaries and scratch space.
#[derive(Debug, Clone, Copy)]
pub struct Setup<'a> {
    /// The pinned Verus and Z3.
    pub installed: &'a Installed,
    /// The `sail-z3` shim.
    pub shim: &'a Path,
    /// The `sail` binary (`sail serve`).
    pub sail: &'a Path,
    /// Scratch directory: work directories, cache roots, the socket.
    pub scratch: &'a Path,
}

impl Setup<'_> {
    /// The socket path for this run's daemons (kept short: see
    /// [`crate::daemon::MAX_SOCKET_PATH`]).
    #[must_use]
    pub fn socket(&self) -> std::path::PathBuf {
        self.scratch.join("d.sock")
    }
}

fn empty_result(unit: &Unit) -> UnitResult {
    UnitResult {
        id: unit.id.clone(),
        functions: 0,
        plain: Samples::default(),
        sail: Samples::default(),
        same: 0,
        different: 0,
        infra_errors: 0,
        unstable: 0,
        fill: Some(Samples::default()),
        cache: Vec::new(),
        cache_bytes: None,
    }
}

/// Runs the warm workload over `units`: one fill round and `warm_rounds`
/// warm rounds per unit.
pub fn run(
    setup: &Setup,
    units: &[Unit],
    warm_rounds: usize,
    mut progress: impl FnMut(&UnitResult),
) -> Vec<UnitResult> {
    let mut results = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        let r = run_one(setup, i, unit, warm_rounds);
        progress(&r);
        results.push(r);
    }
    results
}

fn run_one(setup: &Setup, index: usize, unit: &Unit, warm_rounds: usize) -> UnitResult {
    let mut r = empty_result(unit);
    let cache_root = setup.scratch.join("cache").join(index.to_string());
    let daemon = match Daemon::start(setup.sail, &setup.socket(), &cache_root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: infra error: {e}", unit.id);
            r.infra_errors = warm_rounds + 1;
            return r;
        }
    };
    let sail_solver = Solver::Shim {
        shim: setup.shim.to_path_buf(),
        trace_dir: None,
        daemon_socket: Some(daemon.socket().to_path_buf()),
    };
    let work = setup.scratch.join("work");
    let mut plains: Vec<RunRecord> = Vec::new();
    for round in 0..=warm_rounds {
        let phase = if round == 0 { "fill" } else { "warm" };
        let before = daemon.stats();
        let go =
            |solver: &Solver, side: &str| run_unit(setup.installed, unit, solver, &work.join(side));
        let (p, s) = if round % 2 == 0 {
            let p = go(&Solver::Plain, "plain");
            (p, go(&sail_solver, "sail"))
        } else {
            let s = go(&sail_solver, "sail");
            (go(&Solver::Plain, "plain"), s)
        };
        // Plain Verus never talks to the daemon, so the difference covers
        // exactly the sail run.
        if let (Some(b), Some(a)) = (before, daemon.stats()) {
            r.cache.push(CacheRun::between(phase, &b, &a));
        }
        if round == 0 {
            r.cache_bytes = Some(dir_bytes(&cache_root));
        }
        let (Ok(p), Ok(s)) = (p, s) else {
            r.infra_errors += 1;
            continue;
        };
        plains.push(p);
        let (j, functions, differences) = judge(&plains, &s);
        r.functions = functions;
        match j {
            Judgement::Same => r.same += 1,
            Judgement::Unstable => r.unstable += 1,
            Judgement::Different => {
                r.different += 1;
                eprintln!(
                    "{} ({phase} round {round}) DIFFERENT: {}",
                    unit.id,
                    differences.join("; ")
                );
            }
        }
        if let Some(p) = plains.last() {
            r.plain.push(p);
        }
        if round == 0 {
            if let Some(fill) = r.fill.as_mut() {
                fill.push(&s);
            }
        } else {
            r.sail.push(&s);
        }
    }
    drop(daemon);
    let _ = std::fs::remove_dir_all(&cache_root);
    r
}
