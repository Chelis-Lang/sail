use vstd::prelude::*;

verus! {

proof fn ghost_module_fact() {
    assert(1 + 1 == 2);
}

} // verus!
