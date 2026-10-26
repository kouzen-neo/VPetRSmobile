//! Byte-bounded LRU animation clip cache (default budget: 256 MiB).

use std::collections::{HashMap, VecDeque};

use crate::decode::DecodedClip;

pub const DEFAULT_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug)]
pub struct FrameCache {
    max_bytes: usize,
    used_bytes: usize,
    entries: HashMap<String, DecodedClip>,
    lru_order: VecDeque<String>,
    evictions: u64,
}

impl Default for FrameCache {
    fn default() -> Self {
        Self::new(DEFAULT_CACHE_MAX_BYTES)
    }
}

impl FrameCache {
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes: max_bytes.max(64 * 1024),
            used_bytes: 0,
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            evictions: 0,
        }
    }

    #[must_use]
    pub const fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    #[must_use]
    pub const fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub const fn evictions(&self) -> u64 {
        self.evictions
    }

    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes.max(64 * 1024);
        self.evict_to_fit(0);
    }

    pub fn get(&mut self, key: &str) -> Option<&DecodedClip> {
        if self.entries.contains_key(key) {
            self.promote(key);
        }
        self.entries.get(key)
    }

    pub fn insert(&mut self, clip: DecodedClip) {
        if clip.byte_size > self.max_bytes {
            return;
        }
        if let Some(old) = self.entries.remove(&clip.key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.byte_size);
            self.lru_order.retain(|k| k != &clip.key);
        }
        self.evict_to_fit(clip.byte_size);
        let key = clip.key.clone();
        self.used_bytes += clip.byte_size;
        self.lru_order.push_back(key.clone());
        self.entries.insert(key, clip);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru_order.clear();
        self.used_bytes = 0;
    }

    fn promote(&mut self, key: &str) {
        if self.lru_order.back().is_some_and(|b| b == key) {
            return;
        }
        self.lru_order.retain(|k| k != key);
        self.lru_order.push_back(key.to_owned());
    }

    fn evict_to_fit(&mut self, incoming: usize) {
        while self.used_bytes.saturating_add(incoming) > self.max_bytes {
            let Some(oldest) = self.lru_order.pop_front() else {
                break;
            };
            if let Some(removed) = self.entries.remove(&oldest) {
                self.used_bytes = self.used_bytes.saturating_sub(removed.byte_size);
                self.evictions += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{canvas::procedural_mascot_pixmap, decode::RenderedFrame};

    fn dummy_clip(key: &str, px: u32) -> DecodedClip {
        let pm = Arc::new(procedural_mascot_pixmap(px).unwrap());
        DecodedClip::from_frames(
            key,
            vec![RenderedFrame {
                pixmap: pm,
                duration_ms: 100,
            }],
        )
    }

    #[test]
    fn evicts_least_recently_used_within_byte_budget() {
        // Each 64x64 clip is 16,384 bytes; budget of 70 KiB fits at most 4 clips.
        let mut cache = FrameCache::new(70 * 1024);
        cache.insert(dummy_clip("a", 64));
        cache.insert(dummy_clip("b", 64));
        cache.insert(dummy_clip("c", 64));
        cache.insert(dummy_clip("d", 64));
        assert_eq!(cache.len(), 4);

        // Touch "a" so "b" becomes LRU.
        assert!(cache.get("a").is_some());
        cache.insert(dummy_clip("e", 64));
        assert_eq!(cache.len(), 4);
        assert!(cache.contains("a"));
        assert!(!cache.contains("b"));
        assert!(cache.used_bytes() <= cache.max_bytes());
        assert_eq!(cache.evictions(), 1);
    }
}
