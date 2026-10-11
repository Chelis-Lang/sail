//! Conservative requested-storage accounting on the pinned 64-bit toolchain.
//! This charges capacities and collection allocations, not allocator RSS,
//! kernel watch/lock memory, file descriptors or child-process address spaces.

use std::mem::size_of;

/// The audited `BTree` nodes fit 1024 bytes: eleven <=48-byte entries, twelve
/// edges and node metadata. Charging one node per entry plus an empty root
/// deliberately overcounts sparse and dense nodes alike. Larger layouts must
/// use a separate audit instead of silently inheriting this allowance.
pub(super) fn tree(entries: usize) -> Option<usize> {
    (size_of::<usize>() == 8).then_some(())?;
    entries.checked_add(1)?.checked_mul(1024)
}

pub(super) fn vector<T>(values: &Vec<T>) -> Option<usize> {
    values.capacity().checked_mul(size_of::<T>())
}

/// Rust1.99's standard `HashMap` uses a power-of-two hashbrown table at <=7/8
/// load, or a small minimum table. Capacity is the usable entry count, not the
/// number of allocated buckets. Twice the next power of two plus32 bounds
/// those buckets and minimum size. Thirty-two extra bytes per bucket cover
/// control bytes, SIMD group padding and alignment for the audited <=8-byte
/// aligned key/value tuples. No assumption that capacity equals buckets.
pub(super) fn hash<T>(capacity: usize) -> Option<usize> {
    (size_of::<usize>() == 8 && std::mem::align_of::<T>() <= 8).then_some(())?;
    if capacity == 0 {
        return Some(0);
    }
    capacity
        .checked_next_power_of_two()?
        .checked_mul(2)?
        .checked_add(32)?
        .checked_mul(size_of::<T>().checked_add(32)?)
}

#[cfg(test)]
mod tests {
    use super::{hash, tree, vector};

    #[test]
    fn admission_overflow_refuses_instead_of_wrapping() {
        assert_eq!(tree(usize::MAX), None);
        assert_eq!(hash::<(usize, usize)>(usize::MAX), None);
        assert_eq!(hash::<(usize, usize)>(0), Some(0));
    }

    #[test]
    fn spare_capacity_and_small_minimum_tables_are_charged() {
        let values = Vec::<u64>::with_capacity(64);
        assert_eq!(vector(&values), Some(values.capacity() * 8));
        for requested in [1, 3, 7, 32, 1024, 65536] {
            let map = std::collections::HashMap::<u64, u64>::with_capacity(requested);
            // The charge includes the full capacity's inline entries, not just
            // its (currently empty) length, plus minimum/control storage.
            assert!(
                hash::<(u64, u64)>(map.capacity())
                    .is_some_and(|bytes| { bytes >= map.capacity() * 16 + 32 })
            );
        }
    }
}
