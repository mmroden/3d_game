//! Dual-currency accounts.
//!
//! Both currencies share one generic [`Account<K>`] parameterised by a
//! zero-sized [`Currency`] marker, so a [`ComponentAccount`] and an
//! [`OrganicAccount`] can never be added, spent, or compared against each
//! other by accident — the mismatch is a compile error, not a runtime bug.
//!
//! - [`ComponentAccount`] — earned from mechanical enemy kills, spent in-run,
//!   lost on death.
//! - [`OrganicAccount`] — collected from barrels, permanent across runs.

use std::fmt;
use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

/// A kind of currency. Implemented by zero-sized marker types.
pub trait Currency {
    /// Human-readable name, used in error messages.
    const NAME: &'static str;
}

/// Marker for the in-run, lost-on-death currency from mechanical kills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Component;
impl Currency for Component {
    const NAME: &'static str = "components";
}

/// Marker for the permanent, persists-across-runs currency from barrels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Organic;
impl Currency for Organic {
    const NAME: &'static str = "organics";
}

/// Returned when a spend exceeds the available balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEnough {
    pub balance: u32,
    pub cost: u32,
    pub currency: &'static str,
}

impl fmt::Display for NotEnough {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "need {} {} but only have {}", self.cost, self.currency, self.balance)
    }
}

/// A balance of currency `K`. Two accounts of different `K` are distinct types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub struct Account<K: Currency> {
    pub balance: u32,
    #[serde(skip)]
    _marker: PhantomData<K>,
}

impl<K: Currency> Account<K> {
    pub fn new() -> Self {
        Self { balance: 0, _marker: PhantomData }
    }

    pub fn earn(&mut self, amount: u32) {
        self.balance += amount;
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.balance >= cost
    }

    pub fn spend(&mut self, cost: u32) -> Result<(), NotEnough> {
        if self.can_afford(cost) {
            self.balance -= cost;
            Ok(())
        } else {
            Err(NotEnough { balance: self.balance, cost, currency: K::NAME })
        }
    }
}

impl<K: Currency> Default for Account<K> {
    fn default() -> Self {
        Self::new()
    }
}

/// In-run currency from mechanical kills; lost on death.
pub type ComponentAccount = Account<Component>;
/// Permanent currency from barrels; kept across runs.
pub type OrganicAccount = Account<Organic>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_start_at_zero() {
        assert_eq!(ComponentAccount::new().balance, 0);
    }

    #[test]
    fn organics_start_at_zero() {
        assert_eq!(OrganicAccount::new().balance, 0);
    }

    #[test]
    fn earn_increases_balance() {
        let mut acc = ComponentAccount::new();
        acc.earn(1000);
        acc.earn(500);
        assert_eq!(acc.balance, 1500);
    }

    #[test]
    fn can_afford_check() {
        let mut acc = OrganicAccount::new();
        acc.earn(100);
        assert!(acc.can_afford(100));
        assert!(acc.can_afford(50));
        assert!(!acc.can_afford(101));
    }

    #[test]
    fn spend_deducts_balance() {
        let mut acc = ComponentAccount::new();
        acc.earn(2000);
        assert!(acc.spend(1200).is_ok());
        assert_eq!(acc.balance, 800);
    }

    #[test]
    fn spend_insufficient_fails_unchanged() {
        let mut acc = ComponentAccount::new();
        acc.earn(500);
        let err = acc.spend(1000).unwrap_err();
        assert_eq!(err.balance, 500);
        assert_eq!(err.cost, 1000);
        assert_eq!(err.currency, "components");
        assert_eq!(acc.balance, 500);
    }

    #[test]
    fn spend_exact_balance() {
        let mut acc = OrganicAccount::new();
        acc.earn(300);
        assert!(acc.spend(300).is_ok());
        assert_eq!(acc.balance, 0);
    }

    #[test]
    fn error_names_the_currency() {
        let err = OrganicAccount::new().spend(5).unwrap_err();
        assert_eq!(err.currency, "organics");
    }

    #[test]
    fn serde_roundtrip_preserves_balance() {
        let mut acc = OrganicAccount::new();
        acc.earn(4242);
        let json = serde_json::to_string(&acc).unwrap();
        let back: OrganicAccount = serde_json::from_str(&json).unwrap();
        assert_eq!(back.balance, 4242);
    }
}
