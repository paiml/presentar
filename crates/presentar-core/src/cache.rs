#![allow(clippy::unwrap_used, clippy::disallowed_methods)]
// Data Caching Layer - WASM-first caching for API/network data
//
// Provides:
// - In-memory LRU cache
// - Time-based expiration
// - Cache invalidation patterns
// - Stale-while-revalidate strategy
// - Memory pressure handling

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;

/// Unique identifier for a cache entry
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey(u64);

impl CacheKey {
    pub fn from_str(s: &str) -> Self {
        // Simple hash for string keys
        let mut hash: u64 = 0;
        for byte in s.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
        Self(hash)
    }

    pub fn as_u64(self) -> u64 {
        self.0
    }
}

impl From<u64> for CacheKey {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<&str> for CacheKey {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

/// Cache entry state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheState {
    /// Entry is fresh and valid
    Fresh,
    /// Entry is stale but can be used while revalidating
    Stale,
    /// Entry has expired and should not be used
    Expired,
}

/// Cache entry metadata
#[derive(Debug, Clone)]
pub struct CacheMetadata {
    /// When the entry was created
    pub created_at: u64,
    /// When the entry was last accessed
    pub last_accessed: u64,
    /// Time to live in milliseconds
    pub ttl_ms: u64,
    /// Stale time in milliseconds (for stale-while-revalidate)
    pub stale_ms: u64,
    /// Number of times this entry was accessed
    pub access_count: u64,
    /// Size of the cached data in bytes
    pub size_bytes: usize,
    /// Tags for invalidation
    pub tags: Vec<String>,
}

impl CacheMetadata {
    /// Check current state based on timestamp
    pub fn state(&self, now: u64) -> CacheState {
        let age = now.saturating_sub(self.created_at);
        if age <= self.ttl_ms {
            CacheState::Fresh
        } else if age <= self.ttl_ms + self.stale_ms {
            CacheState::Stale
        } else {
            CacheState::Expired
        }
    }

    /// Check if entry is expired
    pub fn is_expired(&self, now: u64) -> bool {
        self.state(now) == CacheState::Expired
    }
}

/// Configuration for cache
#[derive(Debug, Clone)]
pub struct CacheConfig {
    /// Maximum number of entries
    pub max_entries: usize,
    /// Maximum memory in bytes
    pub max_memory: usize,
    /// Default TTL in milliseconds
    pub default_ttl_ms: u64,
    /// Default stale time in milliseconds
    pub default_stale_ms: u64,
    /// Enable LRU eviction
    pub enable_lru: bool,
    /// Cleanup interval in milliseconds
    pub cleanup_interval_ms: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,
            max_memory: 50 * 1024 * 1024,  // 50 MB
            default_ttl_ms: 5 * 60 * 1000, // 5 minutes
            default_stale_ms: 60 * 1000,   // 1 minute stale
            enable_lru: true,
            cleanup_interval_ms: 60 * 1000, // 1 minute
        }
    }
}

/// Cache entry options
#[derive(Debug, Clone, Default)]
pub struct CacheOptions {
    /// Custom TTL (None = use default)
    pub ttl: Option<Duration>,
    /// Custom stale time (None = use default)
    pub stale: Option<Duration>,
    /// Tags for invalidation
    pub tags: Vec<String>,
    /// Priority (higher = less likely to be evicted)
    pub priority: u8,
}

impl CacheOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_ttl(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    pub fn with_stale(mut self, stale: Duration) -> Self {
        self.stale = Some(stale);
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }
}

/// Cache event for notifications
#[derive(Debug, Clone)]
pub enum CacheEvent {
    /// Entry was added
    Added(CacheKey),
    /// Entry was accessed
    Hit(CacheKey),
    /// Entry was not found
    Miss(CacheKey),
    /// Entry was evicted
    Evicted(CacheKey),
    /// Entry was invalidated
    Invalidated(CacheKey),
    /// Entries were invalidated by tag
    TagInvalidated(String, usize),
    /// Cache was cleared
    Cleared,
}

/// Callback for cache events
pub type CacheCallback = Arc<dyn Fn(CacheEvent) + Send + Sync>;

/// Generic cache entry
struct CacheEntry<V> {
    value: V,
    metadata: CacheMetadata,
    #[allow(dead_code)]
    priority: u8,
}

/// In-memory data cache
pub struct DataCache<K, V>
where
    K: Eq + Hash + Clone,
{
    config: CacheConfig,
    entries: HashMap<K, CacheEntry<V>>,
    /// Access order for LRU
    access_order: Vec<K>,
    /// Current memory usage
    current_memory: usize,
    /// Current timestamp
    timestamp: u64,
    /// Last cleanup timestamp
    last_cleanup: u64,
    /// Event listeners
    listeners: Vec<CacheCallback>,
    /// Statistics
    stats: CacheStats,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub invalidations: u64,
    pub current_entries: usize,
    pub current_memory: usize,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl<K, V> Default for DataCache<K, V>
where
    K: Eq + Hash + Clone,
{
    fn default() -> Self {
        Self::new(CacheConfig::default())
    }
}

impl<K, V> DataCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            access_order: Vec::new(),
            current_memory: 0,
            timestamp: 0,
            last_cleanup: 0,
            listeners: Vec::new(),
            stats: CacheStats::default(),
        }
    }

    /// Get a value from cache
    pub fn get(&mut self, key: &K) -> Option<&V> {
        // Check if cleanup needed
        self.maybe_cleanup();

        if let Some(entry) = self.entries.get_mut(key) {
            // Check expiration
            if entry.metadata.is_expired(self.timestamp) {
                return None;
            }

            // Update access time
            entry.metadata.last_accessed = self.timestamp;
            entry.metadata.access_count += 1;

            // Update LRU order
            if self.config.enable_lru {
                self.access_order.retain(|k| k != key);
                self.access_order.push(key.clone());
            }

            self.stats.hits += 1;
            Some(&entry.value)
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Get a value and its state
    pub fn get_with_state(&mut self, key: &K) -> Option<(&V, CacheState)> {
        self.maybe_cleanup();

        if let Some(entry) = self.entries.get_mut(key) {
            let state = entry.metadata.state(self.timestamp);

            // Don't return expired entries
            if state == CacheState::Expired {
                return None;
            }

            entry.metadata.last_accessed = self.timestamp;
            entry.metadata.access_count += 1;

            if self.config.enable_lru {
                self.access_order.retain(|k| k != key);
                self.access_order.push(key.clone());
            }

            self.stats.hits += 1;
            Some((&entry.value, state))
        } else {
            self.stats.misses += 1;
            None
        }
    }

    /// Check if key exists and is not expired
    pub fn contains(&self, key: &K) -> bool {
        self.entries
            .get(key)
            .is_some_and(|e| !e.metadata.is_expired(self.timestamp))
    }

    /// Insert a value into cache
    pub fn insert(&mut self, key: K, value: V, options: CacheOptions)
    where
        V: CacheSize,
    {
        let size = value.cache_size();
        let ttl = options.ttl.map(|d| d.as_millis() as u64);
        let stale = options.stale.map(|d| d.as_millis() as u64);

        let metadata = CacheMetadata {
            created_at: self.timestamp,
            last_accessed: self.timestamp,
            ttl_ms: ttl.unwrap_or(self.config.default_ttl_ms),
            stale_ms: stale.unwrap_or(self.config.default_stale_ms),
            access_count: 0,
            size_bytes: size,
            tags: options.tags,
        };

        // Remove old entry if exists
        if let Some(old) = self.entries.remove(&key) {
            self.current_memory = self.current_memory.saturating_sub(old.metadata.size_bytes);
            self.access_order.retain(|k| k != &key);
        }

        // Evict if needed
        while self.entries.len() >= self.config.max_entries
            || self.current_memory + size > self.config.max_memory
        {
            if !self.evict_one() {
                break;
            }
        }

        self.current_memory += size;

        let entry = CacheEntry {
            value,
            metadata,
            priority: options.priority,
        };

        self.entries.insert(key.clone(), entry);
        self.access_order.push(key);

        self.stats.current_entries = self.entries.len();
        self.stats.current_memory = self.current_memory;
    }

    /// Insert with default options
    pub fn insert_default(&mut self, key: K, value: V)
    where
        V: CacheSize,
    {
        self.insert(key, value, CacheOptions::default());
    }

    /// Remove a value from cache
    pub fn remove(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.entries.remove(key) {
            self.current_memory = self
                .current_memory
                .saturating_sub(entry.metadata.size_bytes);
            self.access_order.retain(|k| k != key);
            self.stats.current_entries = self.entries.len();
            self.stats.current_memory = self.current_memory;
            self.stats.invalidations += 1;
            Some(entry.value)
        } else {
            None
        }
    }

    /// Invalidate entries by tag
    pub fn invalidate_tag(&mut self, tag: &str) -> usize {
        let keys_to_remove: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| e.metadata.tags.iter().any(|t| t == tag))
            .map(|(k, _)| k.clone())
            .collect();

        let count = keys_to_remove.len();
        for key in keys_to_remove {
            self.remove(&key);
        }

        self.emit(CacheEvent::TagInvalidated(tag.to_string(), count));
        count
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.access_order.clear();
        self.current_memory = 0;
        self.stats.current_entries = 0;
        self.stats.current_memory = 0;
        self.emit(CacheEvent::Cleared);
    }

    /// Get cache statistics
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get current memory usage
    pub fn memory_usage(&self) -> usize {
        self.current_memory
    }

    /// Update timestamp (call each frame or periodically)
    pub fn tick(&mut self, delta_ms: u64) {
        self.timestamp += delta_ms;
        self.maybe_cleanup();
    }

    /// Set timestamp directly
    pub fn set_timestamp(&mut self, timestamp: u64) {
        self.timestamp = timestamp;
    }

    /// Get current timestamp
    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }

    /// Add event listener
    pub fn on_event(&mut self, callback: CacheCallback) {
        self.listeners.push(callback);
    }

    fn emit(&self, event: CacheEvent) {
        for listener in &self.listeners {
            listener(event.clone());
        }
    }

    fn maybe_cleanup(&mut self) {
        if self.timestamp - self.last_cleanup >= self.config.cleanup_interval_ms {
            self.cleanup_expired();
            self.last_cleanup = self.timestamp;
        }
    }

    fn cleanup_expired(&mut self) {
        let now = self.timestamp;
        let keys_to_remove: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, e)| e.metadata.is_expired(now))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            if let Some(entry) = self.entries.remove(&key) {
                self.current_memory = self
                    .current_memory
                    .saturating_sub(entry.metadata.size_bytes);
                self.access_order.retain(|k| k != &key);
                self.stats.evictions += 1;
            }
        }

        self.stats.current_entries = self.entries.len();
        self.stats.current_memory = self.current_memory;
    }

    fn evict_one(&mut self) -> bool {
        if self.config.enable_lru && !self.access_order.is_empty() {
            // LRU eviction
            let key = self.access_order.remove(0);
            if let Some(entry) = self.entries.remove(&key) {
                self.current_memory = self
                    .current_memory
                    .saturating_sub(entry.metadata.size_bytes);
                self.stats.evictions += 1;
                return true;
            }
        } else if !self.entries.is_empty() {
            // Random eviction as fallback
            if let Some(key) = self.entries.keys().next().cloned() {
                if let Some(entry) = self.entries.remove(&key) {
                    self.current_memory = self
                        .current_memory
                        .saturating_sub(entry.metadata.size_bytes);
                    self.access_order.retain(|k| k != &key);
                    self.stats.evictions += 1;
                    return true;
                }
            }
        }
        false
    }
}

/// Trait for getting size of cached values
pub trait CacheSize {
    fn cache_size(&self) -> usize;
}

// Implement CacheSize for common types
impl CacheSize for String {
    fn cache_size(&self) -> usize {
        self.len()
    }
}

impl<T> CacheSize for Vec<T> {
    fn cache_size(&self) -> usize {
        self.len() * std::mem::size_of::<T>()
    }
}

impl<T> CacheSize for Box<T> {
    fn cache_size(&self) -> usize {
        std::mem::size_of::<T>()
    }
}

impl CacheSize for () {
    fn cache_size(&self) -> usize {
        0
    }
}

impl CacheSize for i32 {
    fn cache_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl CacheSize for i64 {
    fn cache_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl CacheSize for f32 {
    fn cache_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

impl CacheSize for f64 {
    fn cache_size(&self) -> usize {
        std::mem::size_of::<Self>()
    }
}

/// Simple string-keyed cache
pub type StringCache<V> = DataCache<String, V>;

/// Builder for cache entries
pub struct CacheBuilder<V> {
    value: V,
    options: CacheOptions,
}

impl<V> CacheBuilder<V> {
    pub fn new(value: V) -> Self {
        Self {
            value,
            options: CacheOptions::default(),
        }
    }

    pub fn ttl(mut self, ttl: Duration) -> Self {
        self.options.ttl = Some(ttl);
        self
    }

    pub fn stale(mut self, stale: Duration) -> Self {
        self.options.stale = Some(stale);
        self
    }

    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.options.tags.push(tag.into());
        self
    }

    pub fn priority(mut self, priority: u8) -> Self {
        self.options.priority = priority;
        self
    }

    pub fn build(self) -> (V, CacheOptions) {
        (self.value, self.options)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_key_from_str() {
        let key1 = CacheKey::from_str("test");
        let key2 = CacheKey::from_str("test");
        let key3 = CacheKey::from_str("different");

        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_cache_key_from_u64() {
        let key: CacheKey = 42u64.into();
        assert_eq!(key.as_u64(), 42);
    }

    #[test]
    fn test_cache_default() {
        let cache: DataCache<String, String> = DataCache::default();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_insert_get() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key".to_string(), "value".to_string());

        let result = cache.get(&"key".to_string());
        assert_eq!(result, Some(&"value".to_string()));
    }

    #[test]
    fn test_cache_miss() {
        let mut cache: DataCache<String, String> = DataCache::default();

        let result = cache.get(&"missing".to_string());
        assert_eq!(result, None);
    }

    #[test]
    fn test_cache_contains() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key".to_string(), "value".to_string());

        assert!(cache.contains(&"key".to_string()));
        assert!(!cache.contains(&"missing".to_string()));
    }

    #[test]
    fn test_cache_remove() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key".to_string(), "value".to_string());
        let removed = cache.remove(&"key".to_string());

        assert_eq!(removed, Some("value".to_string()));
        assert!(!cache.contains(&"key".to_string()));
    }

    #[test]
    fn test_cache_clear() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key1".to_string(), "value1".to_string());
        cache.insert_default("key2".to_string(), "value2".to_string());

        cache.clear();

        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_cache_expiration() {
        let config = CacheConfig {
            default_ttl_ms: 100,
            default_stale_ms: 0,
            ..Default::default()
        };
        let mut cache: DataCache<String, String> = DataCache::new(config);

        cache.insert_default("key".to_string(), "value".to_string());

        // Should be fresh
        assert!(cache.contains(&"key".to_string()));

        // Advance time past TTL
        cache.set_timestamp(200);

        // Should be expired
        assert!(!cache.contains(&"key".to_string()));
        assert!(cache.get(&"key".to_string()).is_none());
    }

    #[test]
    fn test_cache_stale_while_revalidate() {
        let config = CacheConfig {
            default_ttl_ms: 100,
            default_stale_ms: 50,
            ..Default::default()
        };
        let mut cache: DataCache<String, String> = DataCache::new(config);

        cache.insert_default("key".to_string(), "value".to_string());

        // Should be fresh
        let (_, state) = cache.get_with_state(&"key".to_string()).unwrap();
        assert_eq!(state, CacheState::Fresh);

        // Advance into stale period
        cache.set_timestamp(120);
        let (_, state) = cache.get_with_state(&"key".to_string()).unwrap();
        assert_eq!(state, CacheState::Stale);

        // Advance past stale period
        cache.set_timestamp(200);
        assert!(cache.get_with_state(&"key".to_string()).is_none());
    }

    #[test]
    fn test_cache_custom_ttl() {
        let config = CacheConfig {
            default_ttl_ms: 1000,
            default_stale_ms: 0, // No stale period for this test
            ..Default::default()
        };
        let mut cache: DataCache<String, String> = DataCache::new(config);

        let options = CacheOptions::new().with_ttl(Duration::from_millis(50));
        cache.insert("key".to_string(), "value".to_string(), options);

        cache.set_timestamp(100);
        assert!(cache.get(&"key".to_string()).is_none());
    }

    #[test]
    fn test_cache_tags() {
        let mut cache: DataCache<String, String> = DataCache::default();

        let options1 = CacheOptions::new().with_tag("user").with_tag("profile");
        let options2 = CacheOptions::new().with_tag("user");
        let options3 = CacheOptions::new().with_tag("settings");

        cache.insert("key1".to_string(), "value1".to_string(), options1);
        cache.insert("key2".to_string(), "value2".to_string(), options2);
        cache.insert("key3".to_string(), "value3".to_string(), options3);

        // Invalidate by tag
        let count = cache.invalidate_tag("user");
        assert_eq!(count, 2);

        assert!(!cache.contains(&"key1".to_string()));
        assert!(!cache.contains(&"key2".to_string()));
        assert!(cache.contains(&"key3".to_string()));
    }

    #[test]
    fn test_cache_lru_eviction() {
        let config = CacheConfig {
            max_entries: 3,
            enable_lru: true,
            ..Default::default()
        };
        let mut cache: DataCache<String, i32> = DataCache::new(config);

        cache.insert_default("key1".to_string(), 1);
        cache.insert_default("key2".to_string(), 2);
        cache.insert_default("key3".to_string(), 3);

        // Access key1 to make it recently used
        cache.get(&"key1".to_string());

        // Insert new entry, should evict key2 (LRU)
        cache.insert_default("key4".to_string(), 4);

        assert!(cache.contains(&"key1".to_string()));
        assert!(!cache.contains(&"key2".to_string())); // Evicted
        assert!(cache.contains(&"key3".to_string()));
        assert!(cache.contains(&"key4".to_string()));
    }

    #[test]
    fn test_cache_memory_limit() {
        let config = CacheConfig {
            max_memory: 20, // Very small
            ..Default::default()
        };
        let mut cache: DataCache<String, String> = DataCache::new(config);

        cache.insert_default("key1".to_string(), "12345".to_string()); // 5 bytes
        cache.insert_default("key2".to_string(), "12345".to_string()); // 5 bytes
        cache.insert_default("key3".to_string(), "12345".to_string()); // 5 bytes
        cache.insert_default("key4".to_string(), "12345".to_string()); // 5 bytes

        // Should have evicted some entries
        assert!(cache.memory_usage() <= 20);
    }

    #[test]
    fn test_cache_stats() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key".to_string(), "value".to_string());

        cache.get(&"key".to_string()); // Hit
        cache.get(&"key".to_string()); // Hit
        cache.get(&"missing".to_string()); // Miss

        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 0.666).abs() < 0.01);
    }

    #[test]
    fn test_cache_tick() {
        let config = CacheConfig {
            cleanup_interval_ms: 100,
            default_ttl_ms: 50,
            default_stale_ms: 0, // No stale period for this test
            ..Default::default()
        };
        let mut cache: DataCache<String, String> = DataCache::new(config);

        cache.insert_default("key".to_string(), "value".to_string());

        // Tick forward past TTL and cleanup interval
        cache.tick(150);

        // Entry should be cleaned up
        assert!(!cache.contains(&"key".to_string()));
    }

    #[test]
    fn test_cache_metadata_state() {
        let meta = CacheMetadata {
            created_at: 0,
            last_accessed: 0,
            ttl_ms: 100,
            stale_ms: 50,
            access_count: 0,
            size_bytes: 0,
            tags: vec![],
        };

        assert_eq!(meta.state(50), CacheState::Fresh);
        assert_eq!(meta.state(120), CacheState::Stale);
        assert_eq!(meta.state(200), CacheState::Expired);

        assert!(!meta.is_expired(100));
        assert!(meta.is_expired(200));
    }

    #[test]
    fn test_cache_options_builder() {
        let options = CacheOptions::new()
            .with_ttl(Duration::from_secs(60))
            .with_stale(Duration::from_secs(30))
            .with_tag("test")
            .with_priority(5);

        assert_eq!(options.ttl, Some(Duration::from_secs(60)));
        assert_eq!(options.stale, Some(Duration::from_secs(30)));
        assert_eq!(options.tags, vec!["test"]);
        assert_eq!(options.priority, 5);
    }

    #[test]
    fn test_cache_builder() {
        let (value, options) = CacheBuilder::new("test".to_string())
            .ttl(Duration::from_secs(60))
            .stale(Duration::from_secs(30))
            .tag("user")
            .priority(3)
            .build();

        assert_eq!(value, "test");
        assert_eq!(options.ttl, Some(Duration::from_secs(60)));
        assert_eq!(options.tags, vec!["user"]);
    }

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert_eq!(config.max_entries, 1000);
        assert_eq!(config.max_memory, 50 * 1024 * 1024);
        assert_eq!(config.default_ttl_ms, 5 * 60 * 1000);
        assert_eq!(config.default_stale_ms, 60 * 1000);
        assert!(config.enable_lru);
    }

    #[test]
    fn test_cache_size_string() {
        let s = "hello".to_string();
        assert_eq!(s.cache_size(), 5);
    }

    #[test]
    fn test_cache_size_vec() {
        let v: Vec<u8> = vec![1, 2, 3, 4, 5];
        assert_eq!(v.cache_size(), 5);
    }

    #[test]
    fn test_cache_size_i32() {
        let n: i32 = 42;
        assert_eq!(n.cache_size(), 4);
    }

    #[test]
    fn test_cache_update_entry() {
        let mut cache: DataCache<String, String> = DataCache::default();

        cache.insert_default("key".to_string(), "value1".to_string());
        cache.insert_default("key".to_string(), "value2".to_string());

        let result = cache.get(&"key".to_string());
        assert_eq!(result, Some(&"value2".to_string()));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_cache_event_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let mut cache: DataCache<String, String> = DataCache::default();
        let event_count = Arc::new(AtomicUsize::new(0));

        let ec = event_count.clone();
        cache.on_event(Arc::new(move |_event| {
            ec.fetch_add(1, Ordering::SeqCst);
        }));

        cache.invalidate_tag("test");
        cache.clear();

        assert!(event_count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn test_cache_state_variants() {
        assert_eq!(CacheState::Fresh, CacheState::Fresh);
        assert_ne!(CacheState::Fresh, CacheState::Stale);
        assert_ne!(CacheState::Stale, CacheState::Expired);
    }

    #[test]
    fn test_stats_hit_rate_empty() {
        let stats = CacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0);
    }

    #[test]
    fn test_cache_timestamp() {
        let mut cache: DataCache<String, String> = DataCache::default();
        assert_eq!(cache.timestamp(), 0);

        cache.set_timestamp(1000);
        assert_eq!(cache.timestamp(), 1000);

        cache.tick(500);
        assert_eq!(cache.timestamp(), 1500);
    }
}
