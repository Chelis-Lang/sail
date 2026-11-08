// Profile three: the measure grows, so termination fails.

#[sail_contracts::satisfies("LOOP-MEASURE")]
#[sail_contracts::ensures(|ret| ret == n)]
pub fn count(n: u32) -> u32 {
    let mut i: u32 = 0;
    while i < n {
        sail_contracts::invariant!(i <= n);
        sail_contracts::decreases!(i);
        i += 1;
    }
    i
}
