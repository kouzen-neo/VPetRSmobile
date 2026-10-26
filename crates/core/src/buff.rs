//! Timed stat buffs and multipliers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuffState {
    pub id: String,
    pub source_item: String,
    pub ends_at_unix: i64,
    pub exp_multiplier: f64,
    pub decay_multiplier: f64,
}

impl BuffState {
    #[must_use]
    pub fn from_food_tag(tag: &str, source_item: &str, now_unix: i64, duration_mins: u32) -> Self {
        let lower = tag.to_ascii_lowercase();
        let exp_multiplier = if lower.contains("delicious") || lower.contains("absorption") {
            1.15
        } else {
            1.0
        };
        let decay_multiplier = if lower.contains("satiety")
            || lower.contains("thirstrelief")
            || lower.contains("craving")
        {
            0.75
        } else {
            1.0
        };
        Self {
            id: tag.to_owned(),
            source_item: source_item.to_owned(),
            ends_at_unix: now_unix.saturating_add(i64::from(duration_mins) * 60),
            exp_multiplier,
            decay_multiplier,
        }
    }

    #[must_use]
    pub const fn is_expired(&self, now_unix: i64) -> bool {
        now_unix >= self.ends_at_unix
    }
}
