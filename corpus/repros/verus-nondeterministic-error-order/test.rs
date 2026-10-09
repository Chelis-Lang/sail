#![feature(fmt_internals)]

#![allow(unused_imports)]
#![allow(unused_features)]
#![allow(unused_macros)]
#![allow(deprecated)]
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![allow(non_upper_case_globals)]
#![allow(unused_comparisons)]
#![allow(noop_method_call)]
#![feature(allocator_api)]
#![feature(proc_macro_hygiene)]
#![feature(never_type)]
#![feature(core_intrinsics)]
#![feature(ptr_metadata)]
#![feature(sized_hierarchy)]
#![feature(btree_cursors)]
#![feature(const_destruct)]
#![feature(print_internals)]
use verus_builtin::*;
use verus_builtin_macros::*;

::verus_builtin_macros::verus!{
mod M1 {
    pub trait T<A> {
        spec fn req(&self, a: A) -> bool;

        spec fn ens(&self, a: A, r: A) -> bool;

        fn f(&self, a: &A) -> (ra: A)
            requires self.req(*a),
            ensures self.ens(*a, ra); // TRAIT
    }
}

mod M2 {
    pub struct B {
        pub x: bool,
    }
}

mod M3 {
    pub struct I {
        pub x: u64,
    }
}

mod M4 {
    impl crate::M1::T<bool> for crate::M2::B {
        closed spec fn req(&self, a: bool) -> bool {
            a
        }

        closed spec fn ens(&self, a: bool, r: bool) -> bool {
            r == (a && self.x)
        }

        fn f(&self, a: &bool) -> bool {
            *a && self.x
        }
    }
}

mod M5 {
    use verus_builtin::*;
    impl crate::M1::T<u64> for crate::M3::I {
        closed spec fn req(&self, a: u64) -> bool {
            self.x < a && a < 100
        }

        closed spec fn ens(&self, a: u64, r: u64) -> bool {
            self.x <= r && r < 100
        }

        fn f(&self, a: &u64) -> u64 {
            self.x / 2 + a // FAILS
        }
    }
}

mod M6 {
    pub fn p<A, Z: crate::M1::T<A>>(a: &A, z: &Z) -> (rz: A)
        requires z.req(*a)
        ensures z.ens(*a, rz)
    {
        z.f(a)
    }
}

mod M7 {
    fn test() {
        let i = crate::M3::I { x: 30 };
        vstd::pervasive::print_u64(crate::M6::p(&10, &i)); // FAILS
    }
}
}
