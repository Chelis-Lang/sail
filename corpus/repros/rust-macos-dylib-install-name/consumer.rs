extern crate sail_native_identity;

sail_native_identity::identity! {
    fn answer() -> u8 { 42 }
}

fn main() {
    assert_eq!(answer(), 42);
}
