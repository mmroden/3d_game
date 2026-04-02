//! Credit (currency) tracking for the upgrade shop.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEnoughCredits {
    pub balance: u32,
    pub cost: u32,
}

impl fmt::Display for NotEnoughCredits {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "need {} credits but only have {}", self.cost, self.balance)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CreditAccount {
    pub balance: u32,
}

impl CreditAccount {
    pub fn new() -> Self {
        Self { balance: 0 }
    }

    pub fn earn(&mut self, amount: u32) {
        self.balance += amount;
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.balance >= cost
    }

    pub fn spend(&mut self, cost: u32) -> Result<(), NotEnoughCredits> {
        if self.can_afford(cost) {
            self.balance -= cost;
            Ok(())
        } else {
            Err(NotEnoughCredits { balance: self.balance, cost })
        }
    }
}

impl Default for CreditAccount {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let account = CreditAccount::new();
        assert_eq!(account.balance, 0);
    }

    #[test]
    fn earn_increases_balance() {
        let mut account = CreditAccount::new();
        account.earn(1000);
        assert_eq!(account.balance, 1000);
        account.earn(500);
        assert_eq!(account.balance, 1500);
    }

    #[test]
    fn can_afford_check() {
        let mut account = CreditAccount::new();
        account.earn(10_000);
        assert!(account.can_afford(10_000));
        assert!(account.can_afford(5_000));
        assert!(!account.can_afford(10_001));
    }

    #[test]
    fn spend_deducts_balance() {
        let mut account = CreditAccount::new();
        account.earn(20_000);
        assert!(account.spend(10_000).is_ok());
        assert_eq!(account.balance, 10_000);
    }

    #[test]
    fn spend_insufficient_fails() {
        let mut account = CreditAccount::new();
        account.earn(5_000);
        let err = account.spend(10_000).unwrap_err();
        assert_eq!(err.balance, 5_000);
        assert_eq!(err.cost, 10_000);
        assert_eq!(account.balance, 5_000); // unchanged
    }

    #[test]
    fn spend_exact_balance() {
        let mut account = CreditAccount::new();
        account.earn(10_000);
        assert!(account.spend(10_000).is_ok());
        assert_eq!(account.balance, 0);
    }

    #[test]
    fn spend_zero_always_works() {
        let account = CreditAccount::new();
        assert!(account.can_afford(0));
    }
}
