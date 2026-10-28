//! Independent mathematical binary64 equality proposal; no production boundary is selected.
//! Finite numbers are exact signed integers in units of 2^-1074.
use vstd::prelude::*;
use vstd::arithmetic::mul::{lemma_mul_inequality, lemma_mul_strict_inequality,
    lemma_mul_equality_converse, lemma_mul_is_commutative};
use vstd::arithmetic::power2::{pow2, lemma2_to64, lemma_pow2_adds,
    lemma_pow2_pos, lemma_pow2_strictly_increases};

verus! {
const HIDDEN: u64 = 0x0010_0000_0000_0000;

spec fn negative(bits: u64) -> bool { bits & 0x8000_0000_0000_0000u64 != 0 }
spec fn exponent(bits: u64) -> u64 { (bits >> 52u64) & 0x7ffu64 }
spec fn fraction(bits: u64) -> u64 { bits & 0x000f_ffff_ffff_ffffu64 }

/// This is the unchanged original integer-bit predicate, not the denotation.
spec fn original_predicate(left: u64, right: u64) -> bool {
    left & 0x7fff_ffff_ffff_ffffu64 <= 0x7ff0_0000_0000_0000u64
        && right & 0x7fff_ffff_ffff_ffffu64 <= 0x7ff0_0000_0000_0000u64
        && (left == right || left & 0x7fff_ffff_ffff_ffffu64 == 0
            && right & 0x7fff_ffff_ffff_ffffu64 == 0)
}

/// Exact finite magnitude after multiplying the binary64 real value by 2^1074.
/// The subnormal formula and the hidden leading bit of normal values differ.
spec fn finite_magnitude(exp: nat, frac: nat) -> int {
    if exp == 0 { frac as int }
    else { (HIDDEN as int + frac as int) * pow2((exp - 1) as nat) as int }
}

spec fn signed_magnitude(sign: bool, magnitude: int) -> int {
    if sign { -magnitude } else { magnitude }
}

enum Binary64Value { Nan, NegativeInfinity, PositiveInfinity, Finite(int) }

/// Every u64 pattern has a denotation; no finite-only precondition is present.
spec fn denote(bits: u64) -> Binary64Value {
    let exp = exponent(bits);
    let frac = fraction(bits);
    if exp == 0x7ff {
        if frac != 0 { Binary64Value::Nan }
        else if negative(bits) { Binary64Value::NegativeInfinity }
        else { Binary64Value::PositiveInfinity }
    } else {
        Binary64Value::Finite(signed_magnitude(negative(bits),
            finite_magnitude(exp as nat, frac as nat)))
    }
}

/// Numeric denotation equality: NaN is unequal even to itself, infinities retain
/// their sign, and finite values compare as mathematical signed integers.
spec fn denotation_equal(left: Binary64Value, right: Binary64Value) -> bool {
    match (left, right) {
        (Binary64Value::Finite(a), Binary64Value::Finite(b)) => a == b,
        (Binary64Value::NegativeInfinity, Binary64Value::NegativeInfinity) => true,
        (Binary64Value::PositiveInfinity, Binary64Value::PositiveInfinity) => true,
        _ => false,
    }
}

proof fn original_fields(bits: u64)
    ensures
        exponent(bits) <= 0x7ff,
        fraction(bits) < HIDDEN,
        (bits & 0x7fff_ffff_ffff_ffffu64 == 0)
            == (exponent(bits) == 0 && fraction(bits) == 0),
        (bits & 0x7fff_ffff_ffff_ffffu64 > 0x7ff0_0000_0000_0000u64)
            == (exponent(bits) == 0x7ff && fraction(bits) != 0),
{
    assert((bits >> 52u64) & 0x7ffu64 <= 0x7ffu64) by(bit_vector);
    assert(bits & 0x000f_ffff_ffff_ffffu64 < 0x0010_0000_0000_0000u64) by(bit_vector);
    assert((bits & 0x7fff_ffff_ffff_ffffu64 == 0)
        == (((bits >> 52u64) & 0x7ffu64) == 0 && bits & 0x000f_ffff_ffff_ffffu64 == 0)) by(bit_vector);
    assert((bits & 0x7fff_ffff_ffff_ffffu64 > 0x7ff0_0000_0000_0000u64)
        == (((bits >> 52u64) & 0x7ffu64) == 0x7ffu64
            && bits & 0x000f_ffff_ffff_ffffu64 != 0)) by(bit_vector);
}

proof fn original_field_identity(left: u64, right: u64)
    ensures (left == right) == (negative(left) == negative(right)
        && exponent(left) == exponent(right) && fraction(left) == fraction(right)),
{
    assert((left == right) == ((left & 0x8000_0000_0000_0000u64 != 0)
        == (right & 0x8000_0000_0000_0000u64 != 0)
        && (left >> 52u64) & 0x7ffu64 == (right >> 52u64) & 0x7ffu64
        && left & 0x000f_ffff_ffff_ffffu64 == right & 0x000f_ffff_ffff_ffffu64)) by(bit_vector);
}

proof fn hidden_bit_is_power()
    ensures pow2(52) == HIDDEN as nat,
{
    lemma2_to64();
    lemma_pow2_adds(26, 26);
}

proof fn powers_do_not_decrease(left: nat, right: nat)
    requires left <= right,
    ensures pow2(left) <= pow2(right),
{
    if left < right { lemma_pow2_strictly_increases(left, right); }
}

/// Disjoint binary exponent bands, derived from the numeric significand/radix.
proof fn normal_magnitude_band(exp: nat, frac: nat)
    requires exp > 0, frac < HIDDEN as nat,
    ensures pow2(exp + 51) as int <= finite_magnitude(exp, frac),
        finite_magnitude(exp, frac) < pow2(exp + 52) as int,
{
    hidden_bit_is_power();
    lemma_pow2_pos((exp - 1) as nat);
    lemma_pow2_adds(52, (exp - 1) as nat);
    lemma_pow2_adds(53, (exp - 1) as nat);
    lemma_pow2_adds(52, 1);
    lemma2_to64();
    let scale = pow2((exp - 1) as nat) as int;
    lemma_mul_inequality(HIDDEN as int, HIDDEN as int + frac as int, scale);
    lemma_mul_strict_inequality(HIDDEN as int + frac as int, 2 * HIDDEN as int, scale);
}

proof fn finite_magnitude_nonnegative(exp: nat, frac: nat)
    requires frac < HIDDEN as nat,
    ensures finite_magnitude(exp, frac) >= 0,
        (finite_magnitude(exp, frac) == 0) == (exp == 0 && frac == 0),
{
    if exp > 0 {
        normal_magnitude_band(exp, frac);
        lemma_pow2_pos(exp + 51);
    }
}

proof fn earlier_band_is_smaller(left_exp: nat, left_frac: nat, right_exp: nat, right_frac: nat)
    requires left_exp < right_exp, left_frac < HIDDEN as nat, right_frac < HIDDEN as nat,
    ensures finite_magnitude(left_exp, left_frac) < finite_magnitude(right_exp, right_frac),
{
    normal_magnitude_band(right_exp, right_frac);
    if left_exp == 0 {
        hidden_bit_is_power();
        powers_do_not_decrease(52, right_exp + 51);
    } else {
        normal_magnitude_band(left_exp, left_frac);
        powers_do_not_decrease(left_exp + 52, right_exp + 51);
    }
}

/// Equality follows from the exact numeric radix representation, not bit identity.
proof fn finite_magnitude_is_canonical(left_exp: nat, left_frac: nat, right_exp: nat, right_frac: nat)
    requires left_frac < HIDDEN as nat, right_frac < HIDDEN as nat,
    ensures (finite_magnitude(left_exp, left_frac) == finite_magnitude(right_exp, right_frac))
        == (left_exp == right_exp && left_frac == right_frac),
{
    if left_exp < right_exp {
        earlier_band_is_smaller(left_exp, left_frac, right_exp, right_frac);
    } else if right_exp < left_exp {
        earlier_band_is_smaller(right_exp, right_frac, left_exp, left_frac);
    } else if left_exp > 0 {
        let scale = pow2((left_exp - 1) as nat) as int;
        lemma_pow2_pos((left_exp - 1) as nat);
        if finite_magnitude(left_exp, left_frac) == finite_magnitude(right_exp, right_frac) {
            lemma_mul_is_commutative(HIDDEN as int + left_frac as int, scale);
            lemma_mul_is_commutative(HIDDEN as int + right_frac as int, scale);
            lemma_mul_equality_converse(scale,
                HIDDEN as int + left_frac as int, HIDDEN as int + right_frac as int);
        }
    }
}

proof fn signed_numeric_equality(left_sign: bool, left: int, right_sign: bool, right: int)
    requires left >= 0, right >= 0,
    ensures (signed_magnitude(left_sign, left) == signed_magnitude(right_sign, right))
        == (left_sign == right_sign && left == right || left == 0 && right == 0),
{}

/// Full-domain mathematical theorem. All sign/exponent/fraction patterns are
/// quantified, including both zeros, every NaN payload and both infinities.
proof fn all_original_bits_match_numeric_denotation(left: u64, right: u64)
    ensures original_predicate(left, right) == denotation_equal(denote(left), denote(right)),
{
    original_fields(left);
    original_fields(right);
    original_field_identity(left, right);
    if exponent(left) != 0x7ff && exponent(right) != 0x7ff {
        let le = exponent(left) as nat;
        let lf = fraction(left) as nat;
        let re = exponent(right) as nat;
        let rf = fraction(right) as nat;
        finite_magnitude_nonnegative(le, lf);
        finite_magnitude_nonnegative(re, rf);
        finite_magnitude_is_canonical(le, lf, re, rf);
        signed_numeric_equality(negative(left), finite_magnitude(le, lf),
            negative(right), finite_magnitude(re, rf));
    }
}

proof fn nonempty_zero_nan_infinity_and_subnormal_examples()
    ensures denotation_equal(denote(0), denote(0x8000_0000_0000_0000)),
        !denotation_equal(denote(0x7ff0_0000_0000_0001), denote(0x7ff0_0000_0000_0001)),
        denotation_equal(denote(0x7ff0_0000_0000_0000), denote(0x7ff0_0000_0000_0000)),
        !denotation_equal(denote(0x7ff0_0000_0000_0000), denote(0xfff0_0000_0000_0000)),
        denote(1) == Binary64Value::Finite(1),
        denote(0x8000_0000_0000_0001) == Binary64Value::Finite(-1),
{
    assert(0u64 & 0x7fff_ffff_ffff_ffffu64 == 0) by(bit_vector);
    assert(0x8000_0000_0000_0000u64 & 0x7fff_ffff_ffff_ffffu64 == 0) by(bit_vector);
    assert(0x7ff0_0000_0000_0001u64 & 0x7fff_ffff_ffff_ffffu64
        > 0x7ff0_0000_0000_0000u64) by(bit_vector);
    assert(0x7ff0_0000_0000_0000u64 & 0x7fff_ffff_ffff_ffffu64
        == 0x7ff0_0000_0000_0000u64) by(bit_vector);
    assert(0xfff0_0000_0000_0000u64 & 0x7fff_ffff_ffff_ffffu64
        == 0x7ff0_0000_0000_0000u64) by(bit_vector);
    assert(1u64 & 0x8000_0000_0000_0000u64 == 0) by(bit_vector);
    assert(0x8000_0000_0000_0001u64 & 0x8000_0000_0000_0000u64 != 0) by(bit_vector);
    all_original_bits_match_numeric_denotation(0, 0x8000_0000_0000_0000);
    all_original_bits_match_numeric_denotation(0x7ff0_0000_0000_0001, 0x7ff0_0000_0000_0001);
    all_original_bits_match_numeric_denotation(0x7ff0_0000_0000_0000, 0x7ff0_0000_0000_0000);
    all_original_bits_match_numeric_denotation(0x7ff0_0000_0000_0000, 0xfff0_0000_0000_0000);
    assert(1u64 & 0x000f_ffff_ffff_ffffu64 == 1u64) by(bit_vector);
    assert((1u64 >> 52u64) & 0x7ffu64 == 0u64) by(bit_vector);
    assert(0x8000_0000_0000_0001u64 & 0x000f_ffff_ffff_ffffu64 == 1u64) by(bit_vector);
    assert((0x8000_0000_0000_0001u64 >> 52u64) & 0x7ffu64 == 0u64) by(bit_vector);
}

/// Concrete normal endpoints establish the chosen numeric scale, independently
/// of the comparator's equality-only theorem.
proof fn normal_values_have_original_binary64_scale()
    ensures denote(0x0010_0000_0000_0000) == Binary64Value::Finite(HIDDEN as int),
        denote(0x3ff0_0000_0000_0000) == Binary64Value::Finite(pow2(1074) as int),
        denote(0x7fef_ffff_ffff_ffff) == Binary64Value::Finite(
            (2 * HIDDEN as int - 1) * pow2(2045) as int),
{
    assert((0x0010_0000_0000_0000u64 >> 52u64) & 0x7ffu64 == 1u64) by(bit_vector);
    assert(0x0010_0000_0000_0000u64 & 0x000f_ffff_ffff_ffffu64 == 0u64) by(bit_vector);
    assert(0x0010_0000_0000_0000u64 & 0x8000_0000_0000_0000u64 == 0u64) by(bit_vector);
    assert((0x3ff0_0000_0000_0000u64 >> 52u64) & 0x7ffu64 == 1023u64) by(bit_vector);
    assert(0x3ff0_0000_0000_0000u64 & 0x000f_ffff_ffff_ffffu64 == 0u64) by(bit_vector);
    assert(0x3ff0_0000_0000_0000u64 & 0x8000_0000_0000_0000u64 == 0u64) by(bit_vector);
    assert((0x7fef_ffff_ffff_ffffu64 >> 52u64) & 0x7ffu64 == 2046u64) by(bit_vector);
    assert(0x7fef_ffff_ffff_ffffu64 & 0x000f_ffff_ffff_ffffu64 == 0x000f_ffff_ffff_ffffu64) by(bit_vector);
    assert(0x7fef_ffff_ffff_ffffu64 & 0x8000_0000_0000_0000u64 == 0u64) by(bit_vector);
    lemma2_to64();
    hidden_bit_is_power();
    lemma_pow2_adds(52, 1022);
}

}
fn main() {}
