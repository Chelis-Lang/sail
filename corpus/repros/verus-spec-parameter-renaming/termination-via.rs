::verus_builtin_macros::verus! { mod defs {
            use vstd::prelude::*;
            pub open spec fn value(sail_arg: nat) -> nat decreases sail_arg {
                if sail_arg == 0 { 0 } else { 1 + value((sail_arg - 1) as nat) }
            }
            pub open spec fn via_value(sail_arg: nat) -> nat
                decreases sail_arg via via_value_decreases
            {
                if sail_arg == 0 { 0 } else { 1 + via_value((sail_arg - 1) as nat) }
            }
            #[via_fn]
            proof fn via_value_decreases(sail_arg: nat) {
                if sail_arg > 0 {
                    assert(((sail_arg - 1) as nat) < sail_arg);
                }
            }
        }
mod client { use vstd::prelude::*; proof fn check() {
            reveal_with_fuel(crate::defs::value, 2);
            assert(crate::defs::value(1) == 1);
            reveal_with_fuel(crate::defs::via_value, 2);
            assert(crate::defs::via_value(1) == 1);
        } } }
