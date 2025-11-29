//! Layout caching for memoization.

use presentar_core::Size;
use std::collections::HashMap;

/// Cache key combining constraints hash and widget identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    /// Widget identity hash
    pub widget_id: u64,
    /// Constraints hash
    pub constraints_hash: u64,
}

/// Cached layout result.
#[derive(Debug, Clone, Copy)]
pub struct CacheEntry {
    /// Computed size
    pub size: Size,
    /// Frame when this entry was last used
    pub last_used_frame: u64,
}

/// Layout cache for memoizing measure results.
#[derive(Debug, Default)]
pub struct LayoutCache {
    entries: HashMap<CacheKey, CacheEntry>,
    current_frame: u64,
}

impl LayoutCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Look up a cached size.
    #[must_use]
    pub fn get(&mut self, key: CacheKey) -> Option<Size> {
        self.entries.get_mut(&key).map(|entry| {
            entry.last_used_frame = self.current_frame;
            entry.size
        })
    }

    /// Insert a computed size into the cache.
    pub fn insert(&mut self, key: CacheKey, size: Size) {
        self.entries.insert(
            key,
            CacheEntry {
                size,
                last_used_frame: self.current_frame,
            },
        );
    }

    /// Clear the entire cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Advance to the next frame and evict stale entries.
    pub fn advance_frame(&mut self) {
        self.current_frame += 1;

        // Evict entries not used in the last 2 frames
        let threshold = self.current_frame.saturating_sub(2);
        self.entries
            .retain(|_, entry| entry.last_used_frame >= threshold);
    }

    /// Get the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_new() {
        let cache = LayoutCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert_get() {
        let mut cache = LayoutCache::new();
        let key = CacheKey {
            widget_id: 1,
            constraints_hash: 100,
        };
        let size = Size::new(50.0, 50.0);

        cache.insert(key, size);
        assert_eq!(cache.get(key), Some(size));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_miss() {
        let mut cache = LayoutCache::new();
        let key = CacheKey {
            widget_id: 1,
            constraints_hash: 100,
        };

        assert_eq!(cache.get(key), None);
    }

    #[test]
    fn test_cache_clear() {
        let mut cache = LayoutCache::new();
        let key = CacheKey {
            widget_id: 1,
            constraints_hash: 100,
        };

        cache.insert(key, Size::new(10.0, 10.0));
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = LayoutCache::new();
        let key = CacheKey {
            widget_id: 1,
            constraints_hash: 100,
        };

        cache.insert(key, Size::new(10.0, 10.0));

        // Advance 3 frames without using the entry
        cache.advance_frame();
        cache.advance_frame();
        cache.advance_frame();

        // Entry should be evicted
        assert!(cache.is_empty());
    }

    #[test]
    fn test_cache_not_evicted_when_used() {
        let mut cache = LayoutCache::new();
        let key = CacheKey {
            widget_id: 1,
            constraints_hash: 100,
        };

        cache.insert(key, Size::new(10.0, 10.0));

        // Advance frames but keep using the entry
        for _ in 0..5 {
            cache.advance_frame();
            let _ = cache.get(key); // Touch the entry
        }

        assert!(!cache.is_empty());
    }
}
