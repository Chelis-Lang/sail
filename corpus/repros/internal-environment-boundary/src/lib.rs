use vstd::prelude::*;

verus! {
    proof fn injected_runtime_is_present() {
        assert(match option_env!("SAIL_MANAGED_RUNTIME") {
            Some(_) => true,
            None => false,
        });
    }
}
