use vstd::prelude::*;

verus! {
    proof fn ordinary_flag_is_present() {
        assert(match option_env!("ORDINARY_PROOF_FLAG") {
            Some(_) => true,
            None => false,
        });
    }
}
