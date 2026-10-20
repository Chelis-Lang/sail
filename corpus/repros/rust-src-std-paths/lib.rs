use vstd::prelude::*;

verus! {

#[verifier::external_type_specification]
#[verifier::external_body]
pub struct ExPhantomPinned(core::marker::PhantomPinned);

} // verus!
