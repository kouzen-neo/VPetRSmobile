//! Pure in-memory MessagePack (`rmp-serde`) save schema and explicit version migration.
//!
//! Filesystem I/O, atomic rename, `fsync`, and `.bak.1`..`.bak.5` rotation live in `vpet-app`
//! so `vpet-core` stays 100% free of filesystem and wall-clock calls.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    buff::BuffState,
    item::{Inventory, InventoryItem},
    rng::DeterministicRng,
    schedule::ScheduledTask,
    state::World,
    stats::{PetState, Statistics},
};

pub const CURRENT_SAVE_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("failed to serialize save payload: {0}")]
    Encode(#[from] rmp_serde::encode::Error),
    #[error("failed to deserialize save payload: {0}")]
    Decode(#[from] rmp_serde::decode::Error),
    #[error("unsupported future save version {found} (max supported: {CURRENT_SAVE_VERSION})")]
    UnsupportedVersion { found: u16 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaveFile {
    pub version: u16,
    pub saved_at_unix: i64,
    pub profile: String,
    pub pet: PetState,
    pub inventory: Vec<InventoryItem>,
    pub buffs: Vec<BuffState>,
    pub schedule: Vec<ScheduledTask>,
    pub statistics: Statistics,
    pub rng_state: u64,
    pub output_positions: BTreeMap<String, (i32, i32)>,
}

impl SaveFile {
    #[must_use]
    pub fn from_world(world: &World, saved_at_unix: i64) -> Self {
        Self {
            version: CURRENT_SAVE_VERSION,
            saved_at_unix,
            profile: world.profile.clone(),
            pet: world.pet.clone(),
            inventory: world.inventory.to_items(),
            buffs: world.buffs.clone(),
            schedule: world.schedule.clone(),
            statistics: world.statistics.clone(),
            rng_state: world.rng.state(),
            output_positions: world.output_positions.clone(),
        }
    }

    #[must_use]
    pub fn into_world(self) -> World {
        let mut w = World::new(self.profile, self.pet.name.clone(), self.rng_state);
        w.pet = self.pet;
        w.pet.mode = w.pet.cal_mode();
        w.inventory = Inventory::from_items(&self.inventory);
        w.buffs = self.buffs;
        w.schedule = self.schedule;
        w.statistics = self.statistics;
        w.rng = DeterministicRng::from_seed(self.rng_state);
        w.output_positions = self.output_positions;
        w
    }
}

pub fn encode_save(save: &SaveFile) -> Result<Vec<u8>, SaveError> {
    Ok(rmp_serde::to_vec_named(save)?)
}

pub fn decode_save(bytes: &[u8]) -> Result<SaveFile, SaveError> {
    let raw: SaveFile = rmp_serde::from_slice(bytes)?;
    migrate_save(raw)
}

pub fn migrate_save(mut save: SaveFile) -> Result<SaveFile, SaveError> {
    if save.version == 0 {
        save = migrate_v0_to_v1(save);
    }
    if save.version > CURRENT_SAVE_VERSION {
        return Err(SaveError::UnsupportedVersion {
            found: save.version,
        });
    }
    Ok(save)
}

fn migrate_v0_to_v1(mut save: SaveFile) -> SaveFile {
    save.version = 1;
    if save.pet.level == 0 {
        save.pet.level = 1;
    }
    save.pet.mode = save.pet.cal_mode();
    save
}
