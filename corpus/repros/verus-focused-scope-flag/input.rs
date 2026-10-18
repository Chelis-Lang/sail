use vstd::prelude::*;
verus! {
mod selected { pub proof fn checked() { assert(true); } }
mod omitted { pub proof fn not_checked() { assert(false); } }
}
