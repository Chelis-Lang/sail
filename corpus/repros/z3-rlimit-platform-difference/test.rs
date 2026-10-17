use vstd::prelude::*;
use vstd::float::FloatBitsProperties;
use vstd::std_specs::ops::*;
verus! {

// Biased IEEE-754 exponent of |x|: finite iff ex(x) <= 2046, and |x| < 2^(ex(x) - 1022).
pub open spec fn ex(x: f64) -> int { (x.to_bits_abs_spec() as int) / 0x10_0000_0000_0000 }
pub open spec fn fin(x: f64) -> bool { x.is_finite_spec() }

pub broadcast axiom fn ax_add_req(a: f64, b: f64) ensures #[trigger] a.add_req(b);
pub broadcast axiom fn ax_sub_req(a: f64, b: f64) ensures #[trigger] a.sub_req(b);
pub broadcast axiom fn ax_mul_req(a: f64, b: f64) ensures #[trigger] a.mul_req(b);

pub broadcast axiom fn ax_add(a: f64, b: f64, o: f64)
    requires fin(a), fin(b), #[trigger] add_ensures::<f64>(a, b, o),
             (if ex(a) >= ex(b) { ex(a) } else { ex(b) }) + 2 <= 2046,
    ensures fin(o), ex(o) <= (if ex(a) >= ex(b) { ex(a) } else { ex(b) }) + 2;
pub broadcast axiom fn ax_sub(a: f64, b: f64, o: f64)
    requires fin(a), fin(b), #[trigger] sub_ensures::<f64>(a, b, o),
             (if ex(a) >= ex(b) { ex(a) } else { ex(b) }) + 2 <= 2046,
    ensures fin(o), ex(o) <= (if ex(a) >= ex(b) { ex(a) } else { ex(b) }) + 2;
pub broadcast axiom fn ax_mul(a: f64, b: f64, o: f64)
    requires fin(a), fin(b), #[trigger] mul_ensures::<f64>(a, b, o), ex(a) + ex(b) - 1021 <= 2046,
    ensures fin(o), ex(o) <= if ex(a) + ex(b) - 1021 >= 0 { ex(a) + ex(b) - 1021 } else { 0 };
pub broadcast axiom fn ax_div_req(a: f64, b: f64) ensures #[trigger] a.div_req(b);
// |a| < 2^(ex(a)-1022) and |b| >= 2^(ex(b)-1023) for normal b, so |a/b| < 2^(ex(a)-ex(b)+1).
pub broadcast axiom fn ax_div(a: f64, b: f64, o: f64)
    requires fin(a), fin(b), ex(b) >= 1, #[trigger] div_ensures::<f64>(a, b, o), ex(a) - ex(b) + 1024 <= 2046,
    ensures fin(o), ex(o) <= if ex(a) - ex(b) + 1024 >= 0 { ex(a) - ex(b) + 1024 } else { 0 };
// Lower bound: |a*b| >= 2^(ex(a)+ex(b)-2046) for normal a and b; rounding keeps a representable power of two.
pub broadcast axiom fn ax_mul_lo(a: f64, b: f64, o: f64)
    requires fin(a), fin(b), ex(a) >= 1, ex(b) >= 1, #[trigger] mul_ensures::<f64>(a, b, o), ex(a) + ex(b) - 1023 >= 1, ex(a) + ex(b) - 1021 <= 2046,
    ensures ex(o) >= ex(a) + ex(b) - 1023;

pub broadcast group fp_axioms { ax_add_req, ax_sub_req, ax_mul_req, ax_div_req, ax_add, ax_sub, ax_mul, ax_div, ax_mul_lo }

// FPBench fptaylor-extra.fpcore "delta4"; box x1 in [4.0, 6.3504]; x2 in [4.0, 6.3504]; x3 in [4.0, 6.3504]; x4 in [4.0, 6.3504]; x5 in [4.0, 6.3504]; x6 in [4.0, 6.3504]
fn delta4(x1: f64, x2: f64, x3: f64, x4: f64, x5: f64, x6: f64) -> (r: f64)
    requires fin(x1), fin(x2), fin(x3), fin(x4), fin(x5), fin(x6), 1025 <= ex(x1) <= 1025, 1025 <= ex(x2) <= 1025, 1025 <= ex(x3) <= 1025, 1025 <= ex(x4) <= 1025, 1025 <= ex(x5) <= 1025, 1025 <= ex(x6) <= 1025,
    ensures fin(r), ex(r) <= 1043,
{
    broadcast use fp_axioms;
    let t0 = -1.0 * x2;
    let t1 = t0 * x3;
    let t2 = x1 * x4;
    let t3 = t1 - t2;
    let t4 = x2 * x5;
    let t5 = t3 + t4;
    let t6 = x3 * x6;
    let t7 = t5 + t6;
    let t8 = x5 * x6;
    let t9 = t7 - t8;
    let t10 = -1.0 * x1;
    let t11 = t10 + x2;
    let t12 = t11 + x3;
    let t13 = t12 - x4;
    let t14 = t13 + x5;
    let t15 = t14 + x6;
    let t16 = x1 * t15;
    let t17 = t9 + t16;
    t17
}

// FPBench fptaylor-extra.fpcore "delta"; box x1 in [4.0, 6.3504]; x2 in [4.0, 6.3504]; x3 in [4.0, 6.3504]; x4 in [4.0, 6.3504]; x5 in [4.0, 6.3504]; x6 in [4.0, 6.3504]
fn delta(x1: f64, x2: f64, x3: f64, x4: f64, x5: f64, x6: f64) -> (r: f64)
    requires fin(x1), fin(x2), fin(x3), fin(x4), fin(x5), fin(x6), 1025 <= ex(x1) <= 1025, 1025 <= ex(x2) <= 1025, 1025 <= ex(x3) <= 1025, 1025 <= ex(x4) <= 1025, 1025 <= ex(x5) <= 1025, 1025 <= ex(x6) <= 1025,
    ensures fin(r), ex(r) <= 1057,
{
    broadcast use fp_axioms;
    let t18 = x1 * x4;
    let t19 = -1.0 * x1;
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
    let t42 = -1.0 * x2;
    let t43 = t42 * x3;
    let t44 = t43 * x4;
    let t45 = t41 + t44;
    let t46 = -1.0 * x1;
    let t47 = t46 * x3;
    let t48 = t47 * x5;
    let t49 = t45 + t48;
    let t50 = -1.0 * x1;
    let t51 = t50 * x2;
    let t52 = t51 * x6;
    let t53 = t49 + t52;
    let t54 = -1.0 * x4;
    let t55 = t54 * x5;
    let t56 = t55 * x6;
    let t57 = t53 + t56;
    t57
}

// FPBench fptaylor-extra.fpcore "floudas"; box x1 in [0.0, 2.0]; x2 in [0.0, 3.0] (other :pre conjuncts dropped)
fn floudas(x1: f64, x2: f64) -> (r: f64)
    requires fin(x1), fin(x2), ex(x1) <= 1024, ex(x2) <= 1024,
    ensures fin(r), ex(r) <= 1026,
{
    broadcast use fp_axioms;
    let t58 = x1 + x2;
    t58
}

// FPBench fptaylor-extra.fpcore "sum"; box x0 in [1.0, 2.0]; x1 in [1.0, 2.0]; x2 in [1.0, 2.0]
fn sum(x0: f64, x1: f64, x2: f64) -> (r: f64)
    requires fin(x0), fin(x1), fin(x2), 1023 <= ex(x0) <= 1024, 1023 <= ex(x1) <= 1024, 1023 <= ex(x2) <= 1024,
    ensures fin(r), ex(r) <= 1032,
{
    broadcast use fp_axioms;
    let t59 = x0 + x1;
    let t60 = t59 - x2;
    let t61 = x1 + x2;
    let t62 = t61 - x0;
    let t63 = x2 + x0;
    let t64 = t63 - x1;
    let t65 = t60 + t62;
    let t66 = t65 + t64;
    t66
}

// FPBench fptaylor-extra.fpcore "himmilbeau"; box x1 in [-5.0, 5.0]; x2 in [-5.0, 5.0]
fn himmilbeau(x1: f64, x2: f64) -> (r: f64)
    requires fin(x1), fin(x2), ex(x1) <= 1025, ex(x2) <= 1025,
    ensures fin(r), ex(r) <= 1047,
{
    broadcast use fp_axioms;
    let t67 = x1 * x1;
    let t68 = t67 + x2;
    let t69 = t68 - 11.0;
    let t70 = x2 * x2;
    let t71 = x1 + t70;
    let t72 = t71 - 7.0;
    let t73 = t69 * t69;
    let t74 = t72 * t72;
    let t75 = t73 + t74;
    t75
}

fn main() {}
}
