// Profile three: while loops with invariants and termination measures.

#[sail_contracts::satisfies("LOOP-DOUBLE")]
#[sail_contracts::requires(n <= 1000)]
#[sail_contracts::ensures(|ret| ret == n * 2)]
pub fn double(n: u32) -> u32 {
    let mut i: u32 = 0;
    let mut total: u32 = 0;
    while i < n {
        sail_contracts::invariant!(i <= n, total == i * 2, n <= 1000);
        sail_contracts::decreases!(n - i);
        i += 1;
        total += 2;
    }
    total
}

#[sail_contracts::satisfies("LOOP-DRAIN")]
#[sail_contracts::ensures(|ret| ret == 0)]
pub fn drain(start: u8) -> u8 {
    let mut left: u8 = start;
    while left > 0 {
        sail_contracts::decreases!(left);
        left -= 1;
    }
    left
}

#[sail_contracts::satisfies("LOOP-GRID")]
#[sail_contracts::requires(rows <= 1000)]
#[sail_contracts::ensures(|ret| ret == rows * 4)]
pub fn cells(rows: u16) -> u16 {
    let mut row: u16 = 0;
    let mut count: u16 = 0;
    while row < rows {
        sail_contracts::invariant!(row <= rows, count == row * 4, rows <= 1000);
        sail_contracts::decreases!(rows - row);
        let mut column: u16 = 0;
        while column < 4 {
            sail_contracts::invariant!(
                column <= 4,
                count == row * 4 + column,
                row < rows,
                rows <= 1000
            );
            sail_contracts::decreases!(4 - column);
            column += 1;
            count += 1;
        }
        row += 1;
    }
    count
}
