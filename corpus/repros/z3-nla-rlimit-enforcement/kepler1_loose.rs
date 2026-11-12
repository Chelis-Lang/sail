use vstd::prelude::*;
verus! {

// FPBench fptaylor-real2float.fpcore "kepler1": S = 10^2, inputs scaled by S, result scale D = S^3 = 1000000
spec fn kepler1(x1: int, x2: int, x3: int, x4: int) -> int {
    let t46 = x1 * x4;
    let t47 = -x1;
    let t48 = t47 + x2;
    let t49 = t48 + x3;
    let t50 = t49 - x4;
    let t51 = t46 * t50;
    let t52 = x1 - x2;
    let t53 = t52 + x3;
    let t54 = t53 + x4;
    let t55 = x2 * t54;
    let t56 = t51 + t55 * 100;
    let t57 = x1 + x2;
    let t58 = t57 - x3;
    let t59 = t58 + x4;
    let t60 = x3 * t59;
    let t61 = t56 + t60 * 100;
    let t62 = x2 * x3;
    let t63 = t62 * x4;
    let t64 = t61 - t63;
    let t65 = x1 * x3;
    let t66 = t64 - t65 * 100;
    let t67 = x1 * x2;
    let t68 = t66 - t67 * 100;
    let t69 = t68 - x4 * 10000;
    t69
}

proof fn kepler1_loose(x1: int, x2: int, x3: int, x4: int)
    by (nonlinear_arith)
    requires 400 <= x1 <= 636, 400 <= x2 <= 636, 400 <= x3 <= 636, 400 <= x4 <= 636,
    ensures -490320768 <= kepler1(x1, x2, x3, x4) <= 282739712,
{
}

fn main() {}
}
