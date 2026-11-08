// Profile three: the invariant forgets the accumulator, so the guarantee fails.

#[sail_contracts::satisfies("LOOP-WEAK")]
#[sail_contracts::requires(n <= 1000)]
#[sail_contracts::ensures(|ret| ret == n * 2)]
pub fn double(n: u32) -> u32 {
    let mut i: u32 = 0;
    let mut total: u32 = 0;
    while i < n {
        sail_contracts::invariant!(i <= n, total <= i * 2, n <= 1000);
        sail_contracts::decreases!(n - i);
        i += 1;
        total += 2;
    }
    total
}
