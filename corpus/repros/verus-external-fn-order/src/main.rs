#![allow(dead_code)]

mod message_t {
    #![verus::trusted]
    use vstd::prelude::*;

    verus! {
        pub enum Message {
            Get { key: u64 },
            Set { key: u64, value: u64 },
            Reply { key: u64, value: u64 },
        }
    }
}

use vstd::prelude::*;

verus! {
    proof fn smoke() {
        assert(true);
    }
}

fn main() {}
