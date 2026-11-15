#![feature(register_tool)]
#![register_tool(verus)]
#![register_tool(verifier)]
use verus_builtin_macros::verus;
verus! {
pub enum E {
    A { head: u64, tail: u64 },
    B { extra: u64, other: u64 },
    C { first: u64, second: u64, third: u64 },
}
}
