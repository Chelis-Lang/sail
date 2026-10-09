use vstd::prelude::*;
verus! {
proof fn square(x: int, y: int)
    ensures (x + y) * (x + y) == x * x + 2 * x * y + y * y,
{
    assert((x + y) * (x + y) == x * x + 2 * x * y + y * y) by(nonlinear_arith);
}
}
