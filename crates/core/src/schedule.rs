//! Lightweight daily task schedule model (foundation for post-MVP auto-scheduling).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledTask {
    pub work_name: String,
    pub auto_renew: bool,
}
