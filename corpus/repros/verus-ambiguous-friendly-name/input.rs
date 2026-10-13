use vstd::prelude::*;
verus! {
pub trait A { proof fn check(&self); }
pub trait B { proof fn check(&self); }
pub struct S { }
mod first {
    use super::*;
    verus! { impl A for S { proof fn check(&self) { assert(true); } } }
}
mod second {
    use super::*;
    verus! { impl B for S { proof fn check(&self) { assert(true); } } }
}
}
fn main() { }
