//! Work / study / play session state machine (`Idle -> Working -> Finish/Cancel`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkSession {
    pub name: String,
    pub graph: String,
    pub remaining_secs: f64,
    pub total_secs: f64,
    pub money_per_min: f64,
    pub exp_per_min: f64,
    pub food_cost_per_min: f64,
    pub drink_cost_per_min: f64,
    pub feeling_cost_per_min: f64,
    pub finish_bonus: f64,
}

impl WorkSession {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        graph: impl Into<String>,
        time_mins: u32,
        money_per_min: f64,
        exp_per_min: f64,
        food_cost_per_min: f64,
        drink_cost_per_min: f64,
        feeling_cost_per_min: f64,
        finish_bonus: f64,
    ) -> Self {
        let total_secs = f64::from(time_mins.max(1)) * 60.0;
        Self {
            name: name.into(),
            graph: graph.into(),
            remaining_secs: total_secs,
            total_secs,
            money_per_min,
            exp_per_min,
            food_cost_per_min,
            drink_cost_per_min,
            feeling_cost_per_min,
            finish_bonus,
        }
    }

    #[must_use]
    pub const fn is_finished(&self) -> bool {
        self.remaining_secs <= 0.0
    }

    #[must_use]
    pub fn progress_ratio(&self) -> f64 {
        if self.total_secs <= 0.0 {
            1.0
        } else {
            ((self.total_secs - self.remaining_secs) / self.total_secs).clamp(0.0, 1.0)
        }
    }
}
