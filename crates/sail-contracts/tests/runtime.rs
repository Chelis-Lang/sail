//! Real macro expansion and runtime contract checks in every build profile.
use sail_contracts::{Int, ensures, requirement, requires, satisfies};

#[requirement("ADD-ONE")]
fn add_one_predicate(input: u64, output: u64) -> bool {
    output == input + 1
}

#[requires(input < 100)]
#[ensures(|result| add_one_predicate(input, result))]
#[satisfies("ADD-ONE")]
fn add_one(input: u64) -> u64 {
    if input == 0 {
        return 1;
    }
    input + 1
}

#[satisfies("BROKEN")]
#[ensures(|result| result == input)]
fn broken(input: u64) -> u64 {
    input + 1
}

#[derive(Clone, Copy)]
struct Wallet {
    balance: u64,
    calls: u64,
}

impl Wallet {
    #[satisfies("WITHDRAW")]
    #[requires(amount <= self.balance)]
    #[ensures(|result| result == self.balance && self.balance == old(self.balance) - amount && self.calls == old(self.calls) + 1)]
    fn withdraw(&mut self, amount: u64) -> u64 {
        self.calls += 1;
        self.balance -= amount;
        if amount == 0 {
            return self.balance;
        }
        self.balance
    }

    #[satisfies("WALLET-NEW")]
    #[ensures(|result| result.balance == initial && result.calls == 0)]
    fn new(initial: u64) -> Self {
        Self {
            balance: initial,
            calls: 0,
        }
    }

    #[satisfies("WALLET-GET")]
    #[ensures(|result| result == self.balance)]
    fn balance(&self) -> u64 {
        self.balance
    }
}

#[ensures(|result| result == input + 1)]
#[satisfies("CHECKED-SPEC")]
fn specification_overflow(input: u8) -> u8 {
    input
}

#[requirement("SHORT-CIRCUIT")]
fn guarded_division(x: u64, zero: u64) -> bool {
    zero == 0 || x / zero > 1
}

#[requirement("RAW")]
fn r#match(x: i8) -> bool {
    x >= -128i8
}

#[satisfies("RAW")]
#[ensures(|ret| r#match(ret))]
fn raw_name(x: i8) -> i8 {
    x
}

#[satisfies("HYGIENE")]
#[ensures(|ret| ret == old(__sail_old_0) + 1 && __sail_contract_result == 5)]
fn names(__sail_old_0: u64, __sail_contract_result: u64) -> u64 {
    __sail_old_0 + 1
}

#[test]
fn pre_and_postconditions_are_real_checks() {
    assert!(add_one_predicate(1, 2));
    assert_eq!(add_one(0), 1);
    assert_eq!(add_one(40), 41);
    assert!(std::panic::catch_unwind(|| add_one(100)).is_err());
    assert!(std::panic::catch_unwind(|| broken(4)).is_err());
}

#[test]
fn early_returns_run_once_and_old_captures_mutable_state() {
    let mut wallet = Wallet::new(10);
    assert_eq!(wallet.withdraw(0), 10);
    assert_eq!(wallet.calls, 1);
    assert_eq!(wallet.withdraw(3), 7);
    assert_eq!(wallet.calls, 2);
    assert_eq!(wallet.balance(), 7);
}

#[test]
fn failed_precondition_does_not_run_the_body() {
    let mut wallet = Wallet::new(10);
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| wallet.withdraw(11))).is_err()
    );
    assert_eq!(wallet.balance, 10);
    assert_eq!(wallet.calls, 0);
}

#[test]
fn checked_arithmetic_is_independent_of_build_overflow_flags() {
    assert!(std::panic::catch_unwind(|| specification_overflow(u8::MAX)).is_err());
    assert!(std::panic::catch_unwind(|| Int::new(i128::MAX) + Int::new(1)).is_err());
    assert!(std::panic::catch_unwind(|| Int::new(1) / Int::new(0)).is_err());
    assert!(std::panic::catch_unwind(|| -Int::new(i128::MIN)).is_err());
    assert_eq!((Int::new(40) + Int::new(2)).get(), 42);
    assert!(guarded_division(10, 0));
    assert!(guarded_division(10, 2));
    assert_eq!(raw_name(i8::MIN), i8::MIN);
    assert!(r#match(i8::MIN));
    assert_eq!(names(2, 5), 3);
}
