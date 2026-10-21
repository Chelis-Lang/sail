#[derive(Clone, Copy)]
pub struct Wallet { pub balance: u64, pub calls: u64 }

impl Wallet {
    #[sail_contracts::satisfies("WALLET-NEW")]
    #[sail_contracts::ensures(|result| result.balance == initial && result.calls == 0)]
    pub fn new(initial: u64) -> Self { Self { balance: initial, calls: 0 } }

    #[sail_contracts::requires(amount <= self.balance)]
    #[sail_contracts::requires(self.calls < 100)]
    #[sail_contracts::ensures(|result| result == self.balance && self.balance == old(self.balance) - amount && self.calls == old(self.calls) + 1)]
    #[sail_contracts::satisfies("WALLET-WITHDRAW")]
    pub fn withdraw(&mut self, amount: u64) -> u64 {
        self.calls += 1;
        self.balance -= amount;
        if amount == 0 { return self.balance; }
        self.balance
    }
}
