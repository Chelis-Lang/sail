use vstd::prelude::*;
verus! {
    proof fn witness() {
        let data = include_bytes!("data.bin");
        assert(data.len() == 1);
    }
}
fn main() {}
