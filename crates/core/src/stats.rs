//! Upstream VPet (`GameSave_VPet.cs`) stat caps, leveling, digestion, and `ModeType` derivation.

use serde::{Deserialize, Serialize};

use crate::ModeType;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Statistics {
    pub ticks_simulated: u64,
    pub times_fed: u64,
    pub times_petted: u64,
    pub works_completed: u64,
    pub pixels_walked: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PetState {
    pub name: String,
    pub level: u32,
    pub level_max: u32,
    pub exp: f64,
    pub money: f64,
    pub strength: f64,
    pub store_strength: f64,
    pub strength_food: f64,
    pub store_strength_food: f64,
    pub strength_drink: f64,
    pub store_strength_drink: f64,
    pub feeling: f64,
    pub health: f64,
    pub likability: f64,
    pub likability_max: f64,
    pub attr_limit_strength: f64,
    pub attr_limit_feeling: f64,
    pub mode: ModeType,
    pub sleeping: bool,
}

impl Default for PetState {
    fn default() -> Self {
        Self::new("Loris")
    }
}

impl PetState {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        let mut s = Self {
            name: name.into(),
            level: 1,
            level_max: 0,
            exp: 0.0,
            money: 100.0,
            strength: 100.0,
            store_strength: 0.0,
            strength_food: 100.0,
            store_strength_food: 0.0,
            strength_drink: 100.0,
            store_strength_drink: 0.0,
            feeling: 60.0,
            health: 100.0,
            likability: 0.0,
            likability_max: 100.0,
            attr_limit_strength: 0.0,
            attr_limit_feeling: 0.0,
            mode: ModeType::Nomal,
            sleeping: false,
        };
        s.mode = s.cal_mode();
        s
    }

    /// Experience required for the next level (`200 * Level - 100`).
    #[must_use]
    pub fn level_up_need(&self) -> f64 {
        let lv = self.level.max(1);
        f64::from(lv.saturating_mul(200).saturating_sub(100).max(100))
    }

    #[must_use]
    pub fn strength_max(&self) -> f64 {
        let scale = f64::from(self.level.max(1) * (1 + self.level_max))
            .powf(0.75)
            .floor()
            * 4.0;
        ((100.0 + scale) * (1.0 + self.attr_limit_strength)).max(1.0)
    }

    #[must_use]
    pub fn feeling_max(&self) -> f64 {
        let scale = f64::from(self.level.max(1) * (1 + self.level_max))
            .powf(0.75)
            .floor()
            * 2.0;
        ((100.0 + scale) * (1.0 + self.attr_limit_feeling)).max(1.0)
    }

    /// Apply experience delta and return how many level-ups occurred.
    pub fn add_exp(&mut self, delta: f64) -> u32 {
        let mut val = (self.exp + delta).max(0.0);
        let mut gained = 0_u32;
        loop {
            let need = self.level_up_need();
            if val < need {
                break;
            }
            val -= need;
            gained += 1;
            self.likability_max += 10.0;
            self.level += 1;
            if self.level > 1000 + self.level_max * 100 {
                self.level_max += 1;
                self.level = 100 * self.level_max;
            }
        }
        self.exp = val;
        gained
    }

    pub fn set_health(&mut self, value: f64) {
        self.health = value.clamp(0.0, 100.0);
    }

    pub fn add_health(&mut self, delta: f64) {
        self.set_health(self.health + delta);
    }

    pub fn add_likability(&mut self, delta: f64) {
        let next = (self.likability + delta).max(0.0);
        if next > self.likability_max {
            let overflow = next - self.likability_max;
            self.likability = self.likability_max;
            self.add_health(overflow);
        } else {
            self.likability = next;
        }
    }

    pub fn add_strength(&mut self, delta: f64) {
        let cap = self.strength_max().max(self.strength);
        self.strength = (self.strength + delta).clamp(0.0, cap);
    }

    pub fn add_food(&mut self, delta: f64) {
        let cap = self.strength_max().max(self.strength_food);
        let next = (self.strength_food + delta).min(cap);
        if next <= 0.0 {
            if self.strength_food > 0.0 {
                self.add_health(next);
            }
            self.strength_food = 0.0;
        } else {
            self.strength_food = next;
        }
    }

    pub fn add_drink(&mut self, delta: f64) {
        let cap = self.strength_max().max(self.strength_drink);
        let next = (self.strength_drink + delta).min(cap);
        if next <= 0.0 {
            if self.strength_drink > 0.0 {
                self.add_health(next);
            }
            self.strength_drink = 0.0;
        } else {
            self.strength_drink = next;
        }
    }

    pub fn add_feeling(&mut self, delta: f64) {
        let cap = self.feeling_max().max(self.feeling);
        let next = (self.feeling + delta).min(cap);
        if next <= 0.0 {
            if self.feeling > 0.0 {
                self.add_health(next * 0.5);
                self.add_likability(next * 0.5);
            }
            self.feeling = 0.0;
        } else {
            self.feeling = next;
        }
    }

    /// Release 1/10th of buffered nutrients (`GameSave_VPet.StoreTake`).
    pub fn store_take(&mut self) {
        const DIV: f64 = 10.0;

        let s = self.store_strength / DIV;
        self.store_strength -= s;
        if self.store_strength.abs() < 1.0 {
            self.store_strength = 0.0;
        } else {
            self.add_strength(s);
        }

        let d = self.store_strength_drink / DIV;
        self.store_strength_drink -= d;
        if self.store_strength_drink.abs() < 1.0 {
            self.store_strength_drink = 0.0;
        } else {
            self.add_drink(d);
        }

        let f = self.store_strength_food / DIV;
        self.store_strength_food -= f;
        if self.store_strength_food.abs() < 1.0 {
            self.store_strength_food = 0.0;
        } else {
            self.add_food(f);
        }
    }

    /// Derive current `ModeType` (`GameSave_VPet.CalMode`).
    #[must_use]
    pub fn cal_mode(&self) -> ModeType {
        let fmax = self.feeling_max();
        let felps = (self.feeling / fmax).clamp(0.0, 2.0);
        let like_bonus = if self.likability >= 80.0 {
            12
        } else if self.likability >= 40.0 {
            6
        } else {
            0
        };
        let feel_bonus = if felps >= 0.80 { 12 } else { 0 };
        let realhel = f64::from(60 - feel_bonus - like_bonus);

        if self.health <= realhel {
            if self.health <= realhel * 0.5 {
                return ModeType::Ill;
            }
            return ModeType::PoorCondition;
        }

        let realfel = 0.90
            - if self.likability >= 80.0 {
                0.20
            } else if self.likability >= 40.0 {
                0.10
            } else {
                0.0
            };

        if felps >= realfel {
            ModeType::Happy
        } else if felps <= realfel * 0.5 {
            ModeType::PoorCondition
        } else {
            ModeType::Nomal
        }
    }
}
