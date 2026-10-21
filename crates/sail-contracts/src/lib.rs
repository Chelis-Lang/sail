//! Always-checked neutral Rust contracts. Runtime checks are not SMT evidence.
//!
//! A false precondition or postcondition stops execution, including release builds:
//! ```
//! use sail_contracts::{requires, ensures, satisfies};
//! #[requires(x < 100)]
//! #[ensures(|ret| ret == x + 1)]
//! #[satisfies("INC")]
//! fn increment(x: u64) -> u64 { x + 1 }
//! assert_eq!(increment(4), 5);
//! ```
//!
//! Arbitrary cloning code cannot run during an old-value snapshot:
//! ```compile_fail
//! use sail_contracts::{ensures, satisfies};
//! #[derive(Clone, Copy)]
//! struct Record { value: u64 }
//! #[satisfies("OLD")]
//! #[ensures(|ret| old(input).value == ret)]
//! fn read(input: Record) -> u64 { input.value }
//! ```
//!
//! A marker cannot make an effectful predicate admissible:
//! ```compile_fail
//! use sail_contracts::requirement;
//! #[requirement("EFFECT")]
//! fn predicate(value: u64) -> bool { println!("changed"); value > 0 }
//! ```
//!
//! Contract calls require the structurally checked predicate entry:
//! ```compile_fail
//! use sail_contracts::{ensures, satisfies};
//! fn arbitrary(value: u64) -> bool { value > 0 }
//! #[satisfies("UNMARKED")]
//! #[ensures(|ret| arbitrary(ret))]
//! fn identity(value: u64) -> u64 { value }
//! ```
//!
//! Specification operators cannot invoke arbitrary user-defined implementations:
//! ```compile_fail
//! use sail_contracts::requirement;
//! #[derive(Clone, Copy, PartialEq)]
//! struct Custom(u64);
//! #[requirement("CUSTOM")]
//! fn predicate(left: Custom, right: Custom) -> bool { left == right }
//! ```
#![forbid(unsafe_code)]

use std::ops::{Add, Div, Mul, Neg, Rem, Sub};

pub use sail_contracts_macros::{ensures, requirement, requires, satisfies};

/// Runtime mathematical specification integer, represented by checked `i128`.
///
/// Verification uses mathematical integers. Runtime overflow stops execution;
/// it never wraps. That distinction must remain an evidence qualifier.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Int(i128);

impl Int {
    /// Construct a representable runtime specification integer.
    #[must_use]
    pub const fn new(value: i128) -> Self {
        Self(value)
    }

    /// The exact runtime representation.
    #[must_use]
    pub const fn get(self) -> i128 {
        self.0
    }
}

/// A sealed scalar whose entry value can be copied without user code.
pub trait Scalar: sealed::Sealed + Copy {}

mod sealed {
    pub trait Sealed {}
}

macro_rules! scalars {
    ($($kind:ty),* $(,)?) => { $(impl sealed::Sealed for $kind {} impl Scalar for $kind {})* };
}
scalars!(bool, u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, Int);

/// Implementation details used by generated contracts; not a verdict API.
#[doc(hidden)]
pub mod __private {
    use super::Scalar;

    /// Scalar arithmetic which cannot silently wrap or divide by zero.
    pub trait Arithmetic: Scalar {
        /// Checked addition.
        fn checked_add(self, rhs: Self) -> Option<Self>;
        /// Checked subtraction.
        fn checked_sub(self, rhs: Self) -> Option<Self>;
        /// Checked multiplication.
        fn checked_mul(self, rhs: Self) -> Option<Self>;
        /// Checked division.
        fn checked_div(self, rhs: Self) -> Option<Self>;
        /// Checked remainder.
        fn checked_rem(self, rhs: Self) -> Option<Self>;
    }

    /// Scalars admitting checked signed negation.
    pub trait Signed: Scalar {
        /// Checked negation.
        fn checked_neg(self) -> Option<Self>;
    }

    /// Evaluate a contract condition in every build profile.
    #[track_caller]
    #[allow(
        clippy::panic,
        reason = "A failed user contract intentionally stops execution; this is the runtime contract semantics."
    )]
    pub fn check(condition: bool, requirement: &str, phase: &str) {
        assert!(
            condition,
            "SAIL0702: {phase} failed for requirement {requirement}"
        );
    }

    /// A checked specification arithmetic failure, never an internal invariant panic.
    #[track_caller]
    #[allow(
        clippy::panic,
        reason = "Invalid user specification arithmetic intentionally stops execution in all profiles."
    )]
    pub fn arithmetic_failure() -> ! {
        panic!("SAIL0703: specification arithmetic overflow or invalid division");
    }

    /// Copy an entry scalar without calling user-defined cloning code.
    #[must_use]
    pub fn snapshot<T: Scalar>(value: &T) -> T {
        *value
    }

    /// Preserve an owned Copy result while evaluating its postcondition.
    #[must_use]
    pub fn copy_result<T: Copy>(value: &T) -> T {
        *value
    }

    macro_rules! comparison {
        ($name:ident, $bound:ident, $operator:tt, $doc:literal) => {
            #[doc = $doc]
            #[must_use]
            pub fn $name<T: Scalar + $bound>(left: T, right: T) -> bool { left $operator right }
        };
    }
    comparison!(eq, PartialEq, ==, "Pure scalar equality.");
    comparison!(ne, PartialEq, !=, "Pure scalar inequality.");
    comparison!(lt, PartialOrd, <, "Pure scalar less-than comparison.");
    comparison!(le, PartialOrd, <=, "Pure scalar less-than-or-equal comparison.");
    comparison!(gt, PartialOrd, >, "Pure scalar greater-than comparison.");
    comparison!(ge, PartialOrd, >=, "Pure scalar greater-than-or-equal comparison.");

    /// Scalar negation that cannot dispatch to a user-defined effectful operator.
    #[must_use]
    pub fn not<T: Scalar + std::ops::Not<Output = T>>(value: T) -> T {
        !value
    }

    macro_rules! operation {
        ($name:ident, $method:ident, $doc:literal) => {
            #[doc = $doc]
            #[track_caller]
            pub fn $name<T: Arithmetic>(left: T, right: T) -> T {
                left.$method(right).unwrap_or_else(|| arithmetic_failure())
            }
        };
    }
    operation!(add, checked_add, "Checked addition used by specifications.");
    operation!(
        sub,
        checked_sub,
        "Checked subtraction used by specifications."
    );
    operation!(
        mul,
        checked_mul,
        "Checked multiplication used by specifications."
    );
    operation!(div, checked_div, "Checked division used by specifications.");
    operation!(
        rem,
        checked_rem,
        "Checked remainder used by specifications."
    );

    /// Checked signed negation used by specifications.
    #[track_caller]
    pub fn neg<T: Signed>(value: T) -> T {
        value.checked_neg().unwrap_or_else(|| arithmetic_failure())
    }
}

macro_rules! arithmetic {
    ($($kind:ty),* $(,)?) => { $(
        impl __private::Arithmetic for $kind {
            fn checked_add(self, rhs: Self) -> Option<Self> { self.checked_add(rhs) }
            fn checked_sub(self, rhs: Self) -> Option<Self> { self.checked_sub(rhs) }
            fn checked_mul(self, rhs: Self) -> Option<Self> { self.checked_mul(rhs) }
            fn checked_div(self, rhs: Self) -> Option<Self> { self.checked_div(rhs) }
            fn checked_rem(self, rhs: Self) -> Option<Self> { self.checked_rem(rhs) }
        }
    )* };
}
arithmetic!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

macro_rules! signed {
    ($($kind:ty),* $(,)?) => { $(
        impl __private::Signed for $kind {
            fn checked_neg(self) -> Option<Self> { self.checked_neg() }
        }
    )* };
}
signed!(i8, i16, i32, i64, i128);

impl __private::Arithmetic for Int {
    fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }
    fn checked_sub(self, rhs: Self) -> Option<Self> {
        self.0.checked_sub(rhs.0).map(Self)
    }
    fn checked_mul(self, rhs: Self) -> Option<Self> {
        self.0.checked_mul(rhs.0).map(Self)
    }
    fn checked_div(self, rhs: Self) -> Option<Self> {
        self.0.checked_div(rhs.0).map(Self)
    }
    fn checked_rem(self, rhs: Self) -> Option<Self> {
        self.0.checked_rem(rhs.0).map(Self)
    }
}

impl __private::Signed for Int {
    fn checked_neg(self) -> Option<Self> {
        self.0.checked_neg().map(Self)
    }
}

macro_rules! int_operator {
    ($trait:ident, $method:ident, $runtime:ident) => {
        impl $trait for Int {
            type Output = Self;
            #[track_caller]
            fn $method(self, rhs: Self) -> Self {
                __private::$runtime(self, rhs)
            }
        }
    };
}
int_operator!(Add, add, add);
int_operator!(Sub, sub, sub);
int_operator!(Mul, mul, mul);
int_operator!(Div, div, div);
int_operator!(Rem, rem, rem);

impl Neg for Int {
    type Output = Self;
    #[track_caller]
    fn neg(self) -> Self {
        __private::neg(self)
    }
}
