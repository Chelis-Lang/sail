use vstd::prelude::*;
verus! {
    pub proof fn good() { assert(true); }
    pub proof fn bad() { assert(false); }
}
