//! Random Seed Management
//!
//! Provides deterministic random number generation for reproducible testing
//! and benchmarking.
//!
//! # Environment Variables
//!
//! - `RANDOM_SEED`: Global random seed (default: 42)
//! - `TEST_SEED`: Test-specific seed
//! - `BENCH_SEED`: Benchmark-specific seed
//!
//! # Usage
//!
//! ```rust
//! use presentar_terminal::random_seed::{set_global_seed, get_seed, with_seed};
//!
//! // Set seed globally
//! set_global_seed(12345);
//!
//! // Get current seed
//! let seed = get_seed();
//!
//! // Run code with specific seed
//! with_seed(42, || {
//!     // deterministic operations
//! });
//! ```

use std::sync::atomic::{AtomicU64, Ordering};

/// Default random seed for reproducibility
pub const DEFAULT_SEED: u64 = 42;

/// Global seed storage
static GLOBAL_SEED: AtomicU64 = AtomicU64::new(DEFAULT_SEED);

/// Set the global random seed
///
/// This affects all subsequent random operations.
///
/// # Example
///
/// ```rust
/// use presentar_terminal::random_seed::set_global_seed;
///
/// set_global_seed(12345);
/// ```
pub fn set_global_seed(seed: u64) {
    GLOBAL_SEED.store(seed, Ordering::SeqCst);
}

/// Get the current global seed
pub fn get_seed() -> u64 {
    GLOBAL_SEED.load(Ordering::SeqCst)
}

/// Initialize seed from environment variable
///
/// Reads from `RANDOM_SEED` environment variable, falls back to default.
pub fn init_from_env() {
    let seed = std::env::var("RANDOM_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SEED);
    set_global_seed(seed);
}

/// Execute closure with a specific seed, then restore original
pub fn with_seed<F, R>(seed: u64, f: F) -> R
where
    F: FnOnce() -> R,
{
    let original = get_seed();
    set_global_seed(seed);
    let result = f();
    set_global_seed(original);
    result
}

/// Deterministic PRNG using xorshift64
#[derive(Clone, Debug)]
pub struct SeededRng {
    state: u64,
}

impl SeededRng {
    /// Create new RNG with given seed
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 1 } else { seed },
        }
    }

    /// Create RNG from global seed
    pub fn from_global_seed() -> Self {
        Self::new(get_seed())
    }

    /// Generate next random u64
    pub fn next_u64(&mut self) -> u64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        self.state
    }

    /// Generate random f64 in [0, 1)
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() as f64) / (u64::MAX as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_reproducibility() {
        let mut rng1 = SeededRng::new(42);
        let mut rng2 = SeededRng::new(42);

        for _ in 0..100 {
            assert_eq!(rng1.next_u64(), rng2.next_u64());
        }
    }

    #[test]
    fn test_with_seed() {
        set_global_seed(100);
        let result = with_seed(42, || get_seed());
        assert_eq!(result, 42);
        assert_eq!(get_seed(), 100); // restored
    }

    #[test]
    fn test_env_seed() {
        std::env::set_var("RANDOM_SEED", "9999");
        init_from_env();
        assert_eq!(get_seed(), 9999);
        std::env::remove_var("RANDOM_SEED");
    }
}
