use vstd::prelude::*;
verus! {

// FPBench fptaylor-extra.fpcore "delta": S = 10^4, inputs scaled by S, result scale D = S^3 = 1000000000000
spec fn delta(x1: int, x2: int, x3: int, x4: int, x5: int, x6: int) -> int {
    let t18 = x1 * x4;
    let t19 = -x1;
    let t20 = t19 + x2;
    let t21 = t20 + x3;
    let t22 = t21 - x4;
    let t23 = t22 + x5;
    let t24 = t23 + x6;
    let t25 = t18 * t24;
    let t26 = x2 * x5;
    let t27 = x1 - x2;
    let t28 = t27 + x3;
    let t29 = t28 + x4;
    let t30 = t29 - x5;
    let t31 = t30 + x6;
    let t32 = t26 * t31;
    let t33 = t25 + t32;
    let t34 = x3 * x6;
    let t35 = x1 + x2;
    let t36 = t35 - x3;
    let t37 = t36 + x4;
    let t38 = t37 + x5;
    let t39 = t38 - x6;
    let t40 = t34 * t39;
    let t41 = t33 + t40;
    let t42 = -x2;
    let t43 = t42 * x3;
    let t44 = t43 * x4;
    let t45 = t41 + t44;
    let t46 = -x1;
    let t47 = t46 * x3;
    let t48 = t47 * x5;
    let t49 = t45 + t48;
    let t50 = -x1;
    let t51 = t50 * x2;
    let t52 = t51 * x6;
    let t53 = t49 + t52;
    let t54 = -x4;
    let t55 = t54 * x5;
    let t56 = t55 * x6;
    let t57 = t53 + t56;
    t57
}

proof fn delta_loose(x1: int, x2: int, x3: int, x4: int, x5: int, x6: int)
    by (nonlinear_arith)
    requires 40000 <= x1 <= 63504, 40000 <= x2 <= 63504, 40000 <= x3 <= 63504, 40000 <= x4 <= 63504, 40000 <= x5 <= 63504, 40000 <= x6 <= 63504,
    ensures -866023460192256 <= delta(x1, x2, x3, x4, x5, x6) <= 1849293256736768,
{
}

fn main() {}
}
