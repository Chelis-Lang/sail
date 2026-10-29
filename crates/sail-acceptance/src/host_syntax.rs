//! Source-shared host trait declarations; contracts do not supply native I/O implementations.

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
