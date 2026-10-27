//! The actual typed solver/mode-to-key constructor, shared with stock verification.
use crate::{Mode, SolverIdentity};
use sail_acceptance::key_prefix::{HashHost, Prefix};
use sail_acceptance::query_prefix::QueryPrefix;
use sail_acceptance::sail_verified;
#[cfg(sail_verus)]
use vstd::prelude::*;

sail_verified! {
    /// Feeds the actual identity fields and mode variant into the checked v2 encoder.
    #[allow(clippy::redundant_as_str, reason = "Stock Verus specifies the str byte view directly; this preserves the exact String bytes in both builds.")]
    pub(crate) fn new_prefix<Host>(host: Host, solver: &SolverIdentity, mode: &Mode) -> (out: Prefix<Host>)
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (out.material() == sail_acceptance::key_prefix::header(
            vstd::utf8::encode_utf8(solver.name@), vstd::utf8::encode_utf8(solver.version@),
            solver.binary_sha256@, vstd::utf8::encode_utf8(solver.platform@),
            match mode { Mode::Default => None, Mode::OptIn(name) => Some(vstd::utf8::encode_utf8(name@)) }),
            out.length() == 0, out.exact())
    {
        let opt_in = match mode { Mode::Default => None, Mode::OptIn(name) => Some(name.as_str().as_bytes()) };
        Prefix::new(host, solver.name.as_str().as_bytes(), solver.version.as_str().as_bytes(),
            &solver.binary_sha256, solver.platform.as_str().as_bytes(), opt_in)
    }
}

sail_verified! {
    /// Initializes actual solver/mode framing and its one owned raw transcript together.
    pub(crate) fn new_query_prefix<Host>(host: Host, solver: &SolverIdentity, mode: &Mode) -> (out: QueryPrefix<Host>)
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (out.coherent(), out.active(), out.view().len() == 0, out.fed() == 0,
            out.parser_view() == (0u8,Seq::<u8>::empty(),0int),
            out.matches((vstd::utf8::encode_utf8(solver.name@),vstd::utf8::encode_utf8(solver.version@),
                solver.binary_sha256@,vstd::utf8::encode_utf8(solver.platform@),
                match mode { Mode::Default => None, Mode::OptIn(name) => Some(vstd::utf8::encode_utf8(name@)) })))
    {
        QueryPrefix::new(new_prefix(host,solver,mode))
    }
}

sail_verified! {
    /// Jointly constructs the parser owner and independent delivery seed from
    /// one actual typed solver/mode header, before receiving any input.
    pub(crate) fn new_session_seed<Host>(host: Host, solver: &SolverIdentity, mode: &Mode) -> (out: (QueryPrefix<Host>, Prefix<Host>))
        where (Host: HashHost)
        requires (host.bytes().len() == 0)
        ensures (out.0.coherent(), out.0.active(), out.0.view().len() == 0, out.0.fed() == 0,
            out.0.parser_view() == (0u8,Seq::<u8>::empty(),0int),
            out.1.length() == 0, out.1.exact(),
            out.0.material() == out.1.material(),
            out.1.material() == sail_acceptance::key_prefix::header(
                vstd::utf8::encode_utf8(solver.name@),vstd::utf8::encode_utf8(solver.version@),
                solver.binary_sha256@,vstd::utf8::encode_utf8(solver.platform@),
                match mode { Mode::Default => None, Mode::OptIn(name) => Some(vstd::utf8::encode_utf8(name@)) }),
            out.0.matches((vstd::utf8::encode_utf8(solver.name@),vstd::utf8::encode_utf8(solver.version@),
                solver.binary_sha256@,vstd::utf8::encode_utf8(solver.platform@),
                match mode { Mode::Default => None, Mode::OptIn(name) => Some(vstd::utf8::encode_utf8(name@)) })))
    {
        let seed = new_prefix(host.duplicate(),solver,mode);
        let session = new_query_prefix(host,solver,mode);
        (session,seed)
    }
}
