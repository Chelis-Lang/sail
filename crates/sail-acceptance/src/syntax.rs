//! Small syntax envelopes shared by Cargo and direct stock-Verus checks.
//! The contracts never supply unchecked executable implementations.

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_code {
    ($($code:tt)*) => { $($code)* };
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_code {
    ($($code:tt)*) => { ::vstd::prelude::verus! { $($code)* } };
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_spec {
    ($($code:tt)*) => {};
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_spec {
    ($($code:tt)*) => { ::vstd::prelude::verus! { $($code)* } };
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_impl_spec {
    ($($code:tt)*) => {};
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_impl_spec {
    ($($code:tt)*) => { ::verus_builtin_macros::verus_impl! { $($code)* } };
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_proof {
    ($($code:tt)*) => {{}};
}

/// Emit executable items or checked proof syntax for the selected build.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_proof {
    ($($code:tt)*) => { ::verus_builtin_macros::proof! { $($code)* } };
}

/// Compile the same executable signature and body with checked contracts.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_verified {
    ($(#[$attribute:meta])* $visibility:vis fn $name:ident $(<$($generic:tt),*>)?
        ($($argument:tt)*) -> ($result:ident: $return_type:ty)
        $(where ($($bounds:tt)*))?
        requires ($($requires:tt)*) ensures ($($ensures:tt)*)
        $(decreases ($($decreases:tt)*))? $body:block) => {
        $(#[$attribute])* $visibility fn $name $(<$($generic),*>)? ($($argument)*) -> $return_type $(where $($bounds)*)? $body
    };
}

/// Compile the same executable signature and body with checked contracts.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_verified {
    ($(#[$attribute:meta])* $visibility:vis fn $name:ident $(<$($generic:tt),*>)?
        ($($argument:tt)*) -> ($result:ident: $return_type:ty)
        $(where ($($bounds:tt)*))?
        requires ($($requires:tt)*) ensures ($($ensures:tt)*)
        $(decreases ($($decreases:tt)*))? $body:block) => {
        ::vstd::prelude::verus! {
            $(#[$attribute])* $visibility fn $name $(<$($generic),*>)?
                ($($argument)*) -> ($result: $return_type)
                $(where $($bounds)*)?
                requires $($requires)*, ensures $($ensures)*,
                $(decreases $($decreases)*,)? $body
        }
    };
}

/// Compile the same executable signature and body with checked contracts.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_method {
    ($(#[$attribute:meta])* $visibility:vis fn $name:ident $(<$($generic:tt),*>)?
        ($($argument:tt)*) -> ($result:ident: $return_type:ty)
        $(where ($($bounds:tt)*))?
        requires ($($requires:tt)*) ensures ($($ensures:tt)*)
        $(decreases ($($decreases:tt)*))? $body:block) => {
        $(#[$attribute])* $visibility fn $name $(<$($generic),*>)? ($($argument)*) -> $return_type $(where $($bounds)*)? $body
    };
}

/// Compile the same executable signature and body with checked contracts.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_method {
    ($(#[$attribute:meta])* $visibility:vis fn $name:ident $(<$($generic:tt),*>)?
        ($($argument:tt)*) -> ($result:ident: $return_type:ty)
        $(where ($($bounds:tt)*))?
        requires ($($requires:tt)*) ensures ($($ensures:tt)*)
        $(decreases ($($decreases:tt)*))? $body:block) => {
        ::verus_builtin_macros::verus_impl! {
            $(#[$attribute])* $visibility fn $name $(<$($generic),*>)?
                ($($argument)*) -> ($result: $return_type)
                $(where $($bounds)*)?
                requires $($requires)*, ensures $($ensures)*,
                $(decreases $($decreases)*,)? $body
        }
    };
}

/// Compile a loop with the same condition/body and checked invariants.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_while {
    (($condition:expr) invariant ($($invariant:tt)*) decreases ($($decreases:tt)*) $body:block) => {
        while $condition $body
    };
}
/// Compile a loop with the same condition/body and checked invariants.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_while {
    (($condition:expr) invariant ($($invariant:tt)*) decreases ($($decreases:tt)*) $body:block) => {
        ::verus_builtin_macros::verus_exec_expr! { while $condition invariant $($invariant)*, decreases $($decreases)*, $body }
    };
}

/// Declare a host trait with identical executable method signatures.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_trait {
    ($(#[$attribute:meta])* $visibility:vis trait $name:ident $(<$($generic:tt),*>)? : Sized {
        $(spec { $($specification:tt)* })?
        $($(#[$method_attribute:meta])* fn $method:ident ($($argument:tt)*)
            -> ($result:ident: $return_type:ty)
            requires ($($requires:tt)*) ensures ($($ensures:tt)*);)*
    }) => {
        $(#[$attribute])* $visibility trait $name $(<$($generic),*>)? : Sized {
            $($(#[$method_attribute])* fn $method ($($argument)*) -> $return_type;)*
        }
    };
    ($(#[$attribute:meta])* $visibility:vis trait $name:ident $(<$($generic:tt),*>)? {
        $(spec { $($specification:tt)* })?
        $($(#[$method_attribute:meta])* fn $method:ident ($($argument:tt)*)
            -> ($result:ident: $return_type:ty)
            requires ($($requires:tt)*) ensures ($($ensures:tt)*);)*
    }) => {
        $(#[$attribute])* $visibility trait $name $(<$($generic),*>)? {
            $($(#[$method_attribute])* fn $method ($($argument)*) -> $return_type;)*
        }
    };
}

/// Declare the same host interface with checked method contracts.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_trait {
    ($(#[$attribute:meta])* $visibility:vis trait $name:ident $(<$($generic:tt),*>)? : Sized {
        $(spec { $($specification:tt)* })?
        $($(#[$method_attribute:meta])* fn $method:ident ($($argument:tt)*)
            -> ($result:ident: $return_type:ty)
            requires ($($requires:tt)*) ensures ($($ensures:tt)*);)*
    }) => {
        ::vstd::prelude::verus! {
            $(#[$attribute])* $visibility trait $name $(<$($generic),*>)? : Sized {
                $($($specification)*)?
                $($(#[$method_attribute])* fn $method ($($argument)*) -> ($result: $return_type)
                    requires $($requires)*, ensures $($ensures)*;)*
            }
        }
    };
    ($(#[$attribute:meta])* $visibility:vis trait $name:ident $(<$($generic:tt),*>)? {
        $(spec { $($specification:tt)* })?
        $($(#[$method_attribute:meta])* fn $method:ident ($($argument:tt)*)
            -> ($result:ident: $return_type:ty)
            requires ($($requires:tt)*) ensures ($($ensures:tt)*);)*
    }) => {
        ::vstd::prelude::verus! {
            $(#[$attribute])* $visibility trait $name $(<$($generic),*>)? {
                $($($specification)*)?
                $($(#[$method_attribute])* fn $method ($($argument)*) -> ($result: $return_type)
                    requires $($requires)*, ensures $($ensures)*;)*
            }
        }
    };
}

/// Trait specification implementations add no executable Cargo items.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_trait_impl_spec {
    ($($code:tt)*) => {};
}

/// Check trait specification implementations in their actual trait context.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_trait_impl_spec {
    ($($code:tt)*) => { ::verus_builtin_macros::verus_trait_impl! { $($code)* } };
}
