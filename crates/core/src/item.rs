//! Consumable food effects and stack-bounded player inventory.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MAX_ITEM_STACK: u32 = 9_999;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FoodEffect {
    pub name: String,
    pub price: f64,
    pub exp: f64,
    pub strength: f64,
    pub strength_food: f64,
    pub strength_drink: f64,
    pub feeling: f64,
    pub health: f64,
    pub likability: f64,
    pub buff_id: Option<String>,
    pub buff_time_mins: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InventoryItem {
    pub id: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inventory {
    stacks: BTreeMap<String, u32>,
}

impl Inventory {
    #[must_use]
    pub fn from_items(items: &[InventoryItem]) -> Self {
        let mut inv = Self::default();
        for item in items {
            if item.count > 0 {
                inv.add(&item.id, item.count);
            }
        }
        inv
    }

    #[must_use]
    pub fn to_items(&self) -> Vec<InventoryItem> {
        self.stacks
            .iter()
            .filter(|(_, c)| **c > 0)
            .map(|(id, &count)| InventoryItem {
                id: id.clone(),
                count,
            })
            .collect()
    }

    pub fn add(&mut self, id: &str, amount: u32) -> u32 {
        if amount == 0 {
            return self.count(id);
        }
        let entry = self.stacks.entry(id.to_owned()).or_insert(0);
        *entry = entry.saturating_add(amount).min(MAX_ITEM_STACK);
        *entry
    }

    pub fn consume(&mut self, id: &str, amount: u32) -> bool {
        if amount == 0 {
            return true;
        }
        let Some(cur) = self.stacks.get_mut(id) else {
            return false;
        };
        if *cur < amount {
            return false;
        }
        *cur -= amount;
        if *cur == 0 {
            self.stacks.remove(id);
        }
        true
    }

    #[must_use]
    pub fn count(&self, id: &str) -> u32 {
        self.stacks.get(id).copied().unwrap_or(0)
    }
}
