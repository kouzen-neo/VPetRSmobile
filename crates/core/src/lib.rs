//! Pure deterministic pet simulation core.
//!
//! Zero workspace dependencies, zero filesystem I/O, zero Wayland/wall-clock calls.

pub mod buff;
pub mod item;
pub mod rng;
pub mod save;
pub mod schedule;
pub mod state;
pub mod stats;
pub mod work;

use serde::{Deserialize, Serialize};

pub use buff::BuffState;
pub use item::{FoodEffect, Inventory, InventoryItem, MAX_ITEM_STACK};
pub use rng::DeterministicRng;
pub use save::{CURRENT_SAVE_VERSION, SaveError, SaveFile, decode_save, encode_save, migrate_save};
pub use schedule::ScheduledTask;
pub use state::{ActionError, World};
pub use stats::{PetState, Statistics};
pub use work::WorkSession;

/// Pet condition / mood tier matching upstream VPet `IGameSave.ModeType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ModeType {
    Happy,
    #[default]
    Nomal,
    PoorCondition,
    Ill,
}

impl ModeType {
    pub const ALL: [Self; 4] = [Self::Happy, Self::Nomal, Self::PoorCondition, Self::Ill];

    /// Upstream VPet `[Flags]` bit value (`Happy=1, Nomal=2, PoorCondition=4, Ill=8`).
    #[must_use]
    pub const fn bit(self) -> u8 {
        match self {
            Self::Happy => 1,
            Self::Nomal => 2,
            Self::PoorCondition => 4,
            Self::Ill => 8,
        }
    }

    #[must_use]
    pub const fn matches_mask(self, mask: u8) -> bool {
        mask == 0 || (mask & self.bit()) != 0
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Happy => "happy",
            Self::Nomal => "nomal",
            Self::PoorCondition => "poorcondition",
            Self::Ill => "ill",
        }
    }

    #[must_use]
    pub fn parse_loose(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "happy" => Some(Self::Happy),
            "nomal" | "normal" => Some(Self::Nomal),
            "poorcondition" | "poor_condition" | "poor" => Some(Self::PoorCondition),
            "ill" => Some(Self::Ill),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leveling_and_mode_derivation_follow_vpet_formulas() {
        let mut pet = PetState::new("Loris");
        assert_eq!(pet.level, 1);
        assert_eq!(pet.level_up_need(), 100.0);
        assert_eq!(pet.mode, ModeType::Nomal);

        // 100 exp -> Level 2 (`need = 300`), +300 exp -> Level 3 (`need = 500`).
        assert_eq!(pet.add_exp(420.0), 2);
        assert_eq!(pet.level, 3);
        assert!((pet.exp - 20.0).abs() < 1e-6);
        assert!((pet.likability_max - 120.0).abs() < 1e-6);

        // High feeling + high likability transitions to Happy.
        pet.likability = 85.0;
        pet.feeling = pet.feeling_max() * 0.80;
        assert_eq!(pet.cal_mode(), ModeType::Happy);

        // Severe health loss transitions to Ill.
        pet.set_health(10.0);
        assert_eq!(pet.cal_mode(), ModeType::Ill);
    }

    #[test]
    fn deterministic_tick_buff_expiry_and_feeding() {
        let mut w1 = World::new("p", "Loris", 42);
        let mut w2 = World::new("p", "Loris", 42);

        let apple = FoodEffect {
            name: "Apple".to_owned(),
            price: 15.0,
            exp: 50.0,
            strength: 20.0,
            strength_food: 40.0,
            strength_drink: 10.0,
            feeling: 15.0,
            health: 2.0,
            likability: 1.0,
            buff_id: Some("Delicious1".to_owned()),
            buff_time_mins: 1,
        };

        w1.feed(&apple, 1_000).unwrap();
        w2.feed(&apple, 1_000).unwrap();
        assert_eq!(w1.buffs.len(), 1);
        assert!((w1.pet.money - 85.0).abs() < 1e-6);
        assert!((w1.pet.store_strength_food - 20.0).abs() < 1e-6);

        for i in 0..60 {
            w1.tick(0.1, 1_000 + i);
            w2.tick(0.1, 1_000 + i);
        }
        // Identical inputs + seed produce bit-identical state & RNG progression.
        assert_eq!(w1.rng.next_u64(), w2.rng.next_u64());
        assert_eq!(w1.pet, w2.pet);
        // Stored food partially digested after 50 ticks (`store_take`).
        assert!(w1.pet.store_strength_food < 20.0);

        // Advance past `ends_at_unix = 1060` -> buff expires cleanly.
        w1.tick(0.1, 1_061);
        assert!(w1.buffs.is_empty());
    }

    #[test]
    fn save_roundtrip_and_version_migration() {
        let mut world = World::new("slot1", "Loris", 999);
        world.inventory.add("Fruit Juice", 3);
        world
            .output_positions
            .insert("eDP-1".to_owned(), (240, 760));
        let _ = world.pet_head();

        let save = SaveFile::from_world(&world, 1_793_000_000);
        let encoded = encode_save(&save).unwrap();
        let decoded = decode_save(&encoded).unwrap();
        assert_eq!(save, decoded);

        let restored = decoded.into_world();
        assert_eq!(restored.inventory.count("Fruit Juice"), 3);
        assert_eq!(restored.output_positions.get("eDP-1"), Some(&(240, 760)));

        // Legacy v0 migrates to v1; future v99 is rejected safely.
        let mut v0 = save.clone();
        v0.version = 0;
        v0.pet.level = 0;
        let migrated = migrate_save(v0).unwrap();
        assert_eq!(migrated.version, CURRENT_SAVE_VERSION);
        assert_eq!(migrated.pet.level, 1);

        let mut v99 = save;
        v99.version = 99;
        assert!(matches!(
            migrate_save(v99),
            Err(SaveError::UnsupportedVersion { found: 99 })
        ));
    }

    #[test]
    fn soak_24h_simulation_stays_bounded_and_finite() {
        let mut world = World::new("soak", "Loris", 2026);
        let meal = FoodEffect {
            name: "Meal".to_owned(),
            price: 0.0,
            exp: 25.0,
            strength: 50.0,
            strength_food: 80.0,
            strength_drink: 80.0,
            feeling: 30.0,
            health: 5.0,
            likability: 1.0,
            buff_id: Some("Satiety1".to_owned()),
            buff_time_mins: 5,
        };

        // 864,000 ticks at 10 Hz = 24 full hours of continuous desktop runtime.
        for tick in 0..864_000_u64 {
            let now = 1_800_000_000 + (tick / 10) as i64;
            if tick.is_multiple_of(36_000) {
                world.feed(&meal, now).unwrap();
            }
            world.tick(0.1, now);
        }

        assert_eq!(world.statistics.ticks_simulated, 864_000);
        assert!(world.pet.health.is_finite() && (0.0..=100.0).contains(&world.pet.health));
        assert!(world.pet.strength.is_finite() && world.pet.strength >= 0.0);
        assert!(world.pet.strength_food.is_finite() && world.pet.strength_food >= 0.0);
        assert!(world.pet.strength_drink.is_finite() && world.pet.strength_drink >= 0.0);
        assert!(world.buffs.len() <= 1);
    }
}
