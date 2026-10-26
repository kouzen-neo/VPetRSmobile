#![allow(dead_code)]
//! Atomic MessagePack save writer (`temp -> fsync -> rotate .bak.1..5 -> rename -> dir fsync`)
//! and automatic corrupt-save backup recovery.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use tracing::{info, warn};
use vpet_core::{SaveFile, World, decode_save, encode_save};

pub const MAX_BACKUPS: usize = 5;

#[derive(Debug, Clone)]
pub struct SaveStore {
    saves_dir: PathBuf,
}

impl SaveStore {
    #[must_use]
    pub fn new(saves_dir: impl Into<PathBuf>) -> Self {
        Self {
            saves_dir: saves_dir.into(),
        }
    }

    #[must_use]
    pub fn primary_path(&self, profile: &str) -> PathBuf {
        self.saves_dir.join(format!("{profile}.mpak"))
    }

    #[must_use]
    pub fn backup_path(&self, profile: &str, slot: usize) -> PathBuf {
        self.saves_dir.join(format!("{profile}.bak.{slot}"))
    }

    pub fn save_world(&self, world: &World) -> Result<PathBuf> {
        let now_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);
        self.save_world_at(world, now_unix)
    }

    pub fn save_world_at(&self, world: &World, now_unix: i64) -> Result<PathBuf> {
        fs::create_dir_all(&self.saves_dir)
            .with_context(|| format!("failed to create save dir `{}`", self.saves_dir.display()))?;

        let save = SaveFile::from_world(world, now_unix);
        let payload = encode_save(&save).context("failed to encode SaveFile")?;

        let primary = self.primary_path(&world.profile);
        let tmp = self.saves_dir.join(format!(
            ".{}.mpak.tmp.{}",
            world.profile,
            std::process::id()
        ));

        {
            let mut file = OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&tmp)
                .with_context(|| format!("failed to create temp save `{}`", tmp.display()))?;
            file.write_all(&payload)?;
            file.sync_all()?;
        }

        // Rotate existing primary -> .bak.1 .. .bak.5 before atomic replace.
        if primary.is_file() {
            for idx in (1..MAX_BACKUPS).rev() {
                let src = self.backup_path(&world.profile, idx);
                if src.is_file() {
                    let dst = self.backup_path(&world.profile, idx + 1);
                    let _ = fs::rename(&src, &dst);
                }
            }
            let _ = fs::copy(&primary, self.backup_path(&world.profile, 1));
        }

        fs::rename(&tmp, &primary).with_context(|| {
            format!(
                "failed atomic rename `{}` -> `{}`",
                tmp.display(),
                primary.display()
            )
        })?;

        if let Ok(dir_handle) = File::open(&self.saves_dir) {
            let _ = dir_handle.sync_all();
        }

        Ok(primary)
    }

    /// Load `profile` from disk, falling back through `.bak.1`..`.bak.5` if primary is corrupted.
    pub fn load_or_recover(&self, profile: &str) -> Option<World> {
        let mut candidates = Vec::with_capacity(1 + MAX_BACKUPS);
        candidates.push(self.primary_path(profile));
        for slot in 1..=MAX_BACKUPS {
            candidates.push(self.backup_path(profile, slot));
        }

        for (idx, path) in candidates.iter().enumerate() {
            if !path.is_file() {
                continue;
            }
            match fs::read(path) {
                Ok(bytes) => match decode_save(&bytes) {
                    Ok(save) => {
                        if idx > 0 {
                            warn!(
                                backup = %path.display(),
                                "recovered world state from backup after primary save corruption"
                            );
                        } else {
                            info!(save = %path.display(), "loaded save file");
                        }
                        return Some(save.into_world());
                    }
                    Err(err) => {
                        warn!(path = %path.display(), %err, "corrupt save candidate; trying next backup");
                    }
                },
                Err(err) => {
                    warn!(path = %path.display(), %err, "unreadable save candidate; trying next backup");
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotates_five_backups_and_recovers_from_corrupt_primary() {
        let dir = std::env::temp_dir().join(format!("vpet_rs_save_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let store = SaveStore::new(&dir);

        let mut world = World::new("main", "Loris", 77);
        for round in 1..=7 {
            world.pet.money = f64::from(round * 10);
            store
                .save_world_at(&world, 1_700_000_000 + i64::from(round))
                .unwrap();
        }

        for slot in 1..=MAX_BACKUPS {
            assert!(store.backup_path("main", slot).is_file());
        }
        assert!(!store.backup_path("main", 6).exists());

        // Corrupt primary save (`70.0` money) -> `load_or_recover` recovers `.bak.1` (`60.0` money)!
        fs::write(store.primary_path("main"), b"corrupt-truncated-bytes").unwrap();
        let recovered = store
            .load_or_recover("main")
            .expect("recovered from .bak.1");
        assert!((recovered.pet.money - 60.0).abs() < 1e-6);

        let _ = fs::remove_dir_all(&dir);
    }
}
