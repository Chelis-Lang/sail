//! Concrete one-read providers and owned endpoint/EOF refusal controls.
#![forbid(unsafe_code)]

use crate::authority::*;
use crate::inputs::*;
use crate::phase_inputs::*;
use crate::stream_capture::*;
use vstd::prelude::*;

verus! {

/// Finite executable source for one actual primitive outcome. No OS success axiom.
pub struct FinitePipeHost {
    pub owner: u64,
    pub kind: u8,
    pub byte: Option<u8>,
    pub fault: Option<u8>,
    pub events: Ghost<Seq<PipeReadObservation<u64>>>,
}

pub struct FinitePipeIdentity;

impl OwnerIdentity<u64> for FinitePipeIdentity {
    open spec fn identity(owner: &u64) -> int { *owner as int }
    fn same_owner(&self, original: &u64, actual: &u64) -> bool { *original == *actual }
}

impl CompilerPipeReadHost<u64, u8> for FinitePipeHost {
    open spec fn owner_view(&self) -> u64 { self.owner }
    open spec fn kind_view(&self) -> u8 { self.kind }
    open spec fn observations(&self) -> Seq<PipeReadObservation<u64>> { self.events@ }
    open spec fn fault_view(error: &u8) -> u8 {
        if *error == 1 { 1 } else if *error == 2 { 2 } else { 0 }
    }
    open spec fn next_read(&self, capacity: nat) -> Option<PipePrimitiveOutcome<u8>> {
        Some(match self.fault {
            Some(error) => PipePrimitiveOutcome::Failure(error),
            None => if capacity == 0 { PipePrimitiveOutcome::EmptyRead }
                else { match self.byte {
                    Some(byte) => PipePrimitiveOutcome::Bytes(seq![byte]),
                    None => PipePrimitiveOutcome::Eof,
                } },
        })
    }

    fn owner(&self) -> &u64 { &self.owner }
    fn kind(&self) -> u8 { self.kind }
    fn fault_kind(&self, error: &u8) -> u8 {
        if *error == 1 { 1 } else if *error == 2 { 2 } else { 0 }
    }
    fn read(&mut self, buffer: &mut [u8]) -> Result<usize, u8> {
        if let Some(error) = self.fault {
            proof { self.events@ = self.events@.push(PipeReadObservation::Failure {
                owner: self.owner, kind: self.kind, fault: Self::fault_view(&error) }); }
            return Err(error);
        }
        if buffer.is_empty() {
            proof { self.events@ = self.events@.push(PipeReadObservation::EmptyRead {
                owner: self.owner, kind: self.kind }); }
            return Ok(0);
        }
        if let Some(byte) = self.byte {
            buffer[0] = byte;
            proof {
                assert(buffer@.take(1) == seq![byte]);
                self.events@ = self.events@.push(PipeReadObservation::Bytes {
                    owner: self.owner, kind: self.kind, bytes: seq![byte] });
            }
            return Ok(1);
        }
        proof { self.events@ = self.events@.push(PipeReadObservation::Eof {
            owner: self.owner, kind: self.kind }); }
        Ok(0)
    }
}

/// Actual nonempty primitive read, then a separate actual EOF observation.
/// The fixture explicitly supplies each finite operation; native outcomes remain unknown.
pub fn nonempty_pipe_witness(owner: u64, kind: u8, byte: u8) -> (out: PipeReadback<u64>)
    requires kind <= 1,
    ensures pipe_bytes(out) == seq![byte],
        pipe_matches::<u64, FinitePipeIdentity>(out, owner, kind),
{
    let mut host = FinitePipeHost { owner, kind, byte: Some(byte), fault: None,
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(owner, kind);
    let mut buffer = vec![0u8];
    let first = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(first is Ok && first->Ok_0 == 1);
    assert(capture.bytes_view() == seq![byte]);
    host.byte = None;
    host.fault = None;
    let eof = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(eof is Ok && eof->Ok_0 == 0);
    let out = finish_pipe_capture(capture);
    assert(pipe_matches::<u64, FinitePipeIdentity>(out, owner, kind));
    out
}

/// Prefixes remain exact across multiple reads; sealing never invents a suffix.
pub fn successive_nonempty_reads_then_eof_preserve_order() {
    let mut host = FinitePipeHost { owner: 7, kind: 0, byte: Some(65), fault: None,
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 0);
    let mut buffer = vec![0u8];
    let first = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(first is Ok && first->Ok_0 == 1);
    host.byte = Some(66); host.fault = None;
    let second = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(second is Ok && second->Ok_0 == 1);
    host.byte = None; host.fault = None;
    let eof = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(eof is Ok && eof->Ok_0 == 0);
    let out = finish_pipe_capture(capture);
    assert(pipe_bytes(out) == seq![65u8, 66u8]);
    let matches = pipe_readback_matches(&FinitePipeIdentity, &out, &7u64, 0);
    assert(matches);
}

/// Equal visible bytes cannot substitute another actual lease or stream role.
pub fn same_bytes_foreign_owner_or_role_refuse() {
    let stdout = nonempty_pipe_witness(7, 0, 65);
    let foreign = nonempty_pipe_witness(8, 0, 65);
    let stderr = nonempty_pipe_witness(7, 1, 65);
    assert(pipe_bytes(stdout) == pipe_bytes(foreign) && pipe_bytes(stdout) == pipe_bytes(stderr));
    let owner = pipe_readback_matches(&FinitePipeIdentity, &foreign, &7u64, 0);
    let role = pipe_readback_matches(&FinitePipeIdentity, &stderr, &7u64, 0);
    assert(!owner && !role);
}

/// An actual zero-sized read and no reads at all cannot manufacture EOF.
pub fn no_read_and_zero_capacity_refuse() {
    let unused = finish_pipe_capture(begin_pipe_capture(7u64, 0));
    assert(unused is Unavailable);
    let mut host = FinitePipeHost { owner: 7, kind: 0, byte: None, fault: None,
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 0);
    let mut empty: Vec<u8> = Vec::new();
    let read = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut empty);
    assert(read is Ok && read->Ok_0 == 0);
    assert(!capture.eof_view());
    let out = finish_pipe_capture(capture);
    assert(out is Unavailable);
}

/// Fatal read failures remain sticky, including after a later actual EOF.
pub fn fatal_read_then_eof_refuses() {
    let mut host = FinitePipeHost { owner: 7, kind: 1, byte: None, fault: Some(9),
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 1);
    let mut buffer = vec![0u8];
    let failed = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(failed is Err && failed->Err_0 is Read);
    assert(capture.failed_view());
    host.byte = None; host.fault = None;
    let eof = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(eof is Ok && eof->Ok_0 == 0);
    let out = finish_pipe_capture(capture);
    assert(out is Unavailable);
}

/// Retryable primitive errors neither set EOF nor poison a later genuine read.
pub fn retryable_read_then_nonempty_and_eof(error: u8)
    requires error == 1 || error == 2,
{
    let mut host = FinitePipeHost { owner: 7, kind: 0, byte: None, fault: Some(error),
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 0);
    let mut buffer = vec![0u8];
    let retry = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(retry is Err && retry->Err_0 is Read);
    assert(!capture.eof_view() && !capture.failed_view());
    host.byte = Some(65); host.fault = None;
    let byte = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(byte is Ok && byte->Ok_0 == 1);
    host.byte = None; host.fault = None;
    let eof = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(eof is Ok && eof->Ok_0 == 0);
    let out = finish_pipe_capture(capture);
    assert(pipe_bytes(out) == seq![65u8]);
    assert(pipe_matches::<u64, FinitePipeIdentity>(out, 7, 0));
}

/// Mismatched endpoint reads still forward actual prefixes, but never seal authority.
pub fn mismatched_endpoint_keeps_fresh_bytes_and_refuses(owner: u64, kind: u8)
    requires owner != 7 || kind != 0,
{
    let mut host = FinitePipeHost { owner, kind, byte: Some(65), fault: None,
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 0);
    let mut buffer = vec![0u8];
    let read = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(read is Ok && read->Ok_0 == 1);
    assert(capture.failed_view());
    let out = finish_pipe_capture(capture);
    assert(out is Unavailable && pipe_bytes(out) == seq![65u8]);
}

/// A second read after terminal EOF poisons completion, even if it also returns EOF.
pub fn reads_after_eof_refuse() {
    let mut host = FinitePipeHost { owner: 7, kind: 0, byte: None, fault: None,
        events: Ghost(Seq::empty()) };
    let mut capture = begin_pipe_capture(7u64, 0);
    let mut buffer = vec![0u8];
    let first = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(first is Ok && first->Ok_0 == 0);
    host.byte = None; host.fault = None;
    let second = capture_pipe_read(&FinitePipeIdentity, &mut host, &mut capture, &mut buffer);
    assert(second is Ok && second->Ok_0 == 0);
    let out = finish_pipe_capture(capture);
    assert(out is Unavailable);
}

/// Independent finite configuration factory for the pipe-completion controls.
pub fn pipe_configuration_witness() -> (out: Option<ConfiguredIntent>)
    ensures out.is_some(),
{
    let name = vec![65u8];
    let value = vec![55u8];
    let rows = vec![InputFact { namespace: 3, name: &name, value: InputValue::Present(&value) }];
    let one = vec![0usize];
    let environment = seal_command_environment(EnvironmentInheritance::Clear,
        Inventory { rows: &[], order: &[], inverse: &[] },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one },
        Inventory { rows: &rows, order: &one, inverse: &one });
    assert(environment.is_some());
    let environment = match environment { Some(value) => value, None => { assert(false); return None; } };
    let descriptor = ProcessDescriptor { program: vec![99u8], arguments: Vec::new(), directory: None,
        overrides: vec![(vec![65u8], Some(vec![55u8]))] };
    let out = seal_configured_intent(&descriptor, &descriptor, &environment);
    assert(out.is_some());
    out
}

/// Actual read-prefix/EOF tokens feed completion, including failing and signaled waits.
/// Equal bytes with foreign ownership, relabeled roles and unavailable EOF all refuse.
pub fn owned_pipe_completion_constructive_and_refusal_controls() {
    let run = vec![1u8];
    let unit = CompilerUnit { manifest: vec![2u8], name: vec![3u8], crate_type: vec![4u8], cargo_name: None };
    let mut case = 0u8;
    while case < 13
        invariant case <= 13,
        decreases 13 - case,
    {
        let configuration = pipe_configuration_witness();
        let configuration = match configuration { Some(value) => value, None => { assert(false); return; } };
        let mut ledger = AttemptLedger::new(&run, &unit);
        let reserved = ledger.reserve(copy_descriptor(configuration.descriptor()));
        let reserved = match reserved { Some(value) => value, None => { assert(false); return; } };
        let attempt = begin_owned_compiler(reserved, OwnedSpawnFields {
            owner: 17u64, pid: 7, configuration: Some(&configuration) });
        assert(attempt.is_some());
        let attempt = if case == 12 { None } else { attempt };
        let stdout = if case == 5 { PipeReadback::Unavailable(vec![65u8]) }
            else { nonempty_pipe_witness(if case == 3 { 18 } else { 17 }, if case == 4 { 1 } else { 0 }, 65) };
        let stderr = if case == 11 { PipeReadback::Unavailable(vec![66u8]) }
            else { nonempty_pipe_witness(17, if case == 7 { 0 } else { 1 }, 66) };
        let wait = if case == 6 { None } else if case == 2 { Some(None) }
            else if case == 1 { Some(Some(1i32)) } else { Some(Some(0i32)) };
        let fields = OwnedPipeCompletionFields {
            owner: if case == 9 { Some(18u64) } else { Some(17u64) },
            pid: if case == 10 { Some(8u32) } else { Some(7u32) },
            configuration: if case == 8 { None } else { Some(configuration) }, wait, stdout, stderr,
        };
        let completed = complete_owned_compiler_pipes(&FinitePipeIdentity, attempt, fields);
        assert(completed.is_ok() == (case <= 2));
        match completed {
            Ok(observed) => {
                let stdout = observed.stdout(); let stderr = observed.stderr(); let exit = observed.exit();
                assert(stdout@ == seq![65u8] && stderr@ == seq![66u8]);
                assert(exit == if case == 2 { None } else if case == 1 { Some(1i32) } else { Some(0i32) });
            },
            Err(raw) => {
                assert(raw.stdout@ == seq![65u8] && raw.stderr@ == seq![66u8]);
                assert(raw.exit == if case == 2 || case == 6 { None }
                    else if case == 1 { Some(1i32) } else { Some(0i32) });
            },
        }
        case += 1;
    }
}

}
