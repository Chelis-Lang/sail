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

// FPBench fptaylor-tests.fpcore "test02_sum8"; box x0 in [1.0, 2.0]; x1 in [1.0, 2.0]; x2 in [1.0, 2.0]; x3 in [1.0, 2.0]; x4 in [1.0, 2.0]; x5 in [1.0, 2.0]; x6 in [1.0, 2.0]; x7 in [1.0, 2.0]
fn test02_sum8(x0: f64, x1: f64, x2: f64, x3: f64, x4: f64, x5: f64, x6: f64, x7: f64) -> (r: f64)
    requires fin(x0), fin(x1), fin(x2), fin(x3), fin(x4), fin(x5), fin(x6), fin(x7), 1023 <= ex(x0) <= 1024, 1023 <= ex(x1) <= 1024, 1023 <= ex(x2) <= 1024, 1023 <= ex(x3) <= 1024, 1023 <= ex(x4) <= 1024, 1023 <= ex(x5) <= 1024, 1023 <= ex(x6) <= 1024, 1023 <= ex(x7) <= 1024,
    ensures fin(r), ex(r) <= 1038,
{
    broadcast use fp_axioms;
    let t0 = x0 + x1;
    let t1 = t0 + x2;
    let t2 = t1 + x3;
    let t3 = t2 + x4;
    let t4 = t3 + x5;
    let t5 = t4 + x6;
    let t6 = t5 + x7;
    t6
}

// FPBench fptaylor-tests.fpcore "test04_dqmom9"; box m0 in [-1.0, 1.0]; m1 in [-1.0, 1.0]; m2 in [-1.0, 1.0]; w0 in [1e-05, 1.0]; w1 in [1e-05, 1.0]; w2 in [1e-05, 1.0]; a0 in [1e-05, 1.0]; a1 in [1e-05, 1.0]; a2 in [1e-05, 1.0]
#[verifier::rlimit(30)]
fn test04_dqmom9(m0: f64, m1: f64, m2: f64, w0: f64, w1: f64, w2: f64, a0: f64, a1: f64, a2: f64) -> (r: f64)
    requires fin(m0), fin(m1), fin(m2), fin(w0), fin(w1), fin(w2), fin(a0), fin(a1), fin(a2), ex(m0) <= 1023, ex(m1) <= 1023, ex(m2) <= 1023, 1006 <= ex(w0) <= 1023, 1006 <= ex(w1) <= 1023, 1006 <= ex(w2) <= 1023, 1006 <= ex(a0) <= 1023, 1006 <= ex(a1) <= 1023, 1006 <= ex(a2) <= 1023,
    ensures fin(r), ex(r) <= 1082,
{
    broadcast use fp_axioms;
    let t7 = 0.0 - m2;
    let t8 = w2 * t7;
    let t9 = a2 / w2;
    let t10 = 1.0 * t9;
    let t11 = a2 / w2;
    let t12 = t10 * t11;
    let t13 = -3.0 * t12;
    let t14 = t8 * t13;
    let t15 = 0.0 - m1;
    let t16 = w1 * t15;
    let t17 = a1 / w1;
    let t18 = 1.0 * t17;
    let t19 = a1 / w1;
    let t20 = t18 * t19;
    let t21 = -3.0 * t20;
    let t22 = t16 * t21;
    let t23 = 0.0 - m0;
    let t24 = w0 * t23;
    let t25 = a0 / w0;
    let t26 = 1.0 * t25;
    let t27 = a0 / w0;
    let t28 = t26 * t27;
    let t29 = -6.0 * t28;
    let t30 = t24 * t29;
    let t31 = t30 * 1.0;
    let t32 = t22 * 1.0;
    let t33 = t14 * 1.0;
    let t34 = t33 + 0.0;
    let t35 = t32 + t34;
    let t36 = t31 + t35;
    let t37 = 0.0 + t36;
    t37
}

fn main() {}
}
