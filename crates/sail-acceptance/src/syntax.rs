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

/// Compile a checked method that cannot unwind or open invariants.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_no_unwind_method {
    ($(#[$attribute:meta])* $visibility:vis fn $name:ident $(<$($generic:tt),*>)?
        ($($argument:tt)*) -> ($result:ident: $return_type:ty)
        $(where ($($bounds:tt)*))?
        requires ($($requires:tt)*) ensures ($($ensures:tt)*)
        $(decreases ($($decreases:tt)*))? $body:block) => {
        $(#[$attribute])* $visibility fn $name $(<$($generic),*>)? ($($argument)*) -> $return_type $(where $($bounds)*)? $body
    };
}

/// Compile the same method with fixed checked destruction restrictions.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_no_unwind_method {
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
                $(decreases $($decreases)*,)?
                opens_invariants none
                no_unwind
                $body
        }
    };
}

/// Compile the actual Rust Drop method from its one shared body.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_drop {
    (fn drop(&mut $receiver:ident) $body:block) => {
        fn drop(&mut $receiver) $body
    };
}

/// Check that same Drop body without unwinding or invariant opening.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_drop {
    (fn drop(&mut $receiver:ident) $body:block) => {
        ::verus_builtin_macros::verus_trait_impl! {
            fn drop(&mut $receiver)
                opens_invariants none
                no_unwind
                $body
        }
    };
}

/// Erase a proof snapshot without adding a runtime counter or copy.
#[cfg(not(sail_verus))]
#[macro_export]
macro_rules! sail_snapshot {
    (mut $binding:ident = $value:expr) => {};
    ($binding:ident = $value:expr) => {};
}

/// Declare that snapshot in the enclosing checked function's ghost scope.
#[cfg(sail_verus)]
#[macro_export]
macro_rules! sail_snapshot {
    (mut $binding:ident = $value:expr) => {
        ::verus_builtin_macros::proof_decl! { let ghost mut $binding = $value; }
    };
    ($binding:ident = $value:expr) => {
        ::verus_builtin_macros::proof_decl! { let ghost $binding = $value; }
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
