//! The actual typed solver/mode-to-key constructor, shared with stock verification.
use crate::{Mode, SolverIdentity};
use sail_acceptance::key_prefix::{HashHost, Prefix};
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
