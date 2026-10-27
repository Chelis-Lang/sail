//! Unselected full-file support probe for the actual cache writer's Arc operations.
//!
//! This is not a candidate writer implementation or an origin admission model.
//! Run the whole file through the pinned stock verifier with default resources.
//! Unsupported operations and failed witnesses earn no negative-control credit.
#![forbid(unsafe_code)]

use std::sync::Arc;
use vstd::prelude::*;

verus! {

/// Test whether the actual standard-library operation is available at all.
pub fn actual_ptr_comparison(original: &Arc<Vec<u8>>, actual: &Arc<Vec<u8>>)
    -> (same: bool)
{
    Arc::ptr_eq(original, actual)
}

/// A retained clone must name the original nonempty allocation.
pub fn retained_clone_is_original() {
    let original = Arc::new(vec![117u8, 110u8, 115u8, 97u8, 116u8, 10u8]);
    let retained = Arc::clone(&original);
    assert(original@.len() == 6);
    let same = Arc::ptr_eq(&original, &retained);
    assert(same);
}

/// Equal payloads cannot stand in for allocation identity.
pub fn equal_bytes_distinct_allocation() {
    let original = Arc::new(vec![117u8, 110u8, 115u8, 97u8, 116u8, 10u8]);
    let other = Arc::new(vec![117u8, 110u8, 115u8, 97u8, 116u8, 10u8]);
    assert(original@.len() == 6);
    assert(original@ == other@);
    let same = Arc::ptr_eq(&original, &other);
    assert(!same);
}

/// A completion retains the old allocation while an equal-valued replacement lives.
pub fn replacement_is_not_original() {
    let original = Arc::new(vec![117u8, 110u8, 115u8, 97u8, 116u8, 10u8]);
    let completion_original = Arc::clone(&original);
    let replacement = Arc::new(vec![117u8, 110u8, 115u8, 97u8, 116u8, 10u8]);
    assert(original@.len() == 6);
    assert(original@ == replacement@);
    let original_matches = Arc::ptr_eq(&completion_original, &original);
    let replacement_matches = Arc::ptr_eq(&completion_original, &replacement);
    assert(original_matches);
    assert(!replacement_matches);
}

} // verus!

fn main() {}
