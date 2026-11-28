//! Harnesses for the Kani fault arm (ADR 0183). `small` succeeds at once with
//! several checks. `hog` succeeds too, but its CBMC run needs about
//! 2.6 GiB while the crate builds in far less (spike S6's `hog_1000`), so a
//! cgroup memory cap between the two makes the kernel kill CBMC itself.

#[cfg(kani)]
mod harnesses {
    #[kani::proof]
    fn small() {
        let a: u8 = kani::any();
        let b: u8 = kani::any();
        kani::assume(a < 100 && b < 100);
        let sum = a + b;
        assert!(sum < 200, "sum stays below 200");
        assert!(sum >= a, "sum is at least a");
        assert!(sum >= b, "sum is at least b");
        assert!(a.wrapping_add(1) <= 100, "a + 1 stays at most 100");
    }

    #[kani::proof]
    #[kani::unwind(1001)]
    fn hog() {
        let y: u32 = kani::any();
        kani::assume(y <= 1);
        let mut x: u32 = kani::any();
        let mut i = 0u32;
        while i < 1000 {
            x = ((x ^ i) * y) >> 1;
            i += 1;
        }
        assert!(x <= u32::MAX / 2);
    }
}
