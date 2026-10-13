use verus_builtin::*;
use verus_builtin_macros::*;
verus! {
proof fn selected(x: bool) ensures x || !x { }
proof fn other(x: bool) ensures x == x { }
}
