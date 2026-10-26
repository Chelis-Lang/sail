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

    /// Pre-spawn slot whose request paths can be prepared before argv is sealed.
    pub struct UnconfiguredAttempt<'a> {
        run: &'a [u8],
        unit: &'a CompilerUnit,
        slot: usize,
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
            /// Allocates a fresh slot before configuring a command.
            pub fn allocate(&mut self) -> (out: Option<UnconfiguredAttempt<'a>>)
                requires(true)
                ensures(ledger_run(final(self)) == ledger_run(old(self)), ledger_unit(final(self)) == ledger_unit(old(self)),
                    out.is_some() == (ledger_next(old(self)) < usize::MAX),
                    out.is_some() ==> ledger_next(final(self)) == ledger_next(old(self)) + 1,
                    out.is_none() ==> ledger_next(final(self)) == ledger_next(old(self)),
                    out.is_some() ==> unconfigured_run(&out.unwrap()) == ledger_run(final(self)),
                    out.is_some() ==> unconfigured_unit(&out.unwrap()) == ledger_unit(final(self)),
                    out.is_some() ==> unconfigured_slot(&out.unwrap()) == ledger_next(old(self)))
            {
                if self.next == usize::MAX { return None; }
                let slot = self.next;
                self.next += 1;
                Some(UnconfiguredAttempt { run: self.run, unit: self.unit, slot })
            }
        }
        sail_method! {
            /// Allocates and configures an ordinary command in one checked step.
            #[allow(clippy::question_mark, reason = "Keep explicit refusal in the source checked by stock Verus.")]
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
                let Some(allocation) = self.allocate() else { return None; };
                Some(allocation.configure(descriptor))
            }
        }
    }

    impl<'a> UnconfiguredAttempt<'a> {
        sail_method! {
            /// Returns the independently allocated run bytes before request preparation.
            #[must_use]
            pub fn run(&self) -> (out: &'a [u8]) requires(true) ensures(out == unconfigured_run(self)) { self.run }
        }
        sail_method! {
            /// Returns the raw unit whose observation paths are being prepared.
            #[must_use]
            pub fn unit(&self) -> (out: &'a CompilerUnit) requires(true) ensures(out == unconfigured_unit(self)) { self.unit }
        }
        sail_method! {
            /// Returns the slot without claiming that a command or child exists.
            #[must_use]
            pub fn slot(&self) -> (out: usize) requires(true) ensures(out == unconfigured_slot(self)) { self.slot }
        }
        sail_method! {
            /// Seals the final command after operational request paths are added.
            pub fn configure(self, descriptor: ProcessDescriptor) -> (out: ReservedAttempt<'a>)
                requires(true)
                ensures(reserved_run(&out) == unconfigured_run(&self), reserved_unit(&out) == unconfigured_unit(&self),
                    reserved_slot(&out) == unconfigured_slot(&self), reserved_descriptor(&out) == descriptor)
            { ReservedAttempt { run: self.run, unit: self.unit, slot: self.slot, descriptor } }
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
    pub closed spec fn unconfigured_run<'a>(a: &UnconfiguredAttempt<'a>) -> &'a [u8] { a.run }
    pub closed spec fn unconfigured_unit<'a>(a: &UnconfiguredAttempt<'a>) -> &'a CompilerUnit { a.unit }
    pub closed spec fn unconfigured_slot(a: &UnconfiguredAttempt) -> usize { a.slot }
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

sail_spec! {
    pub open spec fn rust_crate_name(cargo: Seq<u8>, rust: Seq<u8>) -> bool {
        cargo.len() == rust.len() && forall |i: int| 0 <= i < cargo.len() ==>
            rust[i] == if cargo[i] == 45u8 { 95u8 } else { cargo[i] }
    }

    pub open spec fn compatible_target_kind(cargo: Seq<u8>, rust: Seq<u8>) -> bool {
        cargo == rust || (cargo == b"lib"@ && rust == b"rlib"@)
    }

    /// Name/kind correspondence only; manifest, source and input closure are separate.
    pub open spec fn compiler_target_name_kind(unit: &CompilerUnit, name: Seq<u8>, kinds: Seq<Vec<u8>>) -> bool {
        rust_crate_name(name, unit.name@)
            && (unit.cargo_name.is_some() ==> unit.cargo_name.unwrap()@ == name)
            && exists |i: int| 0 <= i < kinds.len() && compatible_target_kind(kinds[i]@, unit.crate_type@)
    }
}

sail_verified! {
    /// Checks the existing Cargo hyphen-to-underscore Rust-name mapping bytewise.
    pub fn cargo_crate_name_matches(cargo: &[u8], rust: &[u8]) -> (out: bool)
        requires(true)
        ensures(out == rust_crate_name(cargo@, rust@))
    {
        if cargo.len() != rust.len() { return false; }
        let mut i = 0usize;
        sail_while! { (i < cargo.len())
            invariant(i <= cargo.len(), cargo.len() == rust.len(),
                forall |j: int| 0 <= j < i ==> rust@[j] == if cargo@[j] == 45u8 { 95u8 } else { cargo@[j] })
            decreases(cargo.len() - i)
            {
                let expected = if cargo[i] == b'-' { b'_' } else { cargo[i] };
                if rust[i] != expected { return false; }
                i += 1;
            }
        }
        true
    }
}

sail_verified! {
    /// Checks exact Cargo identity when supplied, including normalized-name collisions.
    pub fn compiler_unit_matches_target(unit: &CompilerUnit, name: &[u8], kinds: &[Vec<u8>]) -> (out: bool)
        requires(true)
        ensures(out == compiler_target_name_kind(unit, name@, kinds@))
    {
        if !cargo_crate_name_matches(name, &unit.name) { return false; }
        let exact_name = match &unit.cargo_name { Some(exact) => exact_bytes(exact, name), None => true };
        if !exact_name { return false; }
        let mut i = 0usize;
        sail_while! { (i < kinds.len())
            invariant(i <= kinds.len(), rust_crate_name(name@, unit.name@),
                unit.cargo_name.is_some() ==> unit.cargo_name.unwrap()@ == name@,
                forall |j: int| 0 <= j < i ==> !compatible_target_kind(kinds@[j]@, unit.crate_type@))
            decreases(kinds.len() - i)
            {
                if exact_bytes(&kinds[i], &unit.crate_type)
                    || (exact_bytes(&kinds[i], b"lib") && exact_bytes(&unit.crate_type, b"rlib")) { return true; }
                i += 1;
            }
        }
        false
    }
}

sail_code! {
    /// Structural kind of the actual parsed function-details field.
    #[derive(Debug, PartialEq, Eq)]
    pub enum EngineFieldShape {
        /// The parsed JSON field is an object.
        Object,
        /// Missing or another JSON type.
        Other,
    }

    /// The optional JSON success field retains missing versus invalid types.
    #[derive(Debug, PartialEq, Eq)]
    pub enum EngineSuccessField {
        /// Field is absent from the object.
        Missing,
        /// Explicit Boolean field.
        Boolean(bool),
        /// Present field with another JSON type.
        Other,
    }

    /// Untrusted decoded final event fields; none is completion authority.
    #[derive(Debug)]
    pub struct EngineTerminalFields {
        /// Actual decoded event version.
        pub version: u64,
        /// Actual decoded event name bytes.
        pub event: Vec<u8>,
        /// Actual decoded phase bytes.
        pub phase: Vec<u8>,
        /// Actual optional normal-termination field.
        pub normal: Option<bool>,
        /// Actual optional success field.
        pub success: Option<bool>,
    }

    /// Original event bytes and one decoded version per physical JSONL record.
    #[derive(Debug)]
    pub struct EngineProtocolCandidate {
        /// Exact bounded bytes read from the engine side channel.
        pub raw: Vec<u8>,
        /// A malformed JSON record or wrong version type maps to None.
        pub versions: Vec<Option<u64>>,
        /// Original final record decoded with duplicate-field refusal.
        pub terminal: Option<EngineTerminalFields>,
    }

    /// Actual selected JSON fields from one original verifier result document.
    #[derive(Debug)]
    pub struct EngineDocumentFields {
        /// Kind of the original function-details value.
        pub functions: EngineFieldShape,
        /// Original non-normalized version text when it is a string.
        pub version: Option<Vec<u8>>,
        /// Original build profile text when it is a string.
        pub profile: Option<Vec<u8>>,
        /// Original toolchain text when it is a string.
        pub toolchain: Option<Vec<u8>>,
        /// Original commit text when it is a string.
        pub commit: Option<Vec<u8>>,
        /// Original platform OS text when it is a string.
        pub os: Option<Vec<u8>>,
        /// Original platform architecture text when it is a string.
        pub arch: Option<Vec<u8>>,
        /// Original encountered-error Boolean, preserving absence/type refusal.
        pub error: Option<bool>,
        /// Original encountered-VIR-error Boolean.
        pub vir_error: Option<bool>,
        /// Original success field, including missing versus wrong type.
        pub success: EngineSuccessField,
        /// Original nonnegative integral verified count, when correctly typed.
        pub verified: Option<u64>,
        /// Original nonnegative integral error count, when correctly typed.
        pub errors: Option<u64>,
        /// Original whole-invocation Boolean, when correctly typed.
        pub whole: Option<bool>,
    }

    /// Parsed result candidates and untouched bytes outside their JSON frames.
    #[derive(Debug)]
    pub struct EngineDocumentCandidates {
        /// Every parsed JSON document, in its original order.
        pub documents: Vec<EngineDocumentFields>,
        /// Exact original remainder from the result framing decoder.
        pub remainder: Vec<u8>,
    }

    /// Completion or the concrete guard that refuses engine completion.
    #[derive(Debug, PartialEq, Eq)]
    pub enum EngineCompletion {
        /// Consistent normal terminal, process status and result document.
        Complete,
        /// Process status is outside the normal verifier result protocol.
        ProcessStatus,
        /// The observed result exceeds the existing resource bound.
        ResultSize,
        /// Side-channel bytes are absent, incomplete, malformed or unsealed.
        Terminal,
        /// Process status contradicts the original terminal success field.
        ExitMismatch,
        /// Original result document fields or framing refuse completion.
        Document,
    }
}

sail_spec! {
    pub open spec fn engine_text_present(value: &Option<Vec<u8>>) -> bool {
        value.is_some() && value.unwrap()@.len() > 0
    }
    pub open spec fn engine_ascii_space(byte: u8) -> bool {
        byte == 9 || byte == 10 || byte == 12 || byte == 13 || byte == 32
    }
    pub open spec fn engine_terminal_fields_valid(t: &EngineTerminalFields) -> bool {
        t.version == 1 && t.event@ == b"compiler-sealed"@ && t.phase@ == b"full"@
    }
    pub open spec fn engine_terminal_valid(p: &EngineProtocolCandidate) -> bool {
        p.raw@.len() > 0 && p.raw@.len() <= 16_777_216 && p.raw@[p.raw@.len() - 1] == 10
            && p.versions@.len() > 0
            && (forall |i: int| 0 <= i < p.versions@.len() ==> p.versions@[i] == Some(1u64))
            && p.terminal.is_some() && engine_terminal_fields_valid(&p.terminal.unwrap())
    }
    pub open spec fn engine_document_valid(d: &EngineDocumentFields, success: bool) -> bool {
        matches!(d.functions, EngineFieldShape::Object)
            && engine_text_present(&d.version) && engine_text_present(&d.profile)
            && engine_text_present(&d.toolchain) && engine_text_present(&d.commit)
            && engine_text_present(&d.os) && engine_text_present(&d.arch)
            && d.error.is_some() && d.vir_error.is_some()
            && (match d.success { EngineSuccessField::Boolean(value) => value == success,
                EngineSuccessField::Missing => true, EngineSuccessField::Other => false })
            && !(d.vir_error.unwrap() && success)
            && (success ==> d.error == Some(false) && d.errors == Some(0u64))
            && (d.vir_error.unwrap() || (d.verified.is_some() && d.errors.is_some() && d.whole.is_some()
                && (d.whole != Some(true) || matches!(d.success, EngineSuccessField::Boolean(_)))))
    }
    pub open spec fn engine_document_stream_valid(d: &EngineDocumentCandidates, success: bool) -> bool {
        d.documents@.len() == 1 && engine_document_valid(&d.documents@[0], success)
            && forall |i: int| 0 <= i < d.remainder@.len() ==> engine_ascii_space(d.remainder@[i])
    }
    pub open spec fn engine_completion_valid(p: &Option<EngineProtocolCandidate>, d: &EngineDocumentCandidates,
        stdout: Seq<u8>, exit: Option<i32>) -> bool {
        (exit == Some(0i32) || exit == Some(1i32)) && stdout.len() <= 33_554_432
            && p.is_some() && engine_terminal_valid(&p.unwrap())
            && p.unwrap().terminal.unwrap().normal == Some(true)
            && p.unwrap().terminal.unwrap().success.is_some()
            && (exit == Some(0i32)) == p.unwrap().terminal.unwrap().success.unwrap()
            && engine_document_stream_valid(d, p.unwrap().terminal.unwrap().success.unwrap())
    }
}

sail_verified! {
    /// Checks a decoded string field without filling absent or wrong-typed data.
    pub fn engine_has_text(value: &Option<Vec<u8>>) -> (out: bool)
        requires(true)
        ensures(out == engine_text_present(value))
    {
        match value { Some(bytes) => !bytes.is_empty(), None => false }
    }
}

sail_verified! {
    /// Checks original terminal fields, without trusting an event label alone.
    pub fn engine_terminal_complete(p: &EngineProtocolCandidate) -> (out: bool)
        requires(true)
        ensures(out == engine_terminal_valid(p))
    {
        if p.raw.is_empty() || p.raw.len() > 16_777_216 || p.raw[p.raw.len() - 1] != b'\n' || p.versions.is_empty() { return false; }
        let mut i = 0usize;
        sail_while! { (i < p.versions.len())
            invariant(i <= p.versions.len(),
                forall |j: int| 0 <= j < i ==> p.versions@[j] == Some(1u64))
            decreases(p.versions.len() - i)
            {
                if p.versions[i] != Some(1u64) { return false; }
                i += 1;
            }
        }
        match &p.terminal {
            Some(t) => t.version == 1 && exact_bytes(&t.event, b"compiler-sealed") && exact_bytes(&t.phase, b"full"),
            None => false,
        }
    }
}

sail_verified! {
    /// Checks the actual header, scalar types and exit-correlated result fields.
    pub fn engine_document_complete(d: &EngineDocumentFields, success: bool) -> (out: bool)
        requires(true)
        ensures(out == engine_document_valid(d, success))
    {
        if !matches!(d.functions, EngineFieldShape::Object) || !engine_has_text(&d.version)
            || !engine_has_text(&d.profile) || !engine_has_text(&d.toolchain)
            || !engine_has_text(&d.commit) || !engine_has_text(&d.os) || !engine_has_text(&d.arch) {
            return false;
        }
        let Some(vir_error) = d.vir_error else { return false; };
        if d.error.is_none() { return false; }
        match d.success {
            EngineSuccessField::Boolean(value) => { if value != success { return false; } },
            EngineSuccessField::Other => return false,
            EngineSuccessField::Missing => (),
        }
        if vir_error && success { return false; }
        if success && (d.error != Some(false) || d.errors != Some(0u64)) { return false; }
        vir_error || (d.verified.is_some() && d.errors.is_some() && d.whole.is_some()
            && (d.whole != Some(true) || matches!(d.success, EngineSuccessField::Boolean(_))))
    }
}

sail_verified! {
    /// Classifies completion from original field candidates and observed status.
    pub fn engine_completion(p: &Option<EngineProtocolCandidate>, d: &EngineDocumentCandidates,
        stdout: &[u8], exit: Option<i32>) -> (out: EngineCompletion)
        requires(true)
        ensures((out == EngineCompletion::Complete) == engine_completion_valid(p, d, stdout@, exit))
    {
        if exit != Some(0i32) && exit != Some(1i32) { return EngineCompletion::ProcessStatus; }
        if stdout.len() > 33_554_432 { return EngineCompletion::ResultSize; }
        let Some(protocol) = p else { return EngineCompletion::Terminal; };
        if !engine_terminal_complete(protocol) { return EngineCompletion::Terminal; }
        let Some(terminal) = &protocol.terminal else { return EngineCompletion::Terminal; };
        if terminal.normal != Some(true) { return EngineCompletion::Terminal; }
        let Some(success) = terminal.success else { return EngineCompletion::Terminal; };
        if (exit == Some(0i32)) != success { return EngineCompletion::ExitMismatch; }
        if !engine_document_stream_complete(d, success) { return EngineCompletion::Document; }
        EngineCompletion::Complete
    }
}

sail_verified! {
    /// Validates one complete original document and its exact framing remainder.
    pub fn engine_document_stream_complete(d: &EngineDocumentCandidates, success: bool) -> (out: bool)
        requires(true)
        ensures(out == engine_document_stream_valid(d, success))
    {
        if d.documents.len() != 1 || !engine_document_complete(&d.documents[0], success) {
            return false;
        }
        let mut i = 0usize;
        sail_while! { (i < d.remainder.len())
            invariant(i <= d.remainder.len(),
                engine_document_valid(&d.documents@[0], success), d.documents@.len() == 1,
                forall |j: int| 0 <= j < i ==> engine_ascii_space(d.remainder@[j]))
            decreases(d.remainder.len() - i)
            {
                let byte = d.remainder[i];
                if byte != b'\t' && byte != b'\n' && byte != 12 && byte != b'\r' && byte != b' ' {
                    return false;
                }
                i += 1;
            }
        }
        true
    }
}

sail_code! {
    /// A normally exited observed child whose typed engine-protocol candidates
    /// refuse completion. Engine image/target and physical file attribution
    /// remain separate native joins. This is not a process failure or a proof.
    pub struct DeclinedEngine<'a, 'b> {
        observed: &'a ObservedAttempt<'b>,
        protocol: &'a Option<EngineProtocolCandidate>,
        documents: &'a EngineDocumentCandidates,
        reason: EngineCompletion,
    }

    impl<'a, 'b> DeclinedEngine<'a, 'b> {
        sail_impl_spec! {
            pub closed spec fn observed(&self) -> &'a ObservedAttempt<'b> { self.observed }
            pub closed spec fn protocol(&self) -> &'a Option<EngineProtocolCandidate> { self.protocol }
            pub closed spec fn documents(&self) -> &'a EngineDocumentCandidates { self.documents }
            pub closed spec fn reason(&self) -> EngineCompletion { self.reason }
            pub open spec fn valid(&self) -> bool { declined_origin_valid(self) }
        }
        sail_method! {
            /// Returns the independently observed child, retaining its actual exit.
            pub fn observed_attempt(&self) -> (out: &'a ObservedAttempt<'b>)
                requires(true) ensures(out == self.observed())
            { self.observed }
        }
        sail_method! {
            /// Borrows the original side-channel candidate, including absence.
            pub fn protocol_candidate(&self) -> (out: &'a Option<EngineProtocolCandidate>)
                requires(true) ensures(out == self.protocol())
            { self.protocol }
        }
        sail_method! {
            /// Borrows every decoded original result candidate and remainder.
            pub fn document_candidates(&self) -> (out: &'a EngineDocumentCandidates)
                requires(true) ensures(out == self.documents())
            { self.documents }
        }
        sail_method! {
            /// Borrows the refusal reason derived from the actual typed candidates.
            pub fn completion_reason(&self) -> (out: &EngineCompletion)
                requires(true) ensures(*out == self.reason())
            { &self.reason }
        }
    }
}

sail_verified! {
    /// Seals actual normal exit-zero and engine-protocol refusal facts.
    /// Missing process observations cannot call this path; the factory does not
    /// establish the child's trusted image or the side-channel file's author.
    pub fn decline_engine<'a, 'b>(observed: &'a ObservedAttempt<'b>,
        protocol: &'a Option<EngineProtocolCandidate>, documents: &'a EngineDocumentCandidates)
        -> (out: Option<DeclinedEngine<'a, 'b>>)
        requires(observation_complete(&observed_running(observed)))
        ensures(out.is_some() == (running_exit(&observed_running(observed)).unwrap() == Some(0i32)
                && !engine_completion_valid(protocol, documents,
                    running_stdout(&observed_running(observed)).unwrap()@, Some(0i32))),
            out.is_some() ==> out.unwrap().valid(),
            out.is_some() ==> out.unwrap().observed() == observed,
            out.is_some() ==> out.unwrap().protocol() == protocol,
            out.is_some() ==> out.unwrap().documents() == documents)
    {
        if observed.exit() != Some(0i32) { return None; }
        let reason = engine_completion(protocol, documents, observed.stdout(), observed.exit());
        if matches!(reason, EngineCompletion::Complete) { return None; }
        Some(DeclinedEngine { observed, protocol, documents, reason })
    }
}

sail_spec! {
    pub open spec fn declined_origin_valid(d: &DeclinedEngine) -> bool {
        observation_complete(&observed_running(d.observed()))
            && running_exit(&observed_running(d.observed())).unwrap() == Some(0i32)
            && !engine_completion_valid(d.protocol(), d.documents(),
                running_stdout(&observed_running(d.observed())).unwrap()@, Some(0i32))
            && d.reason() != EngineCompletion::Complete
    }
}
