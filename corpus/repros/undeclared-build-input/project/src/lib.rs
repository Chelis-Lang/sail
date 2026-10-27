include!(concat!(env!("OUT_DIR"), "/generated.rs"));

#[cfg(feature = "verification")]
vstd::prelude::verus! {
    proof fn external_value_is_seven() {
        assert(EXTERNAL_VALUE == 7);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn show_compiled_external_value() {
        println!("observed_external_value={}", super::EXTERNAL_VALUE);
    }
}
