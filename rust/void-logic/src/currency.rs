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

/// Which currency a cache pickup carries. The only in-level reward is a
/// dropped cache of one of these; nothing is credited without a pickup.
/// Crosses to GDScript as an id (`id`/`from_id`), like `EnemyType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrencyKind {
    /// Blue: in-run components, lost on run-over.
    Components,
    /// Green: permanent organics, kept across runs.
    Organics,
    /// Red: a planet-final boss's hull container. Not a balance — collecting
    /// it grants a random unowned hull (`boss::roll_hull_reward`); the
    /// carried amount is the components fallback if no hull remains.
    HullReward,
}

impl CurrencyKind {
    pub const ALL: &[CurrencyKind] = &[
        CurrencyKind::Components,
        CurrencyKind::Organics,
        CurrencyKind::HullReward,
    ];

    pub fn id(&self) -> i32 {
        Self::ALL.iter().position(|k| k == self)
            .expect("CurrencyKind::ALL must contain every variant") as i32
    }

    pub fn from_id(id: i32) -> Option<CurrencyKind> {
        Self::ALL.get(id as usize).copied()
    }

    /// Glow tint for the in-level cache pickup: blue components, green
    /// organics, red hull container.
    pub fn glow_color(&self) -> [f32; 3] {
        match self {
            Self::Components => [0.2, 0.5, 1.0],
            Self::Organics => [0.2, 0.9, 0.2],
            Self::HullReward => [1.0, 0.15, 0.1],
        }
    }
}

/// Organics carried by each green cache placed at a level's loot spawns.
/// Green income is deliberately slow — it prices the permanent unlocks.
pub const ORGANIC_CACHE_AMOUNT: u32 = 50;

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
    fn currency_kind_id_round_trips() {
        for kind in CurrencyKind::ALL {
            assert_eq!(CurrencyKind::from_id(kind.id()), Some(*kind),
                "{kind:?} must round-trip through its id");
        }
    }

    #[test]
    fn currency_kind_from_id_invalid_is_none() {
        assert_eq!(CurrencyKind::from_id(-1), None);
        assert_eq!(CurrencyKind::from_id(99), None);
    }

    #[test]
    fn cache_glow_colors_match_their_currency() {
        let [r, g, b] = CurrencyKind::Components.glow_color();
        assert!(b > r && b > g, "the components cache glows blue, got [{r}, {g}, {b}]");
        let [r, g, b] = CurrencyKind::Organics.glow_color();
        assert!(g > r && g > b, "the organics cache glows green, got [{r}, {g}, {b}]");
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
