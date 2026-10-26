//! Deterministic 10 Hz `World` simulation loop with injected timestamps.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    ModeType,
    buff::BuffState,
    item::{FoodEffect, Inventory},
    rng::DeterministicRng,
    schedule::ScheduledTask,
    stats::{PetState, Statistics},
    work::WorkSession,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ActionError {
    #[error("insufficient money and item not in inventory")]
    InsufficientFunds,
    #[error("pet level {current} is below required level {required}")]
    LevelTooLow { current: u32, required: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub profile: String,
    pub pet: PetState,
    pub inventory: Inventory,
    pub buffs: Vec<BuffState>,
    pub active_work: Option<WorkSession>,
    pub schedule: Vec<ScheduledTask>,
    pub statistics: Statistics,
    pub rng: DeterministicRng,
    pub output_positions: BTreeMap<String, (i32, i32)>,
    digest_tick_acc: u32,
}

impl Default for World {
    fn default() -> Self {
        Self::new("default", "Loris", 0x5650_4554_5255_5354)
    }
}

impl World {
    #[must_use]
    pub fn new(profile: impl Into<String>, pet_name: impl Into<String>, seed: u64) -> Self {
        Self {
            profile: profile.into(),
            pet: PetState::new(pet_name),
            inventory: Inventory::default(),
            buffs: Vec::new(),
            active_work: None,
            schedule: Vec::new(),
            statistics: Statistics::default(),
            rng: DeterministicRng::from_seed(seed),
            output_positions: BTreeMap::new(),
            digest_tick_acc: 0,
        }
    }

    #[must_use]
    pub fn exp_multiplier(&self) -> f64 {
        self.buffs
            .iter()
            .fold(1.0, |acc, b| acc * b.exp_multiplier)
            .clamp(0.1, 10.0)
    }

    #[must_use]
    pub fn decay_multiplier(&self) -> f64 {
        self.buffs
            .iter()
            .fold(1.0, |acc, b| acc * b.decay_multiplier)
            .clamp(0.1, 5.0)
    }

    /// Feed `food` from inventory if owned, otherwise purchase with `pet.money`.
    pub fn feed(&mut self, food: &FoodEffect, now_unix: i64) -> Result<u32, ActionError> {
        if !self.inventory.consume(&food.name, 1) {
            if food.price > 0.0 && self.pet.money < food.price {
                return Err(ActionError::InsufficientFunds);
            }
            self.pet.money = (self.pet.money - food.price.max(0.0)).max(0.0);
        }

        let gained_levels = self.pet.add_exp(food.exp * self.exp_multiplier());
        let half_s = food.strength * 0.5;
        self.pet.add_strength(half_s);
        self.pet.store_strength += half_s;

        let half_f = food.strength_food * 0.5;
        self.pet.add_food(half_f);
        self.pet.store_strength_food += half_f;

        let half_d = food.strength_drink * 0.5;
        self.pet.add_drink(half_d);
        self.pet.store_strength_drink += half_d;

        self.pet.add_feeling(food.feeling);
        self.pet.add_health(food.health);
        self.pet.add_likability(food.likability);

        if let Some(buff_tag) = &food.buff_id
            && food.buff_time_mins > 0
        {
            self.buffs.retain(|b| &b.id != buff_tag);
            self.buffs.push(BuffState::from_food_tag(
                buff_tag,
                &food.name,
                now_unix,
                food.buff_time_mins,
            ));
        }

        self.pet.mode = self.pet.cal_mode();
        self.statistics.times_fed += 1;
        Ok(gained_levels)
    }

    pub fn pet_head(&mut self) -> ModeType {
        self.pet.add_feeling(1.5);
        self.pet.add_likability(0.4);
        let _ = self.pet.add_exp(1.0 * self.exp_multiplier());
        self.statistics.times_petted += 1;
        self.pet.mode = self.pet.cal_mode();
        self.pet.mode
    }

    pub fn pet_body(&mut self) -> ModeType {
        self.pet.add_feeling(1.0);
        self.pet.add_likability(0.25);
        let _ = self.pet.add_exp(1.0 * self.exp_multiplier());
        self.statistics.times_petted += 1;
        self.pet.mode = self.pet.cal_mode();
        self.pet.mode
    }

    pub fn set_sleeping(&mut self, sleeping: bool) {
        self.pet.sleeping = sleeping;
        if sleeping {
            self.active_work = None;
        }
    }

    pub fn start_work(
        &mut self,
        session: WorkSession,
        required_level: u32,
    ) -> Result<(), ActionError> {
        if self.pet.level < required_level {
            return Err(ActionError::LevelTooLow {
                current: self.pet.level,
                required: required_level,
            });
        }
        self.pet.sleeping = false;
        self.active_work = Some(session);
        Ok(())
    }

    pub fn cancel_work(&mut self) -> Option<WorkSession> {
        self.active_work.take()
    }

    /// Advance the world deterministically by `dt_secs` (standard: `0.1` s at 10 Hz).
    pub fn tick(&mut self, dt_secs: f64, now_unix: i64) {
        let dt = dt_secs.clamp(0.0, 60.0);
        self.statistics.ticks_simulated += 1;

        // 1. Prune expired buffs immediately and recompute multipliers.
        self.buffs.retain(|b| !b.is_expired(now_unix));
        let decay_mult = self.decay_multiplier();
        let exp_mult = self.exp_multiplier();

        // 2. Digest buffered nutrients every ~5 seconds (50 ticks at 10 Hz).
        self.digest_tick_acc += 1;
        if self.digest_tick_acc >= 50 {
            self.digest_tick_acc = 0;
            self.pet.store_take();
        }

        // 3. Activity metabolism & rewards.
        if let Some(mut work) = self.active_work.take() {
            let step = dt.min(work.remaining_secs.max(0.0));
            let mins = step / 60.0;
            self.pet.money += work.money_per_min * mins;
            let _ = self.pet.add_exp(work.exp_per_min * mins * exp_mult);
            self.pet
                .add_food(-work.food_cost_per_min * mins * decay_mult);
            self.pet
                .add_drink(-work.drink_cost_per_min * mins * decay_mult);
            self.pet.add_feeling(-work.feeling_cost_per_min * mins);
            work.remaining_secs -= step;

            if work.is_finished() {
                let bonus_mins = work.total_secs / 60.0;
                self.pet.money += work.money_per_min * bonus_mins * work.finish_bonus;
                self.statistics.works_completed += 1;
            } else {
                self.active_work = Some(work);
            }
        } else if self.pet.sleeping {
            // Sleeping restores stamina and gently recovers health while slowing hunger/thirst.
            self.pet.add_strength(0.35 * dt);
            self.pet.add_food(-0.012 * dt * decay_mult);
            self.pet.add_drink(-0.015 * dt * decay_mult);
            if self.pet.strength_food > 25.0 && self.pet.strength_drink > 25.0 {
                self.pet.add_health(0.04 * dt);
            }
        } else {
            // Passive desktop idle metabolism.
            self.pet.add_strength(-0.015 * dt * decay_mult);
            self.pet.add_food(-0.025 * dt * decay_mult);
            self.pet.add_drink(-0.030 * dt * decay_mult);
            self.pet.add_feeling(-0.012 * dt * decay_mult);
            if self.pet.strength_food > 50.0 && self.pet.strength_drink > 50.0 {
                self.pet.add_health(0.02 * dt);
            }
        }

        self.pet.mode = self.pet.cal_mode();
    }
}
