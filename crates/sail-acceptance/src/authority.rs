//! Exact identity helpers for source-shared positive-result authority.
//!
//! Report validation uses the actual API report structures, so no copied
//! `report_valid` flag stands between their fields and the checked predicate.

/// Exact byte comparison shared with input validation.
pub use super::inputs::byte_eq as exact_bytes;
use super::inputs::{byte_lt, permutation};
#[cfg(sail_verus)]
use super::inputs::{lex_less, lex_less_transitive, permutation_at, valid_permutation};

#[cfg(sail_verus)]
use vstd::prelude::*;

sail_code! {
    /// Distinct policies preserve the receipt and repair contracts.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum CompletionPolicy {
        /// Accept a successful exact-scope receipt, including absent whole-crate `success`.
        Receipt,
        /// Require an explicit successful whole-crate terminal summary.
        Whole,
        /// Require full whole-crate success and reject independent engine fallback.
        Repair,
    }

    /// One independently resolved requested Cargo target, retaining exact bytes.
    #[derive(Debug, PartialEq, Eq)]
    pub struct RequestedTarget {
        /// Exact Cargo package ID.
        pub package: Vec<u8>,
        /// Exact Cargo target name; never its normalized Rust crate name.
        pub name: Vec<u8>,
        /// Exact selected Cargo target kind.
        pub kind: Vec<u8>,
    }
}

sail_verified! {
    /// Copies an exact byte slice with a checked, allocation-only body.
    pub fn copy_bytes(bytes: &[u8]) -> (out: Vec<u8>)
        requires(true)
        ensures(out@ == bytes@)
    {
        let mut out = Vec::new();
        let mut i = 0usize;
        sail_while! { (i < bytes.len())
            invariant(i <= bytes.len(), out@ == bytes@.take(i as int))
            decreases(bytes.len() - i)
            {
                out.push(bytes[i]);
                i += 1;
            }
        }
        out
    }
}

sail_spec! {
    pub open spec fn flag_matches(bytes: Seq<u8>, flag: Seq<u8>) -> bool {
        bytes.len() >= flag.len() && bytes.take(flag.len() as int) == flag
            && (bytes.len() == flag.len() || bytes[flag.len() as int] == 61u8)
    }

    pub open spec fn distinct_id_values(ids: Seq<Vec<u8>>) -> bool {
        forall |i: int, j: int| 0 <= i < j < ids.len() ==> ids[i]@ != ids[j]@
    }

    pub proof fn strict_order_differs(a: Seq<u8>, b: Seq<u8>)
        requires lex_less(a, b),
        ensures a != b,
    {
        reveal(lex_less);
        let i = choose |i: int| super::inputs::lex_at(a, b, i);
        if a == b { assert(a.len() == b.len()); }
    }

    pub open spec fn owned_id(name: &[u8], id: &[u8]) -> bool {
        id@.len() > name@.len() && id@[name@.len() as int] == 35u8
            && forall |i: int| 0 <= i < name@.len() ==> id@[i] == name@[i]
    }
}

sail_verified! {
    /// Matches an exact flag or its `flag=value` spelling, never a prefix alias.
    pub fn argument_flag(bytes: &[u8], flag: &[u8]) -> (out: bool)
        requires(true)
        ensures(out == flag_matches(bytes@, flag@))
    {
        if bytes.len() < flag.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < flag.len())
            invariant(i <= flag.len(), flag.len() <= bytes.len(),
                forall |j: int| 0 <= j < i ==> bytes@[j] == flag@[j])
            decreases(flag.len() - i)
            {
                if bytes[i] != flag[i] { return false; }
                i += 1;
            }
        }
        sail_proof! { assert(bytes@.take(flag@.len() as int) =~= flag@); }
        bytes.len() == flag.len() || bytes[flag.len()] == b'='
    }
}

sail_verified! {
    /// Checks an arbitrary sorting candidate in linear time, including coverage.
    pub fn unique_obligations(ids: &[Vec<u8>], forward: &[usize], inverse: &[usize]) -> (out: bool)
        requires(true)
        ensures(out ==> distinct_id_values(ids@),
            ids@.len() == 0 && forward@.len() == 0 && inverse@.len() == 0 ==> out)
    {
        if !permutation(forward, inverse, ids.len()) { return false; }
        sail_proof! {
            assert forall |k: int| 0 <= k < forward@.len() implies forward@[k] < ids@.len() by {
                assert(permutation_at(forward@, inverse@, ids@.len(), k));
            }
        }
        let mut i = 0usize;
        sail_while! { (i < forward.len())
            invariant(i <= forward.len(), valid_permutation(forward@, inverse@, ids@.len()),
                forall |k: int| 0 <= k < forward@.len() ==> forward@[k] < ids@.len(),
                forall |j: int, k: int| 0 <= j < k < i ==>
                    lex_less(ids@[forward@[j] as int]@, ids@[forward@[k] as int]@))
            decreases(forward.len() - i)
            {
                if i > 0 {
                    if !byte_lt(&ids[forward[i - 1]], &ids[forward[i]]) { return false; }
                    sail_proof! {
                        assert forall |j: int| 0 <= j < i implies
                            lex_less(ids@[forward@[j] as int]@, ids@[forward@[i as int] as int]@) by {
                            if j < i - 1 {
                                lex_less_transitive(ids@[forward@[j] as int]@,
                                    ids@[forward@[i as int - 1] as int]@,
                                    ids@[forward@[i as int] as int]@);
                            }
                        }
                    }
                }
                i += 1;
            }
        }
        sail_proof! {
            assert forall |a: int, b: int| 0 <= a < b < ids@.len() implies ids@[a]@ != ids@[b]@ by {
                assert(permutation_at(forward@, inverse@, ids@.len(), a));
                assert(permutation_at(forward@, inverse@, ids@.len(), b));
                let p = inverse@[a] as int;
                let q = inverse@[b] as int;
                if p < q { strict_order_differs(ids@[a]@, ids@[b]@); }
                else if q < p { strict_order_differs(ids@[b]@, ids@[a]@); }
            }
        }
        true
    }
}

sail_verified! {
    /// Checks the exact `<function>#...` ownership boundary.
    pub fn owned_obligation(name: &[u8], id: &[u8]) -> (out: bool)
        requires(true)
        ensures(out == owned_id(name, id))
    {
        if id.len() <= name.len() { return false; }
        if id[name.len()] != b'#' { return false; }
        let mut i = 0usize;
        sail_while! { (i < name.len())
            invariant(i <= name.len(), name.len() < id.len(),
                forall |j: int| 0 <= j < i ==> name@[j] == id@[j])
            decreases(name.len() - i)
            {
                if name[i] != id[i] { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_code! {
    /// Exact raw compiler-unit fields, before Cargo/source resolution.
    /// This record does not certify that the compiler consumed a source tree.
    #[derive(Debug)]
    pub struct CompilerUnit {
        /// Original manifest path bytes supplied to the compiler driver.
        pub manifest: Vec<u8>,
        /// Actual Rust crate name, preserving its distinction from Cargo names.
        pub name: Vec<u8>,
        /// Actual rustc crate-type bytes.
        pub crate_type: Vec<u8>,
        /// Exact Cargo binary name when the launcher supplied one.
        pub cargo_name: Option<Vec<u8>>,
    }

    /// Command observations captured independently before the corresponding spawn.
    #[derive(Debug)]
    pub struct ProcessDescriptor {
        /// Program bytes passed to `Command`, before OS path resolution.
        pub program: Vec<u8>,
        /// Exact ordered child argument bytes.
        pub arguments: Vec<Vec<u8>>,
        /// Explicit current directory, absent when inherited.
        pub directory: Option<Vec<u8>>,
        /// Explicit environment overrides/removals, not the inherited environment.
        pub overrides: Vec<(Vec<u8>, Option<Vec<u8>>)>,
    }

    /// A monotone attempt namespace independent of PID allocation and reuse.
    pub struct AttemptLedger<'a> {
        run: &'a [u8],
        unit: &'a CompilerUnit,
        next: usize,
    }

    /// Allocated before spawn; constructing a `Child` cannot choose this slot.
    pub struct ReservedAttempt<'a> {
        run: &'a [u8],
        unit: &'a CompilerUnit,
        slot: usize,
        descriptor: ProcessDescriptor,
    }

    enum WaitObservation { Pending, Exited(i32), Signaled }

    /// One child and its independent stream/termination observations.
    pub struct RunningAttempt<'a> {
        reserved: ReservedAttempt<'a>,
        pid: u32,
        stdout: Option<Vec<u8>>,
        stderr: Option<Vec<u8>>,
        exit: WaitObservation,
        invalid: bool,
    }

    /// Complete process observations, never a verifier-proof authority token.
    pub struct ObservedAttempt<'a> {
        running: RunningAttempt<'a>,
    }

    /// Raw output recovered when event ambiguity withholds its observation seal.
    #[derive(Debug)]
    pub struct UnsealedOutput {
        /// Original stdout bytes.
        pub stdout: Vec<u8>,
        /// Original stderr bytes.
        pub stderr: Vec<u8>,
        /// Actual normal exit code, absent on a signal.
        pub exit: Option<i32>,
    }

    impl<'a> AttemptLedger<'a> {
        sail_method! {
            /// Creates an empty namespace for independently retained run/unit bytes.
            pub fn new(run: &'a [u8], unit: &'a CompilerUnit) -> (out: Self)
                requires(true)
                ensures(ledger_run(&out) == run, ledger_unit(&out) == unit, ledger_next(&out) == 0)
            { Self { run, unit, next: 0 } }
        }
        sail_method! {
            /// Allocates a fresh slot before a spawn, refusing counter exhaustion.
            pub fn reserve(&mut self, descriptor: ProcessDescriptor) -> (out: Option<ReservedAttempt<'a>>)
                requires(true)
                ensures(ledger_run(final(self)) == ledger_run(old(self)), ledger_unit(final(self)) == ledger_unit(old(self)),
                    out.is_some() == (ledger_next(old(self)) < usize::MAX),
                    out.is_some() ==> ledger_next(final(self)) == ledger_next(old(self)) + 1,
                    out.is_none() ==> ledger_next(final(self)) == ledger_next(old(self)),
                    out.is_some() ==> reserved_run(&out.unwrap()) == ledger_run(final(self)),
                    out.is_some() ==> reserved_unit(&out.unwrap()) == ledger_unit(final(self)),
                    out.is_some() ==> reserved_slot(&out.unwrap()) == ledger_next(old(self)),
                    out.is_some() ==> reserved_descriptor(&out.unwrap()) == descriptor)
            {
                if self.next == usize::MAX { return None; }
                let slot = self.next;
                self.next += 1;
                Some(ReservedAttempt { run: self.run, unit: self.unit, slot, descriptor })
            }
        }
    }

    impl<'a> ReservedAttempt<'a> {
        sail_method! {
            /// Returns the immutable run bytes for durable launch attribution.
            #[must_use]
            pub fn run(&self) -> (out: &'a [u8]) requires(true) ensures(out == reserved_run(self)) { self.run }
        }
        sail_method! {
            /// Returns the original raw compiler-unit record.
            #[must_use]
            pub fn unit(&self) -> (out: &'a CompilerUnit) requires(true) ensures(out == reserved_unit(self)) { self.unit }
        }
        sail_method! {
            /// Returns the slot allocated before this child was started.
            #[must_use]
            pub fn slot(&self) -> (out: usize) requires(true) ensures(out == reserved_slot(self)) { self.slot }
        }
        sail_method! {
            /// Returns exactly the pre-spawn command observation.
            #[must_use]
            pub fn descriptor(&self) -> (out: &ProcessDescriptor) requires(true) ensures(out == &reserved_descriptor(self)) { &self.descriptor }
        }
        sail_method! {
            /// Attaches the actual returned child PID; zero is not a child handle.
            pub fn started(self, pid: u32) -> (out: Option<RunningAttempt<'a>>)
                requires(true)
                ensures(out.is_some() == (pid != 0),
                    out.is_some() ==> running_reserved(&out.unwrap()) == self,
                    out.is_some() ==> running_pid(&out.unwrap()) == pid,
                    out.is_some() ==> running_stdout(&out.unwrap()).is_none(),
                    out.is_some() ==> running_stderr(&out.unwrap()).is_none(),
                    out.is_some() ==> running_exit(&out.unwrap()).is_none(),
                    out.is_some() ==> !running_invalid(&out.unwrap()))
            {
                if pid == 0 { return None; }
                Some(RunningAttempt { reserved: self, pid, stdout: None, stderr: None, exit: WaitObservation::Pending, invalid: false })
            }
        }
    }

    impl<'a> RunningAttempt<'a> {
        sail_method! {
            /// Records a complete stdout read from this child's pipe.
            #[must_use]
            pub fn stdout_eof(mut self, bytes: Vec<u8>) -> (out: Self)
                requires(true)
                ensures(running_reserved(&out) == running_reserved(&self), running_pid(&out) == running_pid(&self),
                    running_stdout(&out).is_some(),
                    running_stdout(&self).is_none() ==> running_stdout(&out).unwrap() == bytes,
                    running_stdout(&self).is_some() ==> running_stdout(&out) == running_stdout(&self),
                    running_invalid(&out) == (running_invalid(&self) || running_stdout(&self).is_some()),
                    running_stderr(&out) == running_stderr(&self), running_exit(&out) == running_exit(&self))
            {
                if self.stdout.is_some() { self.invalid = true; }
                else { self.stdout = Some(bytes); }
                self
            }
        }
        sail_method! {
            /// Records a complete stderr read from this child's pipe.
            #[must_use]
            pub fn stderr_eof(mut self, bytes: Vec<u8>) -> (out: Self)
                requires(true)
                ensures(running_reserved(&out) == running_reserved(&self), running_pid(&out) == running_pid(&self),
                    running_stderr(&out).is_some(),
                    running_stderr(&self).is_none() ==> running_stderr(&out).unwrap() == bytes,
                    running_stderr(&self).is_some() ==> running_stderr(&out) == running_stderr(&self),
                    running_invalid(&out) == (running_invalid(&self) || running_stderr(&self).is_some()),
                    running_stdout(&out) == running_stdout(&self), running_exit(&out) == running_exit(&self))
            {
                if self.stderr.is_some() { self.invalid = true; }
                else { self.stderr = Some(bytes); }
                self
            }
        }
        sail_method! {
            /// Records the actual wait result, distinguishing signal from no wait.
            #[must_use]
            pub fn exited(mut self, exit: Option<i32>) -> (out: Self)
                requires(true)
                ensures(running_reserved(&out) == running_reserved(&self), running_pid(&out) == running_pid(&self),
                    running_exit(&out).is_some(),
                    running_exit(&self).is_none() ==> running_exit(&out).unwrap() == exit,
                    running_exit(&self).is_some() ==> running_exit(&out) == running_exit(&self),
                    running_invalid(&out) == (running_invalid(&self) || running_exit(&self).is_some()),
                    running_stdout(&out) == running_stdout(&self), running_stderr(&out) == running_stderr(&self))
            {
                if matches!(self.exit, WaitObservation::Pending) {
                    self.exit = match exit { Some(code) => WaitObservation::Exited(code), None => WaitObservation::Signaled };
                } else { self.invalid = true; }
                self
            }
        }
        sail_method! {
            /// Seals only complete, unambiguous child observations.
            pub fn finish(self) -> (out: Option<ObservedAttempt<'a>>)
                requires(true)
                ensures(out.is_some() == observation_complete(&self),
                    out.is_some() ==> observed_running(&out.unwrap()) == self,
                    out.is_some() ==> observation_complete(&observed_running(&out.unwrap())))
            {
                if self.invalid || self.stdout.is_none() || self.stderr.is_none() || matches!(self.exit, WaitObservation::Pending) { return None; }
                Some(ObservedAttempt { running: self })
            }
        }
        sail_method! {
            /// Preserves complete raw output when ambiguous events refuse admission.
            ///
            /// # Errors
            /// Returns the original output when repeated events invalidated admission.
            #[allow(clippy::manual_unwrap_or_default, reason = "Keep the explicit cases in the source checked by stock Verus.")]
            pub fn finish_or_output(self) -> (out: Result<ObservedAttempt<'a>, UnsealedOutput>)
                requires(true)
                ensures(out.is_ok() == observation_complete(&self),
                    out.is_ok() ==> observed_running(&out.unwrap()) == self,
                    match out { Err(bytes) => (running_stdout(&self).is_some() ==> bytes.stdout == running_stdout(&self).unwrap())
                        && (running_stderr(&self).is_some() ==> bytes.stderr == running_stderr(&self).unwrap())
                        && (running_exit(&self).is_some() ==> bytes.exit == running_exit(&self).unwrap()), _ => true })
            {
                if matches!(self.exit, WaitObservation::Pending) || self.stdout.is_none() || self.stderr.is_none() || self.invalid {
                    let stdout = match self.stdout { Some(bytes) => bytes, None => Vec::new() };
                    let stderr = match self.stderr { Some(bytes) => bytes, None => Vec::new() };
                    let exit = match self.exit { WaitObservation::Exited(code) => Some(code), _ => None };
                    Err(UnsealedOutput { stdout, stderr, exit })
                } else { Ok(ObservedAttempt { running: self }) }
            }
        }
    }

    impl<'a> ObservedAttempt<'a> {
        sail_method! {
            /// Returns the exact reservation whose child produced these observations.
            #[must_use]
            pub fn reserved(&self) -> (out: &ReservedAttempt<'a>)
                requires(true) ensures(out == &running_reserved(&observed_running(self)))
            { &self.running.reserved }
        }
        sail_method! {
            /// Returns the PID attached to this reserved attempt.
            #[must_use]
            pub fn pid(&self) -> (out: u32)
                requires(true) ensures(out == running_pid(&observed_running(self)))
            { self.running.pid }
        }
        sail_method! {
            /// Borrows the exact complete observed stdout bytes.
            #[must_use]
            pub fn stdout(&self) -> (out: &[u8])
                requires(observation_complete(&observed_running(self)))
                ensures(out@ == running_stdout(&observed_running(self)).unwrap()@)
            { match &self.running.stdout { Some(bytes) => bytes.as_slice(), None => &[] } }
        }
        sail_method! {
            /// Borrows the exact complete observed stderr bytes.
            #[must_use]
            pub fn stderr(&self) -> (out: &[u8])
                requires(observation_complete(&observed_running(self)))
                ensures(out@ == running_stderr(&observed_running(self)).unwrap()@)
            { match &self.running.stderr { Some(bytes) => bytes.as_slice(), None => &[] } }
        }
        sail_method! {
            /// Returns the actual observed normal status, absent on a signal.
            #[must_use]
            pub fn exit(&self) -> (out: Option<i32>)
                requires(observation_complete(&observed_running(self)))
                ensures(out == running_exit(&observed_running(self)).unwrap())
            { match self.running.exit { WaitObservation::Exited(code) => Some(code), _ => None } }
        }
        sail_method! {
            /// A normal process success is distinct from successful verification.
            #[must_use]
            pub fn normal_success(&self) -> (out: bool)
                requires(observation_complete(&observed_running(self)))
                ensures(out == (running_exit(&observed_running(self)).unwrap() == Some(0)))
            { self.exit() == Some(0) }
        }
        sail_method! {
            /// Returns the complete raw observations after callers bind their history.
            #[allow(clippy::manual_unwrap_or_default, reason = "Keep the explicit constructor cases in the source checked by stock Verus.")]
            pub fn into_output(self) -> (out: (Vec<u8>, Vec<u8>, Option<i32>))
                requires(observation_complete(&observed_running(&self)))
                ensures(out.0 == running_stdout(&observed_running(&self)).unwrap(),
                    out.1 == running_stderr(&observed_running(&self)).unwrap(),
                    out.2 == running_exit(&observed_running(&self)).unwrap())
            {
                let stdout = match self.running.stdout { Some(bytes) => bytes, None => Vec::new() };
                let stderr = match self.running.stderr { Some(bytes) => bytes, None => Vec::new() };
                let exit = match self.running.exit { WaitObservation::Exited(code) => Some(code), _ => None };
                (stdout, stderr, exit)
            }
        }
    }
}

sail_spec! {
    pub closed spec fn ledger_run<'a>(a: &AttemptLedger<'a>) -> &'a [u8] { a.run }
    pub closed spec fn ledger_unit<'a>(a: &AttemptLedger<'a>) -> &'a CompilerUnit { a.unit }
    pub closed spec fn ledger_next(a: &AttemptLedger) -> usize { a.next }
    pub closed spec fn reserved_run<'a>(a: &ReservedAttempt<'a>) -> &'a [u8] { a.run }
    pub closed spec fn reserved_unit<'a>(a: &ReservedAttempt<'a>) -> &'a CompilerUnit { a.unit }
    pub closed spec fn reserved_slot(a: &ReservedAttempt) -> usize { a.slot }
    pub closed spec fn reserved_descriptor(a: &ReservedAttempt) -> ProcessDescriptor { a.descriptor }
    pub closed spec fn running_reserved<'a>(a: &RunningAttempt<'a>) -> ReservedAttempt<'a> { a.reserved }
    pub closed spec fn running_pid(a: &RunningAttempt) -> u32 { a.pid }
    pub closed spec fn running_stdout(a: &RunningAttempt) -> Option<Vec<u8>> { a.stdout }
    pub closed spec fn running_stderr(a: &RunningAttempt) -> Option<Vec<u8>> { a.stderr }
    pub closed spec fn running_exit(a: &RunningAttempt) -> Option<Option<i32>> {
        match a.exit { WaitObservation::Pending => None, WaitObservation::Exited(code) => Some(Some(code)), WaitObservation::Signaled => Some(None) }
    }
    pub closed spec fn running_invalid(a: &RunningAttempt) -> bool { a.invalid }
    pub closed spec fn observed_running<'a>(a: &ObservedAttempt<'a>) -> RunningAttempt<'a> { a.running }
    pub open spec fn observation_complete(a: &RunningAttempt) -> bool {
        !running_invalid(a) && running_stdout(a).is_some() && running_stderr(a).is_some() && running_exit(a).is_some()
    }
}

sail_verified! {
    /// Binds downstream report input to the same complete child observations.
    pub fn observed_payload_matches(observed: &ObservedAttempt, stdout: &[u8], stderr: &[u8], exit: Option<i32>) -> (out: bool)
        requires(observation_complete(&observed_running(observed)))
        ensures(out == (running_stdout(&observed_running(observed)).unwrap()@ == stdout@
            && running_stderr(&observed_running(observed)).unwrap()@ == stderr@
            && running_exit(&observed_running(observed)).unwrap() == exit))
    {
        exact_bytes(observed.stdout(), stdout) && exact_bytes(observed.stderr(), stderr) && observed.exit() == exit
    }
}
