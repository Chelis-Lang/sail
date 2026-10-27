//! Native differential fixtures for the actual request_order operations.

use crate::request_order::OriginalTargetKey;
use std::collections::BTreeSet;

#[test]
fn byte_key_order_matches_the_original_native_tuple_order() {
    let tuples: Vec<(String, String, String)> = vec![
        ("".into(), "".into(), "".into()),
        ("pkg".into(), "target".into(), "lib".into()),
        ("pkg".into(), "target".into(), "bin".into()),
        ("pkg".into(), "t".into(), "lib".into()),
        ("pkg".into(), "target\0".into(), "lib".into()),
        ("pkg".into(), "target".into(), "lib\n".into()),
        ("λ".into(), "target".into(), "lib".into()),
        ("pkg".into(), "λ".into(), "lib".into()),
        ("pkg".into(), "target".into(), "lib".into()),
    ];
    for left in &tuples {
        for right in &tuples {
            let actual_left = OriginalTargetKey::new(left.clone());
            let actual_right = OriginalTargetKey::new(right.clone());
            assert_eq!(actual_left.cmp(&actual_right), left.cmp(right));
            assert_eq!(
                actual_left.partial_cmp(&actual_right),
                left.partial_cmp(right)
            );
            assert_eq!(actual_left == actual_right, left == right);
            assert_eq!(actual_left.clone().as_tuple(), left);
        }
    }
    let original: BTreeSet<_> = tuples.iter().cloned().collect();
    let actual: BTreeSet<_> = tuples.into_iter().map(OriginalTargetKey::new).collect();
    assert_eq!(actual.len(), original.len());
    let projection: Vec<_> = actual.iter().map(OriginalTargetKey::as_tuple).collect();
    assert_eq!(projection, original.iter().collect::<Vec<_>>());
}
