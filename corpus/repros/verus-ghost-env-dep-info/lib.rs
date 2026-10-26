use vstd::prelude::*;

verus! {

// Read only in ghost code: the erased compilation never expands it.
proof fn ordinary_flag_is_present() {
    assert(match option_env!("ORDINARY_PROOF_FLAG") {
        Some(_) => true,
        None => false,
    });
}

} // verus!
