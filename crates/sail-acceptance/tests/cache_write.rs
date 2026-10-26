//! The checked cache transaction executes on nonempty and failed primitive scripts.

#[test]
fn a_completed_write_preserves_its_actual_record_image() {
    assert!(sail_acceptance::cache_write::successful_write_control());
}

#[test]
fn every_primitive_failure_refuses_a_completed_write() {
    for stage in [1, 3, 5, 7, 10] {
        assert!(sail_acceptance::cache_write::failed_write_control(stage));
    }
}

#[test]
fn a_malformed_header_cleans_only_its_owned_temporary_file() {
    assert!(sail_acceptance::cache_write::malformed_write_header_control());
}
