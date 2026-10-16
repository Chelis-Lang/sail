use vstd::prelude::*;

verus! {
    pub proof fn broken()
        ensures
            #![verifier::proof_note("zebra")]
            false,
            #![verifier::proof_note("alpha")]
            false,
    {}
}

fn main() {}
