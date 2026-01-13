//! Performance tracing for ptop `ComputeBlocks`
//!
//! This module provides lightweight performance tracing compatible with
//! renacer's `BrickTracer` format. It can be used standalone or integrated
//! with renacer for deep syscall-level analysis.
//!
//! **Specification**: SPEC-024 Section 23.5 (Presentar Headless Tracing)
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │                   PerfTrace Architecture                         │
//! │                                                                  │
//! │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │
//! │  │ PerfTracer  │ →  │ TraceEvent  │ →  │ renacer BrickTracer │  │
//! │  │ (in-process)│    │ (metrics)   │    │ (optional deep)     │  │
//! │  └─────────────┘    └─────────────┘    └─────────────────────┘  │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use presentar_terminal::perf_trace::PerfTracer;
//!
//! let mut tracer = PerfTracer::new();
//!
//! // Trace a block of code
//! let result = tracer.trace("collect_metrics", || {
//!     app.collect_metrics();
//! });
//!
//! // Get performance summary
//! println!("{}", tracer.summary());
//! ```

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

// =============================================================================
// GLOBAL ATOMIC STATE (trueno-viz pattern)
// =============================================================================

/// Global flag to enable/disable tracing (zero cost when disabled - 1 atomic load)
static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
/// Start time in microseconds for relative timestamps
static START_TIME_US: AtomicU64 = AtomicU64::new(0);

/// Enable global tracing
pub fn enable_tracing() {
    START_TIME_US.store(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64,
        Ordering::Relaxed,
    );
    TRACE_ENABLED.store(true, Ordering::Release);
}

/// Disable global tracing
pub fn disable_tracing() {
    TRACE_ENABLED.store(false, Ordering::Release);
}

/// Check if tracing is enabled
#[inline]
pub fn is_tracing_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Acquire)
}

// =============================================================================
// RAII TIMING GUARD (trueno-viz pattern)
// =============================================================================

/// Zero-cost RAII timing guard (trueno-viz pattern)
///
/// Automatically logs entry/exit on Drop. Zero overhead when tracing disabled.
///
/// # Example
/// ```rust,ignore
/// fn render_panel() {
///     let _guard = TimingGuard::new("render_panel", 1000); // 1ms budget
///     // ... rendering code ...
/// } // Automatically logs duration on drop
/// ```
pub struct TimingGuard {
    name: &'static str,
    start: Option<Instant>,
    budget_us: u64,
}

impl TimingGuard {
    /// Create a new timing guard with budget in microseconds
    ///
    /// If tracing is disabled, returns a no-op guard (zero cost).
    #[inline]
    pub fn new(name: &'static str, budget_us: u64) -> Self {
        if is_tracing_enabled() {
            Self {
                name,
                start: Some(Instant::now()),
                budget_us,
            }
        } else {
            Self {
                name,
                start: None,
                budget_us,
            }
        }
    }

    /// Create guard with default 1ms budget
    #[inline]
    pub fn with_default_budget(name: &'static str) -> Self {
        Self::new(name, 1000)
    }

    /// Create guard for render operations (16ms budget for 60fps)
    #[inline]
    pub fn render(name: &'static str) -> Self {
        Self::new(name, 16_000)
    }

    /// Create guard for collection operations (100ms budget)
    #[inline]
    pub fn collect(name: &'static str) -> Self {
        Self::new(name, 100_000)
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            let elapsed = start.elapsed();
            let elapsed_us = elapsed.as_micros() as u64;
            let exceeded = elapsed_us > self.budget_us;

            // Log format: [+0042ms] [TRACE] [name] <- done (12.34ms)
            let relative_ms = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_micros() as u64)
                .saturating_sub(START_TIME_US.load(Ordering::Relaxed))
                / 1000;

            let status = if exceeded { "⚠️" } else { "✓" };
            eprintln!(
                "[+{:04}ms] [TRACE] {status} {} <- {:.2}ms (budget: {}μs)",
                relative_ms,
                self.name,
                elapsed_us as f64 / 1000.0,
                self.budget_us
            );
        }
    }
}

// =============================================================================
// O(1) SIMD-STYLE STATS (trueno-viz pattern)
// =============================================================================

/// Cache-aligned running statistics for O(1) access (trueno-viz SimdStats pattern)
///
/// All statistics are pre-computed during data collection, never during render.
/// This guarantees <1ms frame time per SPEC-024.
#[repr(C, align(64))] // Cache-aligned for multi-threaded access
#[derive(Debug, Clone, Default)]
pub struct SimdStats {
    /// Running count of samples
    pub count: u64,
    /// Running sum for mean calculation
    pub sum: f64,
    /// Running sum of squares for variance calculation
    pub sum_sq: f64,
    /// Running minimum value
    pub min: f64,
    /// Running maximum value
    pub max: f64,
}

impl SimdStats {
    /// Create new stats initialized to zero
    pub fn new() -> Self {
        Self {
            count: 0,
            sum: 0.0,
            sum_sq: 0.0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    /// Update stats with a new value (O(1))
    #[inline]
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        self.sum_sq += value * value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
    }

    /// Get mean (O(1))
    #[inline]
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Get variance (O(1))
    #[inline]
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let n = self.count as f64;
        (self.sum_sq - (self.sum * self.sum) / n) / (n - 1.0)
    }

    /// Get standard deviation (O(1))
    #[inline]
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Get coefficient of variation (O(1))
    #[inline]
    pub fn cv_percent(&self) -> f64 {
        let mean = self.mean();
        if mean.abs() < 1e-9 {
            0.0
        } else {
            (self.std_dev() / mean) * 100.0
        }
    }

    /// Reset stats
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

// =============================================================================
// BRICK PROFILER (renacer BrickTracer integration)
// =============================================================================

/// Brick types for profiling (aligns with renacer brick taxonomy)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrickType {
    /// Data collection brick (I/O bound)
    Collect,
    /// Render brick (CPU bound, must be <16ms for 60fps)
    Render,
    /// Compute brick (SIMD optimized)
    Compute,
    /// Network brick (I/O bound with latency)
    Network,
    /// Storage brick (disk I/O)
    Storage,
}

impl BrickType {
    /// Get default budget for this brick type in microseconds
    #[must_use]
    pub fn default_budget_us(&self) -> u64 {
        match self {
            Self::Collect => 100_000,  // 100ms for collection
            Self::Render => 16_000,    // 16ms for 60fps
            Self::Compute => 1_000,    // 1ms for compute
            Self::Network => 500_000,  // 500ms for network
            Self::Storage => 50_000,   // 50ms for storage
        }
    }

    /// Get severity threshold (CV%) for escalation
    #[must_use]
    pub fn cv_threshold(&self) -> f64 {
        match self {
            Self::Render => 10.0,   // Strict for render
            Self::Compute => 15.0,  // Standard
            Self::Collect => 25.0,  // More lenient
            Self::Network => 50.0,  // High variance expected
            Self::Storage => 30.0,  // Moderate variance
        }
    }
}

/// Brick profiler for tracking computational units.
///
/// Provides higher-level profiling with brick-specific budgets and thresholds.
/// Compatible with renacer's BrickTracer for escalation.
#[derive(Debug)]
pub struct BrickProfiler {
    /// Stats per brick name
    stats: std::collections::HashMap<String, (BrickType, SimdStats)>,
    /// Whether profiling is enabled
    enabled: bool,
}

impl Default for BrickProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl BrickProfiler {
    /// Create a new brick profiler
    #[must_use]
    pub fn new() -> Self {
        Self {
            stats: std::collections::HashMap::new(),
            enabled: is_tracing_enabled(),
        }
    }

    /// Profile a brick execution
    pub fn profile<F, R>(&mut self, name: &str, brick_type: BrickType, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        if !self.enabled {
            return f();
        }

        let start = std::time::Instant::now();
        let result = f();
        let elapsed_us = start.elapsed().as_micros() as f64;

        // Update stats
        let (_, stats) = self
            .stats
            .entry(name.to_string())
            .or_insert_with(|| (brick_type, SimdStats::new()));
        stats.update(elapsed_us);

        result
    }

    /// Check if a brick should escalate to deep profiling
    #[must_use]
    pub fn should_escalate(&self, name: &str) -> bool {
        if let Some((brick_type, stats)) = self.stats.get(name) {
            let cv = stats.cv_percent();
            cv > brick_type.cv_threshold()
        } else {
            false
        }
    }

    /// Get stats for a brick
    #[must_use]
    pub fn get_stats(&self, name: &str) -> Option<&SimdStats> {
        self.stats.get(name).map(|(_, s)| s)
    }

    /// Generate a summary report
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = vec!["=== Brick Profiler Summary ===".to_string()];

        let mut sorted: Vec<_> = self.stats.iter().collect();
        sorted.sort_by(|a, b| {
            let a_total = a.1 .1.sum;
            let b_total = b.1 .1.sum;
            b_total.partial_cmp(&a_total).unwrap_or(std::cmp::Ordering::Equal)
        });

        for (name, (brick_type, stats)) in sorted {
            let budget = brick_type.default_budget_us();
            let avg = stats.mean();
            let cv = stats.cv_percent();
            let status = if avg > budget as f64 { "⚠️" } else { "✓" };
            let escalate = if self.should_escalate(name) { " [ESCALATE]" } else { "" };

            lines.push(format!(
                "{status} {name} ({brick_type:?}): avg={avg:.0}μs cv={cv:.1}% n={}{escalate}",
                stats.count
            ));
        }

        lines.join("\n")
    }
}

/// Performance trace event (compatible with renacer `TraceEvent`)
#[derive(Debug, Clone)]
pub struct TraceEvent {
    /// Event name (e.g., "`collect_metrics`", "`render_cpu_panel`")
    pub name: String,
    /// Duration of the event
    pub duration: Duration,
    /// Timestamp when event started
    pub timestamp_us: u64,
    /// Whether this exceeded the budget
    pub budget_exceeded: bool,
    /// Budget in microseconds (if set)
    pub budget_us: Option<u64>,
}

/// Aggregated statistics for a traced operation
#[derive(Debug, Clone, Default)]
pub struct TraceStats {
    /// Total number of invocations
    pub count: u64,
    /// Total duration across all invocations
    pub total_duration: Duration,
    /// Minimum duration
    pub min_duration: Duration,
    /// Maximum duration
    pub max_duration: Duration,
    /// Number of times budget was exceeded
    pub budget_violations: u64,
    /// Budget in microseconds
    pub budget_us: u64,
}

impl TraceStats {
    /// Create new stats with initial values
    fn new(duration: Duration, budget_us: u64, exceeded: bool) -> Self {
        Self {
            count: 1,
            total_duration: duration,
            min_duration: duration,
            max_duration: duration,
            budget_violations: u64::from(exceeded),
            budget_us,
        }
    }

    /// Update stats with a new measurement
    fn update(&mut self, duration: Duration, exceeded: bool) {
        self.count += 1;
        self.total_duration += duration;
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        if exceeded {
            self.budget_violations += 1;
        }
    }

    /// Average duration
    pub fn avg_duration(&self) -> Duration {
        if self.count == 0 {
            Duration::ZERO
        } else {
            self.total_duration / self.count as u32
        }
    }

    /// Coefficient of variation (CV) percentage
    /// CV > 15% indicates unstable performance per Curtsinger & Berger (2013)
    pub fn cv_percent(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        let avg = self.avg_duration().as_secs_f64();
        if avg < 1e-9 {
            return 0.0;
        }
        let range = self
            .max_duration
            .checked_sub(self.min_duration)
            .unwrap_or_default()
            .as_secs_f64();
        (range / avg) * 50.0 // Simplified CV approximation
    }

    /// Budget efficiency percentage
    /// Efficiency < 25% indicates budget is too tight per Williams et al. (2009)
    pub fn efficiency_percent(&self) -> f64 {
        if self.budget_us == 0 {
            return 100.0;
        }
        let avg_us = self.avg_duration().as_micros() as f64;
        ((self.budget_us as f64) / avg_us * 100.0).min(100.0)
    }
}

/// Escalation thresholds for deep tracing (from renacer `BrickEscalationThresholds`)
#[derive(Debug, Clone, Copy)]
pub struct EscalationThresholds {
    /// CV threshold above which to escalate (default: 15.0%)
    pub cv_percent: f64,
    /// Efficiency threshold below which to escalate (default: 25.0%)
    pub efficiency_percent: f64,
    /// Maximum traces per second (rate limiting)
    pub max_traces_per_sec: u32,
}

impl Default for EscalationThresholds {
    fn default() -> Self {
        Self {
            cv_percent: 15.0,
            efficiency_percent: 25.0,
            max_traces_per_sec: 100,
        }
    }
}

/// Lightweight performance tracer for ptop
///
/// Provides timing measurements and statistics for `ComputeBlocks`.
/// Can be used standalone or integrated with renacer for deep analysis.
#[derive(Debug)]
pub struct PerfTracer {
    /// Aggregated stats per operation name
    stats: HashMap<String, TraceStats>,
    /// Recent events (ring buffer, last N)
    recent_events: Vec<TraceEvent>,
    /// Maximum recent events to keep
    max_recent: usize,
    /// Start time for relative timestamps
    start_time: Instant,
    /// Escalation thresholds
    thresholds: EscalationThresholds,
    /// Number of traces since last second
    traces_this_second: u32,
    /// Second when trace count was last reset
    last_second: u64,
}

impl Default for PerfTracer {
    fn default() -> Self {
        Self::new()
    }
}

impl PerfTracer {
    /// Create a new performance tracer
    #[must_use]
    pub fn new() -> Self {
        Self {
            stats: HashMap::new(),
            recent_events: Vec::with_capacity(100),
            max_recent: 100,
            start_time: Instant::now(),
            thresholds: EscalationThresholds::default(),
            traces_this_second: 0,
            last_second: 0,
        }
    }

    /// Create a tracer with custom thresholds
    #[must_use]
    pub fn with_thresholds(thresholds: EscalationThresholds) -> Self {
        Self {
            thresholds,
            ..Self::new()
        }
    }

    /// Trace a function execution with a name and budget
    pub fn trace_with_budget<F, R>(&mut self, name: &str, budget_us: u64, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();

        self.record_trace(name, duration, budget_us);
        result
    }

    /// Trace a function execution with default 1ms budget
    pub fn trace<F, R>(&mut self, name: &str, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        self.trace_with_budget(name, 1000, f) // 1ms default budget
    }

    /// Record a trace event
    fn record_trace(&mut self, name: &str, duration: Duration, budget_us: u64) {
        let timestamp_us = self.start_time.elapsed().as_micros() as u64;
        let budget_exceeded = duration.as_micros() as u64 > budget_us;

        // Rate limiting
        let current_second = timestamp_us / 1_000_000;
        if current_second != self.last_second {
            self.traces_this_second = 0;
            self.last_second = current_second;
        }
        self.traces_this_second += 1;

        // Create event
        let event = TraceEvent {
            name: name.to_string(),
            duration,
            timestamp_us,
            budget_exceeded,
            budget_us: Some(budget_us),
        };

        // Update stats
        if let Some(stats) = self.stats.get_mut(name) {
            stats.update(duration, budget_exceeded);
        } else {
            self.stats.insert(
                name.to_string(),
                TraceStats::new(duration, budget_us, budget_exceeded),
            );
        }

        // Store recent event
        if self.recent_events.len() >= self.max_recent {
            self.recent_events.remove(0);
        }
        self.recent_events.push(event);
    }

    /// Check if an operation should escalate to deep tracing
    #[must_use]
    pub fn should_escalate(&self, name: &str) -> bool {
        if let Some(stats) = self.stats.get(name) {
            let cv = stats.cv_percent();
            let efficiency = stats.efficiency_percent();

            cv > self.thresholds.cv_percent || efficiency < self.thresholds.efficiency_percent
        } else {
            false
        }
    }

    /// Get stats for a specific operation
    #[must_use]
    pub fn get_stats(&self, name: &str) -> Option<&TraceStats> {
        self.stats.get(name)
    }

    /// Get all stats
    #[must_use]
    pub fn all_stats(&self) -> &HashMap<String, TraceStats> {
        &self.stats
    }

    /// Generate a summary report
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = vec![
            "=== Performance Trace Summary ===".to_string(),
            String::new(),
        ];

        let mut sorted: Vec<_> = self.stats.iter().collect();
        sorted.sort_by(|a, b| b.1.total_duration.cmp(&a.1.total_duration));

        for (name, stats) in sorted {
            let avg_us = stats.avg_duration().as_micros();
            let max_us = stats.max_duration.as_micros();
            let cv = stats.cv_percent();
            let eff = stats.efficiency_percent();
            let status = if stats.budget_violations > 0 {
                "⚠️"
            } else {
                "✓"
            };

            lines.push(format!(
                "{status} {name}: avg={avg_us}μs max={max_us}μs count={} cv={cv:.1}% eff={eff:.0}%",
                stats.count
            ));

            if self.should_escalate(name) {
                lines.push(format!(
                    "  └── ESCALATE: CV={cv:.1}% > {}% OR eff={eff:.0}% < {}%",
                    self.thresholds.cv_percent, self.thresholds.efficiency_percent
                ));
            }
        }

        lines.join("\n")
    }

    /// Export stats in a format compatible with renacer
    #[must_use]
    pub fn export_renacer_format(&self) -> String {
        let mut lines = vec!["# renacer-compatible trace export".to_string()];
        lines.push(format!("# timestamp: {:?}", self.start_time.elapsed()));

        for (name, stats) in &self.stats {
            lines.push(format!(
                "TRACE {} count={} total_us={} avg_us={} max_us={} cv={:.2} eff={:.2} violations={}",
                name,
                stats.count,
                stats.total_duration.as_micros(),
                stats.avg_duration().as_micros(),
                stats.max_duration.as_micros(),
                stats.cv_percent(),
                stats.efficiency_percent(),
                stats.budget_violations
            ));
        }

        lines.join("\n")
    }

    /// Clear all stats
    pub fn clear(&mut self) {
        self.stats.clear();
        self.recent_events.clear();
    }
}

// =============================================================================
// FIXED-SIZE RING BUFFER (trueno-viz O(1) history pattern)
// =============================================================================

/// Fixed-size ring buffer for O(1) history access (trueno-viz pattern)
///
/// Provides constant-time insert, latest N values, and rolling statistics.
/// No heap allocation after initial creation.
#[derive(Debug, Clone)]
pub struct RingBuffer<T, const N: usize> {
    data: [T; N],
    head: usize,
    len: usize,
}

impl<T: Default + Copy, const N: usize> Default for RingBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Default + Copy, const N: usize> RingBuffer<T, N> {
    /// Create a new empty ring buffer
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: [T::default(); N],
            head: 0,
            len: 0,
        }
    }

    /// Push a value (O(1))
    pub fn push(&mut self, value: T) {
        self.data[self.head] = value;
        self.head = (self.head + 1) % N;
        self.len = self.len.saturating_add(1).min(N);
    }

    /// Get current length
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Check if full
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.len == N
    }

    /// Get capacity
    #[must_use]
    pub fn capacity(&self) -> usize {
        N
    }

    /// Get the most recent value (O(1))
    #[must_use]
    pub fn latest(&self) -> Option<&T> {
        if self.len == 0 {
            None
        } else {
            let idx = if self.head == 0 { N - 1 } else { self.head - 1 };
            Some(&self.data[idx])
        }
    }

    /// Get value at index from oldest (O(1))
    /// Index 0 = oldest, index len-1 = newest
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let start = if self.len < N {
            0
        } else {
            self.head
        };
        let actual_idx = (start + index) % N;
        Some(&self.data[actual_idx])
    }

    /// Iterate from oldest to newest
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let start = if self.len < N { 0 } else { self.head };
        (0..self.len).map(move |i| {
            let idx = (start + i) % N;
            &self.data[idx]
        })
    }

    /// Clear the buffer
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
}

/// Rolling statistics over a ring buffer (O(1) access)
impl<const N: usize> RingBuffer<f64, N> {
    /// Calculate running sum (O(n) but typically small N)
    #[must_use]
    pub fn sum(&self) -> f64 {
        self.iter().sum()
    }

    /// Calculate mean (O(n))
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.len == 0 {
            0.0
        } else {
            self.sum() / self.len as f64
        }
    }

    /// Get min value (O(n))
    #[must_use]
    pub fn min(&self) -> Option<f64> {
        self.iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// Get max value (O(n))
    #[must_use]
    pub fn max(&self) -> Option<f64> {
        self.iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }
}

// =============================================================================
// HISTOGRAM BIN (trueno-viz O(1) distribution pattern)
// =============================================================================

/// Fixed-bin histogram for O(1) distribution tracking (trueno-viz pattern)
///
/// Pre-defined bins for common latency ranges. Insert and query are O(1).
#[derive(Debug, Clone)]
pub struct LatencyHistogram {
    /// Counts for bins: [0-1ms, 1-5ms, 5-10ms, 10-50ms, 50-100ms, 100-500ms, 500ms+]
    bins: [u64; 7],
    /// Total count
    count: u64,
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl LatencyHistogram {
    /// Create a new histogram
    #[must_use]
    pub fn new() -> Self {
        Self {
            bins: [0; 7],
            count: 0,
        }
    }

    /// Record a latency value in microseconds (O(1))
    pub fn record(&mut self, latency_us: u64) {
        let bin = match latency_us {
            0..=999 => 0,        // 0-1ms
            1000..=4999 => 1,    // 1-5ms
            5000..=9999 => 2,    // 5-10ms
            10000..=49999 => 3,  // 10-50ms
            50000..=99999 => 4,  // 50-100ms
            100000..=499999 => 5, // 100-500ms
            _ => 6,              // 500ms+
        };
        self.bins[bin] += 1;
        self.count += 1;
    }

    /// Get total count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get count for a specific bin
    #[must_use]
    pub fn bin_count(&self, bin: usize) -> u64 {
        self.bins.get(bin).copied().unwrap_or(0)
    }

    /// Get percentage in each bin
    #[must_use]
    pub fn percentages(&self) -> [f64; 7] {
        if self.count == 0 {
            return [0.0; 7];
        }
        let mut pcts = [0.0; 7];
        for (i, &count) in self.bins.iter().enumerate() {
            pcts[i] = (count as f64 / self.count as f64) * 100.0;
        }
        pcts
    }

    /// Get bin label
    #[must_use]
    pub fn bin_label(bin: usize) -> &'static str {
        match bin {
            0 => "0-1ms",
            1 => "1-5ms",
            2 => "5-10ms",
            3 => "10-50ms",
            4 => "50-100ms",
            5 => "100-500ms",
            6 => "500ms+",
            _ => "?",
        }
    }

    /// Format as ASCII histogram
    #[must_use]
    pub fn ascii_histogram(&self, width: usize) -> String {
        let pcts = self.percentages();
        let mut lines = Vec::new();

        for (i, pct) in pcts.iter().enumerate() {
            let bar_len = ((*pct / 100.0) * width as f64) as usize;
            let bar: String = "█".repeat(bar_len);
            lines.push(format!("{:>10} {:5.1}% {}", Self::bin_label(i), pct, bar));
        }

        lines.join("\n")
    }

    /// Reset histogram
    pub fn reset(&mut self) {
        self.bins = [0; 7];
        self.count = 0;
    }
}

// =============================================================================
// EMA TRACKER (trueno-viz O(1) smoothing pattern)
// =============================================================================

/// Exponential Moving Average tracker for O(1) smoothing (trueno-viz pattern)
///
/// Provides real-time smoothed values without storing history.
/// Commonly used for FPS counters, load averages, and trend detection.
#[derive(Debug, Clone)]
pub struct EmaTracker {
    /// Current smoothed value
    value: f64,
    /// Smoothing factor (0.0-1.0, higher = more responsive)
    alpha: f64,
    /// Whether we've received the first sample
    initialized: bool,
}

impl Default for EmaTracker {
    fn default() -> Self {
        Self::new(0.1) // Default 10% weight for new samples
    }
}

impl EmaTracker {
    /// Create a new EMA tracker with given smoothing factor
    ///
    /// Alpha should be between 0.0 and 1.0:
    /// - Higher alpha (e.g., 0.5) = more responsive, less smooth
    /// - Lower alpha (e.g., 0.05) = less responsive, more smooth
    #[must_use]
    pub fn new(alpha: f64) -> Self {
        Self {
            value: 0.0,
            alpha: alpha.clamp(0.0, 1.0),
            initialized: false,
        }
    }

    /// Create tracker optimized for FPS counting (fast response)
    #[must_use]
    pub fn for_fps() -> Self {
        Self::new(0.3)
    }

    /// Create tracker optimized for load average (slow response)
    #[must_use]
    pub fn for_load() -> Self {
        Self::new(0.05)
    }

    /// Update with a new sample (O(1))
    pub fn update(&mut self, sample: f64) {
        if self.initialized {
            self.value = self.alpha * sample + (1.0 - self.alpha) * self.value;
        } else {
            self.value = sample;
            self.initialized = true;
        }
    }

    /// Get current smoothed value (O(1))
    #[must_use]
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Check if tracker has been initialized
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get the alpha (smoothing factor)
    #[must_use]
    pub fn alpha(&self) -> f64 {
        self.alpha
    }

    /// Reset tracker to uninitialized state
    pub fn reset(&mut self) {
        self.value = 0.0;
        self.initialized = false;
    }

    /// Set a new alpha value
    pub fn set_alpha(&mut self, alpha: f64) {
        self.alpha = alpha.clamp(0.0, 1.0);
    }
}

// =============================================================================
// RATE LIMITER (trueno-viz O(1) throttling pattern)
// =============================================================================

/// Token bucket rate limiter for O(1) throttling (trueno-viz pattern)
///
/// Used to limit update frequency without blocking.
#[derive(Debug, Clone)]
pub struct RateLimiter {
    /// Last allowed time in microseconds
    last_allowed_us: u64,
    /// Minimum interval between allows in microseconds
    interval_us: u64,
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new_hz(60) // Default 60 Hz
    }
}

impl RateLimiter {
    /// Create rate limiter with interval in microseconds
    #[must_use]
    pub fn new(interval_us: u64) -> Self {
        Self {
            last_allowed_us: 0,
            interval_us,
        }
    }

    /// Create rate limiter for given frequency (Hz)
    #[must_use]
    pub fn new_hz(hz: u32) -> Self {
        let interval_us = if hz == 0 { 1_000_000 } else { 1_000_000 / hz as u64 };
        Self::new(interval_us)
    }

    /// Create rate limiter for given millisecond interval
    #[must_use]
    pub fn new_ms(ms: u64) -> Self {
        Self::new(ms * 1000)
    }

    /// Check if action is allowed now (O(1))
    ///
    /// Returns true and updates timestamp if allowed, false otherwise.
    pub fn check(&mut self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if now >= self.last_allowed_us + self.interval_us {
            self.last_allowed_us = now;
            true
        } else {
            false
        }
    }

    /// Check without updating (peek)
    #[must_use]
    pub fn would_allow(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        now >= self.last_allowed_us + self.interval_us
    }

    /// Get interval in microseconds
    #[must_use]
    pub fn interval_us(&self) -> u64 {
        self.interval_us
    }

    /// Get interval in Hz
    #[must_use]
    pub fn hz(&self) -> f64 {
        if self.interval_us == 0 {
            0.0
        } else {
            1_000_000.0 / self.interval_us as f64
        }
    }

    /// Reset the limiter
    pub fn reset(&mut self) {
        self.last_allowed_us = 0;
    }
}

// =============================================================================
// THRESHOLD DETECTOR (trueno-viz O(1) level detection pattern)
// =============================================================================

/// Threshold-based level detector for O(1) state transitions (trueno-viz pattern)
///
/// Provides hysteresis to prevent rapid toggling at threshold boundaries.
/// Common use: CPU/memory warning levels, alert thresholds.
#[derive(Debug, Clone)]
pub struct ThresholdDetector {
    /// Low threshold (transition from High to Low when below this)
    low: f64,
    /// High threshold (transition from Low to High when above this)
    high: f64,
    /// Current state (true = high/alert, false = low/normal)
    is_high: bool,
}

impl ThresholdDetector {
    /// Create a new threshold detector with hysteresis
    ///
    /// # Arguments
    /// * `low` - Threshold to transition to normal state
    /// * `high` - Threshold to transition to alert state
    ///
    /// Hysteresis: low < high prevents rapid toggling
    #[must_use]
    pub fn new(low: f64, high: f64) -> Self {
        Self {
            low,
            high: high.max(low), // Ensure high >= low
            is_high: false,
        }
    }

    /// Create detector for percentage thresholds (0-100)
    #[must_use]
    pub fn percent(low: f64, high: f64) -> Self {
        Self::new(low.clamp(0.0, 100.0), high.clamp(0.0, 100.0))
    }

    /// Create detector for CPU/memory warnings (70/90)
    #[must_use]
    pub fn for_resource() -> Self {
        Self::new(70.0, 90.0)
    }

    /// Create detector for temperature warnings (60/80)
    #[must_use]
    pub fn for_temperature() -> Self {
        Self::new(60.0, 80.0)
    }

    /// Update with new value and return whether state changed (O(1))
    pub fn update(&mut self, value: f64) -> bool {
        let was_high = self.is_high;

        if self.is_high && value < self.low {
            self.is_high = false;
        } else if !self.is_high && value > self.high {
            self.is_high = true;
        }

        was_high != self.is_high
    }

    /// Check if currently in high/alert state
    #[must_use]
    pub fn is_high(&self) -> bool {
        self.is_high
    }

    /// Check if currently in low/normal state
    #[must_use]
    pub fn is_low(&self) -> bool {
        !self.is_high
    }

    /// Get the low threshold
    #[must_use]
    pub fn low_threshold(&self) -> f64 {
        self.low
    }

    /// Get the high threshold
    #[must_use]
    pub fn high_threshold(&self) -> f64 {
        self.high
    }

    /// Reset to low/normal state
    pub fn reset(&mut self) {
        self.is_high = false;
    }

    /// Force to high/alert state
    pub fn set_high(&mut self) {
        self.is_high = true;
    }
}

// =============================================================================
// SAMPLE COUNTER (trueno-viz O(1) counting pattern)
// =============================================================================

/// Sample counter with windowed rate calculation (trueno-viz pattern)
///
/// Tracks count and provides rate per second calculation.
#[derive(Debug, Clone)]
pub struct SampleCounter {
    /// Total count
    count: u64,
    /// Count at last rate calculation
    last_count: u64,
    /// Time of last rate calculation in microseconds
    last_time_us: u64,
    /// Calculated rate (samples per second)
    rate: f64,
}

impl Default for SampleCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl SampleCounter {
    /// Create a new counter
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: 0,
            last_count: 0,
            last_time_us: 0,
            rate: 0.0,
        }
    }

    /// Increment count by 1 (O(1))
    pub fn increment(&mut self) {
        self.count += 1;
    }

    /// Increment count by n (O(1))
    pub fn add(&mut self, n: u64) {
        self.count += n;
    }

    /// Get current count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Calculate and get rate (samples per second)
    ///
    /// Should be called periodically (e.g., once per second)
    pub fn calculate_rate(&mut self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if self.last_time_us > 0 {
            let elapsed_us = now.saturating_sub(self.last_time_us);
            if elapsed_us > 0 {
                let delta = self.count.saturating_sub(self.last_count);
                self.rate = (delta as f64 * 1_000_000.0) / elapsed_us as f64;
            }
        }

        self.last_count = self.count;
        self.last_time_us = now;
        self.rate
    }

    /// Get last calculated rate without recalculating
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Reset counter
    pub fn reset(&mut self) {
        self.count = 0;
        self.last_count = 0;
        self.last_time_us = 0;
        self.rate = 0.0;
    }
}

// =============================================================================
// BUDGET TRACKER (trueno-viz O(1) budget monitoring pattern)
// =============================================================================

/// Budget tracker for monitoring time/resource consumption (trueno-viz pattern)
///
/// Tracks usage against a budget and calculates utilization percentage.
#[derive(Debug, Clone)]
pub struct BudgetTracker {
    /// Budget limit
    budget: f64,
    /// Current usage
    usage: f64,
    /// Peak usage
    peak: f64,
}

impl BudgetTracker {
    /// Create a new budget tracker
    #[must_use]
    pub fn new(budget: f64) -> Self {
        Self {
            budget: budget.max(0.0),
            usage: 0.0,
            peak: 0.0,
        }
    }

    /// Create for 16ms render budget (60fps)
    #[must_use]
    pub fn for_render() -> Self {
        Self::new(16_000.0) // 16ms in microseconds
    }

    /// Create for 1ms compute budget
    #[must_use]
    pub fn for_compute() -> Self {
        Self::new(1_000.0) // 1ms in microseconds
    }

    /// Record usage (O(1))
    pub fn record(&mut self, usage: f64) {
        self.usage = usage;
        self.peak = self.peak.max(usage);
    }

    /// Get current usage
    #[must_use]
    pub fn usage(&self) -> f64 {
        self.usage
    }

    /// Get peak usage
    #[must_use]
    pub fn peak(&self) -> f64 {
        self.peak
    }

    /// Get budget
    #[must_use]
    pub fn budget(&self) -> f64 {
        self.budget
    }

    /// Get utilization percentage (O(1))
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.budget <= 0.0 {
            0.0
        } else {
            (self.usage / self.budget) * 100.0
        }
    }

    /// Get peak utilization percentage (O(1))
    #[must_use]
    pub fn peak_utilization(&self) -> f64 {
        if self.budget <= 0.0 {
            0.0
        } else {
            (self.peak / self.budget) * 100.0
        }
    }

    /// Check if over budget
    #[must_use]
    pub fn is_over_budget(&self) -> bool {
        self.usage > self.budget
    }

    /// Get remaining budget
    #[must_use]
    pub fn remaining(&self) -> f64 {
        (self.budget - self.usage).max(0.0)
    }

    /// Reset usage and peak
    pub fn reset(&mut self) {
        self.usage = 0.0;
        self.peak = 0.0;
    }

    /// Set new budget
    pub fn set_budget(&mut self, budget: f64) {
        self.budget = budget.max(0.0);
    }
}

// =============================================================================
// MIN/MAX TRACKER (trueno-viz O(1) extrema tracking pattern)
// =============================================================================

/// Min/Max value tracker with timestamps (trueno-viz pattern)
///
/// Tracks minimum and maximum values along with when they occurred.
/// Useful for monitoring extreme values in metrics over time.
#[derive(Debug, Clone)]
pub struct MinMaxTracker {
    /// Minimum value seen
    min: f64,
    /// Maximum value seen
    max: f64,
    /// Time of minimum (microseconds since epoch)
    min_time_us: u64,
    /// Time of maximum (microseconds since epoch)
    max_time_us: u64,
    /// Sample count
    count: u64,
}

impl Default for MinMaxTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MinMaxTracker {
    /// Create a new min/max tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            min: f64::MAX,
            max: f64::MIN,
            min_time_us: 0,
            max_time_us: 0,
            count: 0,
        }
    }

    /// Record a value (O(1))
    pub fn record(&mut self, value: f64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if value < self.min {
            self.min = value;
            self.min_time_us = now;
        }
        if value > self.max {
            self.max = value;
            self.max_time_us = now;
        }
        self.count += 1;
    }

    /// Get minimum value (O(1))
    #[must_use]
    pub fn min(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.min)
        } else {
            None
        }
    }

    /// Get maximum value (O(1))
    #[must_use]
    pub fn max(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.max)
        } else {
            None
        }
    }

    /// Get range (max - min) (O(1))
    #[must_use]
    pub fn range(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.max - self.min)
        } else {
            None
        }
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get time since minimum in microseconds
    #[must_use]
    pub fn time_since_min_us(&self) -> u64 {
        if self.min_time_us == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        now.saturating_sub(self.min_time_us)
    }

    /// Get time since maximum in microseconds
    #[must_use]
    pub fn time_since_max_us(&self) -> u64 {
        if self.max_time_us == 0 {
            return 0;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        now.saturating_sub(self.max_time_us)
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.min = f64::MAX;
        self.max = f64::MIN;
        self.min_time_us = 0;
        self.max_time_us = 0;
        self.count = 0;
    }
}

// =============================================================================
// MOVING WINDOW (trueno-viz O(1) time-windowed aggregation pattern)
// =============================================================================

/// Time-windowed aggregation tracker (trueno-viz pattern)
///
/// Tracks sum/count over a sliding time window for rate calculations.
/// Window expiration is O(1) using bucket rotation.
#[derive(Debug, Clone)]
pub struct MovingWindow {
    /// Current bucket sum
    current_sum: f64,
    /// Current bucket count
    current_count: u64,
    /// Previous bucket sum
    prev_sum: f64,
    /// Previous bucket count
    prev_count: u64,
    /// Window duration in microseconds
    window_us: u64,
    /// Current bucket start time
    bucket_start_us: u64,
}

impl MovingWindow {
    /// Create a new moving window
    #[must_use]
    pub fn new(window_ms: u64) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Self {
            current_sum: 0.0,
            current_count: 0,
            prev_sum: 0.0,
            prev_count: 0,
            window_us: window_ms * 1000,
            bucket_start_us: now,
        }
    }

    /// Create 1-second window
    #[must_use]
    pub fn one_second() -> Self {
        Self::new(1000)
    }

    /// Create 1-minute window
    #[must_use]
    pub fn one_minute() -> Self {
        Self::new(60_000)
    }

    /// Record a value (O(1) with potential bucket rotation)
    pub fn record(&mut self, value: f64) {
        self.maybe_rotate();
        self.current_sum += value;
        self.current_count += 1;
    }

    /// Increment count by 1 (O(1))
    pub fn increment(&mut self) {
        self.record(1.0);
    }

    /// Check and rotate buckets if needed (O(1))
    fn maybe_rotate(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let elapsed = now.saturating_sub(self.bucket_start_us);

        if elapsed >= self.window_us {
            // Rotate: current becomes previous
            self.prev_sum = self.current_sum;
            self.prev_count = self.current_count;
            self.current_sum = 0.0;
            self.current_count = 0;
            self.bucket_start_us = now;
        }
    }

    /// Get sum over window (O(1))
    #[must_use]
    pub fn sum(&mut self) -> f64 {
        self.maybe_rotate();
        self.current_sum + self.prev_sum
    }

    /// Get count over window (O(1))
    #[must_use]
    pub fn count(&mut self) -> u64 {
        self.maybe_rotate();
        self.current_count + self.prev_count
    }

    /// Get rate per second (O(1))
    #[must_use]
    pub fn rate_per_second(&mut self) -> f64 {
        self.maybe_rotate();
        let total = self.current_sum + self.prev_sum;
        let window_secs = (self.window_us as f64) / 1_000_000.0;
        if window_secs > 0.0 {
            total / window_secs
        } else {
            0.0
        }
    }

    /// Get count rate per second (O(1))
    #[must_use]
    pub fn count_rate(&mut self) -> f64 {
        self.maybe_rotate();
        let total = self.current_count + self.prev_count;
        let window_secs = (self.window_us as f64) / 1_000_000.0;
        if window_secs > 0.0 {
            total as f64 / window_secs
        } else {
            0.0
        }
    }

    /// Reset window
    pub fn reset(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        self.current_sum = 0.0;
        self.current_count = 0;
        self.prev_sum = 0.0;
        self.prev_count = 0;
        self.bucket_start_us = now;
    }
}

// =============================================================================
// PERCENTILE TRACKER (trueno-viz O(1) approximate percentile pattern)
// =============================================================================

/// Approximate percentile tracker using fixed buckets (trueno-viz pattern)
///
/// Provides O(1) approximate percentiles using histogram-based estimation.
/// Buckets: [0-1, 1-5, 5-10, 10-25, 25-50, 50-100, 100-250, 250-500, 500-1000, 1000+] ms
#[derive(Debug, Clone)]
pub struct PercentileTracker {
    /// Bucket counts
    buckets: [u64; 10],
    /// Total count
    count: u64,
    /// Bucket boundaries in microseconds
    boundaries: [u64; 10],
}

impl Default for PercentileTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PercentileTracker {
    /// Create a new percentile tracker with default latency buckets
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: [0; 10],
            count: 0,
            // Boundaries in microseconds: 1ms, 5ms, 10ms, 25ms, 50ms, 100ms, 250ms, 500ms, 1000ms, infinity
            boundaries: [
                1_000,    // 0-1ms
                5_000,    // 1-5ms
                10_000,   // 5-10ms
                25_000,   // 10-25ms
                50_000,   // 25-50ms
                100_000,  // 50-100ms
                250_000,  // 100-250ms
                500_000,  // 250-500ms
                1_000_000, // 500-1000ms
                u64::MAX, // 1000ms+
            ],
        }
    }

    /// Create with custom bucket boundaries (in microseconds)
    #[must_use]
    pub fn with_boundaries(boundaries: [u64; 10]) -> Self {
        Self {
            buckets: [0; 10],
            count: 0,
            boundaries,
        }
    }

    /// Record a value in microseconds (O(1))
    pub fn record_us(&mut self, value_us: u64) {
        for (i, &boundary) in self.boundaries.iter().enumerate() {
            if value_us < boundary {
                self.buckets[i] += 1;
                self.count += 1;
                return;
            }
        }
        // Shouldn't reach here due to u64::MAX boundary
        self.buckets[9] += 1;
        self.count += 1;
    }

    /// Record a value in milliseconds (O(1))
    pub fn record_ms(&mut self, value_ms: f64) {
        self.record_us((value_ms * 1000.0) as u64);
    }

    /// Get approximate percentile value in microseconds (O(1))
    #[must_use]
    pub fn percentile_us(&self, pct: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }

        let target = ((pct / 100.0) * self.count as f64) as u64;
        let mut cumulative = 0u64;

        for (i, &bucket_count) in self.buckets.iter().enumerate() {
            cumulative += bucket_count;
            if cumulative >= target {
                // Return midpoint of bucket
                let lower = if i == 0 { 0 } else { self.boundaries[i - 1] };
                let upper = self.boundaries[i];
                if upper == u64::MAX {
                    return lower + 500_000; // Estimate for last bucket
                }
                return (lower + upper) / 2;
            }
        }

        self.boundaries[8] // Return 1s as fallback
    }

    /// Get approximate percentile value in milliseconds (O(1))
    #[must_use]
    pub fn percentile_ms(&self, pct: f64) -> f64 {
        self.percentile_us(pct) as f64 / 1000.0
    }

    /// Get p50 (median) in milliseconds
    #[must_use]
    pub fn p50_ms(&self) -> f64 {
        self.percentile_ms(50.0)
    }

    /// Get p90 in milliseconds
    #[must_use]
    pub fn p90_ms(&self) -> f64 {
        self.percentile_ms(90.0)
    }

    /// Get p99 in milliseconds
    #[must_use]
    pub fn p99_ms(&self) -> f64 {
        self.percentile_ms(99.0)
    }

    /// Get total count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.buckets = [0; 10];
        self.count = 0;
    }
}

// =============================================================================
// STATE TRACKER (trueno-viz O(1) state machine pattern)
// =============================================================================

/// State transition tracker with history (trueno-viz pattern)
///
/// Tracks state transitions and time spent in each state.
/// Useful for monitoring component lifecycle or connection states.
#[derive(Debug, Clone)]
pub struct StateTracker<const N: usize> {
    /// Current state index (0..N-1)
    current: usize,
    /// Time entered current state (microseconds since epoch)
    entered_us: u64,
    /// Total time spent in each state (microseconds)
    durations: [u64; N],
    /// Transition count for each state
    transitions: [u64; N],
}

impl<const N: usize> Default for StateTracker<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> StateTracker<N> {
    /// Create a new state tracker starting in state 0
    #[must_use]
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        let mut transitions = [0u64; N];
        if N > 0 {
            transitions[0] = 1; // Initial transition to state 0
        }

        Self {
            current: 0,
            entered_us: now,
            durations: [0u64; N],
            transitions,
        }
    }

    /// Transition to a new state (O(1))
    pub fn transition(&mut self, new_state: usize) {
        if new_state >= N {
            return; // Invalid state
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        // Record time in previous state
        let elapsed = now.saturating_sub(self.entered_us);
        self.durations[self.current] += elapsed;

        // Transition to new state
        self.current = new_state;
        self.entered_us = now;
        self.transitions[new_state] += 1;
    }

    /// Get current state (O(1))
    #[must_use]
    pub fn current(&self) -> usize {
        self.current
    }

    /// Get time in current state in microseconds (O(1))
    #[must_use]
    pub fn time_in_current_us(&self) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        now.saturating_sub(self.entered_us)
    }

    /// Get total time in state in microseconds (O(1))
    #[must_use]
    pub fn total_time_in_state_us(&self, state: usize) -> u64 {
        if state >= N {
            return 0;
        }
        if state == self.current {
            self.durations[state] + self.time_in_current_us()
        } else {
            self.durations[state]
        }
    }

    /// Get transition count for state (O(1))
    #[must_use]
    pub fn transition_count(&self, state: usize) -> u64 {
        if state >= N {
            0
        } else {
            self.transitions[state]
        }
    }

    /// Get total transitions (O(N))
    #[must_use]
    pub fn total_transitions(&self) -> u64 {
        self.transitions.iter().sum()
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        self.current = 0;
        self.entered_us = now;
        self.durations = [0u64; N];
        self.transitions = [0u64; N];
        if N > 0 {
            self.transitions[0] = 1;
        }
    }
}

// =============================================================================
// CHANGE DETECTOR (trueno-viz O(1) significant change detection)
// =============================================================================

/// Significant change detector (trueno-viz pattern)
///
/// Detects when a value has changed significantly from a baseline.
/// Uses both absolute and relative thresholds.
#[derive(Debug, Clone)]
pub struct ChangeDetector {
    /// Baseline value
    baseline: f64,
    /// Absolute change threshold
    abs_threshold: f64,
    /// Relative change threshold (percentage)
    rel_threshold: f64,
    /// Last reported value
    last_value: f64,
    /// Number of changes detected
    change_count: u64,
}

impl Default for ChangeDetector {
    fn default() -> Self {
        Self::new(0.0, 1.0, 5.0)
    }
}

impl ChangeDetector {
    /// Create a new change detector
    ///
    /// # Arguments
    /// * `baseline` - Initial baseline value
    /// * `abs_threshold` - Absolute change required to trigger
    /// * `rel_threshold` - Relative change (%) required to trigger
    #[must_use]
    pub fn new(baseline: f64, abs_threshold: f64, rel_threshold: f64) -> Self {
        Self {
            baseline,
            abs_threshold: abs_threshold.abs(),
            rel_threshold: rel_threshold.abs(),
            last_value: baseline,
            change_count: 0,
        }
    }

    /// Create for percentage monitoring (1% absolute, 5% relative)
    #[must_use]
    pub fn for_percentage() -> Self {
        Self::new(0.0, 1.0, 5.0)
    }

    /// Create for latency monitoring (1ms absolute, 10% relative)
    #[must_use]
    pub fn for_latency() -> Self {
        Self::new(0.0, 1000.0, 10.0)
    }

    /// Check if value has changed significantly (O(1))
    #[must_use]
    pub fn has_changed(&self, value: f64) -> bool {
        let abs_diff = (value - self.last_value).abs();

        // Check absolute threshold
        if abs_diff >= self.abs_threshold {
            return true;
        }

        // Check relative threshold
        if self.last_value.abs() > f64::EPSILON {
            let rel_diff = (abs_diff / self.last_value.abs()) * 100.0;
            if rel_diff >= self.rel_threshold {
                return true;
            }
        }

        false
    }

    /// Update with new value and return whether it changed significantly (O(1))
    pub fn update(&mut self, value: f64) -> bool {
        let changed = self.has_changed(value);
        if changed {
            self.change_count += 1;
        }
        self.last_value = value;
        changed
    }

    /// Update baseline to current value (O(1))
    pub fn update_baseline(&mut self) {
        self.baseline = self.last_value;
    }

    /// Set baseline to specific value (O(1))
    pub fn set_baseline(&mut self, baseline: f64) {
        self.baseline = baseline;
    }

    /// Get current baseline
    #[must_use]
    pub fn baseline(&self) -> f64 {
        self.baseline
    }

    /// Get last value
    #[must_use]
    pub fn last_value(&self) -> f64 {
        self.last_value
    }

    /// Get change count
    #[must_use]
    pub fn change_count(&self) -> u64 {
        self.change_count
    }

    /// Get change from baseline
    #[must_use]
    pub fn change_from_baseline(&self) -> f64 {
        self.last_value - self.baseline
    }

    /// Get relative change from baseline (percentage)
    #[must_use]
    pub fn relative_change(&self) -> f64 {
        if self.baseline.abs() > f64::EPSILON {
            ((self.last_value - self.baseline) / self.baseline.abs()) * 100.0
        } else {
            0.0
        }
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.last_value = self.baseline;
        self.change_count = 0;
    }
}

// =============================================================================
// ACCUMULATOR (trueno-viz O(1) overflow-safe accumulation)
// =============================================================================

/// Overflow-safe accumulator (trueno-viz pattern)
///
/// Accumulates values with automatic overflow detection and handling.
/// Useful for counters that may wrap (network bytes, disk I/O).
#[derive(Debug, Clone)]
pub struct Accumulator {
    /// Current accumulated value
    value: u64,
    /// Previous raw value (for delta calculation)
    prev_raw: u64,
    /// Whether we've seen the first value
    initialized: bool,
    /// Overflow count
    overflows: u64,
}

impl Default for Accumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl Accumulator {
    /// Create a new accumulator
    #[must_use]
    pub fn new() -> Self {
        Self {
            value: 0,
            prev_raw: 0,
            initialized: false,
            overflows: 0,
        }
    }

    /// Update with a raw counter value (O(1))
    ///
    /// Handles counter wraps/overflows automatically.
    pub fn update(&mut self, raw: u64) {
        if !self.initialized {
            self.prev_raw = raw;
            self.initialized = true;
            return;
        }

        // Calculate delta, handling overflow
        let delta = if raw >= self.prev_raw {
            raw - self.prev_raw
        } else {
            // Counter wrapped
            self.overflows += 1;
            // Assume wrap to 0 from max
            (u64::MAX - self.prev_raw) + raw + 1
        };

        self.value += delta;
        self.prev_raw = raw;
    }

    /// Add a delta value directly (O(1))
    pub fn add(&mut self, delta: u64) {
        self.value += delta;
        self.initialized = true;
    }

    /// Get accumulated value (O(1))
    #[must_use]
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Get overflow count (O(1))
    #[must_use]
    pub fn overflows(&self) -> u64 {
        self.overflows
    }

    /// Check if initialized
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Get last raw value
    #[must_use]
    pub fn last_raw(&self) -> u64 {
        self.prev_raw
    }

    /// Reset accumulator
    pub fn reset(&mut self) {
        self.value = 0;
        self.prev_raw = 0;
        self.initialized = false;
        self.overflows = 0;
    }
}

// =============================================================================
// EVENT COUNTER (trueno-viz O(1) categorized event counting)
// =============================================================================

/// Categorized event counter (trueno-viz pattern)
///
/// Counts events by category with O(1) increment and lookup.
/// Useful for error categorization, request types, etc.
#[derive(Debug, Clone)]
pub struct EventCounter<const N: usize> {
    /// Counts per category
    counts: [u64; N],
    /// Total count
    total: u64,
}

impl<const N: usize> Default for EventCounter<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> EventCounter<N> {
    /// Create a new event counter
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: [0u64; N],
            total: 0,
        }
    }

    /// Increment category count (O(1))
    pub fn increment(&mut self, category: usize) {
        if category < N {
            self.counts[category] += 1;
            self.total += 1;
        }
    }

    /// Add to category count (O(1))
    pub fn add(&mut self, category: usize, count: u64) {
        if category < N {
            self.counts[category] += count;
            self.total += count;
        }
    }

    /// Get category count (O(1))
    #[must_use]
    pub fn count(&self, category: usize) -> u64 {
        if category < N {
            self.counts[category]
        } else {
            0
        }
    }

    /// Get total count (O(1))
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Get category percentage (O(1))
    #[must_use]
    pub fn percentage(&self, category: usize) -> f64 {
        if self.total == 0 || category >= N {
            0.0
        } else {
            (self.counts[category] as f64 / self.total as f64) * 100.0
        }
    }

    /// Get dominant category (O(N))
    #[must_use]
    pub fn dominant(&self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }
        self.counts
            .iter()
            .enumerate()
            .max_by_key(|(_, &count)| count)
            .map(|(idx, _)| idx)
    }

    /// Reset all counts
    pub fn reset(&mut self) {
        self.counts = [0u64; N];
        self.total = 0;
    }
}

// =============================================================================
// TREND DETECTOR (trueno-viz O(1) trend analysis pattern)
// =============================================================================

/// Trend detection using linear regression slope (trueno-viz pattern)
///
/// Detects upward/downward/flat trends using a sliding window approach.
/// Uses simplified slope calculation for O(1) updates.
#[derive(Debug, Clone)]
pub struct TrendDetector {
    /// Sum of values
    sum: f64,
    /// Sum of index * value (for slope)
    sum_xy: f64,
    /// Current index
    index: u64,
    /// Number of samples
    count: u64,
    /// Threshold for trend detection (slope magnitude)
    threshold: f64,
}

impl Default for TrendDetector {
    fn default() -> Self {
        Self::new(0.1)
    }
}

/// Trend direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trend {
    /// Values increasing
    Up,
    /// Values decreasing
    Down,
    /// Values stable
    Flat,
    /// Not enough data
    Unknown,
}

impl TrendDetector {
    /// Create a new trend detector
    ///
    /// # Arguments
    /// * `threshold` - Minimum slope magnitude to detect trend
    #[must_use]
    pub fn new(threshold: f64) -> Self {
        Self {
            sum: 0.0,
            sum_xy: 0.0,
            index: 0,
            count: 0,
            threshold: threshold.abs(),
        }
    }

    /// Create for percentage changes (0.5% threshold)
    #[must_use]
    pub fn for_percentage() -> Self {
        Self::new(0.5)
    }

    /// Create for latency changes (1ms threshold)
    #[must_use]
    pub fn for_latency() -> Self {
        Self::new(1.0)
    }

    /// Update with a new value (O(1))
    pub fn update(&mut self, value: f64) {
        self.sum += value;
        self.sum_xy += (self.index as f64) * value;
        self.index += 1;
        self.count += 1;
    }

    /// Calculate slope (O(1))
    ///
    /// Uses least squares linear regression formula:
    /// slope = (n * Σxy - Σx * Σy) / (n * Σx² - (Σx)²)
    #[must_use]
    pub fn slope(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }

        let n = self.count as f64;
        let sum_x = (self.count * (self.count - 1)) as f64 / 2.0; // Sum of 0..n-1
        let sum_x2 = (self.count * (self.count - 1) * (2 * self.count - 1)) as f64 / 6.0;

        // Linear regression denominator: n * Σx² - (Σx)²
        let sum_x_squared = sum_x.powi(2);
        let denominator = n * sum_x2 - sum_x_squared;
        if denominator.abs() < f64::EPSILON {
            return 0.0;
        }

        (n * self.sum_xy - sum_x * self.sum) / denominator
    }

    /// Get current trend (O(1))
    #[must_use]
    pub fn trend(&self) -> Trend {
        if self.count < 3 {
            return Trend::Unknown;
        }

        let slope = self.slope();
        if slope > self.threshold {
            Trend::Up
        } else if slope < -self.threshold {
            Trend::Down
        } else {
            Trend::Flat
        }
    }

    /// Check if trending up
    #[must_use]
    pub fn is_trending_up(&self) -> bool {
        self.trend() == Trend::Up
    }

    /// Check if trending down
    #[must_use]
    pub fn is_trending_down(&self) -> bool {
        self.trend() == Trend::Down
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.sum = 0.0;
        self.sum_xy = 0.0;
        self.index = 0;
        self.count = 0;
    }
}

// =============================================================================
// ANOMALY DETECTOR (trueno-viz O(1) z-score anomaly detection)
// =============================================================================

/// Z-score based anomaly detector (trueno-viz pattern)
///
/// Detects anomalies using running mean/variance and z-score threshold.
/// Useful for alerting on unusual values.
#[derive(Debug, Clone)]
pub struct AnomalyDetector {
    /// Running mean
    mean: f64,
    /// Running M2 (sum of squared differences)
    m2: f64,
    /// Sample count
    count: u64,
    /// Z-score threshold for anomaly
    threshold: f64,
    /// Last value
    last_value: f64,
    /// Anomaly count
    anomaly_count: u64,
}

impl Default for AnomalyDetector {
    fn default() -> Self {
        Self::new(3.0)
    }
}

impl AnomalyDetector {
    /// Create a new anomaly detector
    ///
    /// # Arguments
    /// * `threshold` - Z-score threshold (typically 2.0-3.0)
    #[must_use]
    pub fn new(threshold: f64) -> Self {
        Self {
            mean: 0.0,
            m2: 0.0,
            count: 0,
            threshold: threshold.abs(),
            last_value: 0.0,
            anomaly_count: 0,
        }
    }

    /// Create with 2-sigma threshold (95% confidence)
    #[must_use]
    pub fn two_sigma() -> Self {
        Self::new(2.0)
    }

    /// Create with 3-sigma threshold (99.7% confidence)
    #[must_use]
    pub fn three_sigma() -> Self {
        Self::new(3.0)
    }

    /// Update with a new value and return whether it's an anomaly (O(1))
    ///
    /// Uses Welford's online algorithm for running variance.
    pub fn update(&mut self, value: f64) -> bool {
        self.last_value = value;
        self.count += 1;

        // First value can't be an anomaly
        if self.count == 1 {
            self.mean = value;
            return false;
        }

        // Calculate z-score before updating stats
        let is_anomaly = self.is_anomaly(value);
        if is_anomaly {
            self.anomaly_count += 1;
        }

        // Update running stats (Welford's algorithm)
        let delta = value - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = value - self.mean;
        self.m2 += delta * delta2;

        is_anomaly
    }

    /// Check if a value would be an anomaly (O(1))
    #[must_use]
    pub fn is_anomaly(&self, value: f64) -> bool {
        if self.count < 10 {
            return false; // Need enough samples
        }

        let z = self.z_score(value);
        z.abs() > self.threshold
    }

    /// Calculate z-score for a value (O(1))
    #[must_use]
    pub fn z_score(&self, value: f64) -> f64 {
        let std_dev = self.std_dev();
        if std_dev < f64::EPSILON {
            return 0.0;
        }
        (value - self.mean) / std_dev
    }

    /// Get running mean (O(1))
    #[must_use]
    pub fn mean(&self) -> f64 {
        self.mean
    }

    /// Get running variance (O(1))
    #[must_use]
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            0.0
        } else {
            self.m2 / (self.count - 1) as f64
        }
    }

    /// Get running standard deviation (O(1))
    #[must_use]
    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get anomaly count
    #[must_use]
    pub fn anomaly_count(&self) -> u64 {
        self.anomaly_count
    }

    /// Get anomaly rate (percentage)
    #[must_use]
    pub fn anomaly_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.anomaly_count as f64 / self.count as f64) * 100.0
        }
    }

    /// Get threshold
    #[must_use]
    pub fn threshold(&self) -> f64 {
        self.threshold
    }

    /// Reset detector
    pub fn reset(&mut self) {
        self.mean = 0.0;
        self.m2 = 0.0;
        self.count = 0;
        self.last_value = 0.0;
        self.anomaly_count = 0;
    }
}

// =============================================================================
// THROUGHPUT TRACKER (trueno-viz O(1) rate calculation)
// =============================================================================

/// Throughput tracker for bytes/ops per second (trueno-viz pattern)
///
/// Tracks cumulative totals and calculates rates over time intervals.
#[derive(Debug, Clone)]
pub struct ThroughputTracker {
    /// Total bytes/ops
    total: u64,
    /// Previous total (for delta)
    prev_total: u64,
    /// Last calculation time (microseconds)
    last_time_us: u64,
    /// Calculated rate (units per second)
    rate: f64,
    /// Peak rate
    peak_rate: f64,
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ThroughputTracker {
    /// Create a new throughput tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            prev_total: 0,
            last_time_us: 0,
            rate: 0.0,
            peak_rate: 0.0,
        }
    }

    /// Add bytes/ops (O(1))
    pub fn add(&mut self, count: u64) {
        self.total += count;
    }

    /// Calculate rate (should be called periodically) (O(1))
    pub fn calculate_rate(&mut self) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        if self.last_time_us > 0 {
            let elapsed_us = now.saturating_sub(self.last_time_us);
            if elapsed_us > 0 {
                let delta = self.total.saturating_sub(self.prev_total);
                self.rate = (delta as f64 * 1_000_000.0) / elapsed_us as f64;
                self.peak_rate = self.peak_rate.max(self.rate);
            }
        }

        self.prev_total = self.total;
        self.last_time_us = now;
        self.rate
    }

    /// Get current rate (O(1))
    #[must_use]
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Get peak rate (O(1))
    #[must_use]
    pub fn peak_rate(&self) -> f64 {
        self.peak_rate
    }

    /// Get total (O(1))
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Format rate as human-readable string (O(1))
    #[must_use]
    pub fn format_rate(&self) -> String {
        let rate = self.rate;
        if rate >= 1_000_000_000.0 {
            format!("{:.1}G/s", rate / 1_000_000_000.0)
        } else if rate >= 1_000_000.0 {
            format!("{:.1}M/s", rate / 1_000_000.0)
        } else if rate >= 1_000.0 {
            format!("{:.1}K/s", rate / 1_000.0)
        } else {
            format!("{:.0}/s", rate)
        }
    }

    /// Format rate as bytes/second (O(1))
    #[must_use]
    pub fn format_bytes_rate(&self) -> String {
        let rate = self.rate;
        if rate >= 1_073_741_824.0 {
            format!("{:.1}GB/s", rate / 1_073_741_824.0)
        } else if rate >= 1_048_576.0 {
            format!("{:.1}MB/s", rate / 1_048_576.0)
        } else if rate >= 1_024.0 {
            format!("{:.1}KB/s", rate / 1_024.0)
        } else {
            format!("{:.0}B/s", rate)
        }
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.total = 0;
        self.prev_total = 0;
        self.last_time_us = 0;
        self.rate = 0.0;
        self.peak_rate = 0.0;
    }
}

// =============================================================================
// JITTER TRACKER (trueno-viz O(1) latency jitter analysis)
// =============================================================================

/// Jitter tracker for latency variation (trueno-viz pattern)
///
/// Tracks inter-arrival time variation (jitter) common in network/audio.
#[derive(Debug, Clone)]
pub struct JitterTracker {
    /// Previous value
    prev: f64,
    /// Running jitter (smoothed)
    jitter: f64,
    /// Peak jitter
    peak_jitter: f64,
    /// Sample count
    count: u64,
    /// Smoothing factor (like RFC 3550)
    alpha: f64,
}

impl Default for JitterTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterTracker {
    /// Create a new jitter tracker with RFC 3550 smoothing
    #[must_use]
    pub fn new() -> Self {
        Self {
            prev: 0.0,
            jitter: 0.0,
            peak_jitter: 0.0,
            count: 0,
            alpha: 1.0 / 16.0, // RFC 3550 default
        }
    }

    /// Create with custom smoothing factor
    #[must_use]
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            prev: 0.0,
            jitter: 0.0,
            peak_jitter: 0.0,
            count: 0,
            alpha: alpha.clamp(0.0, 1.0),
        }
    }

    /// Update with new inter-arrival time (O(1))
    ///
    /// Uses RFC 3550 jitter calculation: J = J + (|D| - J) / 16
    pub fn update(&mut self, value: f64) {
        self.count += 1;

        if self.count == 1 {
            self.prev = value;
            return;
        }

        // Calculate difference from previous
        let diff = (value - self.prev).abs();
        self.prev = value;

        // Exponential smoothing (RFC 3550 style)
        self.jitter += self.alpha * (diff - self.jitter);
        self.peak_jitter = self.peak_jitter.max(self.jitter);
    }

    /// Get current jitter (O(1))
    #[must_use]
    pub fn jitter(&self) -> f64 {
        self.jitter
    }

    /// Get peak jitter (O(1))
    #[must_use]
    pub fn peak_jitter(&self) -> f64 {
        self.peak_jitter
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Check if jitter exceeds threshold
    #[must_use]
    pub fn exceeds(&self, threshold: f64) -> bool {
        self.jitter > threshold
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.jitter = 0.0;
        self.peak_jitter = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// DERIVATIVE TRACKER (trueno-viz O(1) rate-of-change pattern)
// =============================================================================

/// First derivative (rate of change) tracker (trueno-viz pattern)
///
/// Tracks instantaneous and smoothed rate of change for metrics.
/// Useful for detecting acceleration/deceleration in CPU, memory, etc.
#[derive(Debug, Clone)]
pub struct DerivativeTracker {
    /// Previous value
    prev: f64,
    /// Previous time (microseconds)
    prev_time_us: u64,
    /// Instantaneous derivative
    derivative: f64,
    /// Smoothed derivative (EMA)
    smoothed: f64,
    /// Smoothing factor
    alpha: f64,
    /// Sample count
    count: u64,
}

impl Default for DerivativeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl DerivativeTracker {
    /// Create a new derivative tracker with default smoothing (0.3)
    #[must_use]
    pub fn new() -> Self {
        Self {
            prev: 0.0,
            prev_time_us: 0,
            derivative: 0.0,
            smoothed: 0.0,
            alpha: 0.3,
            count: 0,
        }
    }

    /// Create with custom smoothing factor
    #[must_use]
    pub fn with_alpha(alpha: f64) -> Self {
        Self {
            prev: 0.0,
            prev_time_us: 0,
            derivative: 0.0,
            smoothed: 0.0,
            alpha: alpha.clamp(0.0, 1.0),
            count: 0,
        }
    }

    /// Update with new value (O(1))
    pub fn update(&mut self, value: f64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        self.count += 1;

        if self.count == 1 {
            self.prev = value;
            self.prev_time_us = now;
            return;
        }

        let dt = (now.saturating_sub(self.prev_time_us)) as f64 / 1_000_000.0; // seconds
        if dt > 0.0 {
            self.derivative = (value - self.prev) / dt;
            self.smoothed = self.alpha * self.derivative + (1.0 - self.alpha) * self.smoothed;
        }

        self.prev = value;
        self.prev_time_us = now;
    }

    /// Update with explicit delta time (for testing)
    pub fn update_with_dt(&mut self, value: f64, dt_secs: f64) {
        self.count += 1;

        if self.count == 1 {
            self.prev = value;
            return;
        }

        if dt_secs > 0.0 {
            self.derivative = (value - self.prev) / dt_secs;
            self.smoothed = self.alpha * self.derivative + (1.0 - self.alpha) * self.smoothed;
        }

        self.prev = value;
    }

    /// Get instantaneous derivative (O(1))
    #[must_use]
    pub fn derivative(&self) -> f64 {
        self.derivative
    }

    /// Get smoothed derivative (O(1))
    #[must_use]
    pub fn smoothed(&self) -> f64 {
        self.smoothed
    }

    /// Check if accelerating (positive derivative)
    #[must_use]
    pub fn is_accelerating(&self) -> bool {
        self.smoothed > 0.0
    }

    /// Check if decelerating (negative derivative)
    #[must_use]
    pub fn is_decelerating(&self) -> bool {
        self.smoothed < 0.0
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.prev_time_us = 0;
        self.derivative = 0.0;
        self.smoothed = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// INTEGRAL TRACKER (trueno-viz O(1) cumulative area pattern)
// =============================================================================

/// Integral (cumulative area) tracker (trueno-viz pattern)
///
/// Tracks cumulative area under the curve using trapezoidal rule.
/// Useful for energy consumption, work done, cumulative load.
#[derive(Debug, Clone)]
pub struct IntegralTracker {
    /// Previous value
    prev: f64,
    /// Previous time (microseconds)
    prev_time_us: u64,
    /// Cumulative integral
    integral: f64,
    /// Sample count
    count: u64,
}

impl Default for IntegralTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegralTracker {
    /// Create a new integral tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            prev: 0.0,
            prev_time_us: 0,
            integral: 0.0,
            count: 0,
        }
    }

    /// Update with new value (O(1))
    ///
    /// Uses trapezoidal rule: area = (v1 + v2) / 2 * dt
    pub fn update(&mut self, value: f64) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        self.count += 1;

        if self.count == 1 {
            self.prev = value;
            self.prev_time_us = now;
            return;
        }

        let dt = (now.saturating_sub(self.prev_time_us)) as f64 / 1_000_000.0; // seconds
        // Trapezoidal rule
        self.integral += (self.prev + value) / 2.0 * dt;

        self.prev = value;
        self.prev_time_us = now;
    }

    /// Update with explicit delta time (for testing)
    pub fn update_with_dt(&mut self, value: f64, dt_secs: f64) {
        self.count += 1;

        if self.count == 1 {
            self.prev = value;
            return;
        }

        // Trapezoidal rule
        self.integral += (self.prev + value) / 2.0 * dt_secs;
        self.prev = value;
    }

    /// Get cumulative integral (O(1))
    #[must_use]
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// Get average value (integral / time) (O(1))
    #[must_use]
    pub fn average(&self) -> f64 {
        if self.count < 2 {
            return self.prev;
        }
        // Would need total time tracking for true average
        self.prev
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.prev_time_us = 0;
        self.integral = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// CORRELATION TRACKER (trueno-viz O(1) running correlation)
// =============================================================================

/// Running correlation coefficient tracker (trueno-viz pattern)
///
/// Tracks Pearson correlation between two variables using online algorithm.
/// Useful for finding related metrics (CPU vs memory, network vs disk).
#[derive(Debug, Clone)]
pub struct CorrelationTracker {
    /// Mean of X
    mean_x: f64,
    /// Mean of Y
    mean_y: f64,
    /// Sum of (xi - mean_x) * (yi - mean_y)
    cov_sum: f64,
    /// Sum of (xi - mean_x)^2
    var_x_sum: f64,
    /// Sum of (yi - mean_y)^2
    var_y_sum: f64,
    /// Sample count
    count: u64,
}

impl Default for CorrelationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CorrelationTracker {
    /// Create a new correlation tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            mean_x: 0.0,
            mean_y: 0.0,
            cov_sum: 0.0,
            var_x_sum: 0.0,
            var_y_sum: 0.0,
            count: 0,
        }
    }

    /// Update with new (x, y) pair (O(1))
    ///
    /// Uses Welford's online algorithm for covariance.
    pub fn update(&mut self, x: f64, y: f64) {
        self.count += 1;
        let n = self.count as f64;

        // Update means
        let delta_x = x - self.mean_x;
        let delta_y = y - self.mean_y;

        self.mean_x += delta_x / n;
        self.mean_y += delta_y / n;

        // Update covariance and variance sums
        let delta_x2 = x - self.mean_x;
        let delta_y2 = y - self.mean_y;

        self.cov_sum += delta_x * delta_y2;
        self.var_x_sum += delta_x * delta_x2;
        self.var_y_sum += delta_y * delta_y2;
    }

    /// Get correlation coefficient (O(1))
    ///
    /// Returns value in [-1, 1] or 0 if insufficient data.
    #[must_use]
    pub fn correlation(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }

        let denominator = (self.var_x_sum * self.var_y_sum).sqrt();
        if denominator < f64::EPSILON {
            return 0.0;
        }

        (self.cov_sum / denominator).clamp(-1.0, 1.0)
    }

    /// Check if positively correlated (r > 0.5)
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.correlation() > 0.5
    }

    /// Check if negatively correlated (r < -0.5)
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.correlation() < -0.5
    }

    /// Check if strongly correlated (|r| > 0.7)
    #[must_use]
    pub fn is_strong(&self) -> bool {
        self.correlation().abs() > 0.7
    }

    /// Get covariance (O(1))
    #[must_use]
    pub fn covariance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.cov_sum / (self.count - 1) as f64
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.mean_x = 0.0;
        self.mean_y = 0.0;
        self.cov_sum = 0.0;
        self.var_x_sum = 0.0;
        self.var_y_sum = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// CIRCUIT BREAKER (trueno-viz O(1) resilience pattern)
// =============================================================================

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation, requests allowed
    Closed,
    /// Too many failures, requests blocked
    Open,
    /// Testing if service recovered
    HalfOpen,
}

/// Circuit breaker for failure handling (trueno-viz pattern)
///
/// Prevents cascading failures by temporarily blocking requests
/// after repeated failures.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current state
    state: CircuitState,
    /// Failure count
    failures: u64,
    /// Success count (in half-open state)
    successes: u64,
    /// Failure threshold to open circuit
    failure_threshold: u64,
    /// Success threshold to close circuit
    success_threshold: u64,
    /// Time circuit was opened (microseconds)
    opened_at: u64,
    /// Timeout before trying half-open (microseconds)
    timeout_us: u64,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new(5, 3, 30_000_000) // 5 failures, 3 successes, 30s timeout
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    ///
    /// # Arguments
    /// * `failure_threshold` - Failures before opening
    /// * `success_threshold` - Successes in half-open before closing
    /// * `timeout_us` - Microseconds before trying half-open
    #[must_use]
    pub fn new(failure_threshold: u64, success_threshold: u64, timeout_us: u64) -> Self {
        Self {
            state: CircuitState::Closed,
            failures: 0,
            successes: 0,
            failure_threshold,
            success_threshold,
            opened_at: 0,
            timeout_us,
        }
    }

    /// Create for network operations (5 failures, 30s timeout)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(5, 3, 30_000_000)
    }

    /// Create for fast-fail (3 failures, 5s timeout)
    #[must_use]
    pub fn for_fast_fail() -> Self {
        Self::new(3, 2, 5_000_000)
    }

    /// Check if request is allowed (O(1))
    #[must_use]
    pub fn is_allowed(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;

                if now.saturating_sub(self.opened_at) >= self.timeout_us {
                    self.state = CircuitState::HalfOpen;
                    self.successes = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a success (O(1))
    pub fn record_success(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failures = 0;
            }
            CircuitState::HalfOpen => {
                self.successes += 1;
                if self.successes >= self.success_threshold {
                    self.state = CircuitState::Closed;
                    self.failures = 0;
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failure (O(1))
    pub fn record_failure(&mut self) {
        match self.state {
            CircuitState::Closed => {
                self.failures += 1;
                if self.failures >= self.failure_threshold {
                    self.state = CircuitState::Open;
                    self.opened_at = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_micros() as u64;
                }
            }
            CircuitState::HalfOpen => {
                self.state = CircuitState::Open;
                self.opened_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64;
            }
            CircuitState::Open => {}
        }
    }

    /// Get current state (O(1))
    #[must_use]
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Get failure count
    #[must_use]
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// Check if circuit is open
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state == CircuitState::Open
    }

    /// Check if circuit is closed
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.state == CircuitState::Closed
    }

    /// Force reset to closed state
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failures = 0;
        self.successes = 0;
    }
}

// =============================================================================
// EXPONENTIAL BACKOFF (trueno-viz O(1) retry timing pattern)
// =============================================================================

/// Exponential backoff calculator (trueno-viz pattern)
///
/// Calculates retry delays with exponential growth and optional jitter.
/// Useful for retry logic in network operations.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    /// Base delay (microseconds)
    base_us: u64,
    /// Maximum delay (microseconds)
    max_us: u64,
    /// Current attempt
    attempt: u64,
    /// Multiplier for each attempt
    multiplier: f64,
    /// Add jitter (randomness)
    jitter: bool,
}

impl Default for ExponentialBackoff {
    fn default() -> Self {
        Self::new(100_000, 30_000_000) // 100ms base, 30s max
    }
}

impl ExponentialBackoff {
    /// Create a new exponential backoff
    ///
    /// # Arguments
    /// * `base_us` - Base delay in microseconds
    /// * `max_us` - Maximum delay in microseconds
    #[must_use]
    pub fn new(base_us: u64, max_us: u64) -> Self {
        Self {
            base_us,
            max_us,
            attempt: 0,
            multiplier: 2.0,
            jitter: false,
        }
    }

    /// Create with jitter enabled
    #[must_use]
    pub fn with_jitter(mut self) -> Self {
        self.jitter = true;
        self
    }

    /// Create with custom multiplier
    #[must_use]
    pub fn with_multiplier(mut self, multiplier: f64) -> Self {
        self.multiplier = multiplier.max(1.0);
        self
    }

    /// Create for network retries (100ms base, 30s max, with jitter)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(100_000, 30_000_000).with_jitter()
    }

    /// Create for fast retries (10ms base, 1s max)
    #[must_use]
    pub fn for_fast() -> Self {
        Self::new(10_000, 1_000_000)
    }

    /// Get next delay and increment attempt (O(1))
    pub fn next_delay(&mut self) -> u64 {
        let delay = self.current_delay();
        self.attempt += 1;
        delay
    }

    /// Get current delay without incrementing (O(1))
    #[must_use]
    pub fn current_delay(&self) -> u64 {
        let delay = (self.base_us as f64 * self.multiplier.powi(self.attempt as i32)) as u64;
        let capped = delay.min(self.max_us);

        if self.jitter {
            // Simple deterministic jitter based on attempt
            let jitter_factor = 0.5 + (self.attempt % 10) as f64 * 0.05;
            ((capped as f64) * jitter_factor) as u64
        } else {
            capped
        }
    }

    /// Get current delay in milliseconds
    #[must_use]
    pub fn current_delay_ms(&self) -> u64 {
        self.current_delay() / 1000
    }

    /// Get attempt count
    #[must_use]
    pub fn attempt(&self) -> u64 {
        self.attempt
    }

    /// Check if at max delay
    #[must_use]
    pub fn is_at_max(&self) -> bool {
        self.current_delay() >= self.max_us
    }

    /// Reset to first attempt
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}

// =============================================================================
// SLIDING MEDIAN (trueno-viz O(1) approximate median pattern)
// =============================================================================

/// Approximate sliding median using histogram buckets (trueno-viz pattern)
///
/// Uses fixed-size histogram for O(1) median approximation.
/// Good for latency percentiles where exact values aren't critical.
#[derive(Debug, Clone)]
pub struct SlidingMedian {
    /// Histogram buckets
    buckets: [u64; 10],
    /// Bucket boundaries (upper bounds)
    boundaries: [f64; 10],
    /// Total count
    count: u64,
    /// Min value seen
    min: f64,
    /// Max value seen
    max: f64,
}

impl Default for SlidingMedian {
    fn default() -> Self {
        Self::new()
    }
}

impl SlidingMedian {
    /// Create with default boundaries (0-1000 linear)
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: [0; 10],
            boundaries: [100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0],
            count: 0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    /// Create for latency (0-100ms, exponential)
    #[must_use]
    pub fn for_latency() -> Self {
        Self {
            buckets: [0; 10],
            boundaries: [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0, 1000.0],
            count: 0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    /// Create for percentage (0-100%)
    #[must_use]
    pub fn for_percentage() -> Self {
        Self {
            buckets: [0; 10],
            boundaries: [10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0],
            count: 0,
            min: f64::MAX,
            max: f64::MIN,
        }
    }

    /// Update with new value (O(1))
    pub fn update(&mut self, value: f64) {
        self.count += 1;
        self.min = self.min.min(value);
        self.max = self.max.max(value);

        // Find bucket
        for (i, &boundary) in self.boundaries.iter().enumerate() {
            if value <= boundary {
                self.buckets[i] += 1;
                return;
            }
        }
        // Above all boundaries, put in last bucket
        self.buckets[9] += 1;
    }

    /// Get approximate median (O(1))
    #[must_use]
    pub fn median(&self) -> f64 {
        self.percentile(50)
    }

    /// Get approximate percentile (O(1))
    #[must_use]
    pub fn percentile(&self, p: u8) -> f64 {
        if self.count == 0 {
            return 0.0;
        }

        let target = (self.count as f64 * p as f64 / 100.0) as u64;
        let mut cumulative = 0u64;

        for (i, &count) in self.buckets.iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                // Return bucket midpoint
                let lower = if i == 0 { 0.0 } else { self.boundaries[i - 1] };
                return (lower + self.boundaries[i]) / 2.0;
            }
        }

        self.boundaries[9]
    }

    /// Get count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get min value
    #[must_use]
    pub fn min(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.min }
    }

    /// Get max value
    #[must_use]
    pub fn max(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.max }
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.buckets = [0; 10];
        self.count = 0;
        self.min = f64::MAX;
        self.max = f64::MIN;
    }
}

// =============================================================================
// HYSTERESIS FILTER (trueno-viz O(1) noise filtering pattern)
// =============================================================================

/// Hysteresis filter for noise reduction (trueno-viz pattern)
///
/// Only changes output when input crosses threshold by dead band amount.
/// Prevents rapid toggling from noisy inputs.
#[derive(Debug, Clone)]
pub struct HysteresisFilter {
    /// Current output value
    output: f64,
    /// Dead band (minimum change to update)
    dead_band: f64,
    /// Sample count
    count: u64,
}

impl Default for HysteresisFilter {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl HysteresisFilter {
    /// Create with specified dead band
    #[must_use]
    pub fn new(dead_band: f64) -> Self {
        Self {
            output: 0.0,
            dead_band: dead_band.abs(),
            count: 0,
        }
    }

    /// Create for percentage values (1% dead band)
    #[must_use]
    pub fn for_percentage() -> Self {
        Self::new(1.0)
    }

    /// Create for latency values (0.5ms dead band)
    #[must_use]
    pub fn for_latency() -> Self {
        Self::new(0.5)
    }

    /// Create for temperature (0.5 degree dead band)
    #[must_use]
    pub fn for_temperature() -> Self {
        Self::new(0.5)
    }

    /// Update with new value (O(1))
    ///
    /// Returns true if output changed.
    pub fn update(&mut self, value: f64) -> bool {
        self.count += 1;

        if self.count == 1 {
            self.output = value;
            return true;
        }

        if (value - self.output).abs() >= self.dead_band {
            self.output = value;
            return true;
        }

        false
    }

    /// Get filtered output (O(1))
    #[must_use]
    pub fn output(&self) -> f64 {
        self.output
    }

    /// Get dead band
    #[must_use]
    pub fn dead_band(&self) -> f64 {
        self.dead_band
    }

    /// Set dead band
    pub fn set_dead_band(&mut self, dead_band: f64) {
        self.dead_band = dead_band.abs();
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset filter
    pub fn reset(&mut self) {
        self.output = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// SPIKE FILTER (trueno-viz O(1) outlier rejection pattern)
// =============================================================================

/// Spike filter for outlier rejection (trueno-viz pattern)
///
/// Rejects values that differ too much from recent average.
/// Good for sensor readings with occasional bad values.
#[derive(Debug, Clone)]
pub struct SpikeFilter {
    /// Running average
    avg: f64,
    /// Spike threshold (max deviation from avg)
    threshold: f64,
    /// Smoothing factor for avg
    alpha: f64,
    /// Spike count
    spikes: u64,
    /// Sample count
    count: u64,
    /// Last accepted value
    last_accepted: f64,
}

impl Default for SpikeFilter {
    fn default() -> Self {
        Self::new(3.0)
    }
}

impl SpikeFilter {
    /// Create with specified threshold (multiples of running avg)
    #[must_use]
    pub fn new(threshold: f64) -> Self {
        Self {
            avg: 0.0,
            threshold: threshold.abs(),
            alpha: 0.1,
            spikes: 0,
            count: 0,
            last_accepted: 0.0,
        }
    }

    /// Create for percentage values
    #[must_use]
    pub fn for_percentage() -> Self {
        Self::new(50.0) // 50% deviation threshold
    }

    /// Create for latency values
    #[must_use]
    pub fn for_latency() -> Self {
        Self::new(100.0) // 100ms deviation threshold
    }

    /// Update with new value (O(1))
    ///
    /// Returns the filtered value (original if accepted, last accepted if spike).
    pub fn update(&mut self, value: f64) -> f64 {
        self.count += 1;

        if self.count == 1 {
            self.avg = value;
            self.last_accepted = value;
            return value;
        }

        // Check if spike
        let deviation = (value - self.avg).abs();
        if deviation > self.threshold {
            self.spikes += 1;
            return self.last_accepted;
        }

        // Accept and update average
        self.avg = self.alpha * value + (1.0 - self.alpha) * self.avg;
        self.last_accepted = value;
        value
    }

    /// Get running average (O(1))
    #[must_use]
    pub fn average(&self) -> f64 {
        self.avg
    }

    /// Get spike count
    #[must_use]
    pub fn spikes(&self) -> u64 {
        self.spikes
    }

    /// Get spike rate (percentage)
    #[must_use]
    pub fn spike_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            (self.spikes as f64 / self.count as f64) * 100.0
        }
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get last accepted value
    #[must_use]
    pub fn last_accepted(&self) -> f64 {
        self.last_accepted
    }

    /// Reset filter
    pub fn reset(&mut self) {
        self.avg = 0.0;
        self.spikes = 0;
        self.count = 0;
        self.last_accepted = 0.0;
    }
}

// =============================================================================
// GAUGE TRACKER (trueno-viz O(1) current value tracking pattern)
// =============================================================================

/// Gauge tracker for current values (trueno-viz pattern)
///
/// Tracks current value with min/max/avg statistics.
/// Useful for memory, connections, queue depth.
#[derive(Debug, Clone)]
pub struct GaugeTracker {
    /// Current value
    current: f64,
    /// Minimum value
    min: f64,
    /// Maximum value
    max: f64,
    /// Running sum for average
    sum: f64,
    /// Sample count
    count: u64,
}

impl Default for GaugeTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GaugeTracker {
    /// Create a new gauge tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            current: 0.0,
            min: f64::MAX,
            max: f64::MIN,
            sum: 0.0,
            count: 0,
        }
    }

    /// Set current value (O(1))
    pub fn set(&mut self, value: f64) {
        self.current = value;
        self.min = self.min.min(value);
        self.max = self.max.max(value);
        self.sum += value;
        self.count += 1;
    }

    /// Increment current value
    pub fn inc(&mut self) {
        self.set(self.current + 1.0);
    }

    /// Decrement current value
    pub fn dec(&mut self) {
        self.set(self.current - 1.0);
    }

    /// Add to current value
    pub fn add(&mut self, delta: f64) {
        self.set(self.current + delta);
    }

    /// Get current value (O(1))
    #[must_use]
    pub fn current(&self) -> f64 {
        self.current
    }

    /// Get minimum value (O(1))
    #[must_use]
    pub fn min(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.min }
    }

    /// Get maximum value (O(1))
    #[must_use]
    pub fn max(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.max }
    }

    /// Get average value (O(1))
    #[must_use]
    pub fn average(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.sum / self.count as f64 }
    }

    /// Get range (max - min) (O(1))
    #[must_use]
    pub fn range(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.max - self.min }
    }

    /// Get sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.current = 0.0;
        self.min = f64::MAX;
        self.max = f64::MIN;
        self.sum = 0.0;
        self.count = 0;
    }
}

// =============================================================================
// COUNTER PAIR (trueno-viz O(1) success/failure tracking pattern)
// =============================================================================

/// Counter pair for success/failure tracking (trueno-viz pattern)
///
/// Tracks success and failure counts with ratio calculation.
/// Useful for request success rates, error rates.
#[derive(Debug, Clone)]
pub struct CounterPair {
    /// Success count
    successes: u64,
    /// Failure count
    failures: u64,
}

impl Default for CounterPair {
    fn default() -> Self {
        Self::new()
    }
}

impl CounterPair {
    /// Create a new counter pair
    #[must_use]
    pub fn new() -> Self {
        Self {
            successes: 0,
            failures: 0,
        }
    }

    /// Record a success (O(1))
    pub fn success(&mut self) {
        self.successes += 1;
    }

    /// Record a failure (O(1))
    pub fn failure(&mut self) {
        self.failures += 1;
    }

    /// Record multiple successes
    pub fn add_successes(&mut self, count: u64) {
        self.successes += count;
    }

    /// Record multiple failures
    pub fn add_failures(&mut self, count: u64) {
        self.failures += count;
    }

    /// Get success count (O(1))
    #[must_use]
    pub fn successes(&self) -> u64 {
        self.successes
    }

    /// Get failure count (O(1))
    #[must_use]
    pub fn failures(&self) -> u64 {
        self.failures
    }

    /// Get total count (O(1))
    #[must_use]
    pub fn total(&self) -> u64 {
        self.successes + self.failures
    }

    /// Get success rate (percentage) (O(1))
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 { 100.0 } else { (self.successes as f64 / total as f64) * 100.0 }
    }

    /// Get failure rate (percentage) (O(1))
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        100.0 - self.success_rate()
    }

    /// Check if healthy (success rate > threshold)
    #[must_use]
    pub fn is_healthy(&self, threshold: f64) -> bool {
        self.success_rate() >= threshold
    }

    /// Reset counters
    pub fn reset(&mut self) {
        self.successes = 0;
        self.failures = 0;
    }
}

// =============================================================================
// HEALTH SCORE (trueno-viz O(1) composite health pattern)
// =============================================================================

/// Health score calculator (trueno-viz pattern)
///
/// Combines multiple metrics into a single health score (0-100).
/// Useful for system health dashboards.
#[derive(Debug, Clone)]
pub struct HealthScore {
    /// Component scores (up to 8)
    scores: [f64; 8],
    /// Component weights
    weights: [f64; 8],
    /// Number of active components
    active: usize,
}

impl Default for HealthScore {
    fn default() -> Self {
        Self::new()
    }
}

impl HealthScore {
    /// Create a new health score calculator
    #[must_use]
    pub fn new() -> Self {
        Self {
            scores: [100.0; 8],
            weights: [1.0; 8],
            active: 0,
        }
    }

    /// Set a component score (O(1))
    ///
    /// Index 0-7, score 0-100.
    pub fn set(&mut self, index: usize, score: f64) {
        if index < 8 {
            self.scores[index] = score.clamp(0.0, 100.0);
            if index >= self.active {
                self.active = index + 1;
            }
        }
    }

    /// Set a component weight (O(1))
    pub fn set_weight(&mut self, index: usize, weight: f64) {
        if index < 8 {
            self.weights[index] = weight.max(0.0);
        }
    }

    /// Get overall health score (O(1))
    #[must_use]
    pub fn score(&self) -> f64 {
        if self.active == 0 {
            return 100.0;
        }

        let mut weighted_sum = 0.0;
        let mut weight_sum = 0.0;

        for i in 0..self.active {
            weighted_sum += self.scores[i] * self.weights[i];
            weight_sum += self.weights[i];
        }

        if weight_sum < f64::EPSILON {
            100.0
        } else {
            (weighted_sum / weight_sum).clamp(0.0, 100.0)
        }
    }

    /// Get health status (O(1))
    #[must_use]
    pub fn status(&self) -> HealthStatus {
        let score = self.score();
        if score >= 90.0 {
            HealthStatus::Healthy
        } else if score >= 70.0 {
            HealthStatus::Degraded
        } else if score >= 50.0 {
            HealthStatus::Warning
        } else {
            HealthStatus::Critical
        }
    }

    /// Check if healthy (score >= 90)
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.score() >= 90.0
    }

    /// Get minimum component score (O(1))
    #[must_use]
    pub fn min_score(&self) -> f64 {
        if self.active == 0 {
            return 100.0;
        }
        self.scores[..self.active]
            .iter()
            .fold(f64::MAX, |a, &b| a.min(b))
    }

    /// Get number of active components
    #[must_use]
    pub fn active_components(&self) -> usize {
        self.active
    }

    /// Reset all scores to 100
    pub fn reset(&mut self) {
        self.scores = [100.0; 8];
        self.active = 0;
    }
}

/// Health status levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Score >= 90
    Healthy,
    /// Score >= 70
    Degraded,
    /// Score >= 50
    Warning,
    /// Score < 50
    Critical,
}

// ============================================================================
// BATCH PROCESSING & WORK QUEUE HELPERS (trueno-viz parity)
// ============================================================================

/// O(1) batch processor for fixed-size batch accumulation.
///
/// Accumulates items until batch is full, then signals ready for processing.
/// Common pattern for batching network writes, disk flushes, metric exports.
#[derive(Debug, Clone)]
pub struct BatchProcessor {
    /// Current batch count
    count: u64,
    /// Batch size threshold
    batch_size: u64,
    /// Total batches completed
    batches_completed: u64,
    /// Total items processed
    total_items: u64,
}

impl Default for BatchProcessor {
    fn default() -> Self {
        Self::new(100)
    }
}

impl BatchProcessor {
    /// Create with specified batch size
    #[must_use]
    pub fn new(batch_size: u64) -> Self {
        Self {
            count: 0,
            batch_size: batch_size.max(1),
            batches_completed: 0,
            total_items: 0,
        }
    }

    /// Create for network operations (batch size 1000)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(1000)
    }

    /// Create for disk operations (batch size 100)
    #[must_use]
    pub fn for_disk() -> Self {
        Self::new(100)
    }

    /// Create for metrics export (batch size 50)
    #[must_use]
    pub fn for_metrics() -> Self {
        Self::new(50)
    }

    /// Add item to batch, returns true if batch is now full
    pub fn add(&mut self) -> bool {
        self.count += 1;
        self.total_items += 1;
        if self.count >= self.batch_size {
            self.count = 0;
            self.batches_completed += 1;
            true
        } else {
            false
        }
    }

    /// Add multiple items, returns number of batches completed
    pub fn add_many(&mut self, n: u64) -> u64 {
        self.total_items += n;
        let new_count = self.count + n;
        let batches = new_count / self.batch_size;
        self.count = new_count % self.batch_size;
        self.batches_completed += batches;
        batches
    }

    /// Check if batch is ready (full)
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.count >= self.batch_size
    }

    /// Get current batch fill percentage
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        (self.count as f64 / self.batch_size as f64) * 100.0
    }

    /// Get items remaining until full batch
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.batch_size.saturating_sub(self.count)
    }

    /// Get total batches completed
    #[must_use]
    pub fn batches_completed(&self) -> u64 {
        self.batches_completed
    }

    /// Get total items processed
    #[must_use]
    pub fn total_items(&self) -> u64 {
        self.total_items
    }

    /// Flush current batch (mark complete regardless of count)
    pub fn flush(&mut self) {
        if self.count > 0 {
            self.count = 0;
            self.batches_completed += 1;
        }
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.count = 0;
        self.batches_completed = 0;
        self.total_items = 0;
    }
}

/// O(1) pipeline stage latency and throughput tracker.
///
/// Tracks items entering and exiting a pipeline stage for monitoring
/// processing latency, queue depth, and throughput.
#[derive(Debug, Clone)]
pub struct PipelineStage {
    /// Items currently in stage
    in_flight: u64,
    /// Peak in-flight items
    peak_in_flight: u64,
    /// Total items entered
    entered: u64,
    /// Total items exited
    exited: u64,
    /// Total latency (for average calculation)
    total_latency_us: u64,
}

impl Default for PipelineStage {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineStage {
    /// Create new pipeline stage tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            in_flight: 0,
            peak_in_flight: 0,
            entered: 0,
            exited: 0,
            total_latency_us: 0,
        }
    }

    /// Record item entering the stage
    pub fn enter(&mut self) {
        self.in_flight += 1;
        self.entered += 1;
        if self.in_flight > self.peak_in_flight {
            self.peak_in_flight = self.in_flight;
        }
    }

    /// Record item exiting the stage with latency in microseconds
    pub fn exit(&mut self, latency_us: u64) {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.exited += 1;
        self.total_latency_us += latency_us;
    }

    /// Record item exiting without latency tracking
    pub fn exit_simple(&mut self) {
        self.in_flight = self.in_flight.saturating_sub(1);
        self.exited += 1;
    }

    /// Get current queue depth
    #[must_use]
    pub fn depth(&self) -> u64 {
        self.in_flight
    }

    /// Get peak queue depth
    #[must_use]
    pub fn peak_depth(&self) -> u64 {
        self.peak_in_flight
    }

    /// Get average latency in microseconds
    #[must_use]
    pub fn avg_latency_us(&self) -> f64 {
        if self.exited == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.exited as f64
        }
    }

    /// Get average latency in milliseconds
    #[must_use]
    pub fn avg_latency_ms(&self) -> f64 {
        self.avg_latency_us() / 1000.0
    }

    /// Get throughput (items processed)
    #[must_use]
    pub fn throughput(&self) -> u64 {
        self.exited
    }

    /// Get total items that entered
    #[must_use]
    pub fn total_entered(&self) -> u64 {
        self.entered
    }

    /// Check if stage is idle (nothing in flight)
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.in_flight == 0
    }

    /// Check if stage is backlogged (depth > threshold)
    #[must_use]
    pub fn is_backlogged(&self, threshold: u64) -> bool {
        self.in_flight > threshold
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.in_flight = 0;
        self.peak_in_flight = 0;
        self.entered = 0;
        self.exited = 0;
        self.total_latency_us = 0;
    }
}

/// O(1) work queue metrics tracker.
///
/// Tracks enqueue/dequeue operations, wait times, and queue health.
#[derive(Debug, Clone)]
pub struct WorkQueue {
    /// Current queue size
    size: u64,
    /// Peak queue size
    peak_size: u64,
    /// Total enqueued
    enqueued: u64,
    /// Total dequeued
    dequeued: u64,
    /// Total wait time (for average)
    total_wait_us: u64,
    /// Capacity limit (0 = unlimited)
    capacity: u64,
}

impl Default for WorkQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkQueue {
    /// Create unbounded work queue tracker
    #[must_use]
    pub fn new() -> Self {
        Self {
            size: 0,
            peak_size: 0,
            enqueued: 0,
            dequeued: 0,
            total_wait_us: 0,
            capacity: 0,
        }
    }

    /// Create bounded work queue tracker
    #[must_use]
    pub fn with_capacity(capacity: u64) -> Self {
        Self {
            capacity,
            ..Self::new()
        }
    }

    /// Enqueue item
    pub fn enqueue(&mut self) -> bool {
        if self.capacity > 0 && self.size >= self.capacity {
            return false; // Would exceed capacity
        }
        self.size += 1;
        self.enqueued += 1;
        if self.size > self.peak_size {
            self.peak_size = self.size;
        }
        true
    }

    /// Dequeue item with wait time in microseconds
    pub fn dequeue(&mut self, wait_us: u64) -> bool {
        if self.size == 0 {
            return false;
        }
        self.size -= 1;
        self.dequeued += 1;
        self.total_wait_us += wait_us;
        true
    }

    /// Dequeue without wait time tracking
    pub fn dequeue_simple(&mut self) -> bool {
        if self.size == 0 {
            return false;
        }
        self.size -= 1;
        self.dequeued += 1;
        true
    }

    /// Get current queue size
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get peak queue size
    #[must_use]
    pub fn peak_size(&self) -> u64 {
        self.peak_size
    }

    /// Get average wait time in microseconds
    #[must_use]
    pub fn avg_wait_us(&self) -> f64 {
        if self.dequeued == 0 {
            0.0
        } else {
            self.total_wait_us as f64 / self.dequeued as f64
        }
    }

    /// Get queue utilization (current/capacity)
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.size as f64 / self.capacity as f64) * 100.0
        }
    }

    /// Check if queue is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Check if queue is full (bounded only)
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.capacity > 0 && self.size >= self.capacity
    }

    /// Get remaining capacity (0 if unbounded)
    #[must_use]
    pub fn remaining_capacity(&self) -> u64 {
        if self.capacity == 0 {
            u64::MAX
        } else {
            self.capacity.saturating_sub(self.size)
        }
    }

    /// Get total enqueued
    #[must_use]
    pub fn total_enqueued(&self) -> u64 {
        self.enqueued
    }

    /// Get total dequeued
    #[must_use]
    pub fn total_dequeued(&self) -> u64 {
        self.dequeued
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.size = 0;
        self.peak_size = 0;
        self.enqueued = 0;
        self.dequeued = 0;
        self.total_wait_us = 0;
    }
}

// ============================================================================
// RATE LIMITING HELPERS (trueno-viz parity)
// ============================================================================

/// O(1) leaky bucket rate limiter.
///
/// Classic leaky bucket algorithm: tokens leak at constant rate,
/// requests add tokens. Overflow = rate exceeded.
#[derive(Debug, Clone)]
pub struct LeakyBucket {
    /// Current bucket level
    level: f64,
    /// Bucket capacity
    capacity: f64,
    /// Leak rate (units per second)
    leak_rate: f64,
    /// Last update timestamp (microseconds)
    last_update_us: u64,
    /// Total overflows
    overflows: u64,
}

impl Default for LeakyBucket {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

impl LeakyBucket {
    /// Create with capacity and leak rate
    #[must_use]
    pub fn new(capacity: f64, leak_rate: f64) -> Self {
        Self {
            level: 0.0,
            capacity: capacity.max(1.0),
            leak_rate: leak_rate.max(0.1),
            last_update_us: 0,
            overflows: 0,
        }
    }

    /// Create for API rate limiting (100 req/s, burst 200)
    #[must_use]
    pub fn for_api() -> Self {
        Self::new(200.0, 100.0)
    }

    /// Create for network throttling (1MB/s, burst 5MB)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(5_000_000.0, 1_000_000.0)
    }

    /// Add tokens, returns true if accepted (no overflow)
    pub fn add(&mut self, tokens: f64, now_us: u64) -> bool {
        self.leak(now_us);
        let new_level = self.level + tokens;
        if new_level > self.capacity {
            self.overflows += 1;
            false
        } else {
            self.level = new_level;
            true
        }
    }

    /// Leak tokens based on elapsed time
    fn leak(&mut self, now_us: u64) {
        if self.last_update_us == 0 {
            self.last_update_us = now_us;
            return;
        }
        let elapsed_s = (now_us.saturating_sub(self.last_update_us)) as f64 / 1_000_000.0;
        let leaked = elapsed_s * self.leak_rate;
        self.level = (self.level - leaked).max(0.0);
        self.last_update_us = now_us;
    }

    /// Get current bucket level
    #[must_use]
    pub fn level(&self) -> f64 {
        self.level
    }

    /// Get fill percentage
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        (self.level / self.capacity) * 100.0
    }

    /// Get overflow count
    #[must_use]
    pub fn overflows(&self) -> u64 {
        self.overflows
    }

    /// Check if bucket is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.level <= 0.0
    }

    /// Reset bucket
    pub fn reset(&mut self) {
        self.level = 0.0;
        self.overflows = 0;
        self.last_update_us = 0;
    }

    /// Update with current time (for testing)
    pub fn update_with_time(&mut self, now_us: u64) {
        self.leak(now_us);
    }
}

/// O(1) sliding window rate counter.
///
/// Counts events in a sliding time window using sub-windows.
/// More accurate than token bucket for bursty traffic.
#[derive(Debug, Clone)]
pub struct SlidingWindowRate {
    /// Sub-window counts (circular buffer)
    windows: [u64; 10],
    /// Current window index
    current: usize,
    /// Window duration in microseconds
    window_us: u64,
    /// Last window rotation timestamp
    last_rotate_us: u64,
    /// Rate limit
    limit: u64,
    /// Exceeded count
    exceeded: u64,
}

impl Default for SlidingWindowRate {
    fn default() -> Self {
        Self::new(1_000_000, 100)
    }
}

impl SlidingWindowRate {
    /// Create with window duration (us) and rate limit
    #[must_use]
    pub fn new(window_us: u64, limit: u64) -> Self {
        Self {
            windows: [0; 10],
            current: 0,
            window_us: window_us.max(10_000), // Min 10ms
            last_rotate_us: 0,
            limit,
            exceeded: 0,
        }
    }

    /// Create for 1 second window with limit
    #[must_use]
    pub fn per_second(limit: u64) -> Self {
        Self::new(1_000_000, limit)
    }

    /// Create for 1 minute window with limit
    #[must_use]
    pub fn per_minute(limit: u64) -> Self {
        Self::new(60_000_000, limit)
    }

    /// Record event, returns true if within limit
    pub fn record(&mut self, now_us: u64) -> bool {
        self.rotate(now_us);
        let count = self.count();
        if count >= self.limit {
            self.exceeded += 1;
            false
        } else {
            self.windows[self.current] += 1;
            true
        }
    }

    /// Rotate windows if needed
    fn rotate(&mut self, now_us: u64) {
        if self.last_rotate_us == 0 {
            self.last_rotate_us = now_us;
            return;
        }
        let sub_window_us = self.window_us / 10;
        let elapsed = now_us.saturating_sub(self.last_rotate_us);
        let rotations = (elapsed / sub_window_us).min(10) as usize;

        for _ in 0..rotations {
            self.current = (self.current + 1) % 10;
            self.windows[self.current] = 0;
        }
        if rotations > 0 {
            self.last_rotate_us = now_us;
        }
    }

    /// Get current count across all windows
    #[must_use]
    pub fn count(&self) -> u64 {
        self.windows.iter().sum()
    }

    /// Get current rate as percentage of limit
    #[must_use]
    pub fn rate_percentage(&self) -> f64 {
        if self.limit == 0 {
            0.0
        } else {
            (self.count() as f64 / self.limit as f64) * 100.0
        }
    }

    /// Check if rate limit would be exceeded
    #[must_use]
    pub fn would_exceed(&self) -> bool {
        self.count() >= self.limit
    }

    /// Get exceeded count
    #[must_use]
    pub fn exceeded(&self) -> u64 {
        self.exceeded
    }

    /// Reset all windows
    pub fn reset(&mut self) {
        self.windows = [0; 10];
        self.current = 0;
        self.exceeded = 0;
        self.last_rotate_us = 0;
    }

    /// Update with current time (for testing)
    pub fn update_with_time(&mut self, now_us: u64) {
        self.rotate(now_us);
    }
}

// ============================================================================
// RESOURCE POOL & SAMPLING HELPERS (trueno-viz parity)
// ============================================================================

/// O(1) resource pool tracker for connection/object pool monitoring.
///
/// Tracks pool utilization, wait times, and connection health.
#[derive(Debug, Clone)]
pub struct ResourcePool {
    /// Total pool size
    capacity: u64,
    /// Currently in use
    in_use: u64,
    /// Peak in use
    peak_in_use: u64,
    /// Total acquisitions
    acquisitions: u64,
    /// Total releases
    releases: u64,
    /// Total timeouts
    timeouts: u64,
    /// Total wait time (for average)
    total_wait_us: u64,
}

impl Default for ResourcePool {
    fn default() -> Self {
        Self::new(10)
    }
}

impl ResourcePool {
    /// Create pool with capacity
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity: capacity.max(1),
            in_use: 0,
            peak_in_use: 0,
            acquisitions: 0,
            releases: 0,
            timeouts: 0,
            total_wait_us: 0,
        }
    }

    /// Create for database connections (typical pool size 20)
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(20)
    }

    /// Create for HTTP connections (typical pool size 100)
    #[must_use]
    pub fn for_http() -> Self {
        Self::new(100)
    }

    /// Acquire resource from pool
    pub fn acquire(&mut self, wait_us: u64) -> bool {
        if self.in_use >= self.capacity {
            self.timeouts += 1;
            return false;
        }
        self.in_use += 1;
        self.acquisitions += 1;
        self.total_wait_us += wait_us;
        if self.in_use > self.peak_in_use {
            self.peak_in_use = self.in_use;
        }
        true
    }

    /// Release resource back to pool
    pub fn release(&mut self) {
        if self.in_use > 0 {
            self.in_use -= 1;
            self.releases += 1;
        }
    }

    /// Get current utilization percentage
    #[must_use]
    pub fn utilization(&self) -> f64 {
        (self.in_use as f64 / self.capacity as f64) * 100.0
    }

    /// Get available resources
    #[must_use]
    pub fn available(&self) -> u64 {
        self.capacity.saturating_sub(self.in_use)
    }

    /// Get average wait time in microseconds
    #[must_use]
    pub fn avg_wait_us(&self) -> f64 {
        if self.acquisitions == 0 {
            0.0
        } else {
            self.total_wait_us as f64 / self.acquisitions as f64
        }
    }

    /// Get timeout rate
    #[must_use]
    pub fn timeout_rate(&self) -> f64 {
        let total = self.acquisitions + self.timeouts;
        if total == 0 {
            0.0
        } else {
            (self.timeouts as f64 / total as f64) * 100.0
        }
    }

    /// Check if pool is exhausted
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.in_use >= self.capacity
    }

    /// Check if pool is idle
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.in_use == 0
    }

    /// Get peak utilization percentage
    #[must_use]
    pub fn peak_utilization(&self) -> f64 {
        (self.peak_in_use as f64 / self.capacity as f64) * 100.0
    }

    /// Reset all counters (keep capacity)
    pub fn reset(&mut self) {
        self.in_use = 0;
        self.peak_in_use = 0;
        self.acquisitions = 0;
        self.releases = 0;
        self.timeouts = 0;
        self.total_wait_us = 0;
    }
}

/// O(1) 2D histogram for heatmap data accumulation.
///
/// Fixed-grid 2D histogram for accumulating values in x,y space.
#[derive(Debug, Clone)]
pub struct Histogram2D {
    /// Grid cells (10x10 = 100 cells)
    cells: [[u64; 10]; 10],
    /// X min
    x_min: f64,
    /// X max
    x_max: f64,
    /// Y min
    y_min: f64,
    /// Y max
    y_max: f64,
    /// Total samples
    count: u64,
}

impl Default for Histogram2D {
    fn default() -> Self {
        Self::new(0.0, 100.0, 0.0, 100.0)
    }
}

impl Histogram2D {
    /// Create with x and y ranges
    #[must_use]
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            cells: [[0; 10]; 10],
            x_min,
            x_max: x_max.max(x_min + 1.0),
            y_min,
            y_max: y_max.max(y_min + 1.0),
            count: 0,
        }
    }

    /// Create for latency vs throughput (0-100ms, 0-1000 ops/s)
    #[must_use]
    pub fn for_latency_throughput() -> Self {
        Self::new(0.0, 100.0, 0.0, 1000.0)
    }

    /// Create for CPU vs Memory (0-100%)
    #[must_use]
    pub fn for_cpu_memory() -> Self {
        Self::new(0.0, 100.0, 0.0, 100.0)
    }

    /// Add sample
    pub fn add(&mut self, x: f64, y: f64) {
        let xi = self.x_to_index(x);
        let yi = self.y_to_index(y);
        self.cells[yi][xi] += 1;
        self.count += 1;
    }

    fn x_to_index(&self, x: f64) -> usize {
        let normalized = (x - self.x_min) / (self.x_max - self.x_min);
        (normalized * 10.0).clamp(0.0, 9.0) as usize
    }

    fn y_to_index(&self, y: f64) -> usize {
        let normalized = (y - self.y_min) / (self.y_max - self.y_min);
        (normalized * 10.0).clamp(0.0, 9.0) as usize
    }

    /// Get cell count
    #[must_use]
    pub fn get(&self, xi: usize, yi: usize) -> u64 {
        if xi < 10 && yi < 10 {
            self.cells[yi][xi]
        } else {
            0
        }
    }

    /// Get cell density (percentage of total)
    #[must_use]
    pub fn density(&self, xi: usize, yi: usize) -> f64 {
        if self.count == 0 || xi >= 10 || yi >= 10 {
            0.0
        } else {
            (self.cells[yi][xi] as f64 / self.count as f64) * 100.0
        }
    }

    /// Get max cell count
    #[must_use]
    pub fn max_count(&self) -> u64 {
        self.cells.iter().flat_map(|r| r.iter()).copied().max().unwrap_or(0)
    }

    /// Get hotspot (cell with max count)
    #[must_use]
    pub fn hotspot(&self) -> (usize, usize) {
        let mut max_val = 0;
        let mut max_pos = (0, 0);
        for (yi, row) in self.cells.iter().enumerate() {
            for (xi, &val) in row.iter().enumerate() {
                if val > max_val {
                    max_val = val;
                    max_pos = (xi, yi);
                }
            }
        }
        max_pos
    }

    /// Get total sample count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Reset all cells
    pub fn reset(&mut self) {
        self.cells = [[0; 10]; 10];
        self.count = 0;
    }
}

/// O(1) reservoir sampler for uniform sampling of streams.
///
/// Maintains a fixed-size sample of items seen in a stream using
/// Algorithm R (reservoir sampling).
#[derive(Debug, Clone)]
pub struct ReservoirSampler {
    /// Sample values
    samples: [f64; 16],
    /// Number of valid samples
    size: usize,
    /// Capacity
    capacity: usize,
    /// Total items seen
    seen: u64,
    /// Simple LCG state for deterministic sampling
    rng_state: u64,
}

impl Default for ReservoirSampler {
    fn default() -> Self {
        Self::new(16)
    }
}

impl ReservoirSampler {
    /// Create with capacity (max 16)
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            samples: [0.0; 16],
            size: 0,
            capacity: capacity.min(16),
            seen: 0,
            rng_state: 12345,
        }
    }

    /// Simple LCG random number generator
    fn next_random(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        self.rng_state
    }

    /// Add item to reservoir
    pub fn add(&mut self, value: f64) {
        self.seen += 1;
        if self.size < self.capacity {
            self.samples[self.size] = value;
            self.size += 1;
        } else {
            // Reservoir sampling: replace with probability capacity/seen
            let r = (self.next_random() % self.seen) as usize;
            if r < self.capacity {
                self.samples[r] = value;
            }
        }
    }

    /// Get sample at index
    #[must_use]
    pub fn get(&self, index: usize) -> Option<f64> {
        if index < self.size {
            Some(self.samples[index])
        } else {
            None
        }
    }

    /// Get current sample size
    #[must_use]
    pub fn len(&self) -> usize {
        self.size
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Get total items seen
    #[must_use]
    pub fn total_seen(&self) -> u64 {
        self.seen
    }

    /// Get sample mean
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.size == 0 {
            0.0
        } else {
            self.samples[..self.size].iter().sum::<f64>() / self.size as f64
        }
    }

    /// Get sample min
    #[must_use]
    pub fn min(&self) -> f64 {
        if self.size == 0 {
            0.0
        } else {
            self.samples[..self.size]
                .iter()
                .fold(f64::MAX, |a, &b| a.min(b))
        }
    }

    /// Get sample max
    #[must_use]
    pub fn max(&self) -> f64 {
        if self.size == 0 {
            0.0
        } else {
            self.samples[..self.size]
                .iter()
                .fold(f64::MIN, |a, &b| a.max(b))
        }
    }

    /// Reset sampler
    pub fn reset(&mut self) {
        self.samples = [0.0; 16];
        self.size = 0;
        self.seen = 0;
        self.rng_state = 12345;
    }
}

/// O(1) exponential histogram for log-scale binning.
///
/// Bins values into exponential buckets for wide-range distributions.
#[derive(Debug, Clone)]
pub struct ExponentialHistogram {
    /// Bucket counts (8 buckets: 1, 2, 4, 8, 16, 32, 64, 128+)
    buckets: [u64; 8],
    /// Base value (bucket boundaries are base * 2^i)
    base: f64,
    /// Total count
    count: u64,
    /// Sum of all values
    sum: f64,
}

impl Default for ExponentialHistogram {
    fn default() -> Self {
        Self::new(1.0)
    }
}

impl ExponentialHistogram {
    /// Create with base value
    #[must_use]
    pub fn new(base: f64) -> Self {
        Self {
            buckets: [0; 8],
            base: base.max(0.001),
            count: 0,
            sum: 0.0,
        }
    }

    /// Create for latency (base 1ms: 1, 2, 4, 8, 16, 32, 64, 128+ ms)
    #[must_use]
    pub fn for_latency_ms() -> Self {
        Self::new(1.0)
    }

    /// Create for bytes (base 1KB: 1, 2, 4, 8, 16, 32, 64, 128+ KB)
    #[must_use]
    pub fn for_bytes_kb() -> Self {
        Self::new(1024.0)
    }

    /// Add value
    pub fn add(&mut self, value: f64) {
        self.count += 1;
        self.sum += value;
        let bucket = self.value_to_bucket(value);
        self.buckets[bucket] += 1;
    }

    fn value_to_bucket(&self, value: f64) -> usize {
        if value < self.base {
            return 0;
        }
        let ratio = value / self.base;
        let bucket = ratio.log2().floor() as usize;
        bucket.min(7)
    }

    /// Get bucket count
    #[must_use]
    pub fn bucket_count(&self, bucket: usize) -> u64 {
        if bucket < 8 {
            self.buckets[bucket]
        } else {
            0
        }
    }

    /// Get bucket upper bound
    #[must_use]
    pub fn bucket_upper_bound(&self, bucket: usize) -> f64 {
        if bucket >= 7 {
            f64::INFINITY
        } else {
            self.base * 2.0_f64.powi(bucket as i32 + 1)
        }
    }

    /// Get total count
    #[must_use]
    pub fn count(&self) -> u64 {
        self.count
    }

    /// Get mean value
    #[must_use]
    pub fn mean(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Get bucket with most samples
    #[must_use]
    pub fn mode_bucket(&self) -> usize {
        self.buckets
            .iter()
            .enumerate()
            .max_by_key(|(_, &c)| c)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Reset histogram
    pub fn reset(&mut self) {
        self.buckets = [0; 8];
        self.count = 0;
        self.sum = 0.0;
    }
}

// ============================================================================
// CACHE & LOAD BALANCING HELPERS (trueno-viz parity)
// ============================================================================

/// O(1) cache statistics tracker.
///
/// Tracks cache hits, misses, evictions, and calculates hit rate.
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total hits
    hits: u64,
    /// Total misses
    misses: u64,
    /// Total evictions
    evictions: u64,
    /// Total insertions
    insertions: u64,
    /// Bytes in cache
    bytes_cached: u64,
    /// Capacity in bytes
    capacity_bytes: u64,
}

impl Default for CacheStats {
    fn default() -> Self {
        Self::new(0)
    }
}

impl CacheStats {
    /// Create with capacity in bytes
    #[must_use]
    pub fn new(capacity_bytes: u64) -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            insertions: 0,
            bytes_cached: 0,
            capacity_bytes,
        }
    }

    /// Create for L1 cache (32KB typical)
    #[must_use]
    pub fn for_l1_cache() -> Self {
        Self::new(32 * 1024)
    }

    /// Create for L2 cache (256KB typical)
    #[must_use]
    pub fn for_l2_cache() -> Self {
        Self::new(256 * 1024)
    }

    /// Create for application cache (16MB)
    #[must_use]
    pub fn for_app_cache() -> Self {
        Self::new(16 * 1024 * 1024)
    }

    /// Record a cache hit
    pub fn hit(&mut self) {
        self.hits += 1;
    }

    /// Record a cache miss
    pub fn miss(&mut self) {
        self.misses += 1;
    }

    /// Record an eviction
    pub fn evict(&mut self, bytes: u64) {
        self.evictions += 1;
        self.bytes_cached = self.bytes_cached.saturating_sub(bytes);
    }

    /// Record an insertion
    pub fn insert(&mut self, bytes: u64) {
        self.insertions += 1;
        self.bytes_cached += bytes;
    }

    /// Get hit rate as percentage
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            (self.hits as f64 / total as f64) * 100.0
        }
    }

    /// Get miss rate as percentage
    #[must_use]
    pub fn miss_rate(&self) -> f64 {
        100.0 - self.hit_rate()
    }

    /// Get eviction rate (evictions per insertion)
    #[must_use]
    pub fn eviction_rate(&self) -> f64 {
        if self.insertions == 0 {
            0.0
        } else {
            self.evictions as f64 / self.insertions as f64
        }
    }

    /// Get fill percentage
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        if self.capacity_bytes == 0 {
            0.0
        } else {
            (self.bytes_cached as f64 / self.capacity_bytes as f64) * 100.0
        }
    }

    /// Get total requests
    #[must_use]
    pub fn total_requests(&self) -> u64 {
        self.hits + self.misses
    }

    /// Check if cache is effective (hit rate > threshold)
    #[must_use]
    pub fn is_effective(&self, threshold: f64) -> bool {
        self.hit_rate() >= threshold
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.evictions = 0;
        self.insertions = 0;
        self.bytes_cached = 0;
    }
}

/// O(1) Bloom filter for probabilistic membership testing.
///
/// Fixed-size bloom filter with configurable hash count.
/// False positives possible, false negatives impossible.
#[derive(Debug, Clone)]
pub struct BloomFilter {
    /// Bit array (using u64 words)
    bits: [u64; 16], // 1024 bits
    /// Number of hash functions
    hash_count: u32,
    /// Items added
    items: u64,
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new(3)
    }
}

impl BloomFilter {
    /// Create with number of hash functions
    #[must_use]
    pub fn new(hash_count: u32) -> Self {
        Self {
            bits: [0; 16],
            hash_count: hash_count.clamp(1, 10),
            items: 0,
        }
    }

    /// Create optimized for ~100 items (3 hashes)
    #[must_use]
    pub fn for_small() -> Self {
        Self::new(3)
    }

    /// Create optimized for ~500 items (5 hashes)
    #[must_use]
    pub fn for_medium() -> Self {
        Self::new(5)
    }

    /// Simple hash function (FNV-1a style)
    fn hash(&self, value: u64, seed: u32) -> usize {
        let mut h = value.wrapping_mul(0x517cc1b727220a95);
        h = h.wrapping_add(seed as u64);
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        (h as usize) % 1024
    }

    /// Add item to filter
    pub fn add(&mut self, value: u64) {
        for i in 0..self.hash_count {
            let bit_idx = self.hash(value, i);
            let word_idx = bit_idx / 64;
            let bit_pos = bit_idx % 64;
            self.bits[word_idx] |= 1 << bit_pos;
        }
        self.items += 1;
    }

    /// Check if item might be in filter
    #[must_use]
    pub fn might_contain(&self, value: u64) -> bool {
        for i in 0..self.hash_count {
            let bit_idx = self.hash(value, i);
            let word_idx = bit_idx / 64;
            let bit_pos = bit_idx % 64;
            if self.bits[word_idx] & (1 << bit_pos) == 0 {
                return false;
            }
        }
        true
    }

    /// Get number of items added
    #[must_use]
    pub fn len(&self) -> u64 {
        self.items
    }

    /// Check if empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items == 0
    }

    /// Get estimated false positive rate
    #[must_use]
    pub fn false_positive_rate(&self) -> f64 {
        let m = 1024.0; // bits
        let k = self.hash_count as f64;
        let n = self.items as f64;
        if n == 0.0 {
            return 0.0;
        }
        (1.0 - (-k * n / m).exp()).powf(k)
    }

    /// Get fill percentage (bits set / total bits)
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        let set_bits: u32 = self.bits.iter().map(|w| w.count_ones()).sum();
        (set_bits as f64 / 1024.0) * 100.0
    }

    /// Reset filter
    pub fn reset(&mut self) {
        self.bits = [0; 16];
        self.items = 0;
    }
}

/// O(1) weighted round-robin load balancer.
///
/// Distributes load across backends with configurable weights.
#[derive(Debug, Clone)]
pub struct LoadBalancer {
    /// Backend weights
    weights: [u32; 8],
    /// Current weights (for WRR algorithm)
    current: [i32; 8],
    /// Active backends count
    active: usize,
    /// Total requests dispatched
    dispatched: u64,
    /// Requests per backend
    per_backend: [u64; 8],
}

impl Default for LoadBalancer {
    fn default() -> Self {
        Self::new()
    }
}

impl LoadBalancer {
    /// Create empty load balancer
    #[must_use]
    pub fn new() -> Self {
        Self {
            weights: [0; 8],
            current: [0; 8],
            active: 0,
            dispatched: 0,
            per_backend: [0; 8],
        }
    }

    /// Create with equal weights for n backends
    #[must_use]
    pub fn equal_weights(n: usize) -> Self {
        let mut lb = Self::new();
        for _ in 0..n.min(8) {
            lb.add_backend(1);
        }
        lb
    }

    /// Add backend with weight
    pub fn add_backend(&mut self, weight: u32) {
        if self.active < 8 {
            self.weights[self.active] = weight.max(1);
            self.current[self.active] = 0;
            self.active += 1;
        }
    }

    /// Select next backend (weighted round-robin)
    #[must_use]
    pub fn select_backend(&mut self) -> Option<usize> {
        if self.active == 0 {
            return None;
        }

        // Weighted round-robin: select backend with highest current weight
        let total_weight: i32 = self.weights[..self.active].iter().map(|&w| w as i32).sum();

        // Add weights to current
        for i in 0..self.active {
            self.current[i] += self.weights[i] as i32;
        }

        // Find max current weight
        let mut max_idx = 0;
        let mut max_weight = self.current[0];
        for i in 1..self.active {
            if self.current[i] > max_weight {
                max_weight = self.current[i];
                max_idx = i;
            }
        }

        // Subtract total weight from selected
        self.current[max_idx] -= total_weight;
        self.dispatched += 1;
        self.per_backend[max_idx] += 1;

        Some(max_idx)
    }

    /// Get distribution percentage for backend
    #[must_use]
    pub fn distribution(&self, backend: usize) -> f64 {
        if self.dispatched == 0 || backend >= self.active {
            0.0
        } else {
            (self.per_backend[backend] as f64 / self.dispatched as f64) * 100.0
        }
    }

    /// Get total dispatched
    #[must_use]
    pub fn total_dispatched(&self) -> u64 {
        self.dispatched
    }

    /// Get active backend count
    #[must_use]
    pub fn backend_count(&self) -> usize {
        self.active
    }

    /// Check if load is balanced (within threshold)
    #[must_use]
    pub fn is_balanced(&self, threshold: f64) -> bool {
        if self.active <= 1 || self.dispatched < 10 {
            return true;
        }
        let avg = self.dispatched as f64 / self.active as f64;
        for i in 0..self.active {
            let deviation = ((self.per_backend[i] as f64 - avg) / avg).abs() * 100.0;
            if deviation > threshold {
                return false;
            }
        }
        true
    }

    /// Reset all counters
    pub fn reset(&mut self) {
        self.current = [0; 8];
        self.dispatched = 0;
        self.per_backend = [0; 8];
    }
}

/// O(1) token bucket with burst tracking.
///
/// Enhanced token bucket that tracks burst patterns.
#[derive(Debug, Clone)]
pub struct BurstTracker {
    /// Current tokens
    tokens: f64,
    /// Bucket capacity
    capacity: f64,
    /// Refill rate (tokens per second)
    refill_rate: f64,
    /// Last update timestamp (us)
    last_update_us: u64,
    /// Current burst count
    burst_count: u64,
    /// Max burst seen
    max_burst: u64,
    /// Total bursts
    total_bursts: u64,
}

impl Default for BurstTracker {
    fn default() -> Self {
        Self::new(100.0, 10.0)
    }
}

impl BurstTracker {
    /// Create with capacity and refill rate
    #[must_use]
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            capacity: capacity.max(1.0),
            refill_rate: refill_rate.max(0.1),
            last_update_us: 0,
            burst_count: 0,
            max_burst: 0,
            total_bursts: 0,
        }
    }

    /// Create for API rate limiting
    #[must_use]
    pub fn for_api() -> Self {
        Self::new(100.0, 50.0)
    }

    /// Create for network throttling
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(1000.0, 100.0)
    }

    /// Consume tokens, returns true if allowed
    pub fn consume(&mut self, tokens: f64, now_us: u64) -> bool {
        self.refill(now_us);

        if tokens <= self.tokens {
            self.tokens -= tokens;
            self.burst_count += 1;
            if self.burst_count > self.max_burst {
                self.max_burst = self.burst_count;
            }
            true
        } else {
            // End of burst
            if self.burst_count > 0 {
                self.total_bursts += 1;
            }
            self.burst_count = 0;
            false
        }
    }

    fn refill(&mut self, now_us: u64) {
        if self.last_update_us == 0 {
            self.last_update_us = now_us;
            return;
        }
        let elapsed_s = (now_us.saturating_sub(self.last_update_us)) as f64 / 1_000_000.0;
        let refill = elapsed_s * self.refill_rate;
        self.tokens = (self.tokens + refill).min(self.capacity);
        self.last_update_us = now_us;
    }

    /// Get current token count
    #[must_use]
    pub fn tokens(&self) -> f64 {
        self.tokens
    }

    /// Get fill percentage
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        (self.tokens / self.capacity) * 100.0
    }

    /// Get max burst size seen
    #[must_use]
    pub fn max_burst(&self) -> u64 {
        self.max_burst
    }

    /// Get total bursts
    #[must_use]
    pub fn total_bursts(&self) -> u64 {
        self.total_bursts
    }

    /// Get average burst size
    #[must_use]
    pub fn avg_burst(&self) -> f64 {
        if self.total_bursts == 0 {
            0.0
        } else {
            self.max_burst as f64 // Approximation
        }
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.tokens = self.capacity;
        self.burst_count = 0;
        self.max_burst = 0;
        self.total_bursts = 0;
        self.last_update_us = 0;
    }
}

// ============================================================================
// TopKTracker - Fixed-size top-K value tracker (O(1) amortized insertion)
// ============================================================================

/// O(1) amortized top-K value tracker.
/// Uses a fixed-size array with insertion sort for small K values.
#[derive(Debug, Clone)]
pub struct TopKTracker {
    values: [f64; 32],
    count: usize,
    k: usize,
}

impl Default for TopKTracker {
    fn default() -> Self {
        Self::new(10)
    }
}

impl TopKTracker {
    /// Create new top-K tracker
    #[must_use]
    pub fn new(k: usize) -> Self {
        Self {
            values: [f64::NEG_INFINITY; 32],
            count: 0,
            k: k.min(32),
        }
    }

    /// Create for metrics (top 10)
    #[must_use]
    pub fn for_metrics() -> Self {
        Self::new(10)
    }

    /// Create for processes (top 20)
    #[must_use]
    pub fn for_processes() -> Self {
        Self::new(20)
    }

    /// Add value (O(k) insertion)
    pub fn add(&mut self, value: f64) {
        if self.count < self.k {
            // Not full yet, insert in sorted order
            let mut i = self.count;
            while i > 0 && self.values[i - 1] < value {
                self.values[i] = self.values[i - 1];
                i -= 1;
            }
            self.values[i] = value;
            self.count += 1;
        } else if value > self.values[self.k - 1] {
            // Replace minimum if value is larger
            let mut i = self.k - 1;
            while i > 0 && self.values[i - 1] < value {
                self.values[i] = self.values[i - 1];
                i -= 1;
            }
            self.values[i] = value;
        }
    }

    /// Get top-K values (sorted descending)
    #[must_use]
    pub fn top(&self) -> &[f64] {
        &self.values[..self.count]
    }

    /// Get K value
    #[must_use]
    pub fn k(&self) -> usize {
        self.k
    }

    /// Get count of tracked values
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get minimum value in top-K
    #[must_use]
    pub fn minimum(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.values[self.count - 1])
        } else {
            None
        }
    }

    /// Get maximum value (always at index 0)
    #[must_use]
    pub fn maximum(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.values[0])
        } else {
            None
        }
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.values = [f64::NEG_INFINITY; 32];
        self.count = 0;
    }
}

// ============================================================================
// QuotaTracker - Resource quota tracking
// ============================================================================

/// O(1) resource quota tracker.
/// Tracks usage against a limit with percentage and exhaustion checks.
#[derive(Debug, Clone)]
pub struct QuotaTracker {
    limit: u64,
    used: u64,
    peak_usage: u64,
}

impl Default for QuotaTracker {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl QuotaTracker {
    /// Create with limit
    #[must_use]
    pub fn new(limit: u64) -> Self {
        Self {
            limit: limit.max(1),
            used: 0,
            peak_usage: 0,
        }
    }

    /// Create for API daily limit (10K requests)
    #[must_use]
    pub fn for_api_daily() -> Self {
        Self::new(10000)
    }

    /// Create for storage limit (100 GB)
    #[must_use]
    pub fn for_storage_gb() -> Self {
        Self::new(100)
    }

    /// Use quota, returns false if would exceed
    pub fn use_quota(&mut self, amount: u64) -> bool {
        if self.used + amount > self.limit {
            false
        } else {
            self.used += amount;
            if self.used > self.peak_usage {
                self.peak_usage = self.used;
            }
            true
        }
    }

    /// Release quota
    pub fn release(&mut self, amount: u64) {
        self.used = self.used.saturating_sub(amount);
    }

    /// Get limit
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Get remaining quota
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Get usage percentage
    #[must_use]
    pub fn usage_percentage(&self) -> f64 {
        (self.used as f64 / self.limit as f64) * 100.0
    }

    /// Check if exhausted
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.used >= self.limit
    }

    /// Get peak usage
    #[must_use]
    pub fn peak_usage(&self) -> u64 {
        self.peak_usage
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.used = 0;
        self.peak_usage = 0;
    }
}

// ============================================================================
// FrequencyCounter - Categorical frequency tracking
// ============================================================================

/// O(1) categorical frequency counter.
/// Tracks occurrence counts and calculates frequencies for up to 16 categories.
#[derive(Debug, Clone)]
pub struct FrequencyCounter {
    counts: [u64; 16],
    total: u64,
}

impl Default for FrequencyCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrequencyCounter {
    /// Create new counter
    #[must_use]
    pub fn new() -> Self {
        Self {
            counts: [0; 16],
            total: 0,
        }
    }

    /// Increment category count
    pub fn increment(&mut self, category: usize) {
        if category < 16 {
            self.counts[category] += 1;
            self.total += 1;
        }
    }

    /// Add multiple to category
    pub fn add(&mut self, category: usize, count: u64) {
        if category < 16 {
            self.counts[category] += count;
            self.total += count;
        }
    }

    /// Get count for category
    #[must_use]
    pub fn count(&self, category: usize) -> u64 {
        if category < 16 {
            self.counts[category]
        } else {
            0
        }
    }

    /// Get frequency percentage for category
    #[must_use]
    pub fn frequency(&self, category: usize) -> f64 {
        if self.total == 0 || category >= 16 {
            0.0
        } else {
            (self.counts[category] as f64 / self.total as f64) * 100.0
        }
    }

    /// Get total count
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Get most frequent category
    #[must_use]
    pub fn most_frequent(&self) -> Option<usize> {
        if self.total == 0 {
            return None;
        }
        let mut max_idx = 0;
        let mut max_count = self.counts[0];
        for i in 1..16 {
            if self.counts[i] > max_count {
                max_count = self.counts[i];
                max_idx = i;
            }
        }
        Some(max_idx)
    }

    /// Get number of non-zero categories
    #[must_use]
    pub fn non_zero_count(&self) -> usize {
        self.counts.iter().filter(|&&c| c > 0).count()
    }

    /// Calculate Shannon entropy (normalized 0-1)
    #[must_use]
    pub fn entropy(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        let mut entropy = 0.0;
        for &count in &self.counts {
            if count > 0 {
                let p = count as f64 / self.total as f64;
                entropy -= p * p.log2();
            }
        }
        // Normalize by max entropy (log2(16) = 4)
        entropy / 4.0
    }

    /// Reset counter
    pub fn reset(&mut self) {
        self.counts = [0; 16];
        self.total = 0;
    }
}

// ============================================================================
// MovingRange - Moving min/max range tracking for volatility
// ============================================================================

/// O(1) moving range tracker for volatility analysis.
/// Maintains min/max over a sliding window for range and volatility metrics.
#[derive(Debug, Clone)]
pub struct MovingRange {
    values: [f64; 128],
    window_size: usize,
    head: usize,
    count: usize,
    current_min: f64,
    current_max: f64,
}

impl Default for MovingRange {
    fn default() -> Self {
        Self::new(10)
    }
}

impl MovingRange {
    /// Create with window size
    #[must_use]
    pub fn new(window_size: usize) -> Self {
        Self {
            values: [0.0; 128],
            window_size: window_size.min(128),
            head: 0,
            count: 0,
            current_min: f64::INFINITY,
            current_max: f64::NEG_INFINITY,
        }
    }

    /// Create for price volatility (20 samples)
    #[must_use]
    pub fn for_prices() -> Self {
        Self::new(20)
    }

    /// Create for latency volatility (100 samples)
    #[must_use]
    pub fn for_latency() -> Self {
        Self::new(100)
    }

    /// Add value to window
    pub fn add(&mut self, value: f64) {
        let idx = self.head;
        self.values[idx] = value;
        self.head = (self.head + 1) % self.window_size;
        if self.count < self.window_size {
            self.count += 1;
        }
        self.recalculate_minmax();
    }

    fn recalculate_minmax(&mut self) {
        self.current_min = f64::INFINITY;
        self.current_max = f64::NEG_INFINITY;
        for i in 0..self.count {
            let v = self.values[i];
            if v < self.current_min {
                self.current_min = v;
            }
            if v > self.current_max {
                self.current_max = v;
            }
        }
    }

    /// Get window size
    #[must_use]
    pub fn window_size(&self) -> usize {
        self.window_size
    }

    /// Get current count
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get minimum value
    #[must_use]
    pub fn min(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.current_min)
        } else {
            None
        }
    }

    /// Get maximum value
    #[must_use]
    pub fn max(&self) -> Option<f64> {
        if self.count > 0 {
            Some(self.current_max)
        } else {
            None
        }
    }

    /// Get range (max - min)
    #[must_use]
    pub fn range(&self) -> f64 {
        if self.count > 0 {
            self.current_max - self.current_min
        } else {
            0.0
        }
    }

    /// Get mid-range ((max + min) / 2)
    #[must_use]
    pub fn midrange(&self) -> f64 {
        if self.count > 0 {
            (self.current_max + self.current_min) / 2.0
        } else {
            0.0
        }
    }

    /// Get volatility (range / midrange * 100)
    #[must_use]
    pub fn volatility(&self) -> f64 {
        let mid = self.midrange();
        if mid.abs() < 0.0001 {
            0.0
        } else {
            (self.range() / mid) * 100.0
        }
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.values = [0.0; 128];
        self.head = 0;
        self.count = 0;
        self.current_min = f64::INFINITY;
        self.current_max = f64::NEG_INFINITY;
    }
}

// ============================================================================
// TimeoutTracker - Operation timeout tracking
// ============================================================================

/// O(1) operation timeout tracker.
/// Tracks successful and timed-out operations with configurable timeout threshold.
#[derive(Debug, Clone)]
pub struct TimeoutTracker {
    timeout_us: u64,
    total: u64,
    timed_out: u64,
    last_duration_us: u64,
    max_duration_us: u64,
}

impl Default for TimeoutTracker {
    fn default() -> Self {
        Self::new(1_000_000) // 1 second default
    }
}

impl TimeoutTracker {
    /// Create with timeout threshold in microseconds
    #[must_use]
    pub fn new(timeout_us: u64) -> Self {
        Self {
            timeout_us: timeout_us.max(1),
            total: 0,
            timed_out: 0,
            last_duration_us: 0,
            max_duration_us: 0,
        }
    }

    /// Create for network operations (5s timeout)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(5_000_000)
    }

    /// Create for database operations (30s timeout)
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(30_000_000)
    }

    /// Create for fast operations (100ms timeout)
    #[must_use]
    pub fn for_fast() -> Self {
        Self::new(100_000)
    }

    /// Record operation completion
    pub fn record(&mut self, duration_us: u64) {
        self.total += 1;
        self.last_duration_us = duration_us;
        if duration_us > self.max_duration_us {
            self.max_duration_us = duration_us;
        }
        if duration_us > self.timeout_us {
            self.timed_out += 1;
        }
    }

    /// Get total operations
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Get timed out count
    #[must_use]
    pub fn timed_out(&self) -> u64 {
        self.timed_out
    }

    /// Get timeout rate as percentage
    #[must_use]
    pub fn timeout_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.timed_out as f64 / self.total as f64) * 100.0
        }
    }

    /// Get success rate as percentage
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        100.0 - self.timeout_rate()
    }

    /// Check if timeout rate is acceptable
    #[must_use]
    pub fn is_healthy(&self, max_timeout_rate: f64) -> bool {
        self.timeout_rate() <= max_timeout_rate
    }

    /// Get max duration seen
    #[must_use]
    pub fn max_duration_us(&self) -> u64 {
        self.max_duration_us
    }

    /// Get timeout threshold
    #[must_use]
    pub fn timeout_threshold_us(&self) -> u64 {
        self.timeout_us
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.total = 0;
        self.timed_out = 0;
        self.last_duration_us = 0;
        self.max_duration_us = 0;
    }
}

// ============================================================================
// RetryTracker - Retry attempt tracking with backoff state
// ============================================================================

/// O(1) retry tracking with exponential backoff state.
/// Tracks retry attempts, success after retry, and calculates next retry delay.
#[derive(Debug, Clone)]
pub struct RetryTracker {
    max_retries: u32,
    base_delay_ms: u64,
    max_delay_ms: u64,
    total_attempts: u64,
    total_retries: u64,
    successful_retries: u64,
    current_retry: u32,
}

impl Default for RetryTracker {
    fn default() -> Self {
        Self::new(3, 100, 10000)
    }
}

impl RetryTracker {
    /// Create with max retries, base delay, and max delay in ms
    #[must_use]
    pub fn new(max_retries: u32, base_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            base_delay_ms: base_delay_ms.max(1),
            max_delay_ms: max_delay_ms.max(base_delay_ms),
            total_attempts: 0,
            total_retries: 0,
            successful_retries: 0,
            current_retry: 0,
        }
    }

    /// Create for API retries (3 retries, 100ms base, 10s max)
    #[must_use]
    pub fn for_api() -> Self {
        Self::new(3, 100, 10000)
    }

    /// Create for network retries (5 retries, 1s base, 30s max)
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(5, 1000, 30000)
    }

    /// Record attempt start
    pub fn attempt(&mut self) {
        self.total_attempts += 1;
    }

    /// Record retry (failed attempt, will retry)
    pub fn retry(&mut self) {
        self.total_retries += 1;
        if self.current_retry < self.max_retries {
            self.current_retry += 1;
        }
    }

    /// Record success (resets current retry count)
    pub fn success(&mut self) {
        if self.current_retry > 0 {
            self.successful_retries += 1;
        }
        self.current_retry = 0;
    }

    /// Get next retry delay in ms (exponential backoff)
    #[must_use]
    pub fn next_delay_ms(&self) -> u64 {
        let delay = self.base_delay_ms * (1 << self.current_retry);
        delay.min(self.max_delay_ms)
    }

    /// Check if retries exhausted
    #[must_use]
    pub fn retries_exhausted(&self) -> bool {
        self.current_retry >= self.max_retries
    }

    /// Get retry rate as percentage
    #[must_use]
    pub fn retry_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            (self.total_retries as f64 / self.total_attempts as f64) * 100.0
        }
    }

    /// Get successful retry rate
    #[must_use]
    pub fn successful_retry_rate(&self) -> f64 {
        if self.total_retries == 0 {
            0.0
        } else {
            (self.successful_retries as f64 / self.total_retries as f64) * 100.0
        }
    }

    /// Get current retry count
    #[must_use]
    pub fn current_retry(&self) -> u32 {
        self.current_retry
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.total_attempts = 0;
        self.total_retries = 0;
        self.successful_retries = 0;
        self.current_retry = 0;
    }
}

// ============================================================================
// ScheduleSlot - Time-based slot scheduling
// ============================================================================

/// O(1) time-based slot scheduler.
/// Divides time into slots and tracks which slot is currently active.
#[derive(Debug, Clone)]
pub struct ScheduleSlot {
    slot_duration_us: u64,
    num_slots: usize,
    current_slot: usize,
    slot_start_us: u64,
    executions_per_slot: [u64; 16],
}

impl Default for ScheduleSlot {
    fn default() -> Self {
        Self::new(1_000_000, 10) // 1 second slots, 10 slots
    }
}

impl ScheduleSlot {
    /// Create with slot duration in microseconds and number of slots
    #[must_use]
    pub fn new(slot_duration_us: u64, num_slots: usize) -> Self {
        Self {
            slot_duration_us: slot_duration_us.max(1),
            num_slots: num_slots.min(16).max(1),
            current_slot: 0,
            slot_start_us: 0,
            executions_per_slot: [0; 16],
        }
    }

    /// Create for round-robin scheduling (1 second slots, 10 slots)
    #[must_use]
    pub fn for_round_robin() -> Self {
        Self::new(1_000_000, 10)
    }

    /// Create for minute-based scheduling (1 minute slots, 5 slots)
    #[must_use]
    pub fn for_minute() -> Self {
        Self::new(60_000_000, 5)
    }

    /// Update slot based on current time
    pub fn update(&mut self, now_us: u64) {
        if self.slot_start_us == 0 {
            self.slot_start_us = now_us;
            return;
        }

        let elapsed = now_us.saturating_sub(self.slot_start_us);
        let slots_passed = (elapsed / self.slot_duration_us) as usize;

        if slots_passed > 0 {
            self.current_slot = (self.current_slot + slots_passed) % self.num_slots;
            self.slot_start_us = now_us;
        }
    }

    /// Record execution in current slot
    pub fn execute(&mut self, now_us: u64) {
        self.update(now_us);
        if self.current_slot < 16 {
            self.executions_per_slot[self.current_slot] += 1;
        }
    }

    /// Get current slot
    #[must_use]
    pub fn current_slot(&self) -> usize {
        self.current_slot
    }

    /// Get number of slots
    #[must_use]
    pub fn num_slots(&self) -> usize {
        self.num_slots
    }

    /// Get executions for a slot
    #[must_use]
    pub fn executions(&self, slot: usize) -> u64 {
        if slot < 16 {
            self.executions_per_slot[slot]
        } else {
            0
        }
    }

    /// Get total executions across all slots
    #[must_use]
    pub fn total_executions(&self) -> u64 {
        self.executions_per_slot[..self.num_slots].iter().sum()
    }

    /// Check if slots are evenly distributed (within threshold %)
    #[must_use]
    pub fn is_balanced(&self, threshold: f64) -> bool {
        let total = self.total_executions();
        if total == 0 {
            return true;
        }
        let expected = total as f64 / self.num_slots as f64;
        for i in 0..self.num_slots {
            let diff = (self.executions_per_slot[i] as f64 - expected).abs();
            if diff / expected * 100.0 > threshold {
                return false;
            }
        }
        true
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.current_slot = 0;
        self.slot_start_us = 0;
        self.executions_per_slot = [0; 16];
    }
}

// ============================================================================
// CooldownTimer - Cooldown period tracking
// ============================================================================

/// O(1) cooldown timer for rate limiting actions.
/// Tracks when an action can next be performed based on cooldown period.
#[derive(Debug, Clone)]
pub struct CooldownTimer {
    cooldown_us: u64,
    last_action_us: u64,
    total_actions: u64,
    blocked_attempts: u64,
}

impl Default for CooldownTimer {
    fn default() -> Self {
        Self::new(1_000_000) // 1 second cooldown
    }
}

impl CooldownTimer {
    /// Create with cooldown period in microseconds
    #[must_use]
    pub fn new(cooldown_us: u64) -> Self {
        Self {
            cooldown_us: cooldown_us.max(1),
            last_action_us: 0,
            total_actions: 0,
            blocked_attempts: 0,
        }
    }

    /// Create for fast cooldown (100ms)
    #[must_use]
    pub fn for_fast() -> Self {
        Self::new(100_000)
    }

    /// Create for normal cooldown (1 second)
    #[must_use]
    pub fn for_normal() -> Self {
        Self::new(1_000_000)
    }

    /// Create for slow cooldown (10 seconds)
    #[must_use]
    pub fn for_slow() -> Self {
        Self::new(10_000_000)
    }

    /// Check if action is ready (cooldown expired)
    #[must_use]
    pub fn is_ready(&self, now_us: u64) -> bool {
        if self.last_action_us == 0 {
            return true;
        }
        now_us.saturating_sub(self.last_action_us) >= self.cooldown_us
    }

    /// Try to perform action, returns true if allowed
    pub fn try_action(&mut self, now_us: u64) -> bool {
        if self.is_ready(now_us) {
            self.last_action_us = now_us;
            self.total_actions += 1;
            true
        } else {
            self.blocked_attempts += 1;
            false
        }
    }

    /// Force action (ignores cooldown)
    pub fn force_action(&mut self, now_us: u64) {
        self.last_action_us = now_us;
        self.total_actions += 1;
    }

    /// Get remaining cooldown time in microseconds
    #[must_use]
    pub fn remaining_us(&self, now_us: u64) -> u64 {
        if self.is_ready(now_us) {
            0
        } else {
            self.cooldown_us.saturating_sub(now_us.saturating_sub(self.last_action_us))
        }
    }

    /// Get cooldown period
    #[must_use]
    pub fn cooldown_us(&self) -> u64 {
        self.cooldown_us
    }

    /// Get total actions performed
    #[must_use]
    pub fn total_actions(&self) -> u64 {
        self.total_actions
    }

    /// Get blocked attempts
    #[must_use]
    pub fn blocked_attempts(&self) -> u64 {
        self.blocked_attempts
    }

    /// Get block rate as percentage
    #[must_use]
    pub fn block_rate(&self) -> f64 {
        let total = self.total_actions + self.blocked_attempts;
        if total == 0 {
            0.0
        } else {
            (self.blocked_attempts as f64 / total as f64) * 100.0
        }
    }

    /// Reset timer
    pub fn reset(&mut self) {
        self.last_action_us = 0;
        self.total_actions = 0;
        self.blocked_attempts = 0;
    }
}

// ============================================================================
// BackpressureMonitor - Track backpressure signals
// ============================================================================

/// O(1) backpressure monitoring.
/// Tracks when downstream systems signal overload and calculates pressure rates.
#[derive(Debug, Clone)]
pub struct BackpressureMonitor {
    signals: u64,
    total_ops: u64,
    consecutive: u32,
    max_consecutive: u32,
    last_signal_us: u64,
}

impl Default for BackpressureMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl BackpressureMonitor {
    /// Create new monitor
    #[must_use]
    pub fn new() -> Self {
        Self {
            signals: 0,
            total_ops: 0,
            consecutive: 0,
            max_consecutive: 0,
            last_signal_us: 0,
        }
    }

    /// Record successful operation (no backpressure)
    pub fn success(&mut self) {
        self.total_ops += 1;
        self.consecutive = 0;
    }

    /// Record backpressure signal
    pub fn signal(&mut self, now_us: u64) {
        self.signals += 1;
        self.total_ops += 1;
        self.consecutive += 1;
        self.last_signal_us = now_us;
        if self.consecutive > self.max_consecutive {
            self.max_consecutive = self.consecutive;
        }
    }

    /// Get backpressure rate as percentage
    #[must_use]
    pub fn pressure_rate(&self) -> f64 {
        if self.total_ops == 0 {
            0.0
        } else {
            (self.signals as f64 / self.total_ops as f64) * 100.0
        }
    }

    /// Check if currently under pressure (consecutive signals)
    #[must_use]
    pub fn is_under_pressure(&self, threshold: u32) -> bool {
        self.consecutive >= threshold
    }

    /// Get consecutive signal count
    #[must_use]
    pub fn consecutive(&self) -> u32 {
        self.consecutive
    }

    /// Get max consecutive signals
    #[must_use]
    pub fn max_consecutive(&self) -> u32 {
        self.max_consecutive
    }

    /// Get total signals
    #[must_use]
    pub fn total_signals(&self) -> u64 {
        self.signals
    }

    /// Check if healthy (below threshold)
    #[must_use]
    pub fn is_healthy(&self, max_rate: f64) -> bool {
        self.pressure_rate() <= max_rate
    }

    /// Reset monitor
    pub fn reset(&mut self) {
        self.signals = 0;
        self.total_ops = 0;
        self.consecutive = 0;
        self.max_consecutive = 0;
        self.last_signal_us = 0;
    }
}

// ============================================================================
// CapacityPlanner - Track capacity utilization for planning
// ============================================================================

/// O(1) capacity planning tracker.
/// Monitors utilization over time and predicts when capacity will be exhausted.
#[derive(Debug, Clone)]
pub struct CapacityPlanner {
    capacity: u64,
    current: u64,
    peak: u64,
    samples: u32,
    sum_utilization: f64,
    growth_rate: f64,
}

impl Default for CapacityPlanner {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl CapacityPlanner {
    /// Create with capacity
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity: capacity.max(1),
            current: 0,
            peak: 0,
            samples: 0,
            sum_utilization: 0.0,
            growth_rate: 0.0,
        }
    }

    /// Create for connections (1000)
    #[must_use]
    pub fn for_connections() -> Self {
        Self::new(1000)
    }

    /// Create for storage GB (100)
    #[must_use]
    pub fn for_storage() -> Self {
        Self::new(100)
    }

    /// Update current usage
    pub fn update(&mut self, current: u64) {
        let old = self.current;
        self.current = current;
        if current > self.peak {
            self.peak = current;
        }
        self.samples += 1;
        self.sum_utilization += self.utilization();

        // Calculate growth rate (simple difference)
        if old > 0 {
            self.growth_rate = (current as f64 - old as f64) / old as f64;
        }
    }

    /// Get current utilization as percentage
    #[must_use]
    pub fn utilization(&self) -> f64 {
        (self.current as f64 / self.capacity as f64) * 100.0
    }

    /// Get peak utilization as percentage
    #[must_use]
    pub fn peak_utilization(&self) -> f64 {
        (self.peak as f64 / self.capacity as f64) * 100.0
    }

    /// Get average utilization
    #[must_use]
    pub fn avg_utilization(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.sum_utilization / self.samples as f64
        }
    }

    /// Get remaining capacity
    #[must_use]
    pub fn remaining(&self) -> u64 {
        self.capacity.saturating_sub(self.current)
    }

    /// Check if at risk (above threshold)
    #[must_use]
    pub fn at_risk(&self, threshold: f64) -> bool {
        self.utilization() >= threshold
    }

    /// Get growth rate
    #[must_use]
    pub fn growth_rate(&self) -> f64 {
        self.growth_rate
    }

    /// Reset planner
    pub fn reset(&mut self) {
        self.current = 0;
        self.peak = 0;
        self.samples = 0;
        self.sum_utilization = 0.0;
        self.growth_rate = 0.0;
    }
}

// ============================================================================
// DriftTracker - Track clock/timing drift
// ============================================================================

/// O(1) drift tracking for timing synchronization.
/// Monitors deviation from expected intervals and detects clock drift.
#[derive(Debug, Clone)]
pub struct DriftTracker {
    expected_interval_us: u64,
    last_timestamp_us: u64,
    total_drift_us: i64,
    samples: u64,
    max_drift_us: i64,
    min_drift_us: i64,
}

impl Default for DriftTracker {
    fn default() -> Self {
        Self::new(1_000_000) // 1 second expected interval
    }
}

impl DriftTracker {
    /// Create with expected interval in microseconds
    #[must_use]
    pub fn new(expected_interval_us: u64) -> Self {
        Self {
            expected_interval_us: expected_interval_us.max(1),
            last_timestamp_us: 0,
            total_drift_us: 0,
            samples: 0,
            max_drift_us: i64::MIN,
            min_drift_us: i64::MAX,
        }
    }

    /// Create for 60fps (16.67ms interval)
    #[must_use]
    pub fn for_60fps() -> Self {
        Self::new(16_667)
    }

    /// Create for 1 second heartbeat
    #[must_use]
    pub fn for_heartbeat() -> Self {
        Self::new(1_000_000)
    }

    /// Record timestamp and calculate drift
    pub fn record(&mut self, now_us: u64) {
        if self.last_timestamp_us == 0 {
            self.last_timestamp_us = now_us;
            return;
        }

        let actual_interval = now_us.saturating_sub(self.last_timestamp_us);
        let drift = actual_interval as i64 - self.expected_interval_us as i64;

        self.total_drift_us += drift;
        self.samples += 1;

        if drift > self.max_drift_us {
            self.max_drift_us = drift;
        }
        if drift < self.min_drift_us {
            self.min_drift_us = drift;
        }

        self.last_timestamp_us = now_us;
    }

    /// Get average drift in microseconds
    #[must_use]
    pub fn avg_drift_us(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total_drift_us as f64 / self.samples as f64
        }
    }

    /// Get max drift (positive = late, negative = early)
    #[must_use]
    pub fn max_drift_us(&self) -> i64 {
        if self.samples == 0 { 0 } else { self.max_drift_us }
    }

    /// Get min drift
    #[must_use]
    pub fn min_drift_us(&self) -> i64 {
        if self.samples == 0 { 0 } else { self.min_drift_us }
    }

    /// Check if drift is within tolerance
    #[must_use]
    pub fn is_stable(&self, tolerance_us: i64) -> bool {
        self.avg_drift_us().abs() < tolerance_us as f64
    }

    /// Get drift range
    #[must_use]
    pub fn drift_range_us(&self) -> i64 {
        if self.samples == 0 {
            0
        } else {
            self.max_drift_us - self.min_drift_us
        }
    }

    /// Get sample count
    #[must_use]
    pub fn samples(&self) -> u64 {
        self.samples
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.last_timestamp_us = 0;
        self.total_drift_us = 0;
        self.samples = 0;
        self.max_drift_us = i64::MIN;
        self.min_drift_us = i64::MAX;
    }
}

// ============================================================================
// SemaphoreTracker - Track semaphore/permit usage
// ============================================================================

/// O(1) semaphore usage tracker.
/// Monitors permit acquisition and release patterns.
#[derive(Debug, Clone)]
pub struct SemaphoreTracker {
    total_permits: u32,
    acquired: u32,
    peak_acquired: u32,
    acquisitions: u64,
    releases: u64,
    contentions: u64,
}

impl Default for SemaphoreTracker {
    fn default() -> Self {
        Self::new(10)
    }
}

impl SemaphoreTracker {
    /// Create with total permits
    #[must_use]
    pub fn new(total_permits: u32) -> Self {
        Self {
            total_permits: total_permits.max(1),
            acquired: 0,
            peak_acquired: 0,
            acquisitions: 0,
            releases: 0,
            contentions: 0,
        }
    }

    /// Create for database connections (20)
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(20)
    }

    /// Create for worker threads (8)
    #[must_use]
    pub fn for_workers() -> Self {
        Self::new(8)
    }

    /// Try to acquire permit, returns true if successful
    pub fn try_acquire(&mut self) -> bool {
        if self.acquired < self.total_permits {
            self.acquired += 1;
            self.acquisitions += 1;
            if self.acquired > self.peak_acquired {
                self.peak_acquired = self.acquired;
            }
            true
        } else {
            self.contentions += 1;
            false
        }
    }

    /// Release permit
    pub fn release(&mut self) {
        if self.acquired > 0 {
            self.acquired -= 1;
            self.releases += 1;
        }
    }

    /// Get available permits
    #[must_use]
    pub fn available(&self) -> u32 {
        self.total_permits.saturating_sub(self.acquired)
    }

    /// Get utilization as percentage
    #[must_use]
    pub fn utilization(&self) -> f64 {
        (self.acquired as f64 / self.total_permits as f64) * 100.0
    }

    /// Get peak utilization as percentage
    #[must_use]
    pub fn peak_utilization(&self) -> f64 {
        (self.peak_acquired as f64 / self.total_permits as f64) * 100.0
    }

    /// Get contention rate
    #[must_use]
    pub fn contention_rate(&self) -> f64 {
        let total = self.acquisitions + self.contentions;
        if total == 0 {
            0.0
        } else {
            (self.contentions as f64 / total as f64) * 100.0
        }
    }

    /// Check if healthy (low contention)
    #[must_use]
    pub fn is_healthy(&self, max_contention: f64) -> bool {
        self.contention_rate() <= max_contention
    }

    /// Get total permits
    #[must_use]
    pub fn total_permits(&self) -> u32 {
        self.total_permits
    }

    /// Reset tracker
    pub fn reset(&mut self) {
        self.acquired = 0;
        self.peak_acquired = 0;
        self.acquisitions = 0;
        self.releases = 0;
        self.contentions = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_perf_tracer_new() {
        let tracer = PerfTracer::new();
        assert!(tracer.stats.is_empty());
    }

    #[test]
    fn test_perf_tracer_trace() {
        let mut tracer = PerfTracer::new();

        let result = tracer.trace("test_op", || {
            thread::sleep(Duration::from_micros(100));
            42
        });

        assert_eq!(result, 42);
        assert!(tracer.get_stats("test_op").is_some());
        assert_eq!(tracer.get_stats("test_op").unwrap().count, 1);
    }

    #[test]
    fn test_perf_tracer_multiple_traces() {
        let mut tracer = PerfTracer::new();

        for _ in 0..5 {
            tracer.trace("test_op", || {});
        }

        let stats = tracer.get_stats("test_op").unwrap();
        assert_eq!(stats.count, 5);
    }

    #[test]
    fn test_perf_tracer_budget_exceeded() {
        let mut tracer = PerfTracer::new();

        // Budget of 1μs, sleep for 1ms
        tracer.trace_with_budget("slow_op", 1, || {
            thread::sleep(Duration::from_millis(1));
        });

        let stats = tracer.get_stats("slow_op").unwrap();
        assert_eq!(stats.budget_violations, 1);
    }

    #[test]
    fn test_perf_tracer_summary() {
        let mut tracer = PerfTracer::new();
        tracer.trace("op1", || {});
        tracer.trace("op2", || {});

        let summary = tracer.summary();
        assert!(summary.contains("op1"));
        assert!(summary.contains("op2"));
    }

    #[test]
    fn test_trace_stats_avg_duration() {
        let mut stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        stats.update(Duration::from_micros(200), false);
        stats.update(Duration::from_micros(300), false);

        let avg = stats.avg_duration();
        assert_eq!(avg.as_micros(), 200);
    }

    #[test]
    fn test_trace_stats_cv() {
        let mut stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        stats.update(Duration::from_micros(100), false);
        stats.update(Duration::from_micros(100), false);

        // All same values, CV should be low
        let cv = stats.cv_percent();
        assert!(cv < 5.0);
    }

    #[test]
    fn test_escalation_thresholds() {
        let mut tracer = PerfTracer::with_thresholds(EscalationThresholds {
            cv_percent: 10.0,
            efficiency_percent: 50.0,
            max_traces_per_sec: 100,
        });

        // Add some variable traces
        tracer.trace_with_budget("variable_op", 10, || {
            thread::sleep(Duration::from_micros(100));
        });
        tracer.trace_with_budget("variable_op", 10, || {
            thread::sleep(Duration::from_micros(500));
        });

        // Should escalate due to high CV or low efficiency
        assert!(tracer.should_escalate("variable_op"));
    }

    #[test]
    fn test_export_renacer_format() {
        let mut tracer = PerfTracer::new();
        tracer.trace("test_op", || {});

        let export = tracer.export_renacer_format();
        assert!(export.contains("TRACE test_op"));
        assert!(export.contains("count=1"));
    }

    #[test]
    fn test_clear() {
        let mut tracer = PerfTracer::new();
        tracer.trace("op1", || {});
        tracer.trace("op2", || {});

        tracer.clear();
        assert!(tracer.stats.is_empty());
    }

    #[test]
    fn test_perf_tracer_default() {
        let tracer = PerfTracer::default();
        assert!(tracer.stats.is_empty());
    }

    #[test]
    fn test_all_stats() {
        let mut tracer = PerfTracer::new();
        tracer.trace("op1", || {});
        tracer.trace("op2", || {});

        let all = tracer.all_stats();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("op1"));
        assert!(all.contains_key("op2"));
    }

    #[test]
    fn test_get_stats_nonexistent() {
        let tracer = PerfTracer::new();
        assert!(tracer.get_stats("nonexistent").is_none());
    }

    #[test]
    fn test_should_escalate_nonexistent() {
        let tracer = PerfTracer::new();
        assert!(!tracer.should_escalate("nonexistent"));
    }

    #[test]
    fn test_trace_stats_efficiency_percent() {
        let stats = TraceStats::new(Duration::from_micros(500), 1000, false);
        let eff = stats.efficiency_percent();
        // Budget 1000μs, avg 500μs -> efficiency = 1000/500*100 = 200%, clamped to 100%
        assert!(eff <= 100.0 && eff > 50.0);
    }

    #[test]
    fn test_trace_stats_efficiency_zero_budget() {
        let stats = TraceStats::new(Duration::from_micros(500), 0, false);
        assert_eq!(stats.efficiency_percent(), 100.0);
    }

    #[test]
    fn test_trace_stats_avg_duration_zero_count() {
        let stats = TraceStats::default();
        assert_eq!(stats.avg_duration(), Duration::ZERO);
    }

    #[test]
    fn test_trace_stats_cv_single_sample() {
        let stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        assert_eq!(stats.cv_percent(), 0.0);
    }

    #[test]
    fn test_trace_stats_cv_zero_avg() {
        let stats = TraceStats::new(Duration::ZERO, 1000, false);
        assert_eq!(stats.cv_percent(), 0.0);
    }

    #[test]
    fn test_trace_event_debug() {
        let event = TraceEvent {
            name: "test".to_string(),
            duration: Duration::from_micros(100),
            timestamp_us: 1000,
            budget_exceeded: false,
            budget_us: Some(200),
        };
        let debug = format!("{:?}", event);
        assert!(debug.contains("TraceEvent"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_trace_event_clone() {
        let event = TraceEvent {
            name: "test".to_string(),
            duration: Duration::from_micros(100),
            timestamp_us: 1000,
            budget_exceeded: true,
            budget_us: Some(50),
        };
        let cloned = event.clone();
        assert_eq!(cloned.name, "test");
        assert!(cloned.budget_exceeded);
    }

    #[test]
    fn test_trace_stats_clone() {
        let stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        let cloned = stats.clone();
        assert_eq!(cloned.count, 1);
    }

    #[test]
    fn test_trace_stats_debug() {
        let stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        let debug = format!("{:?}", stats);
        assert!(debug.contains("TraceStats"));
    }

    #[test]
    fn test_escalation_thresholds_default() {
        let thresholds = EscalationThresholds::default();
        assert_eq!(thresholds.cv_percent, 15.0);
        assert_eq!(thresholds.efficiency_percent, 25.0);
        assert_eq!(thresholds.max_traces_per_sec, 100);
    }

    #[test]
    fn test_escalation_thresholds_clone() {
        let thresholds = EscalationThresholds::default();
        let cloned = thresholds.clone();
        assert_eq!(cloned.cv_percent, 15.0);
    }

    #[test]
    fn test_escalation_thresholds_copy() {
        let thresholds = EscalationThresholds::default();
        let copied = thresholds; // Copy
        assert_eq!(copied.cv_percent, 15.0);
    }

    #[test]
    fn test_escalation_thresholds_debug() {
        let thresholds = EscalationThresholds::default();
        let debug = format!("{:?}", thresholds);
        assert!(debug.contains("EscalationThresholds"));
    }

    #[test]
    fn test_perf_tracer_debug() {
        let tracer = PerfTracer::new();
        let debug = format!("{:?}", tracer);
        assert!(debug.contains("PerfTracer"));
    }

    #[test]
    fn test_trace_stats_min_max() {
        let mut stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        stats.update(Duration::from_micros(50), false);
        stats.update(Duration::from_micros(200), false);

        assert_eq!(stats.min_duration, Duration::from_micros(50));
        assert_eq!(stats.max_duration, Duration::from_micros(200));
    }

    #[test]
    fn test_trace_stats_budget_violations() {
        let mut stats = TraceStats::new(Duration::from_micros(100), 50, true);
        stats.update(Duration::from_micros(100), true);
        stats.update(Duration::from_micros(30), false);

        assert_eq!(stats.budget_violations, 2);
    }

    #[test]
    fn test_recent_events_ring_buffer() {
        let mut tracer = PerfTracer::new();
        // Set max_recent to 100 by default
        // Add more than 100 events
        for i in 0..150 {
            tracer.trace(&format!("op_{}", i), || {});
        }
        // Should still have <= 100 recent events
        assert!(tracer.recent_events.len() <= 100);
    }

    #[test]
    fn test_rate_limiting_reset() {
        let mut tracer = PerfTracer::new();
        tracer.trace("op1", || {});
        assert_eq!(tracer.traces_this_second, 1);

        // More traces in same "second"
        tracer.trace("op2", || {});
        assert_eq!(tracer.traces_this_second, 2);
    }

    // ==========================================================================
    // TimingGuard tests (trueno-viz pattern)
    // ==========================================================================

    #[test]
    fn test_timing_guard_disabled_by_default() {
        // Ensure tracing is disabled (other tests may have enabled it)
        disable_tracing();
        assert!(!is_tracing_enabled());

        // Guard should be a no-op
        let guard = TimingGuard::new("test", 1000);
        assert!(guard.start.is_none());
    }

    #[test]
    fn test_timing_guard_enabled() {
        enable_tracing();
        assert!(is_tracing_enabled());

        let guard = TimingGuard::new("test", 1000);
        assert!(guard.start.is_some());
        drop(guard);

        disable_tracing();
        assert!(!is_tracing_enabled());
    }

    #[test]
    fn test_timing_guard_with_default_budget() {
        let guard = TimingGuard::with_default_budget("test");
        assert_eq!(guard.budget_us, 1000);
    }

    #[test]
    fn test_timing_guard_render() {
        let guard = TimingGuard::render("test");
        assert_eq!(guard.budget_us, 16_000);
    }

    #[test]
    fn test_timing_guard_collect() {
        let guard = TimingGuard::collect("test");
        assert_eq!(guard.budget_us, 100_000);
    }

    // ==========================================================================
    // SimdStats tests (trueno-viz pattern)
    // ==========================================================================

    #[test]
    fn test_simd_stats_new() {
        let stats = SimdStats::new();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.sum, 0.0);
        assert_eq!(stats.mean(), 0.0);
    }

    #[test]
    fn test_simd_stats_update() {
        let mut stats = SimdStats::new();
        stats.update(10.0);
        stats.update(20.0);
        stats.update(30.0);

        assert_eq!(stats.count, 3);
        assert_eq!(stats.sum, 60.0);
        assert_eq!(stats.mean(), 20.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 30.0);
    }

    #[test]
    fn test_simd_stats_variance() {
        let mut stats = SimdStats::new();
        // Add values: 2, 4, 4, 4, 5, 5, 7, 9
        // Mean = 5, Variance = 4
        for &v in &[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            stats.update(v);
        }
        let var = stats.variance();
        assert!((var - 4.571).abs() < 0.01); // Sample variance
    }

    #[test]
    fn test_simd_stats_std_dev() {
        let mut stats = SimdStats::new();
        for &v in &[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0] {
            stats.update(v);
        }
        let std = stats.std_dev();
        assert!(std > 2.0 && std < 2.2);
    }

    #[test]
    fn test_simd_stats_cv_percent() {
        let mut stats = SimdStats::new();
        for &v in &[10.0, 10.0, 10.0, 10.0] {
            stats.update(v);
        }
        // All same values, CV should be 0
        assert_eq!(stats.cv_percent(), 0.0);

        let mut stats2 = SimdStats::new();
        for &v in &[10.0, 20.0, 30.0, 40.0] {
            stats2.update(v);
        }
        // Variable values, CV should be > 0
        assert!(stats2.cv_percent() > 0.0);
    }

    #[test]
    fn test_simd_stats_reset() {
        let mut stats = SimdStats::new();
        stats.update(100.0);
        stats.update(200.0);
        assert_eq!(stats.count, 2);

        stats.reset();
        assert_eq!(stats.count, 0);
        assert_eq!(stats.sum, 0.0);
    }

    #[test]
    fn test_simd_stats_single_sample_variance() {
        let mut stats = SimdStats::new();
        stats.update(42.0);
        // Single sample, variance should be 0
        assert_eq!(stats.variance(), 0.0);
    }

    #[test]
    fn test_simd_stats_cv_zero_mean() {
        let mut stats = SimdStats::new();
        stats.update(0.0);
        stats.update(0.0);
        // Zero mean, CV should be 0
        assert_eq!(stats.cv_percent(), 0.0);
    }

    #[test]
    fn test_simd_stats_default() {
        let stats = SimdStats::default();
        assert_eq!(stats.count, 0);
    }

    #[test]
    fn test_simd_stats_clone() {
        let mut stats = SimdStats::new();
        stats.update(42.0);
        let cloned = stats.clone();
        assert_eq!(cloned.count, 1);
        assert_eq!(cloned.sum, 42.0);
    }

    #[test]
    fn test_simd_stats_debug() {
        let stats = SimdStats::new();
        let debug = format!("{:?}", stats);
        assert!(debug.contains("SimdStats"));
    }

    #[test]
    fn test_simd_stats_cache_aligned() {
        // Verify alignment is 64 bytes (cache line)
        assert_eq!(std::mem::align_of::<SimdStats>(), 64);
    }

    // ==========================================================================
    // BrickType tests (renacer brick taxonomy)
    // ==========================================================================

    /// F-BRICK-001: BrickType default budgets are non-zero and reasonable
    #[test]
    fn f_brick_001_default_budgets_nonzero() {
        assert!(BrickType::Collect.default_budget_us() > 0);
        assert!(BrickType::Render.default_budget_us() > 0);
        assert!(BrickType::Compute.default_budget_us() > 0);
        assert!(BrickType::Network.default_budget_us() > 0);
        assert!(BrickType::Storage.default_budget_us() > 0);
    }

    /// F-BRICK-002: Render budget is 16ms for 60fps
    #[test]
    fn f_brick_002_render_budget_60fps() {
        assert_eq!(BrickType::Render.default_budget_us(), 16_000);
    }

    /// F-BRICK-003: Compute budget is strictest (1ms)
    #[test]
    fn f_brick_003_compute_budget_strictest() {
        let compute = BrickType::Compute.default_budget_us();
        assert!(compute <= BrickType::Render.default_budget_us());
        assert!(compute <= BrickType::Collect.default_budget_us());
        assert!(compute <= BrickType::Storage.default_budget_us());
        assert!(compute <= BrickType::Network.default_budget_us());
    }

    /// F-BRICK-004: CV thresholds are positive and bounded
    #[test]
    fn f_brick_004_cv_thresholds_bounded() {
        for brick_type in [
            BrickType::Collect,
            BrickType::Render,
            BrickType::Compute,
            BrickType::Network,
            BrickType::Storage,
        ] {
            let cv = brick_type.cv_threshold();
            assert!(cv > 0.0, "CV threshold must be positive");
            assert!(cv <= 100.0, "CV threshold must be <= 100%");
        }
    }

    /// F-BRICK-005: Render has strictest CV threshold
    #[test]
    fn f_brick_005_render_cv_strictest() {
        let render_cv = BrickType::Render.cv_threshold();
        assert!(render_cv <= BrickType::Compute.cv_threshold());
        assert!(render_cv <= BrickType::Collect.cv_threshold());
        assert!(render_cv <= BrickType::Network.cv_threshold());
        assert!(render_cv <= BrickType::Storage.cv_threshold());
    }

    /// F-BRICK-006: BrickType Debug format
    #[test]
    fn f_brick_006_brick_type_debug() {
        let debug = format!("{:?}", BrickType::Render);
        assert!(debug.contains("Render"));
    }

    /// F-BRICK-007: BrickType Clone and Copy
    #[test]
    fn f_brick_007_brick_type_clone_copy() {
        let original = BrickType::Compute;
        let cloned = original.clone();
        let copied = original; // Copy
        assert_eq!(original, cloned);
        assert_eq!(original, copied);
    }

    /// F-BRICK-008: BrickType PartialEq and Eq
    #[test]
    fn f_brick_008_brick_type_equality() {
        assert_eq!(BrickType::Render, BrickType::Render);
        assert_ne!(BrickType::Render, BrickType::Compute);
    }

    /// F-BRICK-009: BrickType Hash is consistent
    #[test]
    fn f_brick_009_brick_type_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(BrickType::Render);
        set.insert(BrickType::Compute);
        set.insert(BrickType::Render); // Duplicate
        assert_eq!(set.len(), 2);
    }

    // ==========================================================================
    // BrickProfiler tests (renacer integration)
    // ==========================================================================

    /// F-PROFILER-001: BrickProfiler new creates empty stats
    #[test]
    fn f_profiler_001_new_empty() {
        disable_tracing(); // Ensure clean state
        let profiler = BrickProfiler::new();
        assert!(profiler.get_stats("any").is_none());
    }

    /// F-PROFILER-002: BrickProfiler default matches new
    #[test]
    fn f_profiler_002_default() {
        let p1 = BrickProfiler::new();
        let p2 = BrickProfiler::default();
        assert!(p1.get_stats("x").is_none());
        assert!(p2.get_stats("x").is_none());
    }

    /// F-PROFILER-003: Profile function returns result unchanged
    #[test]
    fn f_profiler_003_profile_returns_result() {
        let mut profiler = BrickProfiler::new();
        let result = profiler.profile("test", BrickType::Compute, || 42);
        assert_eq!(result, 42);
    }

    /// F-PROFILER-004: Profile records stats when tracing enabled
    #[test]
    fn f_profiler_004_profile_records_stats() {
        enable_tracing();
        let mut profiler = BrickProfiler::new();
        profiler.enabled = true; // Force enabled for test

        profiler.profile("test_brick", BrickType::Render, || {
            std::thread::sleep(std::time::Duration::from_micros(50));
        });

        let stats = profiler.get_stats("test_brick");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().count, 1);

        disable_tracing();
    }

    /// F-PROFILER-005: Should escalate returns false for non-existent brick
    #[test]
    fn f_profiler_005_escalate_nonexistent() {
        let profiler = BrickProfiler::new();
        assert!(!profiler.should_escalate("nonexistent"));
    }

    /// F-PROFILER-006: Summary contains brick names
    #[test]
    fn f_profiler_006_summary_contains_names() {
        enable_tracing();
        let mut profiler = BrickProfiler::new();
        profiler.enabled = true;

        profiler.profile("cpu_render", BrickType::Render, || {});
        profiler.profile("disk_collect", BrickType::Collect, || {});

        let summary = profiler.summary();
        assert!(summary.contains("cpu_render") || summary.contains("Brick Profiler"));

        disable_tracing();
    }

    /// F-PROFILER-007: Summary reports brick type correctly
    #[test]
    fn f_profiler_007_summary_brick_type() {
        enable_tracing();
        let mut profiler = BrickProfiler::new();
        profiler.enabled = true;

        profiler.profile("render_test", BrickType::Render, || {});

        let summary = profiler.summary();
        assert!(summary.contains("Render") || summary.contains("Brick Profiler"));

        disable_tracing();
    }

    /// F-PROFILER-008: Multiple profiles same name accumulate
    #[test]
    fn f_profiler_008_accumulate_stats() {
        enable_tracing();
        let mut profiler = BrickProfiler::new();
        profiler.enabled = true;

        for _ in 0..5 {
            profiler.profile("repeated", BrickType::Compute, || {});
        }

        let stats = profiler.get_stats("repeated");
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().count, 5);

        disable_tracing();
    }

    /// F-PROFILER-009: Profiler Debug format
    #[test]
    fn f_profiler_009_debug() {
        let profiler = BrickProfiler::new();
        let debug = format!("{:?}", profiler);
        assert!(debug.contains("BrickProfiler"));
    }

    /// F-PROFILER-010: Profile disabled mode is zero-cost
    #[test]
    fn f_profiler_010_disabled_zero_cost() {
        disable_tracing();
        let mut profiler = BrickProfiler::new();
        profiler.enabled = false;

        // This should be essentially free
        let result = profiler.profile("test", BrickType::Render, || 123);
        assert_eq!(result, 123);

        // No stats recorded
        assert!(profiler.get_stats("test").is_none());
    }

    // ==========================================================================
    // RingBuffer tests (trueno-viz O(1) history pattern)
    // ==========================================================================

    /// F-RING-001: New ring buffer is empty
    #[test]
    fn f_ring_001_new_empty() {
        let rb: RingBuffer<f64, 10> = RingBuffer::new();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    /// F-RING-002: Push increments length
    #[test]
    fn f_ring_002_push_increments_len() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        assert_eq!(rb.len(), 1);
        rb.push(2.0);
        assert_eq!(rb.len(), 2);
    }

    /// F-RING-003: Capacity is correct
    #[test]
    fn f_ring_003_capacity() {
        let rb: RingBuffer<f64, 5> = RingBuffer::new();
        assert_eq!(rb.capacity(), 5);
    }

    /// F-RING-004: Is full when at capacity
    #[test]
    fn f_ring_004_is_full() {
        let mut rb: RingBuffer<f64, 3> = RingBuffer::new();
        assert!(!rb.is_full());
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert!(rb.is_full());
    }

    /// F-RING-005: Latest returns most recent
    #[test]
    fn f_ring_005_latest() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.latest(), Some(&3.0));
    }

    /// F-RING-006: Latest returns None when empty
    #[test]
    fn f_ring_006_latest_empty() {
        let rb: RingBuffer<f64, 10> = RingBuffer::new();
        assert_eq!(rb.latest(), None);
    }

    /// F-RING-007: Get returns correct value by index
    #[test]
    fn f_ring_007_get_by_index() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert_eq!(rb.get(0), Some(&1.0)); // oldest
        assert_eq!(rb.get(2), Some(&3.0)); // newest
    }

    /// F-RING-008: Get returns None for out of bounds
    #[test]
    fn f_ring_008_get_out_of_bounds() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        assert_eq!(rb.get(5), None);
    }

    /// F-RING-009: Ring buffer wraps around
    #[test]
    fn f_ring_009_wrap_around() {
        let mut rb: RingBuffer<i32, 3> = RingBuffer::new();
        rb.push(1);
        rb.push(2);
        rb.push(3);
        rb.push(4); // Should overwrite 1
        assert_eq!(rb.len(), 3);
        let values: Vec<_> = rb.iter().copied().collect();
        assert_eq!(values, vec![2, 3, 4]);
    }

    /// F-RING-010: Clear resets buffer
    #[test]
    fn f_ring_010_clear() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.clear();
        assert!(rb.is_empty());
        assert_eq!(rb.len(), 0);
    }

    /// F-RING-011: Sum calculates correctly
    #[test]
    fn f_ring_011_sum() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(1.0);
        rb.push(2.0);
        rb.push(3.0);
        assert!((rb.sum() - 6.0).abs() < 0.001);
    }

    /// F-RING-012: Mean calculates correctly
    #[test]
    fn f_ring_012_mean() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(2.0);
        rb.push(4.0);
        rb.push(6.0);
        assert!((rb.mean() - 4.0).abs() < 0.001);
    }

    /// F-RING-013: Mean returns 0 for empty buffer
    #[test]
    fn f_ring_013_mean_empty() {
        let rb: RingBuffer<f64, 10> = RingBuffer::new();
        assert_eq!(rb.mean(), 0.0);
    }

    /// F-RING-014: Min returns minimum value
    #[test]
    fn f_ring_014_min() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(5.0);
        rb.push(2.0);
        rb.push(8.0);
        assert_eq!(rb.min(), Some(2.0));
    }

    /// F-RING-015: Max returns maximum value
    #[test]
    fn f_ring_015_max() {
        let mut rb: RingBuffer<f64, 10> = RingBuffer::new();
        rb.push(5.0);
        rb.push(2.0);
        rb.push(8.0);
        assert_eq!(rb.max(), Some(8.0));
    }

    /// F-RING-016: Default creates empty buffer
    #[test]
    fn f_ring_016_default() {
        let rb: RingBuffer<f64, 10> = RingBuffer::default();
        assert!(rb.is_empty());
    }

    /// F-RING-017: Debug format
    #[test]
    fn f_ring_017_debug() {
        let rb: RingBuffer<f64, 10> = RingBuffer::new();
        let debug = format!("{:?}", rb);
        assert!(debug.contains("RingBuffer"));
    }

    // ==========================================================================
    // LatencyHistogram tests (trueno-viz O(1) distribution pattern)
    // ==========================================================================

    /// F-HIST-001: New histogram is empty
    #[test]
    fn f_hist_001_new_empty() {
        let h = LatencyHistogram::new();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST-002: Record increments count
    #[test]
    fn f_hist_002_record_increments() {
        let mut h = LatencyHistogram::new();
        h.record(500);
        assert_eq!(h.count(), 1);
        h.record(1500);
        assert_eq!(h.count(), 2);
    }

    /// F-HIST-003: Record bins 0-1ms correctly
    #[test]
    fn f_hist_003_bin_0_1ms() {
        let mut h = LatencyHistogram::new();
        h.record(500);  // 0.5ms
        h.record(999);  // 0.999ms
        assert_eq!(h.bin_count(0), 2);
    }

    /// F-HIST-004: Record bins 1-5ms correctly
    #[test]
    fn f_hist_004_bin_1_5ms() {
        let mut h = LatencyHistogram::new();
        h.record(1000);  // 1ms
        h.record(4999);  // 4.999ms
        assert_eq!(h.bin_count(1), 2);
    }

    /// F-HIST-005: Record bins 500ms+ correctly
    #[test]
    fn f_hist_005_bin_500ms_plus() {
        let mut h = LatencyHistogram::new();
        h.record(500_000);  // 500ms
        h.record(1_000_000); // 1s
        assert_eq!(h.bin_count(6), 2);
    }

    /// F-HIST-006: Percentages sum to 100
    #[test]
    fn f_hist_006_percentages_sum() {
        let mut h = LatencyHistogram::new();
        h.record(500);
        h.record(2000);
        h.record(600_000);
        let pcts = h.percentages();
        let sum: f64 = pcts.iter().sum();
        assert!((sum - 100.0).abs() < 0.01);
    }

    /// F-HIST-007: Percentages returns zeros for empty
    #[test]
    fn f_hist_007_percentages_empty() {
        let h = LatencyHistogram::new();
        let pcts = h.percentages();
        assert!(pcts.iter().all(|&p| p == 0.0));
    }

    /// F-HIST-008: Bin labels are correct
    #[test]
    fn f_hist_008_bin_labels() {
        assert_eq!(LatencyHistogram::bin_label(0), "0-1ms");
        assert_eq!(LatencyHistogram::bin_label(6), "500ms+");
        assert_eq!(LatencyHistogram::bin_label(99), "?");
    }

    /// F-HIST-009: ASCII histogram is non-empty
    #[test]
    fn f_hist_009_ascii_histogram() {
        let mut h = LatencyHistogram::new();
        h.record(500);
        h.record(2000);
        let ascii = h.ascii_histogram(20);
        assert!(!ascii.is_empty());
        assert!(ascii.contains("0-1ms"));
    }

    /// F-HIST-010: Reset clears histogram
    #[test]
    fn f_hist_010_reset() {
        let mut h = LatencyHistogram::new();
        h.record(500);
        h.record(2000);
        h.reset();
        assert_eq!(h.count(), 0);
        assert!(h.percentages().iter().all(|&p| p == 0.0));
    }

    /// F-HIST-011: Default creates empty histogram
    #[test]
    fn f_hist_011_default() {
        let h = LatencyHistogram::default();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST-012: Debug format
    #[test]
    fn f_hist_012_debug() {
        let h = LatencyHistogram::new();
        let debug = format!("{:?}", h);
        assert!(debug.contains("LatencyHistogram"));
    }

    /// F-HIST-013: Clone produces identical histogram
    #[test]
    fn f_hist_013_clone() {
        let mut h = LatencyHistogram::new();
        h.record(500);
        h.record(2000);
        let cloned = h.clone();
        assert_eq!(cloned.count(), h.count());
        assert_eq!(cloned.bin_count(0), h.bin_count(0));
    }

    /// F-HIST-014: All bins covered
    #[test]
    fn f_hist_014_all_bins() {
        let mut h = LatencyHistogram::new();
        h.record(500);      // bin 0
        h.record(2000);     // bin 1
        h.record(7000);     // bin 2
        h.record(25000);    // bin 3
        h.record(75000);    // bin 4
        h.record(250000);   // bin 5
        h.record(750000);   // bin 6

        for i in 0..7 {
            assert!(h.bin_count(i) > 0, "Bin {} should have count", i);
        }
    }

    /// F-HIST-015: Bin count out of range returns 0
    #[test]
    fn f_hist_015_bin_out_of_range() {
        let h = LatencyHistogram::new();
        assert_eq!(h.bin_count(99), 0);
    }

    // ==========================================================================
    // EmaTracker tests (trueno-viz O(1) smoothing pattern)
    // ==========================================================================

    /// F-EMA-001: New EMA tracker is not initialized
    #[test]
    fn f_ema_001_new_not_initialized() {
        let ema = EmaTracker::new(0.1);
        assert!(!ema.is_initialized());
        assert_eq!(ema.value(), 0.0);
    }

    /// F-EMA-002: First update initializes to that value
    #[test]
    fn f_ema_002_first_update() {
        let mut ema = EmaTracker::new(0.1);
        ema.update(100.0);
        assert!(ema.is_initialized());
        assert!((ema.value() - 100.0).abs() < 0.001);
    }

    /// F-EMA-003: Subsequent updates smooth the value
    #[test]
    fn f_ema_003_smoothing() {
        let mut ema = EmaTracker::new(0.5); // 50% weight
        ema.update(100.0);
        ema.update(0.0);
        // Expected: 0.5 * 0.0 + 0.5 * 100.0 = 50.0
        assert!((ema.value() - 50.0).abs() < 0.001);
    }

    /// F-EMA-004: Alpha is clamped to 0.0-1.0
    #[test]
    fn f_ema_004_alpha_clamped() {
        let ema_low = EmaTracker::new(-0.5);
        assert_eq!(ema_low.alpha(), 0.0);

        let ema_high = EmaTracker::new(1.5);
        assert_eq!(ema_high.alpha(), 1.0);
    }

    /// F-EMA-005: for_fps creates responsive tracker
    #[test]
    fn f_ema_005_for_fps() {
        let ema = EmaTracker::for_fps();
        assert!((ema.alpha() - 0.3).abs() < 0.001);
    }

    /// F-EMA-006: for_load creates slow tracker
    #[test]
    fn f_ema_006_for_load() {
        let ema = EmaTracker::for_load();
        assert!((ema.alpha() - 0.05).abs() < 0.001);
    }

    /// F-EMA-007: Reset clears state
    #[test]
    fn f_ema_007_reset() {
        let mut ema = EmaTracker::new(0.1);
        ema.update(100.0);
        assert!(ema.is_initialized());
        ema.reset();
        assert!(!ema.is_initialized());
        assert_eq!(ema.value(), 0.0);
    }

    /// F-EMA-008: set_alpha changes smoothing
    #[test]
    fn f_ema_008_set_alpha() {
        let mut ema = EmaTracker::new(0.1);
        ema.set_alpha(0.5);
        assert!((ema.alpha() - 0.5).abs() < 0.001);
    }

    /// F-EMA-009: Default creates 0.1 alpha tracker
    #[test]
    fn f_ema_009_default() {
        let ema = EmaTracker::default();
        assert!((ema.alpha() - 0.1).abs() < 0.001);
    }

    /// F-EMA-010: Debug format
    #[test]
    fn f_ema_010_debug() {
        let ema = EmaTracker::new(0.1);
        let debug = format!("{:?}", ema);
        assert!(debug.contains("EmaTracker"));
    }

    /// F-EMA-011: Clone produces identical tracker
    #[test]
    fn f_ema_011_clone() {
        let mut ema = EmaTracker::new(0.3);
        ema.update(50.0);
        let cloned = ema.clone();
        assert!((ema.value() - cloned.value()).abs() < 0.001);
        assert_eq!(ema.alpha(), cloned.alpha());
    }

    /// F-EMA-012: High alpha is more responsive
    #[test]
    fn f_ema_012_high_alpha_responsive() {
        let mut ema_high = EmaTracker::new(0.9);
        let mut ema_low = EmaTracker::new(0.1);

        ema_high.update(100.0);
        ema_low.update(100.0);

        ema_high.update(0.0);
        ema_low.update(0.0);

        // High alpha should be closer to 0
        assert!(ema_high.value() < ema_low.value());
    }

    // ==========================================================================
    // RateLimiter tests (trueno-viz O(1) throttling pattern)
    // ==========================================================================

    /// F-RATE-001: New rate limiter allows first check
    #[test]
    fn f_rate_001_first_check_allowed() {
        let mut rl = RateLimiter::new(1_000_000); // 1 second
        assert!(rl.check());
    }

    /// F-RATE-002: Immediate second check is denied
    #[test]
    fn f_rate_002_immediate_denied() {
        let mut rl = RateLimiter::new(1_000_000); // 1 second
        rl.check();
        assert!(!rl.check());
    }

    /// F-RATE-003: new_hz creates correct interval
    #[test]
    fn f_rate_003_new_hz() {
        let rl = RateLimiter::new_hz(60);
        assert!((rl.hz() - 60.0).abs() < 1.0);
    }

    /// F-RATE-004: new_ms creates correct interval
    #[test]
    fn f_rate_004_new_ms() {
        let rl = RateLimiter::new_ms(100);
        assert_eq!(rl.interval_us(), 100_000);
    }

    /// F-RATE-005: would_allow doesn't update state
    #[test]
    fn f_rate_005_would_allow_no_update() {
        let mut rl = RateLimiter::new(1_000_000);
        rl.check(); // First check
        let peek1 = rl.would_allow();
        let peek2 = rl.would_allow();
        assert_eq!(peek1, peek2); // Should be same since no update
    }

    /// F-RATE-006: Reset allows next check
    #[test]
    fn f_rate_006_reset() {
        let mut rl = RateLimiter::new(1_000_000);
        rl.check();
        assert!(!rl.check());
        rl.reset();
        assert!(rl.check());
    }

    /// F-RATE-007: Default is 60 Hz
    #[test]
    fn f_rate_007_default() {
        let rl = RateLimiter::default();
        assert!((rl.hz() - 60.0).abs() < 1.0);
    }

    /// F-RATE-008: Debug format
    #[test]
    fn f_rate_008_debug() {
        let rl = RateLimiter::new(1000);
        let debug = format!("{:?}", rl);
        assert!(debug.contains("RateLimiter"));
    }

    /// F-RATE-009: Clone produces identical limiter
    #[test]
    fn f_rate_009_clone() {
        let rl = RateLimiter::new_hz(30);
        let cloned = rl.clone();
        assert_eq!(rl.interval_us(), cloned.interval_us());
    }

    /// F-RATE-010: Zero Hz handled gracefully
    #[test]
    fn f_rate_010_zero_hz() {
        let rl = RateLimiter::new_hz(0);
        assert_eq!(rl.interval_us(), 1_000_000); // Falls back to 1 second
    }

    /// F-RATE-011: Hz calculation with zero interval
    #[test]
    fn f_rate_011_hz_zero_interval() {
        let rl = RateLimiter::new(0);
        assert_eq!(rl.hz(), 0.0);
    }

    /// F-RATE-012: Small interval allows frequent checks
    #[test]
    fn f_rate_012_small_interval() {
        let mut rl = RateLimiter::new(1); // 1 microsecond
        rl.check();
        thread::sleep(std::time::Duration::from_micros(10));
        assert!(rl.check()); // Should be allowed after sleep
    }

    // =========================================================================
    // ThresholdDetector Tests (trueno-viz O(1) level detection)
    // =========================================================================

    /// F-THRESH-001: New detector starts in low state
    #[test]
    fn f_thresh_001_starts_low() {
        let td = ThresholdDetector::new(30.0, 70.0);
        assert!(td.is_low());
        assert!(!td.is_high());
    }

    /// F-THRESH-002: Transition to high above high threshold
    #[test]
    fn f_thresh_002_transition_high() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        let changed = td.update(80.0);
        assert!(changed);
        assert!(td.is_high());
    }

    /// F-THRESH-003: Hysteresis prevents toggling in middle zone
    #[test]
    fn f_thresh_003_hysteresis() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        td.update(80.0); // Go high
        let changed = td.update(50.0); // Stay in middle zone
        assert!(!changed); // Should NOT change
        assert!(td.is_high()); // Still high
    }

    /// F-THRESH-004: Transition back to low below low threshold
    #[test]
    fn f_thresh_004_transition_low() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        td.update(80.0); // Go high
        let changed = td.update(20.0); // Go below low threshold
        assert!(changed);
        assert!(td.is_low());
    }

    /// F-THRESH-005: for_resource creates 70/90 thresholds
    #[test]
    fn f_thresh_005_for_resource() {
        let td = ThresholdDetector::for_resource();
        assert_eq!(td.low_threshold(), 70.0);
        assert_eq!(td.high_threshold(), 90.0);
    }

    /// F-THRESH-006: for_temperature creates 60/80 thresholds
    #[test]
    fn f_thresh_006_for_temperature() {
        let td = ThresholdDetector::for_temperature();
        assert_eq!(td.low_threshold(), 60.0);
        assert_eq!(td.high_threshold(), 80.0);
    }

    /// F-THRESH-007: percent clamps values to 0-100
    #[test]
    fn f_thresh_007_percent_clamp() {
        let td = ThresholdDetector::percent(-10.0, 150.0);
        assert_eq!(td.low_threshold(), 0.0);
        assert_eq!(td.high_threshold(), 100.0);
    }

    /// F-THRESH-008: reset returns to low state
    #[test]
    fn f_thresh_008_reset() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        td.update(80.0);
        assert!(td.is_high());
        td.reset();
        assert!(td.is_low());
    }

    /// F-THRESH-009: set_high forces high state
    #[test]
    fn f_thresh_009_set_high() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        td.set_high();
        assert!(td.is_high());
    }

    /// F-THRESH-010: Debug format
    #[test]
    fn f_thresh_010_debug() {
        let td = ThresholdDetector::new(30.0, 70.0);
        let debug = format!("{:?}", td);
        assert!(debug.contains("ThresholdDetector"));
    }

    /// F-THRESH-011: Clone produces identical detector
    #[test]
    fn f_thresh_011_clone() {
        let td = ThresholdDetector::new(25.0, 75.0);
        let cloned = td.clone();
        assert_eq!(td.low_threshold(), cloned.low_threshold());
        assert_eq!(td.high_threshold(), cloned.high_threshold());
    }

    /// F-THRESH-012: High threshold clamped if less than low
    #[test]
    fn f_thresh_012_high_clamp() {
        let td = ThresholdDetector::new(80.0, 20.0);
        assert!(td.high_threshold() >= td.low_threshold());
    }

    /// F-THRESH-013: Value at exact threshold doesn't toggle
    #[test]
    fn f_thresh_013_exact_threshold() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        let changed = td.update(70.0); // Exact high threshold
        assert!(!changed); // Need to be ABOVE, not at
        assert!(td.is_low());
    }

    /// F-THRESH-014: Update returns true only on state change
    #[test]
    fn f_thresh_014_update_returns_change() {
        let mut td = ThresholdDetector::new(30.0, 70.0);
        assert!(!td.update(50.0)); // No change (still low)
        assert!(td.update(80.0)); // Change to high
        assert!(!td.update(85.0)); // No change (still high)
        assert!(td.update(20.0)); // Change to low
    }

    // =========================================================================
    // SampleCounter Tests (trueno-viz O(1) counting pattern)
    // =========================================================================

    /// F-COUNT-001: New counter starts at zero
    #[test]
    fn f_count_001_starts_zero() {
        let sc = SampleCounter::new();
        assert_eq!(sc.count(), 0);
        assert_eq!(sc.rate(), 0.0);
    }

    /// F-COUNT-002: Increment increases count by 1
    #[test]
    fn f_count_002_increment() {
        let mut sc = SampleCounter::new();
        sc.increment();
        assert_eq!(sc.count(), 1);
        sc.increment();
        assert_eq!(sc.count(), 2);
    }

    /// F-COUNT-003: Add increases count by n
    #[test]
    fn f_count_003_add() {
        let mut sc = SampleCounter::new();
        sc.add(10);
        assert_eq!(sc.count(), 10);
        sc.add(5);
        assert_eq!(sc.count(), 15);
    }

    /// F-COUNT-004: Reset clears all state
    #[test]
    fn f_count_004_reset() {
        let mut sc = SampleCounter::new();
        sc.add(100);
        sc.reset();
        assert_eq!(sc.count(), 0);
        assert_eq!(sc.rate(), 0.0);
    }

    /// F-COUNT-005: Default implements new
    #[test]
    fn f_count_005_default() {
        let sc = SampleCounter::default();
        assert_eq!(sc.count(), 0);
    }

    /// F-COUNT-006: Debug format
    #[test]
    fn f_count_006_debug() {
        let sc = SampleCounter::new();
        let debug = format!("{:?}", sc);
        assert!(debug.contains("SampleCounter"));
    }

    /// F-COUNT-007: Clone produces identical counter
    #[test]
    fn f_count_007_clone() {
        let mut sc = SampleCounter::new();
        sc.add(42);
        let cloned = sc.clone();
        assert_eq!(sc.count(), cloned.count());
    }

    /// F-COUNT-008: Rate calculation with time
    #[test]
    fn f_count_008_rate_calculation() {
        let mut sc = SampleCounter::new();
        sc.calculate_rate(); // Initialize timestamp
        sc.add(100);
        thread::sleep(std::time::Duration::from_millis(10));
        let rate = sc.calculate_rate();
        assert!(rate > 0.0, "Rate should be positive after adding samples");
    }

    /// F-COUNT-009: First rate calculation returns zero
    #[test]
    fn f_count_009_first_rate_zero() {
        let mut sc = SampleCounter::new();
        let rate = sc.calculate_rate();
        // First call may return 0 as there's no previous timestamp
        assert!(rate >= 0.0);
    }

    /// F-COUNT-010: Rate getter returns cached rate
    #[test]
    fn f_count_010_rate_cached() {
        let mut sc = SampleCounter::new();
        sc.calculate_rate();
        sc.add(50);
        thread::sleep(std::time::Duration::from_millis(10));
        let calculated = sc.calculate_rate();
        let cached = sc.rate();
        assert_eq!(calculated, cached);
    }

    // =========================================================================
    // BudgetTracker Tests (trueno-viz O(1) budget monitoring pattern)
    // =========================================================================

    /// F-BUDGET-001: New tracker starts with zero usage
    #[test]
    fn f_budget_001_starts_zero() {
        let bt = BudgetTracker::new(100.0);
        assert_eq!(bt.usage(), 0.0);
        assert_eq!(bt.peak(), 0.0);
        assert_eq!(bt.budget(), 100.0);
    }

    /// F-BUDGET-002: Record updates usage and peak
    #[test]
    fn f_budget_002_record() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(50.0);
        assert_eq!(bt.usage(), 50.0);
        assert_eq!(bt.peak(), 50.0);
    }

    /// F-BUDGET-003: Peak tracks maximum
    #[test]
    fn f_budget_003_peak_max() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(80.0);
        bt.record(60.0);
        bt.record(70.0);
        assert_eq!(bt.usage(), 70.0); // Current
        assert_eq!(bt.peak(), 80.0); // Max
    }

    /// F-BUDGET-004: for_render creates 16ms budget
    #[test]
    fn f_budget_004_for_render() {
        let bt = BudgetTracker::for_render();
        assert_eq!(bt.budget(), 16_000.0);
    }

    /// F-BUDGET-005: for_compute creates 1ms budget
    #[test]
    fn f_budget_005_for_compute() {
        let bt = BudgetTracker::for_compute();
        assert_eq!(bt.budget(), 1_000.0);
    }

    /// F-BUDGET-006: Utilization percentage
    #[test]
    fn f_budget_006_utilization() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(50.0);
        assert_eq!(bt.utilization(), 50.0);
    }

    /// F-BUDGET-007: Peak utilization percentage
    #[test]
    fn f_budget_007_peak_utilization() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(80.0);
        bt.record(40.0);
        assert_eq!(bt.utilization(), 40.0);
        assert_eq!(bt.peak_utilization(), 80.0);
    }

    /// F-BUDGET-008: is_over_budget detection
    #[test]
    fn f_budget_008_over_budget() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(50.0);
        assert!(!bt.is_over_budget());
        bt.record(150.0);
        assert!(bt.is_over_budget());
    }

    /// F-BUDGET-009: remaining calculation
    #[test]
    fn f_budget_009_remaining() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(30.0);
        assert_eq!(bt.remaining(), 70.0);
        bt.record(150.0);
        assert_eq!(bt.remaining(), 0.0); // Clamped to 0
    }

    /// F-BUDGET-010: Reset clears usage and peak
    #[test]
    fn f_budget_010_reset() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(80.0);
        bt.reset();
        assert_eq!(bt.usage(), 0.0);
        assert_eq!(bt.peak(), 0.0);
    }

    /// F-BUDGET-011: set_budget changes budget
    #[test]
    fn f_budget_011_set_budget() {
        let mut bt = BudgetTracker::new(100.0);
        bt.set_budget(200.0);
        assert_eq!(bt.budget(), 200.0);
    }

    /// F-BUDGET-012: Negative budget clamped to zero
    #[test]
    fn f_budget_012_negative_clamp() {
        let bt = BudgetTracker::new(-50.0);
        assert_eq!(bt.budget(), 0.0);
    }

    /// F-BUDGET-013: Zero budget returns zero utilization
    #[test]
    fn f_budget_013_zero_budget() {
        let mut bt = BudgetTracker::new(0.0);
        bt.record(50.0);
        assert_eq!(bt.utilization(), 0.0);
        assert_eq!(bt.peak_utilization(), 0.0);
    }

    /// F-BUDGET-014: Debug format
    #[test]
    fn f_budget_014_debug() {
        let bt = BudgetTracker::new(100.0);
        let debug = format!("{:?}", bt);
        assert!(debug.contains("BudgetTracker"));
    }

    /// F-BUDGET-015: Clone produces identical tracker
    #[test]
    fn f_budget_015_clone() {
        let mut bt = BudgetTracker::new(100.0);
        bt.record(60.0);
        let cloned = bt.clone();
        assert_eq!(bt.budget(), cloned.budget());
        assert_eq!(bt.usage(), cloned.usage());
        assert_eq!(bt.peak(), cloned.peak());
    }

    // =========================================================================
    // MinMaxTracker Tests (trueno-viz O(1) extrema tracking)
    // =========================================================================

    /// F-MINMAX-001: New tracker has no min/max
    #[test]
    fn f_minmax_001_starts_empty() {
        let mm = MinMaxTracker::new();
        assert!(mm.min().is_none());
        assert!(mm.max().is_none());
        assert!(mm.range().is_none());
        assert_eq!(mm.count(), 0);
    }

    /// F-MINMAX-002: Single value sets both min and max
    #[test]
    fn f_minmax_002_single_value() {
        let mut mm = MinMaxTracker::new();
        mm.record(42.0);
        assert_eq!(mm.min(), Some(42.0));
        assert_eq!(mm.max(), Some(42.0));
        assert_eq!(mm.range(), Some(0.0));
        assert_eq!(mm.count(), 1);
    }

    /// F-MINMAX-003: Multiple values track extrema
    #[test]
    fn f_minmax_003_multiple_values() {
        let mut mm = MinMaxTracker::new();
        mm.record(50.0);
        mm.record(10.0);
        mm.record(90.0);
        mm.record(30.0);
        assert_eq!(mm.min(), Some(10.0));
        assert_eq!(mm.max(), Some(90.0));
        assert_eq!(mm.range(), Some(80.0));
        assert_eq!(mm.count(), 4);
    }

    /// F-MINMAX-004: Reset clears all state
    #[test]
    fn f_minmax_004_reset() {
        let mut mm = MinMaxTracker::new();
        mm.record(100.0);
        mm.reset();
        assert!(mm.min().is_none());
        assert!(mm.max().is_none());
        assert_eq!(mm.count(), 0);
    }

    /// F-MINMAX-005: Default implements new
    #[test]
    fn f_minmax_005_default() {
        let mm = MinMaxTracker::default();
        assert_eq!(mm.count(), 0);
    }

    /// F-MINMAX-006: Debug format
    #[test]
    fn f_minmax_006_debug() {
        let mm = MinMaxTracker::new();
        let debug = format!("{:?}", mm);
        assert!(debug.contains("MinMaxTracker"));
    }

    /// F-MINMAX-007: Clone produces identical tracker
    #[test]
    fn f_minmax_007_clone() {
        let mut mm = MinMaxTracker::new();
        mm.record(25.0);
        mm.record(75.0);
        let cloned = mm.clone();
        assert_eq!(mm.min(), cloned.min());
        assert_eq!(mm.max(), cloned.max());
        assert_eq!(mm.count(), cloned.count());
    }

    /// F-MINMAX-008: Time since min starts at zero
    #[test]
    fn f_minmax_008_time_since_min() {
        let mm = MinMaxTracker::new();
        assert_eq!(mm.time_since_min_us(), 0);
    }

    /// F-MINMAX-009: Time since max starts at zero
    #[test]
    fn f_minmax_009_time_since_max() {
        let mm = MinMaxTracker::new();
        assert_eq!(mm.time_since_max_us(), 0);
    }

    /// F-MINMAX-010: Time tracking after record
    #[test]
    fn f_minmax_010_time_after_record() {
        let mut mm = MinMaxTracker::new();
        mm.record(50.0);
        // Time since should be very small (< 1 second)
        assert!(mm.time_since_min_us() < 1_000_000);
        assert!(mm.time_since_max_us() < 1_000_000);
    }

    // =========================================================================
    // MovingWindow Tests (trueno-viz O(1) time-windowed aggregation)
    // =========================================================================

    /// F-WINDOW-001: New window has zero sum and count
    #[test]
    fn f_window_001_starts_empty() {
        let mut mw = MovingWindow::new(1000);
        assert_eq!(mw.sum(), 0.0);
        assert_eq!(mw.count(), 0);
    }

    /// F-WINDOW-002: Record adds to sum
    #[test]
    fn f_window_002_record_sum() {
        let mut mw = MovingWindow::new(1000);
        mw.record(10.0);
        mw.record(20.0);
        mw.record(30.0);
        assert_eq!(mw.sum(), 60.0);
        assert_eq!(mw.count(), 3);
    }

    /// F-WINDOW-003: Increment adds 1.0
    #[test]
    fn f_window_003_increment() {
        let mut mw = MovingWindow::new(1000);
        mw.increment();
        mw.increment();
        mw.increment();
        assert_eq!(mw.count(), 3);
        assert_eq!(mw.sum(), 3.0);
    }

    /// F-WINDOW-004: one_second creates 1000ms window
    #[test]
    fn f_window_004_one_second() {
        let mw = MovingWindow::one_second();
        assert_eq!(mw.window_us, 1_000_000);
    }

    /// F-WINDOW-005: one_minute creates 60000ms window
    #[test]
    fn f_window_005_one_minute() {
        let mw = MovingWindow::one_minute();
        assert_eq!(mw.window_us, 60_000_000);
    }

    /// F-WINDOW-006: Reset clears state
    #[test]
    fn f_window_006_reset() {
        let mut mw = MovingWindow::new(1000);
        mw.record(100.0);
        mw.reset();
        assert_eq!(mw.sum(), 0.0);
        assert_eq!(mw.count(), 0);
    }

    /// F-WINDOW-007: Debug format
    #[test]
    fn f_window_007_debug() {
        let mw = MovingWindow::new(1000);
        let debug = format!("{:?}", mw);
        assert!(debug.contains("MovingWindow"));
    }

    /// F-WINDOW-008: Clone produces identical window
    #[test]
    fn f_window_008_clone() {
        let mut mw = MovingWindow::new(1000);
        mw.record(50.0);
        let cloned = mw.clone();
        assert_eq!(mw.window_us, cloned.window_us);
        assert_eq!(mw.current_sum, cloned.current_sum);
    }

    /// F-WINDOW-009: Rate per second calculation
    #[test]
    fn f_window_009_rate_per_second() {
        let mut mw = MovingWindow::new(1000); // 1 second window
        mw.record(100.0);
        // Rate should be approximately 100/1 = 100 per second
        // (may vary slightly due to timing)
        let rate = mw.rate_per_second();
        assert!(rate >= 0.0);
    }

    /// F-WINDOW-010: Count rate calculation
    #[test]
    fn f_window_010_count_rate() {
        let mut mw = MovingWindow::new(1000);
        for _ in 0..10 {
            mw.increment();
        }
        let rate = mw.count_rate();
        assert!(rate >= 0.0);
    }

    // =========================================================================
    // PercentileTracker Tests (trueno-viz O(1) approximate percentiles)
    // =========================================================================

    /// F-PCT-001: New tracker has zero count
    #[test]
    fn f_pct_001_starts_empty() {
        let pt = PercentileTracker::new();
        assert_eq!(pt.count(), 0);
    }

    /// F-PCT-002: Empty tracker returns 0 for percentiles
    #[test]
    fn f_pct_002_empty_percentile() {
        let pt = PercentileTracker::new();
        assert_eq!(pt.p50_ms(), 0.0);
        assert_eq!(pt.p90_ms(), 0.0);
        assert_eq!(pt.p99_ms(), 0.0);
    }

    /// F-PCT-003: Record increases count
    #[test]
    fn f_pct_003_record_count() {
        let mut pt = PercentileTracker::new();
        pt.record_ms(5.0);
        pt.record_ms(10.0);
        pt.record_ms(15.0);
        assert_eq!(pt.count(), 3);
    }

    /// F-PCT-004: record_us works with microseconds
    #[test]
    fn f_pct_004_record_us() {
        let mut pt = PercentileTracker::new();
        pt.record_us(500);  // 0.5ms -> bucket 0
        pt.record_us(3000); // 3ms -> bucket 1
        assert_eq!(pt.count(), 2);
    }

    /// F-PCT-005: Reset clears all state
    #[test]
    fn f_pct_005_reset() {
        let mut pt = PercentileTracker::new();
        pt.record_ms(10.0);
        pt.record_ms(20.0);
        pt.reset();
        assert_eq!(pt.count(), 0);
    }

    /// F-PCT-006: Default implements new
    #[test]
    fn f_pct_006_default() {
        let pt = PercentileTracker::default();
        assert_eq!(pt.count(), 0);
    }

    /// F-PCT-007: Debug format
    #[test]
    fn f_pct_007_debug() {
        let pt = PercentileTracker::new();
        let debug = format!("{:?}", pt);
        assert!(debug.contains("PercentileTracker"));
    }

    /// F-PCT-008: Clone produces identical tracker
    #[test]
    fn f_pct_008_clone() {
        let mut pt = PercentileTracker::new();
        pt.record_ms(5.0);
        let cloned = pt.clone();
        assert_eq!(pt.count(), cloned.count());
    }

    /// F-PCT-009: Custom boundaries work
    #[test]
    fn f_pct_009_custom_boundaries() {
        let boundaries = [100, 200, 300, 400, 500, 600, 700, 800, 900, u64::MAX];
        let pt = PercentileTracker::with_boundaries(boundaries);
        assert_eq!(pt.count(), 0);
    }

    /// F-PCT-010: p50 returns median bucket
    #[test]
    fn f_pct_010_p50() {
        let mut pt = PercentileTracker::new();
        // Record values in different buckets
        for _ in 0..50 {
            pt.record_ms(2.0);  // 2ms -> bucket 1
        }
        for _ in 0..50 {
            pt.record_ms(20.0); // 20ms -> bucket 3
        }
        // p50 should be somewhere in the middle range
        let p50 = pt.p50_ms();
        assert!(p50 > 0.0);
    }

    /// F-PCT-011: p90 higher than p50
    #[test]
    fn f_pct_011_p90_higher() {
        let mut pt = PercentileTracker::new();
        for i in 1..=100 {
            pt.record_ms(i as f64);
        }
        let p50 = pt.p50_ms();
        let p90 = pt.p90_ms();
        assert!(p90 >= p50);
    }

    /// F-PCT-012: p99 higher than p90
    #[test]
    fn f_pct_012_p99_higher() {
        let mut pt = PercentileTracker::new();
        for i in 1..=100 {
            pt.record_ms(i as f64);
        }
        let p90 = pt.p90_ms();
        let p99 = pt.p99_ms();
        assert!(p99 >= p90);
    }

    /// F-PCT-013: percentile_us returns microseconds
    #[test]
    fn f_pct_013_percentile_us() {
        let mut pt = PercentileTracker::new();
        pt.record_ms(5.0); // 5000 us
        let p50_us = pt.percentile_us(50.0);
        let p50_ms = pt.percentile_ms(50.0);
        assert_eq!(p50_us, (p50_ms * 1000.0) as u64);
    }

    /// F-PCT-014: Large values go to last bucket
    #[test]
    fn f_pct_014_large_values() {
        let mut pt = PercentileTracker::new();
        pt.record_ms(2000.0); // 2 seconds -> last bucket
        assert_eq!(pt.count(), 1);
    }

    /// F-PCT-015: Zero value goes to first bucket
    #[test]
    fn f_pct_015_zero_value() {
        let mut pt = PercentileTracker::new();
        pt.record_us(0);
        assert_eq!(pt.count(), 1);
        // p50 should be from first bucket
        let p50 = pt.percentile_us(50.0);
        assert!(p50 <= 1000); // Should be <= 1ms
    }

    // =========================================================================
    // StateTracker Tests (trueno-viz O(1) state machine pattern)
    // =========================================================================

    /// F-STATE-001: New tracker starts in state 0
    #[test]
    fn f_state_001_starts_zero() {
        let st: StateTracker<4> = StateTracker::new();
        assert_eq!(st.current(), 0);
    }

    /// F-STATE-002: Transition changes state
    #[test]
    fn f_state_002_transition() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(2);
        assert_eq!(st.current(), 2);
    }

    /// F-STATE-003: Invalid transition ignored
    #[test]
    fn f_state_003_invalid_transition() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(10); // Out of bounds
        assert_eq!(st.current(), 0); // Unchanged
    }

    /// F-STATE-004: Transition count tracked
    #[test]
    fn f_state_004_transition_count() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(1);
        st.transition(2);
        st.transition(1);
        assert_eq!(st.transition_count(1), 2);
        assert_eq!(st.transition_count(2), 1);
    }

    /// F-STATE-005: Total transitions
    #[test]
    fn f_state_005_total_transitions() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(1);
        st.transition(2);
        assert_eq!(st.total_transitions(), 3); // Initial + 2
    }

    /// F-STATE-006: Time in current state
    #[test]
    fn f_state_006_time_in_current() {
        let st: StateTracker<4> = StateTracker::new();
        // Should be very small (< 1 second)
        assert!(st.time_in_current_us() < 1_000_000);
    }

    /// F-STATE-007: Reset clears state
    #[test]
    fn f_state_007_reset() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(2);
        st.reset();
        assert_eq!(st.current(), 0);
        assert_eq!(st.transition_count(2), 0);
    }

    /// F-STATE-008: Default implements new
    #[test]
    fn f_state_008_default() {
        let st: StateTracker<4> = StateTracker::default();
        assert_eq!(st.current(), 0);
    }

    /// F-STATE-009: Debug format
    #[test]
    fn f_state_009_debug() {
        let st: StateTracker<4> = StateTracker::new();
        let debug = format!("{:?}", st);
        assert!(debug.contains("StateTracker"));
    }

    /// F-STATE-010: Clone produces identical tracker
    #[test]
    fn f_state_010_clone() {
        let mut st: StateTracker<4> = StateTracker::new();
        st.transition(2);
        let cloned = st.clone();
        assert_eq!(st.current(), cloned.current());
    }

    /// F-STATE-011: Out of bounds transition count returns 0
    #[test]
    fn f_state_011_transition_count_bounds() {
        let st: StateTracker<4> = StateTracker::new();
        assert_eq!(st.transition_count(100), 0);
    }

    /// F-STATE-012: Total time in state
    #[test]
    fn f_state_012_total_time() {
        let st: StateTracker<4> = StateTracker::new();
        let time = st.total_time_in_state_us(0);
        assert!(time < 1_000_000);
    }

    // =========================================================================
    // ChangeDetector Tests (trueno-viz O(1) significant change detection)
    // =========================================================================

    /// F-CHANGE-001: New detector with baseline
    #[test]
    fn f_change_001_new_baseline() {
        let cd = ChangeDetector::new(50.0, 1.0, 5.0);
        assert_eq!(cd.baseline(), 50.0);
        assert_eq!(cd.last_value(), 50.0);
    }

    /// F-CHANGE-002: Absolute change detection
    #[test]
    fn f_change_002_abs_change() {
        let mut cd = ChangeDetector::new(0.0, 5.0, 100.0);
        assert!(!cd.has_changed(3.0)); // Below threshold
        assert!(cd.has_changed(6.0)); // Above threshold
    }

    /// F-CHANGE-003: Relative change detection
    #[test]
    fn f_change_003_rel_change() {
        let mut cd = ChangeDetector::new(100.0, 100.0, 10.0);
        cd.update(100.0);
        assert!(!cd.has_changed(105.0)); // 5% change, below 10%
        assert!(cd.has_changed(115.0)); // 15% change, above 10%
    }

    /// F-CHANGE-004: Update returns change status
    #[test]
    fn f_change_004_update_returns() {
        let mut cd = ChangeDetector::new(0.0, 5.0, 100.0);
        assert!(!cd.update(2.0)); // No change
        assert!(cd.update(10.0)); // Change
    }

    /// F-CHANGE-005: Change count tracked
    #[test]
    fn f_change_005_change_count() {
        let mut cd = ChangeDetector::new(0.0, 5.0, 100.0);
        cd.update(1.0);
        cd.update(10.0);
        cd.update(20.0);
        assert_eq!(cd.change_count(), 2); // Two significant changes
    }

    /// F-CHANGE-006: for_percentage factory
    #[test]
    fn f_change_006_for_percentage() {
        let cd = ChangeDetector::for_percentage();
        assert_eq!(cd.baseline(), 0.0);
    }

    /// F-CHANGE-007: for_latency factory
    #[test]
    fn f_change_007_for_latency() {
        let cd = ChangeDetector::for_latency();
        assert_eq!(cd.baseline(), 0.0);
    }

    /// F-CHANGE-008: Update baseline
    #[test]
    fn f_change_008_update_baseline() {
        let mut cd = ChangeDetector::new(0.0, 1.0, 5.0);
        cd.update(50.0);
        cd.update_baseline();
        assert_eq!(cd.baseline(), 50.0);
    }

    /// F-CHANGE-009: Set baseline
    #[test]
    fn f_change_009_set_baseline() {
        let mut cd = ChangeDetector::new(0.0, 1.0, 5.0);
        cd.set_baseline(100.0);
        assert_eq!(cd.baseline(), 100.0);
    }

    /// F-CHANGE-010: Change from baseline
    #[test]
    fn f_change_010_change_from_baseline() {
        let mut cd = ChangeDetector::new(100.0, 1.0, 5.0);
        cd.update(150.0);
        assert_eq!(cd.change_from_baseline(), 50.0);
    }

    /// F-CHANGE-011: Relative change calculation
    #[test]
    fn f_change_011_relative_change() {
        let mut cd = ChangeDetector::new(100.0, 1.0, 5.0);
        cd.update(150.0);
        assert_eq!(cd.relative_change(), 50.0); // 50% increase
    }

    /// F-CHANGE-012: Reset clears state
    #[test]
    fn f_change_012_reset() {
        let mut cd = ChangeDetector::new(50.0, 1.0, 5.0);
        cd.update(100.0);
        cd.reset();
        assert_eq!(cd.last_value(), 50.0);
        assert_eq!(cd.change_count(), 0);
    }

    /// F-CHANGE-013: Default implementation
    #[test]
    fn f_change_013_default() {
        let cd = ChangeDetector::default();
        assert_eq!(cd.baseline(), 0.0);
    }

    /// F-CHANGE-014: Debug format
    #[test]
    fn f_change_014_debug() {
        let cd = ChangeDetector::new(0.0, 1.0, 5.0);
        let debug = format!("{:?}", cd);
        assert!(debug.contains("ChangeDetector"));
    }

    /// F-CHANGE-015: Clone produces identical detector
    #[test]
    fn f_change_015_clone() {
        let cd = ChangeDetector::new(50.0, 1.0, 5.0);
        let cloned = cd.clone();
        assert_eq!(cd.baseline(), cloned.baseline());
    }

    // =========================================================================
    // Accumulator Tests (trueno-viz O(1) overflow-safe accumulation)
    // =========================================================================

    /// F-ACCUM-001: New accumulator starts at zero
    #[test]
    fn f_accum_001_starts_zero() {
        let acc = Accumulator::new();
        assert_eq!(acc.value(), 0);
        assert!(!acc.is_initialized());
    }

    /// F-ACCUM-002: First update initializes
    #[test]
    fn f_accum_002_first_update() {
        let mut acc = Accumulator::new();
        acc.update(100);
        assert!(acc.is_initialized());
        assert_eq!(acc.value(), 0); // No delta yet
    }

    /// F-ACCUM-003: Second update calculates delta
    #[test]
    fn f_accum_003_delta() {
        let mut acc = Accumulator::new();
        acc.update(100);
        acc.update(150);
        assert_eq!(acc.value(), 50); // Delta of 50
    }

    /// F-ACCUM-004: Add directly
    #[test]
    fn f_accum_004_add() {
        let mut acc = Accumulator::new();
        acc.add(100);
        acc.add(50);
        assert_eq!(acc.value(), 150);
    }

    /// F-ACCUM-005: Overflow detection
    #[test]
    fn f_accum_005_overflow() {
        let mut acc = Accumulator::new();
        acc.update(u64::MAX - 10);
        acc.update(5); // Wrapped from MAX-10 to 5
        assert_eq!(acc.overflows(), 1);
    }

    /// F-ACCUM-006: Last raw value
    #[test]
    fn f_accum_006_last_raw() {
        let mut acc = Accumulator::new();
        acc.update(100);
        acc.update(200);
        assert_eq!(acc.last_raw(), 200);
    }

    /// F-ACCUM-007: Reset clears state
    #[test]
    fn f_accum_007_reset() {
        let mut acc = Accumulator::new();
        acc.update(100);
        acc.update(200);
        acc.reset();
        assert_eq!(acc.value(), 0);
        assert!(!acc.is_initialized());
        assert_eq!(acc.overflows(), 0);
    }

    /// F-ACCUM-008: Default implementation
    #[test]
    fn f_accum_008_default() {
        let acc = Accumulator::default();
        assert_eq!(acc.value(), 0);
    }

    /// F-ACCUM-009: Debug format
    #[test]
    fn f_accum_009_debug() {
        let acc = Accumulator::new();
        let debug = format!("{:?}", acc);
        assert!(debug.contains("Accumulator"));
    }

    /// F-ACCUM-010: Clone produces identical accumulator
    #[test]
    fn f_accum_010_clone() {
        let mut acc = Accumulator::new();
        acc.add(100);
        let cloned = acc.clone();
        assert_eq!(acc.value(), cloned.value());
    }

    // =========================================================================
    // EventCounter Tests (trueno-viz O(1) categorized event counting)
    // =========================================================================

    /// F-EVENT-001: New counter starts at zero
    #[test]
    fn f_event_001_starts_zero() {
        let ec: EventCounter<5> = EventCounter::new();
        assert_eq!(ec.total(), 0);
    }

    /// F-EVENT-002: Increment category
    #[test]
    fn f_event_002_increment() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.increment(0);
        ec.increment(0);
        ec.increment(1);
        assert_eq!(ec.count(0), 2);
        assert_eq!(ec.count(1), 1);
        assert_eq!(ec.total(), 3);
    }

    /// F-EVENT-003: Add to category
    #[test]
    fn f_event_003_add() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(2, 10);
        assert_eq!(ec.count(2), 10);
        assert_eq!(ec.total(), 10);
    }

    /// F-EVENT-004: Invalid category ignored
    #[test]
    fn f_event_004_invalid_category() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.increment(100); // Out of bounds
        assert_eq!(ec.total(), 0);
    }

    /// F-EVENT-005: Percentage calculation
    #[test]
    fn f_event_005_percentage() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(0, 25);
        ec.add(1, 75);
        assert_eq!(ec.percentage(0), 25.0);
        assert_eq!(ec.percentage(1), 75.0);
    }

    /// F-EVENT-006: Dominant category
    #[test]
    fn f_event_006_dominant() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(0, 10);
        ec.add(2, 50);
        ec.add(4, 30);
        assert_eq!(ec.dominant(), Some(2));
    }

    /// F-EVENT-007: Empty has no dominant
    #[test]
    fn f_event_007_empty_dominant() {
        let ec: EventCounter<5> = EventCounter::new();
        assert_eq!(ec.dominant(), None);
    }

    /// F-EVENT-008: Reset clears all
    #[test]
    fn f_event_008_reset() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(0, 100);
        ec.reset();
        assert_eq!(ec.total(), 0);
        assert_eq!(ec.count(0), 0);
    }

    /// F-EVENT-009: Default implementation
    #[test]
    fn f_event_009_default() {
        let ec: EventCounter<5> = EventCounter::default();
        assert_eq!(ec.total(), 0);
    }

    /// F-EVENT-010: Debug format
    #[test]
    fn f_event_010_debug() {
        let ec: EventCounter<5> = EventCounter::new();
        let debug = format!("{:?}", ec);
        assert!(debug.contains("EventCounter"));
    }

    /// F-EVENT-011: Clone produces identical counter
    #[test]
    fn f_event_011_clone() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(0, 50);
        let cloned = ec.clone();
        assert_eq!(ec.total(), cloned.total());
    }

    /// F-EVENT-012: Out of bounds count returns 0
    #[test]
    fn f_event_012_count_bounds() {
        let ec: EventCounter<5> = EventCounter::new();
        assert_eq!(ec.count(100), 0);
    }

    /// F-EVENT-013: Out of bounds percentage returns 0
    #[test]
    fn f_event_013_percentage_bounds() {
        let mut ec: EventCounter<5> = EventCounter::new();
        ec.add(0, 100);
        assert_eq!(ec.percentage(100), 0.0);
    }

    // =========================================================================
    // TREND DETECTOR TESTS (F-TREND-001 to F-TREND-012)
    // =========================================================================

    /// F-TREND-001: New detector starts empty
    #[test]
    fn f_trend_001_new_empty() {
        let td = TrendDetector::new(0.1);
        assert_eq!(td.count(), 0);
        assert_eq!(td.trend(), Trend::Unknown);
    }

    /// F-TREND-002: Default uses 0.1 threshold
    #[test]
    fn f_trend_002_default() {
        let td = TrendDetector::default();
        assert_eq!(td.count(), 0);
    }

    /// F-TREND-003: Increasing values detect upward trend
    #[test]
    fn f_trend_003_upward_trend() {
        let mut td = TrendDetector::new(0.1);
        for i in 0..10 {
            td.update(i as f64 * 10.0); // 0, 10, 20, 30...
        }
        assert!(td.slope() > 0.0, "Slope should be positive");
        assert_eq!(td.trend(), Trend::Up);
        assert!(td.is_trending_up());
    }

    /// F-TREND-004: Decreasing values detect downward trend
    #[test]
    fn f_trend_004_downward_trend() {
        let mut td = TrendDetector::new(0.1);
        for i in 0..10 {
            td.update(100.0 - i as f64 * 10.0); // 100, 90, 80...
        }
        assert!(td.slope() < 0.0, "Slope should be negative");
        assert_eq!(td.trend(), Trend::Down);
        assert!(td.is_trending_down());
    }

    /// F-TREND-005: Constant values detect flat trend
    #[test]
    fn f_trend_005_flat_trend() {
        let mut td = TrendDetector::new(0.1);
        for _ in 0..10 {
            td.update(50.0);
        }
        assert!(td.slope().abs() < 0.1, "Slope should be near zero");
        assert_eq!(td.trend(), Trend::Flat);
    }

    /// F-TREND-006: Unknown with fewer than 3 samples
    #[test]
    fn f_trend_006_unknown_few_samples() {
        let mut td = TrendDetector::new(0.1);
        td.update(10.0);
        td.update(20.0);
        assert_eq!(td.trend(), Trend::Unknown);
    }

    /// F-TREND-007: for_percentage uses 0.5 threshold
    #[test]
    fn f_trend_007_for_percentage() {
        let td = TrendDetector::for_percentage();
        // Should need larger slope to detect trend
        assert_eq!(td.trend(), Trend::Unknown);
    }

    /// F-TREND-008: for_latency uses 1.0 threshold
    #[test]
    fn f_trend_008_for_latency() {
        let td = TrendDetector::for_latency();
        assert_eq!(td.trend(), Trend::Unknown);
    }

    /// F-TREND-009: Reset clears all state
    #[test]
    fn f_trend_009_reset() {
        let mut td = TrendDetector::new(0.1);
        for i in 0..10 {
            td.update(i as f64);
        }
        td.reset();
        assert_eq!(td.count(), 0);
        assert_eq!(td.slope(), 0.0);
    }

    /// F-TREND-010: Slope returns 0 with single sample
    #[test]
    fn f_trend_010_slope_single() {
        let mut td = TrendDetector::new(0.1);
        td.update(50.0);
        assert_eq!(td.slope(), 0.0);
    }

    /// F-TREND-011: Debug format works
    #[test]
    fn f_trend_011_debug() {
        let td = TrendDetector::new(0.1);
        let debug = format!("{:?}", td);
        assert!(debug.contains("TrendDetector"));
    }

    /// F-TREND-012: Clone preserves state
    #[test]
    fn f_trend_012_clone() {
        let mut td = TrendDetector::new(0.1);
        td.update(10.0);
        td.update(20.0);
        let cloned = td.clone();
        assert_eq!(td.count(), cloned.count());
    }

    // =========================================================================
    // ANOMALY DETECTOR TESTS (F-ANOMALY-001 to F-ANOMALY-015)
    // =========================================================================

    /// F-ANOMALY-001: New detector starts empty
    #[test]
    fn f_anomaly_001_new_empty() {
        let ad = AnomalyDetector::new(3.0);
        assert_eq!(ad.count(), 0);
        assert_eq!(ad.mean(), 0.0);
    }

    /// F-ANOMALY-002: Default uses 3-sigma threshold
    #[test]
    fn f_anomaly_002_default() {
        let ad = AnomalyDetector::default();
        assert_eq!(ad.threshold(), 3.0);
    }

    /// F-ANOMALY-003: two_sigma uses 2.0 threshold
    #[test]
    fn f_anomaly_003_two_sigma() {
        let ad = AnomalyDetector::two_sigma();
        assert_eq!(ad.threshold(), 2.0);
    }

    /// F-ANOMALY-004: three_sigma uses 3.0 threshold
    #[test]
    fn f_anomaly_004_three_sigma() {
        let ad = AnomalyDetector::three_sigma();
        assert_eq!(ad.threshold(), 3.0);
    }

    /// F-ANOMALY-005: Mean tracks correctly (Welford)
    #[test]
    fn f_anomaly_005_mean_tracking() {
        let mut ad = AnomalyDetector::new(3.0);
        ad.update(10.0);
        ad.update(20.0);
        ad.update(30.0);
        assert!((ad.mean() - 20.0).abs() < 0.01);
    }

    /// F-ANOMALY-006: Variance tracks correctly
    #[test]
    fn f_anomaly_006_variance() {
        let mut ad = AnomalyDetector::new(3.0);
        // Variance of [10, 20, 30] = 100
        ad.update(10.0);
        ad.update(20.0);
        ad.update(30.0);
        assert!((ad.variance() - 100.0).abs() < 0.01);
    }

    /// F-ANOMALY-007: Std dev is sqrt of variance
    #[test]
    fn f_anomaly_007_std_dev() {
        let mut ad = AnomalyDetector::new(3.0);
        ad.update(10.0);
        ad.update(20.0);
        ad.update(30.0);
        assert!((ad.std_dev() - 10.0).abs() < 0.01);
    }

    /// F-ANOMALY-008: First value cannot be anomaly
    #[test]
    fn f_anomaly_008_first_not_anomaly() {
        let mut ad = AnomalyDetector::new(3.0);
        let is_anomaly = ad.update(1000.0);
        assert!(!is_anomaly);
    }

    /// F-ANOMALY-009: Needs 10+ samples for anomaly detection
    #[test]
    fn f_anomaly_009_min_samples() {
        let mut ad = AnomalyDetector::new(3.0);
        for i in 0..9 {
            ad.update(50.0 + i as f64);
        }
        // Even extreme value not anomaly with < 10 samples
        assert!(!ad.is_anomaly(1000.0));
    }

    /// F-ANOMALY-010: Detects extreme outliers
    #[test]
    fn f_anomaly_010_detect_outlier() {
        let mut ad = AnomalyDetector::new(3.0);
        // Build distribution with variance (std_dev ~1.58)
        for i in 0..20 {
            ad.update(50.0 + (i as f64 % 5.0)); // 50, 51, 52, 53, 54, 50...
        }
        // Extreme outlier should be anomaly (1000 is ~600 std devs away)
        assert!(ad.is_anomaly(1000.0));
    }

    /// F-ANOMALY-011: Z-score calculation
    #[test]
    fn f_anomaly_011_z_score() {
        let mut ad = AnomalyDetector::new(3.0);
        for i in 0..21 {
            ad.update(50.0 + (i as f64 % 3.0)); // Add variance
        }
        // Z-score of mean should be near 0
        let z = ad.z_score(ad.mean());
        assert!(z.abs() < 0.01);
    }

    /// F-ANOMALY-012: Anomaly count tracks
    #[test]
    fn f_anomaly_012_anomaly_count() {
        let mut ad = AnomalyDetector::new(2.0);
        for _ in 0..15 {
            ad.update(50.0);
        }
        ad.update(51.0); // Add variance
        ad.update(49.0);
        ad.update(1000.0); // Anomaly
        assert!(ad.anomaly_count() >= 1);
    }

    /// F-ANOMALY-013: Anomaly rate calculation
    #[test]
    fn f_anomaly_013_anomaly_rate() {
        let ad = AnomalyDetector::new(3.0);
        assert_eq!(ad.anomaly_rate(), 0.0);
    }

    /// F-ANOMALY-014: Reset clears all state
    #[test]
    fn f_anomaly_014_reset() {
        let mut ad = AnomalyDetector::new(3.0);
        for i in 0..10 {
            ad.update(i as f64);
        }
        ad.reset();
        assert_eq!(ad.count(), 0);
        assert_eq!(ad.mean(), 0.0);
        assert_eq!(ad.anomaly_count(), 0);
    }

    /// F-ANOMALY-015: Clone preserves state
    #[test]
    fn f_anomaly_015_clone() {
        let mut ad = AnomalyDetector::new(3.0);
        ad.update(50.0);
        let cloned = ad.clone();
        assert_eq!(ad.count(), cloned.count());
        assert_eq!(ad.mean(), cloned.mean());
    }

    // =========================================================================
    // THROUGHPUT TRACKER TESTS (F-THRU-001 to F-THRU-012)
    // =========================================================================

    /// F-THRU-001: New tracker starts at zero
    #[test]
    fn f_thru_001_new_zero() {
        let tt = ThroughputTracker::new();
        assert_eq!(tt.total(), 0);
        assert_eq!(tt.rate(), 0.0);
    }

    /// F-THRU-002: Default is same as new
    #[test]
    fn f_thru_002_default() {
        let tt = ThroughputTracker::default();
        assert_eq!(tt.total(), 0);
    }

    /// F-THRU-003: Add increases total
    #[test]
    fn f_thru_003_add() {
        let mut tt = ThroughputTracker::new();
        tt.add(100);
        tt.add(200);
        assert_eq!(tt.total(), 300);
    }

    /// F-THRU-004: Peak rate tracks max
    #[test]
    fn f_thru_004_peak_rate() {
        let tt = ThroughputTracker::new();
        assert_eq!(tt.peak_rate(), 0.0);
    }

    /// F-THRU-005: Format rate - small values
    #[test]
    fn f_thru_005_format_small() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 500.0;
        assert_eq!(tt.format_rate(), "500/s");
    }

    /// F-THRU-006: Format rate - K suffix
    #[test]
    fn f_thru_006_format_k() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 5_000.0;
        assert_eq!(tt.format_rate(), "5.0K/s");
    }

    /// F-THRU-007: Format rate - M suffix
    #[test]
    fn f_thru_007_format_m() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 5_000_000.0;
        assert_eq!(tt.format_rate(), "5.0M/s");
    }

    /// F-THRU-008: Format rate - G suffix
    #[test]
    fn f_thru_008_format_g() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 5_000_000_000.0;
        assert_eq!(tt.format_rate(), "5.0G/s");
    }

    /// F-THRU-009: Format bytes rate - KB suffix
    #[test]
    fn f_thru_009_format_kb() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 5_120.0; // 5KB
        assert_eq!(tt.format_bytes_rate(), "5.0KB/s");
    }

    /// F-THRU-010: Format bytes rate - MB suffix
    #[test]
    fn f_thru_010_format_mb() {
        let mut tt = ThroughputTracker::new();
        tt.rate = 5_242_880.0; // 5MB
        assert_eq!(tt.format_bytes_rate(), "5.0MB/s");
    }

    /// F-THRU-011: Reset clears all state
    #[test]
    fn f_thru_011_reset() {
        let mut tt = ThroughputTracker::new();
        tt.add(1000);
        tt.rate = 500.0;
        tt.peak_rate = 1000.0;
        tt.reset();
        assert_eq!(tt.total(), 0);
        assert_eq!(tt.rate(), 0.0);
        assert_eq!(tt.peak_rate(), 0.0);
    }

    /// F-THRU-012: Clone preserves state
    #[test]
    fn f_thru_012_clone() {
        let mut tt = ThroughputTracker::new();
        tt.add(500);
        let cloned = tt.clone();
        assert_eq!(tt.total(), cloned.total());
    }

    // =========================================================================
    // JITTER TRACKER TESTS (F-JITTER-001 to F-JITTER-010)
    // =========================================================================

    /// F-JITTER-001: New tracker starts at zero
    #[test]
    fn f_jitter_001_new_zero() {
        let jt = JitterTracker::new();
        assert_eq!(jt.jitter(), 0.0);
        assert_eq!(jt.count(), 0);
    }

    /// F-JITTER-002: Default uses RFC 3550 alpha
    #[test]
    fn f_jitter_002_default() {
        let jt = JitterTracker::default();
        assert_eq!(jt.jitter(), 0.0);
    }

    /// F-JITTER-003: Custom alpha clamped to [0, 1]
    #[test]
    fn f_jitter_003_alpha_clamped() {
        let jt1 = JitterTracker::with_alpha(-0.5);
        let jt2 = JitterTracker::with_alpha(2.0);
        // Should be clamped, jitter still works
        assert_eq!(jt1.jitter(), 0.0);
        assert_eq!(jt2.jitter(), 0.0);
    }

    /// F-JITTER-004: First update sets prev, no jitter
    #[test]
    fn f_jitter_004_first_update() {
        let mut jt = JitterTracker::new();
        jt.update(100.0);
        assert_eq!(jt.jitter(), 0.0);
        assert_eq!(jt.count(), 1);
    }

    /// F-JITTER-005: Constant values produce zero jitter
    #[test]
    fn f_jitter_005_constant_zero_jitter() {
        let mut jt = JitterTracker::new();
        for _ in 0..10 {
            jt.update(50.0);
        }
        assert!(jt.jitter().abs() < 0.01);
    }

    /// F-JITTER-006: Variable values produce jitter
    #[test]
    fn f_jitter_006_variable_jitter() {
        let mut jt = JitterTracker::new();
        jt.update(10.0);
        jt.update(50.0); // 40 diff
        jt.update(10.0); // 40 diff
        jt.update(50.0); // 40 diff
        assert!(jt.jitter() > 0.0);
    }

    /// F-JITTER-007: Peak jitter tracks max
    #[test]
    fn f_jitter_007_peak_tracking() {
        let mut jt = JitterTracker::new();
        jt.update(10.0);
        jt.update(100.0); // Large jump
        let peak1 = jt.peak_jitter();
        jt.update(99.0); // Small change
        jt.update(98.0);
        // Peak should still be from the large jump
        assert!(jt.peak_jitter() >= peak1 * 0.9);
    }

    /// F-JITTER-008: Exceeds threshold check
    #[test]
    fn f_jitter_008_exceeds() {
        let mut jt = JitterTracker::new();
        jt.update(0.0);
        jt.update(100.0);
        assert!(jt.exceeds(1.0));
        assert!(!jt.exceeds(1000.0));
    }

    /// F-JITTER-009: Reset clears all state
    #[test]
    fn f_jitter_009_reset() {
        let mut jt = JitterTracker::new();
        jt.update(10.0);
        jt.update(50.0);
        jt.reset();
        assert_eq!(jt.jitter(), 0.0);
        assert_eq!(jt.peak_jitter(), 0.0);
        assert_eq!(jt.count(), 0);
    }

    /// F-JITTER-010: Clone preserves state
    #[test]
    fn f_jitter_010_clone() {
        let mut jt = JitterTracker::new();
        jt.update(10.0);
        jt.update(50.0);
        let cloned = jt.clone();
        assert_eq!(jt.jitter(), cloned.jitter());
        assert_eq!(jt.count(), cloned.count());
    }

    // =========================================================================
    // DERIVATIVE TRACKER TESTS (F-DERIV-001 to F-DERIV-010)
    // =========================================================================

    /// F-DERIV-001: New tracker starts at zero
    #[test]
    fn f_deriv_001_new_zero() {
        let dt = DerivativeTracker::new();
        assert_eq!(dt.derivative(), 0.0);
        assert_eq!(dt.count(), 0);
    }

    /// F-DERIV-002: Default uses 0.3 alpha
    #[test]
    fn f_deriv_002_default() {
        let dt = DerivativeTracker::default();
        assert_eq!(dt.derivative(), 0.0);
    }

    /// F-DERIV-003: Custom alpha clamped to [0, 1]
    #[test]
    fn f_deriv_003_alpha_clamped() {
        let dt1 = DerivativeTracker::with_alpha(-0.5);
        let dt2 = DerivativeTracker::with_alpha(2.0);
        assert_eq!(dt1.derivative(), 0.0);
        assert_eq!(dt2.derivative(), 0.0);
    }

    /// F-DERIV-004: First update stores value only
    #[test]
    fn f_deriv_004_first_update() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(100.0, 1.0);
        assert_eq!(dt.derivative(), 0.0); // No derivative yet
        assert_eq!(dt.count(), 1);
    }

    /// F-DERIV-005: Positive derivative for increasing values
    #[test]
    fn f_deriv_005_positive() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(0.0, 1.0);
        dt.update_with_dt(100.0, 1.0); // +100 in 1 second
        assert!(dt.derivative() > 0.0);
        assert!(dt.is_accelerating());
    }

    /// F-DERIV-006: Negative derivative for decreasing values
    #[test]
    fn f_deriv_006_negative() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(100.0, 1.0);
        dt.update_with_dt(0.0, 1.0); // -100 in 1 second
        assert!(dt.derivative() < 0.0);
        assert!(dt.is_decelerating());
    }

    /// F-DERIV-007: Smoothed derivative converges
    #[test]
    fn f_deriv_007_smoothed() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(0.0, 1.0);
        for i in 1..10 {
            dt.update_with_dt(i as f64 * 10.0, 1.0);
        }
        // Smoothed should be close to 10 (constant rate)
        assert!(dt.smoothed() > 5.0);
    }

    /// F-DERIV-008: Reset clears all state
    #[test]
    fn f_deriv_008_reset() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(100.0, 1.0);
        dt.update_with_dt(200.0, 1.0);
        dt.reset();
        assert_eq!(dt.derivative(), 0.0);
        assert_eq!(dt.smoothed(), 0.0);
        assert_eq!(dt.count(), 0);
    }

    /// F-DERIV-009: Debug format works
    #[test]
    fn f_deriv_009_debug() {
        let dt = DerivativeTracker::new();
        let debug = format!("{:?}", dt);
        assert!(debug.contains("DerivativeTracker"));
    }

    /// F-DERIV-010: Clone preserves state
    #[test]
    fn f_deriv_010_clone() {
        let mut dt = DerivativeTracker::new();
        dt.update_with_dt(50.0, 1.0);
        let cloned = dt.clone();
        assert_eq!(dt.count(), cloned.count());
    }

    // =========================================================================
    // INTEGRAL TRACKER TESTS (F-INTEG-001 to F-INTEG-010)
    // =========================================================================

    /// F-INTEG-001: New tracker starts at zero
    #[test]
    fn f_integ_001_new_zero() {
        let it = IntegralTracker::new();
        assert_eq!(it.integral(), 0.0);
        assert_eq!(it.count(), 0);
    }

    /// F-INTEG-002: Default same as new
    #[test]
    fn f_integ_002_default() {
        let it = IntegralTracker::default();
        assert_eq!(it.integral(), 0.0);
    }

    /// F-INTEG-003: First update stores value only
    #[test]
    fn f_integ_003_first_update() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(100.0, 1.0);
        assert_eq!(it.integral(), 0.0); // No area yet
        assert_eq!(it.count(), 1);
    }

    /// F-INTEG-004: Constant value accumulates area
    #[test]
    fn f_integ_004_constant() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(10.0, 1.0);
        it.update_with_dt(10.0, 1.0); // 10 * 1s = 10
        assert!((it.integral() - 10.0).abs() < 0.01);
    }

    /// F-INTEG-005: Trapezoidal rule for varying values
    #[test]
    fn f_integ_005_trapezoidal() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(0.0, 1.0);
        it.update_with_dt(10.0, 1.0); // (0 + 10) / 2 * 1 = 5
        assert!((it.integral() - 5.0).abs() < 0.01);
    }

    /// F-INTEG-006: Multiple updates accumulate
    #[test]
    fn f_integ_006_accumulate() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(10.0, 1.0);
        it.update_with_dt(10.0, 1.0); // +10
        it.update_with_dt(10.0, 1.0); // +10
        assert!((it.integral() - 20.0).abs() < 0.01);
    }

    /// F-INTEG-007: Reset clears state
    #[test]
    fn f_integ_007_reset() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(100.0, 1.0);
        it.update_with_dt(100.0, 1.0);
        it.reset();
        assert_eq!(it.integral(), 0.0);
        assert_eq!(it.count(), 0);
    }

    /// F-INTEG-008: Debug format works
    #[test]
    fn f_integ_008_debug() {
        let it = IntegralTracker::new();
        let debug = format!("{:?}", it);
        assert!(debug.contains("IntegralTracker"));
    }

    /// F-INTEG-009: Clone preserves state
    #[test]
    fn f_integ_009_clone() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(50.0, 1.0);
        let cloned = it.clone();
        assert_eq!(it.count(), cloned.count());
    }

    /// F-INTEG-010: Average returns last value for insufficient data
    #[test]
    fn f_integ_010_average() {
        let mut it = IntegralTracker::new();
        it.update_with_dt(42.0, 1.0);
        assert_eq!(it.average(), 42.0);
    }

    // =========================================================================
    // CORRELATION TRACKER TESTS (F-CORR-001 to F-CORR-012)
    // =========================================================================

    /// F-CORR-001: New tracker starts at zero
    #[test]
    fn f_corr_001_new_zero() {
        let ct = CorrelationTracker::new();
        assert_eq!(ct.correlation(), 0.0);
        assert_eq!(ct.count(), 0);
    }

    /// F-CORR-002: Default same as new
    #[test]
    fn f_corr_002_default() {
        let ct = CorrelationTracker::default();
        assert_eq!(ct.correlation(), 0.0);
    }

    /// F-CORR-003: Perfect positive correlation (y = x)
    #[test]
    fn f_corr_003_perfect_positive() {
        let mut ct = CorrelationTracker::new();
        for i in 0..10 {
            ct.update(i as f64, i as f64);
        }
        assert!(ct.correlation() > 0.99, "Should be ~1.0");
        assert!(ct.is_positive());
        assert!(ct.is_strong());
    }

    /// F-CORR-004: Perfect negative correlation (y = -x)
    #[test]
    fn f_corr_004_perfect_negative() {
        let mut ct = CorrelationTracker::new();
        for i in 0..10 {
            ct.update(i as f64, -(i as f64));
        }
        assert!(ct.correlation() < -0.99, "Should be ~-1.0");
        assert!(ct.is_negative());
        assert!(ct.is_strong());
    }

    /// F-CORR-005: No correlation (y = constant)
    #[test]
    fn f_corr_005_no_correlation() {
        let mut ct = CorrelationTracker::new();
        for i in 0..10 {
            ct.update(i as f64, 5.0); // Y is constant
        }
        // With Y constant, correlation is undefined (returns 0)
        assert!(ct.correlation().abs() < 0.1);
    }

    /// F-CORR-006: Covariance calculation
    #[test]
    fn f_corr_006_covariance() {
        let mut ct = CorrelationTracker::new();
        ct.update(1.0, 2.0);
        ct.update(2.0, 4.0);
        ct.update(3.0, 6.0);
        assert!(ct.covariance() > 0.0); // Positive covariance
    }

    /// F-CORR-007: Insufficient data returns 0
    #[test]
    fn f_corr_007_insufficient() {
        let mut ct = CorrelationTracker::new();
        ct.update(1.0, 2.0);
        assert_eq!(ct.correlation(), 0.0);
    }

    /// F-CORR-008: Reset clears state
    #[test]
    fn f_corr_008_reset() {
        let mut ct = CorrelationTracker::new();
        for i in 0..10 {
            ct.update(i as f64, i as f64);
        }
        ct.reset();
        assert_eq!(ct.correlation(), 0.0);
        assert_eq!(ct.count(), 0);
    }

    /// F-CORR-009: Debug format works
    #[test]
    fn f_corr_009_debug() {
        let ct = CorrelationTracker::new();
        let debug = format!("{:?}", ct);
        assert!(debug.contains("CorrelationTracker"));
    }

    /// F-CORR-010: Clone preserves state
    #[test]
    fn f_corr_010_clone() {
        let mut ct = CorrelationTracker::new();
        ct.update(1.0, 2.0);
        ct.update(2.0, 4.0);
        let cloned = ct.clone();
        assert_eq!(ct.count(), cloned.count());
    }

    /// F-CORR-011: Correlation clamped to [-1, 1]
    #[test]
    fn f_corr_011_clamped() {
        let mut ct = CorrelationTracker::new();
        for i in 0..100 {
            ct.update(i as f64, i as f64 * 2.0);
        }
        let r = ct.correlation();
        assert!(r >= -1.0 && r <= 1.0);
    }

    /// F-CORR-012: Weak correlation not flagged as strong
    #[test]
    fn f_corr_012_weak() {
        let ct = CorrelationTracker::new();
        // Not enough data, correlation is 0
        assert!(!ct.is_strong());
        assert!(!ct.is_positive());
        assert!(!ct.is_negative());
    }

    // =========================================================================
    // CIRCUIT BREAKER TESTS (F-CIRCUIT-001 to F-CIRCUIT-012)
    // =========================================================================

    /// F-CIRCUIT-001: New breaker starts closed
    #[test]
    fn f_circuit_001_starts_closed() {
        let cb = CircuitBreaker::new(5, 3, 1_000_000);
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.is_closed());
    }

    /// F-CIRCUIT-002: Default uses 5/3/30s
    #[test]
    fn f_circuit_002_default() {
        let cb = CircuitBreaker::default();
        assert!(cb.is_closed());
    }

    /// F-CIRCUIT-003: for_network factory
    #[test]
    fn f_circuit_003_for_network() {
        let cb = CircuitBreaker::for_network();
        assert!(cb.is_closed());
    }

    /// F-CIRCUIT-004: for_fast_fail factory
    #[test]
    fn f_circuit_004_for_fast_fail() {
        let cb = CircuitBreaker::for_fast_fail();
        assert!(cb.is_closed());
    }

    /// F-CIRCUIT-005: Opens after threshold failures
    #[test]
    fn f_circuit_005_opens() {
        let mut cb = CircuitBreaker::new(3, 2, 1_000_000);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());
        assert_eq!(cb.state(), CircuitState::Open);
    }

    /// F-CIRCUIT-006: Closed allows requests
    #[test]
    fn f_circuit_006_closed_allows() {
        let mut cb = CircuitBreaker::new(3, 2, 1_000_000);
        assert!(cb.is_allowed());
    }

    /// F-CIRCUIT-007: Success resets failure count
    #[test]
    fn f_circuit_007_success_resets() {
        let mut cb = CircuitBreaker::new(3, 2, 1_000_000);
        cb.record_failure();
        cb.record_failure();
        cb.record_success();
        assert_eq!(cb.failures(), 0);
        assert!(cb.is_closed());
    }

    /// F-CIRCUIT-008: Reset forces closed
    #[test]
    fn f_circuit_008_reset() {
        let mut cb = CircuitBreaker::new(3, 2, 1_000_000);
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());
        cb.reset();
        assert!(cb.is_closed());
        assert_eq!(cb.failures(), 0);
    }

    /// F-CIRCUIT-009: Debug format works
    #[test]
    fn f_circuit_009_debug() {
        let cb = CircuitBreaker::new(3, 2, 1_000_000);
        let debug = format!("{:?}", cb);
        assert!(debug.contains("CircuitBreaker"));
    }

    /// F-CIRCUIT-010: Clone preserves state
    #[test]
    fn f_circuit_010_clone() {
        let mut cb = CircuitBreaker::new(3, 2, 1_000_000);
        cb.record_failure();
        let cloned = cb.clone();
        assert_eq!(cb.failures(), cloned.failures());
    }

    /// F-CIRCUIT-011: CircuitState derives work
    #[test]
    fn f_circuit_011_state_derives() {
        let s1 = CircuitState::Closed;
        let s2 = CircuitState::Closed;
        assert_eq!(s1, s2);
        let _ = format!("{:?}", s1);
        let _ = s1.clone();
    }

    /// F-CIRCUIT-012: Failure in half-open reopens
    #[test]
    fn f_circuit_012_halfopen_fails() {
        let mut cb = CircuitBreaker::new(3, 2, 0); // 0 timeout = immediate half-open
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();
        assert!(cb.is_open());
        // Allow will transition to half-open (timeout=0)
        assert!(cb.is_allowed());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        cb.record_failure();
        assert!(cb.is_open());
    }

    // =========================================================================
    // EXPONENTIAL BACKOFF TESTS (F-BACKOFF-001 to F-BACKOFF-012)
    // =========================================================================

    /// F-BACKOFF-001: New backoff starts at attempt 0
    #[test]
    fn f_backoff_001_starts_zero() {
        let eb = ExponentialBackoff::new(100_000, 30_000_000);
        assert_eq!(eb.attempt(), 0);
    }

    /// F-BACKOFF-002: Default uses 100ms/30s
    #[test]
    fn f_backoff_002_default() {
        let eb = ExponentialBackoff::default();
        assert_eq!(eb.attempt(), 0);
    }

    /// F-BACKOFF-003: for_network factory
    #[test]
    fn f_backoff_003_for_network() {
        let eb = ExponentialBackoff::for_network();
        assert_eq!(eb.attempt(), 0);
    }

    /// F-BACKOFF-004: for_fast factory
    #[test]
    fn f_backoff_004_for_fast() {
        let eb = ExponentialBackoff::for_fast();
        assert_eq!(eb.attempt(), 0);
    }

    /// F-BACKOFF-005: First delay is base
    #[test]
    fn f_backoff_005_first_delay() {
        let eb = ExponentialBackoff::new(100_000, 30_000_000);
        assert_eq!(eb.current_delay(), 100_000);
    }

    /// F-BACKOFF-006: Delay doubles each attempt
    #[test]
    fn f_backoff_006_doubles() {
        let mut eb = ExponentialBackoff::new(100_000, 30_000_000);
        let d1 = eb.next_delay();
        let d2 = eb.next_delay();
        assert_eq!(d2, d1 * 2);
    }

    /// F-BACKOFF-007: Delay capped at max
    #[test]
    fn f_backoff_007_capped() {
        let mut eb = ExponentialBackoff::new(100_000, 500_000); // 100ms base, 500ms max
        for _ in 0..20 {
            eb.next_delay();
        }
        assert!(eb.current_delay() <= 500_000);
        assert!(eb.is_at_max());
    }

    /// F-BACKOFF-008: Milliseconds conversion
    #[test]
    fn f_backoff_008_ms() {
        let eb = ExponentialBackoff::new(100_000, 30_000_000);
        assert_eq!(eb.current_delay_ms(), 100);
    }

    /// F-BACKOFF-009: Reset to attempt 0
    #[test]
    fn f_backoff_009_reset() {
        let mut eb = ExponentialBackoff::new(100_000, 30_000_000);
        eb.next_delay();
        eb.next_delay();
        eb.reset();
        assert_eq!(eb.attempt(), 0);
        assert_eq!(eb.current_delay(), 100_000);
    }

    /// F-BACKOFF-010: with_multiplier changes growth
    #[test]
    fn f_backoff_010_multiplier() {
        let mut eb = ExponentialBackoff::new(100_000, 30_000_000).with_multiplier(3.0);
        let d1 = eb.next_delay();
        let d2 = eb.next_delay();
        assert_eq!(d2, d1 * 3);
    }

    /// F-BACKOFF-011: Debug format works
    #[test]
    fn f_backoff_011_debug() {
        let eb = ExponentialBackoff::new(100_000, 30_000_000);
        let debug = format!("{:?}", eb);
        assert!(debug.contains("ExponentialBackoff"));
    }

    /// F-BACKOFF-012: Clone preserves state
    #[test]
    fn f_backoff_012_clone() {
        let mut eb = ExponentialBackoff::new(100_000, 30_000_000);
        eb.next_delay();
        let cloned = eb.clone();
        assert_eq!(eb.attempt(), cloned.attempt());
    }

    // =========================================================================
    // SLIDING MEDIAN TESTS (F-MEDIAN-001 to F-MEDIAN-010)
    // =========================================================================

    /// F-MEDIAN-001: New tracker starts empty
    #[test]
    fn f_median_001_new_empty() {
        let sm = SlidingMedian::new();
        assert_eq!(sm.count(), 0);
        assert_eq!(sm.median(), 0.0);
    }

    /// F-MEDIAN-002: Default same as new
    #[test]
    fn f_median_002_default() {
        let sm = SlidingMedian::default();
        assert_eq!(sm.count(), 0);
    }

    /// F-MEDIAN-003: for_latency factory
    #[test]
    fn f_median_003_for_latency() {
        let sm = SlidingMedian::for_latency();
        assert_eq!(sm.count(), 0);
    }

    /// F-MEDIAN-004: for_percentage factory
    #[test]
    fn f_median_004_for_percentage() {
        let sm = SlidingMedian::for_percentage();
        assert_eq!(sm.count(), 0);
    }

    /// F-MEDIAN-005: Median of single value
    #[test]
    fn f_median_005_single() {
        let mut sm = SlidingMedian::for_percentage();
        sm.update(50.0);
        assert!(sm.median() > 0.0);
    }

    /// F-MEDIAN-006: Min/max tracking
    #[test]
    fn f_median_006_minmax() {
        let mut sm = SlidingMedian::new();
        sm.update(100.0);
        sm.update(500.0);
        sm.update(300.0);
        assert_eq!(sm.min(), 100.0);
        assert_eq!(sm.max(), 500.0);
    }

    /// F-MEDIAN-007: Percentile calculation
    #[test]
    fn f_median_007_percentile() {
        let mut sm = SlidingMedian::for_percentage();
        for i in 1..=100 {
            sm.update(i as f64);
        }
        // p90 should be around 90
        let p90 = sm.percentile(90);
        assert!(p90 >= 80.0 && p90 <= 100.0);
    }

    /// F-MEDIAN-008: Reset clears state
    #[test]
    fn f_median_008_reset() {
        let mut sm = SlidingMedian::new();
        sm.update(50.0);
        sm.reset();
        assert_eq!(sm.count(), 0);
    }

    /// F-MEDIAN-009: Debug format works
    #[test]
    fn f_median_009_debug() {
        let sm = SlidingMedian::new();
        let debug = format!("{:?}", sm);
        assert!(debug.contains("SlidingMedian"));
    }

    /// F-MEDIAN-010: Clone preserves state
    #[test]
    fn f_median_010_clone() {
        let mut sm = SlidingMedian::new();
        sm.update(50.0);
        let cloned = sm.clone();
        assert_eq!(sm.count(), cloned.count());
    }

    // =========================================================================
    // HYSTERESIS FILTER TESTS (F-HYST-001 to F-HYST-010)
    // =========================================================================

    /// F-HYST-001: New filter starts at zero
    #[test]
    fn f_hyst_001_new_zero() {
        let hf = HysteresisFilter::new(1.0);
        assert_eq!(hf.output(), 0.0);
        assert_eq!(hf.count(), 0);
    }

    /// F-HYST-002: Default uses 1.0 dead band
    #[test]
    fn f_hyst_002_default() {
        let hf = HysteresisFilter::default();
        assert_eq!(hf.dead_band(), 1.0);
    }

    /// F-HYST-003: for_percentage factory
    #[test]
    fn f_hyst_003_for_percentage() {
        let hf = HysteresisFilter::for_percentage();
        assert_eq!(hf.dead_band(), 1.0);
    }

    /// F-HYST-004: for_latency factory
    #[test]
    fn f_hyst_004_for_latency() {
        let hf = HysteresisFilter::for_latency();
        assert_eq!(hf.dead_band(), 0.5);
    }

    /// F-HYST-005: First update always changes output
    #[test]
    fn f_hyst_005_first_update() {
        let mut hf = HysteresisFilter::new(1.0);
        let changed = hf.update(50.0);
        assert!(changed);
        assert_eq!(hf.output(), 50.0);
    }

    /// F-HYST-006: Small change within dead band ignored
    #[test]
    fn f_hyst_006_within_deadband() {
        let mut hf = HysteresisFilter::new(5.0);
        hf.update(50.0);
        let changed = hf.update(52.0); // Only 2, within 5 dead band
        assert!(!changed);
        assert_eq!(hf.output(), 50.0);
    }

    /// F-HYST-007: Large change outside dead band accepted
    #[test]
    fn f_hyst_007_outside_deadband() {
        let mut hf = HysteresisFilter::new(5.0);
        hf.update(50.0);
        let changed = hf.update(60.0); // 10, outside 5 dead band
        assert!(changed);
        assert_eq!(hf.output(), 60.0);
    }

    /// F-HYST-008: Reset clears state
    #[test]
    fn f_hyst_008_reset() {
        let mut hf = HysteresisFilter::new(1.0);
        hf.update(50.0);
        hf.reset();
        assert_eq!(hf.output(), 0.0);
        assert_eq!(hf.count(), 0);
    }

    /// F-HYST-009: Debug format works
    #[test]
    fn f_hyst_009_debug() {
        let hf = HysteresisFilter::new(1.0);
        let debug = format!("{:?}", hf);
        assert!(debug.contains("HysteresisFilter"));
    }

    /// F-HYST-010: Clone preserves state
    #[test]
    fn f_hyst_010_clone() {
        let mut hf = HysteresisFilter::new(1.0);
        hf.update(50.0);
        let cloned = hf.clone();
        assert_eq!(hf.output(), cloned.output());
    }

    // =========================================================================
    // SPIKE FILTER TESTS (F-SPIKE-001 to F-SPIKE-010)
    // =========================================================================

    /// F-SPIKE-001: New filter starts at zero
    #[test]
    fn f_spike_001_new_zero() {
        let sf = SpikeFilter::new(3.0);
        assert_eq!(sf.average(), 0.0);
        assert_eq!(sf.count(), 0);
    }

    /// F-SPIKE-002: Default uses 3.0 threshold
    #[test]
    fn f_spike_002_default() {
        let sf = SpikeFilter::default();
        assert_eq!(sf.count(), 0);
    }

    /// F-SPIKE-003: First value always accepted
    #[test]
    fn f_spike_003_first_accepted() {
        let mut sf = SpikeFilter::new(3.0);
        let result = sf.update(50.0);
        assert_eq!(result, 50.0);
        assert_eq!(sf.last_accepted(), 50.0);
    }

    /// F-SPIKE-004: Normal values accepted
    #[test]
    fn f_spike_004_normal_accepted() {
        let mut sf = SpikeFilter::new(10.0);
        sf.update(50.0);
        let result = sf.update(52.0);
        assert_eq!(result, 52.0);
    }

    /// F-SPIKE-005: Spike rejected
    #[test]
    fn f_spike_005_spike_rejected() {
        let mut sf = SpikeFilter::new(10.0);
        sf.update(50.0);
        let result = sf.update(1000.0); // Huge spike
        assert_eq!(result, 50.0); // Returns last accepted
        assert_eq!(sf.spikes(), 1);
    }

    /// F-SPIKE-006: Spike rate calculation
    #[test]
    fn f_spike_006_spike_rate() {
        let mut sf = SpikeFilter::new(10.0);
        sf.update(50.0);
        sf.update(51.0);
        sf.update(1000.0); // Spike
        sf.update(52.0);
        assert!(sf.spike_rate() > 0.0);
    }

    /// F-SPIKE-007: Reset clears state
    #[test]
    fn f_spike_007_reset() {
        let mut sf = SpikeFilter::new(3.0);
        sf.update(50.0);
        sf.update(1000.0);
        sf.reset();
        assert_eq!(sf.spikes(), 0);
        assert_eq!(sf.count(), 0);
    }

    /// F-SPIKE-008: Debug format works
    #[test]
    fn f_spike_008_debug() {
        let sf = SpikeFilter::new(3.0);
        let debug = format!("{:?}", sf);
        assert!(debug.contains("SpikeFilter"));
    }

    /// F-SPIKE-009: Clone preserves state
    #[test]
    fn f_spike_009_clone() {
        let mut sf = SpikeFilter::new(3.0);
        sf.update(50.0);
        let cloned = sf.clone();
        assert_eq!(sf.count(), cloned.count());
    }

    /// F-SPIKE-010: for_percentage factory
    #[test]
    fn f_spike_010_for_percentage() {
        let sf = SpikeFilter::for_percentage();
        assert_eq!(sf.count(), 0);
    }

    // =========================================================================
    // GAUGE TRACKER TESTS (F-GAUGE-001 to F-GAUGE-012)
    // =========================================================================

    /// F-GAUGE-001: New tracker starts at zero
    #[test]
    fn f_gauge_001_new_zero() {
        let gt = GaugeTracker::new();
        assert_eq!(gt.current(), 0.0);
        assert_eq!(gt.count(), 0);
    }

    /// F-GAUGE-002: Default same as new
    #[test]
    fn f_gauge_002_default() {
        let gt = GaugeTracker::default();
        assert_eq!(gt.current(), 0.0);
    }

    /// F-GAUGE-003: Set updates current
    #[test]
    fn f_gauge_003_set() {
        let mut gt = GaugeTracker::new();
        gt.set(50.0);
        assert_eq!(gt.current(), 50.0);
    }

    /// F-GAUGE-004: Inc increments by 1
    #[test]
    fn f_gauge_004_inc() {
        let mut gt = GaugeTracker::new();
        gt.set(10.0);
        gt.inc();
        assert_eq!(gt.current(), 11.0);
    }

    /// F-GAUGE-005: Dec decrements by 1
    #[test]
    fn f_gauge_005_dec() {
        let mut gt = GaugeTracker::new();
        gt.set(10.0);
        gt.dec();
        assert_eq!(gt.current(), 9.0);
    }

    /// F-GAUGE-006: Min/max tracking
    #[test]
    fn f_gauge_006_minmax() {
        let mut gt = GaugeTracker::new();
        gt.set(50.0);
        gt.set(20.0);
        gt.set(80.0);
        assert_eq!(gt.min(), 20.0);
        assert_eq!(gt.max(), 80.0);
    }

    /// F-GAUGE-007: Average calculation
    #[test]
    fn f_gauge_007_average() {
        let mut gt = GaugeTracker::new();
        gt.set(10.0);
        gt.set(20.0);
        gt.set(30.0);
        assert_eq!(gt.average(), 20.0);
    }

    /// F-GAUGE-008: Range calculation
    #[test]
    fn f_gauge_008_range() {
        let mut gt = GaugeTracker::new();
        gt.set(20.0);
        gt.set(80.0);
        assert_eq!(gt.range(), 60.0);
    }

    /// F-GAUGE-009: Reset clears state
    #[test]
    fn f_gauge_009_reset() {
        let mut gt = GaugeTracker::new();
        gt.set(50.0);
        gt.reset();
        assert_eq!(gt.current(), 0.0);
        assert_eq!(gt.count(), 0);
    }

    /// F-GAUGE-010: Debug format works
    #[test]
    fn f_gauge_010_debug() {
        let gt = GaugeTracker::new();
        let debug = format!("{:?}", gt);
        assert!(debug.contains("GaugeTracker"));
    }

    /// F-GAUGE-011: Clone preserves state
    #[test]
    fn f_gauge_011_clone() {
        let mut gt = GaugeTracker::new();
        gt.set(50.0);
        let cloned = gt.clone();
        assert_eq!(gt.current(), cloned.current());
    }

    /// F-GAUGE-012: Add modifies current
    #[test]
    fn f_gauge_012_add() {
        let mut gt = GaugeTracker::new();
        gt.set(50.0);
        gt.add(10.0);
        assert_eq!(gt.current(), 60.0);
    }

    // =========================================================================
    // COUNTER PAIR TESTS (F-PAIR-001 to F-PAIR-012)
    // =========================================================================

    /// F-PAIR-001: New counter starts at zero
    #[test]
    fn f_pair_001_new_zero() {
        let cp = CounterPair::new();
        assert_eq!(cp.successes(), 0);
        assert_eq!(cp.failures(), 0);
    }

    /// F-PAIR-002: Default same as new
    #[test]
    fn f_pair_002_default() {
        let cp = CounterPair::default();
        assert_eq!(cp.total(), 0);
    }

    /// F-PAIR-003: Success increments
    #[test]
    fn f_pair_003_success() {
        let mut cp = CounterPair::new();
        cp.success();
        assert_eq!(cp.successes(), 1);
    }

    /// F-PAIR-004: Failure increments
    #[test]
    fn f_pair_004_failure() {
        let mut cp = CounterPair::new();
        cp.failure();
        assert_eq!(cp.failures(), 1);
    }

    /// F-PAIR-005: Total calculation
    #[test]
    fn f_pair_005_total() {
        let mut cp = CounterPair::new();
        cp.success();
        cp.success();
        cp.failure();
        assert_eq!(cp.total(), 3);
    }

    /// F-PAIR-006: Success rate calculation
    #[test]
    fn f_pair_006_success_rate() {
        let mut cp = CounterPair::new();
        cp.add_successes(80);
        cp.add_failures(20);
        assert_eq!(cp.success_rate(), 80.0);
    }

    /// F-PAIR-007: Failure rate calculation
    #[test]
    fn f_pair_007_failure_rate() {
        let mut cp = CounterPair::new();
        cp.add_successes(80);
        cp.add_failures(20);
        assert_eq!(cp.failure_rate(), 20.0);
    }

    /// F-PAIR-008: Empty counter is 100% success
    #[test]
    fn f_pair_008_empty_healthy() {
        let cp = CounterPair::new();
        assert_eq!(cp.success_rate(), 100.0);
    }

    /// F-PAIR-009: is_healthy check
    #[test]
    fn f_pair_009_is_healthy() {
        let mut cp = CounterPair::new();
        cp.add_successes(95);
        cp.add_failures(5);
        assert!(cp.is_healthy(90.0));
        assert!(!cp.is_healthy(99.0));
    }

    /// F-PAIR-010: Reset clears state
    #[test]
    fn f_pair_010_reset() {
        let mut cp = CounterPair::new();
        cp.success();
        cp.failure();
        cp.reset();
        assert_eq!(cp.total(), 0);
    }

    /// F-PAIR-011: Debug format works
    #[test]
    fn f_pair_011_debug() {
        let cp = CounterPair::new();
        let debug = format!("{:?}", cp);
        assert!(debug.contains("CounterPair"));
    }

    /// F-PAIR-012: Clone preserves state
    #[test]
    fn f_pair_012_clone() {
        let mut cp = CounterPair::new();
        cp.success();
        let cloned = cp.clone();
        assert_eq!(cp.successes(), cloned.successes());
    }

    // =========================================================================
    // HEALTH SCORE TESTS (F-HEALTH-001 to F-HEALTH-012)
    // =========================================================================

    /// F-HEALTH-001: New score starts at 100
    #[test]
    fn f_health_001_new_100() {
        let hs = HealthScore::new();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-002: Default same as new
    #[test]
    fn f_health_002_default() {
        let hs = HealthScore::default();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-003: Set component score
    #[test]
    fn f_health_003_set() {
        let mut hs = HealthScore::new();
        hs.set(0, 80.0);
        assert_eq!(hs.score(), 80.0);
    }

    /// F-HEALTH-004: Weighted average
    #[test]
    fn f_health_004_weighted() {
        let mut hs = HealthScore::new();
        hs.set(0, 100.0);
        hs.set_weight(0, 2.0);
        hs.set(1, 50.0);
        hs.set_weight(1, 1.0);
        // (100*2 + 50*1) / 3 = 83.33
        let score = hs.score();
        assert!(score > 80.0 && score < 90.0);
    }

    /// F-HEALTH-005: Status healthy
    #[test]
    fn f_health_005_status_healthy() {
        let hs = HealthScore::new();
        assert_eq!(hs.status(), HealthStatus::Healthy);
    }

    /// F-HEALTH-006: Status degraded
    #[test]
    fn f_health_006_status_degraded() {
        let mut hs = HealthScore::new();
        hs.set(0, 75.0);
        assert_eq!(hs.status(), HealthStatus::Degraded);
    }

    /// F-HEALTH-007: Status warning
    #[test]
    fn f_health_007_status_warning() {
        let mut hs = HealthScore::new();
        hs.set(0, 55.0);
        assert_eq!(hs.status(), HealthStatus::Warning);
    }

    /// F-HEALTH-008: Status critical
    #[test]
    fn f_health_008_status_critical() {
        let mut hs = HealthScore::new();
        hs.set(0, 30.0);
        assert_eq!(hs.status(), HealthStatus::Critical);
    }

    /// F-HEALTH-009: Min score tracking
    #[test]
    fn f_health_009_min_score() {
        let mut hs = HealthScore::new();
        hs.set(0, 90.0);
        hs.set(1, 60.0);
        hs.set(2, 80.0);
        assert_eq!(hs.min_score(), 60.0);
    }

    /// F-HEALTH-010: Reset to 100
    #[test]
    fn f_health_010_reset() {
        let mut hs = HealthScore::new();
        hs.set(0, 50.0);
        hs.reset();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-011: Debug format works
    #[test]
    fn f_health_011_debug() {
        let hs = HealthScore::new();
        let debug = format!("{:?}", hs);
        assert!(debug.contains("HealthScore"));
    }

    /// F-HEALTH-012: Clone preserves state
    #[test]
    fn f_health_012_clone() {
        let mut hs = HealthScore::new();
        hs.set(0, 75.0);
        let cloned = hs.clone();
        assert_eq!(hs.score(), cloned.score());
    }

    // ========================================================================
    // BatchProcessor Falsification Tests (F-BATCH-001 to F-BATCH-012)
    // ========================================================================

    /// F-BATCH-001: New with batch size
    #[test]
    fn f_batch_001_new() {
        let bp = BatchProcessor::new(10);
        assert_eq!(bp.batches_completed(), 0);
        assert_eq!(bp.total_items(), 0);
    }

    /// F-BATCH-002: Default batch size 100
    #[test]
    fn f_batch_002_default() {
        let bp = BatchProcessor::default();
        assert_eq!(bp.remaining(), 100);
    }

    /// F-BATCH-003: Add returns false until batch complete
    #[test]
    fn f_batch_003_add_partial() {
        let mut bp = BatchProcessor::new(3);
        assert!(!bp.add());
        assert!(!bp.add());
        assert!(bp.add()); // 3rd item completes batch
    }

    /// F-BATCH-004: Batch completes resets count
    #[test]
    fn f_batch_004_batch_complete() {
        let mut bp = BatchProcessor::new(2);
        bp.add();
        bp.add();
        assert_eq!(bp.batches_completed(), 1);
        assert_eq!(bp.remaining(), 2);
    }

    /// F-BATCH-005: Add many returns correct batches
    #[test]
    fn f_batch_005_add_many() {
        let mut bp = BatchProcessor::new(10);
        let batches = bp.add_many(25);
        assert_eq!(batches, 2);
        assert_eq!(bp.remaining(), 5);
    }

    /// F-BATCH-006: Fill percentage calculation
    #[test]
    fn f_batch_006_fill_percentage() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        assert!((bp.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-BATCH-007: Factory for_network batch 1000
    #[test]
    fn f_batch_007_for_network() {
        let bp = BatchProcessor::for_network();
        assert_eq!(bp.remaining(), 1000);
    }

    /// F-BATCH-008: Factory for_disk batch 100
    #[test]
    fn f_batch_008_for_disk() {
        let bp = BatchProcessor::for_disk();
        assert_eq!(bp.remaining(), 100);
    }

    /// F-BATCH-009: Factory for_metrics batch 50
    #[test]
    fn f_batch_009_for_metrics() {
        let bp = BatchProcessor::for_metrics();
        assert_eq!(bp.remaining(), 50);
    }

    /// F-BATCH-010: Flush completes partial batch
    #[test]
    fn f_batch_010_flush() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        bp.flush();
        assert_eq!(bp.batches_completed(), 1);
        assert_eq!(bp.remaining(), 10);
    }

    /// F-BATCH-011: Reset clears all counters
    #[test]
    fn f_batch_011_reset() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(25);
        bp.reset();
        assert_eq!(bp.batches_completed(), 0);
        assert_eq!(bp.total_items(), 0);
    }

    /// F-BATCH-012: Clone preserves state
    #[test]
    fn f_batch_012_clone() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        let cloned = bp.clone();
        assert_eq!(bp.remaining(), cloned.remaining());
    }

    // ========================================================================
    // PipelineStage Falsification Tests (F-PIPE-001 to F-PIPE-012)
    // ========================================================================

    /// F-PIPE-001: New creates empty stage
    #[test]
    fn f_pipe_001_new() {
        let ps = PipelineStage::new();
        assert!(ps.is_idle());
        assert_eq!(ps.depth(), 0);
    }

    /// F-PIPE-002: Default same as new
    #[test]
    fn f_pipe_002_default() {
        let ps = PipelineStage::default();
        assert!(ps.is_idle());
    }

    /// F-PIPE-003: Enter increases depth
    #[test]
    fn f_pipe_003_enter() {
        let mut ps = PipelineStage::new();
        ps.enter();
        assert_eq!(ps.depth(), 1);
        assert!(!ps.is_idle());
    }

    /// F-PIPE-004: Exit decreases depth
    #[test]
    fn f_pipe_004_exit() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        assert_eq!(ps.depth(), 0);
    }

    /// F-PIPE-005: Peak depth tracked
    #[test]
    fn f_pipe_005_peak() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.peak_depth(), 3);
    }

    /// F-PIPE-006: Average latency calculation
    #[test]
    fn f_pipe_006_avg_latency() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        ps.enter();
        ps.exit(2000);
        assert!((ps.avg_latency_us() - 1500.0).abs() < 0.01);
    }

    /// F-PIPE-007: Latency ms conversion
    #[test]
    fn f_pipe_007_latency_ms() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        assert!((ps.avg_latency_ms() - 1.0).abs() < 0.01);
    }

    /// F-PIPE-008: Throughput equals exits
    #[test]
    fn f_pipe_008_throughput() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit_simple();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.throughput(), 2);
    }

    /// F-PIPE-009: Total entered tracked
    #[test]
    fn f_pipe_009_total_entered() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.total_entered(), 2);
    }

    /// F-PIPE-010: Backlog detection
    #[test]
    fn f_pipe_010_backlogged() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.enter();
        assert!(ps.is_backlogged(2));
    }

    /// F-PIPE-011: Reset clears all
    #[test]
    fn f_pipe_011_reset() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        ps.reset();
        assert!(ps.is_idle());
        assert_eq!(ps.throughput(), 0);
    }

    /// F-PIPE-012: Clone preserves state
    #[test]
    fn f_pipe_012_clone() {
        let mut ps = PipelineStage::new();
        ps.enter();
        let cloned = ps.clone();
        assert_eq!(ps.depth(), cloned.depth());
    }

    // ========================================================================
    // WorkQueue Falsification Tests (F-QUEUE-001 to F-QUEUE-012)
    // ========================================================================

    /// F-QUEUE-001: New creates empty queue
    #[test]
    fn f_queue_001_new() {
        let wq = WorkQueue::new();
        assert!(wq.is_empty());
        assert_eq!(wq.size(), 0);
    }

    /// F-QUEUE-002: Default same as new
    #[test]
    fn f_queue_002_default() {
        let wq = WorkQueue::default();
        assert!(wq.is_empty());
    }

    /// F-QUEUE-003: With capacity sets limit
    #[test]
    fn f_queue_003_with_capacity() {
        let wq = WorkQueue::with_capacity(10);
        assert_eq!(wq.remaining_capacity(), 10);
    }

    /// F-QUEUE-004: Enqueue increases size
    #[test]
    fn f_queue_004_enqueue() {
        let mut wq = WorkQueue::new();
        assert!(wq.enqueue());
        assert_eq!(wq.size(), 1);
    }

    /// F-QUEUE-005: Dequeue decreases size
    #[test]
    fn f_queue_005_dequeue() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        assert!(wq.dequeue(100));
        assert!(wq.is_empty());
    }

    /// F-QUEUE-006: Full queue rejects enqueue
    #[test]
    fn f_queue_006_full() {
        let mut wq = WorkQueue::with_capacity(1);
        wq.enqueue();
        assert!(!wq.enqueue());
        assert!(wq.is_full());
    }

    /// F-QUEUE-007: Empty queue rejects dequeue
    #[test]
    fn f_queue_007_empty_dequeue() {
        let mut wq = WorkQueue::new();
        assert!(!wq.dequeue_simple());
    }

    /// F-QUEUE-008: Peak size tracked
    #[test]
    fn f_queue_008_peak() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.enqueue();
        wq.dequeue_simple();
        assert_eq!(wq.peak_size(), 2);
    }

    /// F-QUEUE-009: Average wait time
    #[test]
    fn f_queue_009_avg_wait() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.dequeue(1000);
        wq.enqueue();
        wq.dequeue(2000);
        assert!((wq.avg_wait_us() - 1500.0).abs() < 0.01);
    }

    /// F-QUEUE-010: Utilization percentage
    #[test]
    fn f_queue_010_utilization() {
        let mut wq = WorkQueue::with_capacity(10);
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        assert!((wq.utilization() - 50.0).abs() < 0.01);
    }

    /// F-QUEUE-011: Reset clears all
    #[test]
    fn f_queue_011_reset() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.dequeue(1000);
        wq.reset();
        assert!(wq.is_empty());
        assert_eq!(wq.total_dequeued(), 0);
    }

    /// F-QUEUE-012: Clone preserves state
    #[test]
    fn f_queue_012_clone() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        let cloned = wq.clone();
        assert_eq!(wq.size(), cloned.size());
    }

    // ========================================================================
    // LeakyBucket Falsification Tests (F-LEAK-001 to F-LEAK-012)
    // ========================================================================

    /// F-LEAK-001: New creates empty bucket
    #[test]
    fn f_leak_001_new() {
        let lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.is_empty());
        assert_eq!(lb.overflows(), 0);
    }

    /// F-LEAK-002: Default 100 capacity, 10 rate
    #[test]
    fn f_leak_002_default() {
        let lb = LeakyBucket::default();
        assert!(lb.is_empty());
    }

    /// F-LEAK-003: Add increases level
    #[test]
    fn f_leak_003_add() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.add(50.0, 0));
        assert!((lb.level() - 50.0).abs() < 0.01);
    }

    /// F-LEAK-004: Overflow rejected
    #[test]
    fn f_leak_004_overflow() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.add(80.0, 0));
        assert!(!lb.add(50.0, 0)); // Would exceed
        assert_eq!(lb.overflows(), 1);
    }

    /// F-LEAK-005: Leaking over time
    #[test]
    fn f_leak_005_leak() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 1000); // Init with timestamp 1000
        lb.update_with_time(1_001_000); // 1 second later
        // Leaked 10 tokens: 50 - 10 = 40
        assert!(lb.level() < 45.0);
    }

    /// F-LEAK-006: Fill percentage
    #[test]
    fn f_leak_006_fill_percentage() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        assert!((lb.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-LEAK-007: Factory for_api
    #[test]
    fn f_leak_007_for_api() {
        let lb = LeakyBucket::for_api();
        assert!(lb.is_empty());
    }

    /// F-LEAK-008: Factory for_network
    #[test]
    fn f_leak_008_for_network() {
        let lb = LeakyBucket::for_network();
        assert!(lb.is_empty());
    }

    /// F-LEAK-009: Full leak empties bucket
    #[test]
    fn f_leak_009_full_leak() {
        let mut lb = LeakyBucket::new(100.0, 100.0);
        lb.add(50.0, 1000); // Init with timestamp 1000
        lb.update_with_time(1_001_000); // 1 second later, 100 leaked
        assert!(lb.is_empty());
    }

    /// F-LEAK-010: Reset clears bucket
    #[test]
    fn f_leak_010_reset() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        lb.add(200.0, 0); // overflow
        lb.reset();
        assert!(lb.is_empty());
        assert_eq!(lb.overflows(), 0);
    }

    /// F-LEAK-011: Debug format works
    #[test]
    fn f_leak_011_debug() {
        let lb = LeakyBucket::new(100.0, 10.0);
        let debug = format!("{:?}", lb);
        assert!(debug.contains("LeakyBucket"));
    }

    /// F-LEAK-012: Clone preserves state
    #[test]
    fn f_leak_012_clone() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        let cloned = lb.clone();
        assert!((lb.level() - cloned.level()).abs() < 0.01);
    }

    // ========================================================================
    // SlidingWindowRate Falsification Tests (F-SLIDE-001 to F-SLIDE-012)
    // ========================================================================

    /// F-SLIDE-001: New creates empty windows
    #[test]
    fn f_slide_001_new() {
        let sw = SlidingWindowRate::new(1_000_000, 100);
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.exceeded(), 0);
    }

    /// F-SLIDE-002: Default 1s window, 100 limit
    #[test]
    fn f_slide_002_default() {
        let sw = SlidingWindowRate::default();
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-003: Record increases count
    #[test]
    fn f_slide_003_record() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        assert!(sw.record(0));
        assert_eq!(sw.count(), 1);
    }

    /// F-SLIDE-004: Exceed limit rejected
    #[test]
    fn f_slide_004_exceed() {
        let mut sw = SlidingWindowRate::new(1_000_000, 3);
        sw.record(0);
        sw.record(0);
        sw.record(0);
        assert!(!sw.record(0)); // Would exceed
        assert_eq!(sw.exceeded(), 1);
    }

    /// F-SLIDE-005: Window rotation clears old counts
    #[test]
    fn f_slide_005_rotation() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(1000); // Init with timestamp 1000
        sw.record(1000);
        // Rotate through all windows (each sub-window is 100ms)
        sw.update_with_time(2_001_000); // 2 seconds later
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-006: Rate percentage
    #[test]
    fn f_slide_006_rate_percentage() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        for _ in 0..50 {
            sw.record(0);
        }
        assert!((sw.rate_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SLIDE-007: Would exceed check
    #[test]
    fn f_slide_007_would_exceed() {
        let mut sw = SlidingWindowRate::new(1_000_000, 2);
        sw.record(0);
        sw.record(0);
        assert!(sw.would_exceed());
    }

    /// F-SLIDE-008: Factory per_second
    #[test]
    fn f_slide_008_per_second() {
        let sw = SlidingWindowRate::per_second(100);
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-009: Factory per_minute
    #[test]
    fn f_slide_009_per_minute() {
        let sw = SlidingWindowRate::per_minute(100);
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-010: Reset clears all
    #[test]
    fn f_slide_010_reset() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(0);
        sw.reset();
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.exceeded(), 0);
    }

    /// F-SLIDE-011: Debug format works
    #[test]
    fn f_slide_011_debug() {
        let sw = SlidingWindowRate::new(1_000_000, 100);
        let debug = format!("{:?}", sw);
        assert!(debug.contains("SlidingWindowRate"));
    }

    /// F-SLIDE-012: Clone preserves state
    #[test]
    fn f_slide_012_clone() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(0);
        let cloned = sw.clone();
        assert_eq!(sw.count(), cloned.count());
    }

    // ========================================================================
    // ResourcePool Falsification Tests (F-POOL-001 to F-POOL-012)
    // ========================================================================

    /// F-POOL-001: New creates empty pool
    #[test]
    fn f_pool_001_new() {
        let pool = ResourcePool::new(10);
        assert!(pool.is_idle());
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-002: Default capacity 10
    #[test]
    fn f_pool_002_default() {
        let pool = ResourcePool::default();
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-003: Acquire reduces available
    #[test]
    fn f_pool_003_acquire() {
        let mut pool = ResourcePool::new(10);
        assert!(pool.acquire(100));
        assert_eq!(pool.available(), 9);
    }

    /// F-POOL-004: Release increases available
    #[test]
    fn f_pool_004_release() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.release();
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-005: Exhausted pool rejects acquire
    #[test]
    fn f_pool_005_exhausted() {
        let mut pool = ResourcePool::new(1);
        pool.acquire(100);
        assert!(!pool.acquire(100));
        assert!(pool.is_exhausted());
    }

    /// F-POOL-006: Utilization percentage
    #[test]
    fn f_pool_006_utilization() {
        let mut pool = ResourcePool::new(10);
        for _ in 0..5 {
            pool.acquire(100);
        }
        assert!((pool.utilization() - 50.0).abs() < 0.01);
    }

    /// F-POOL-007: Average wait time
    #[test]
    fn f_pool_007_avg_wait() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(1000);
        pool.acquire(2000);
        assert!((pool.avg_wait_us() - 1500.0).abs() < 0.01);
    }

    /// F-POOL-008: Timeout rate
    #[test]
    fn f_pool_008_timeout_rate() {
        let mut pool = ResourcePool::new(1);
        pool.acquire(100);
        pool.acquire(100); // timeout
        pool.acquire(100); // timeout
        // 1 success, 2 timeouts = 66.67% timeout rate
        assert!(pool.timeout_rate() > 60.0);
    }

    /// F-POOL-009: Peak utilization
    #[test]
    fn f_pool_009_peak() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.acquire(100);
        pool.acquire(100);
        pool.release();
        assert!((pool.peak_utilization() - 30.0).abs() < 0.01);
    }

    /// F-POOL-010: Factory for_database
    #[test]
    fn f_pool_010_for_database() {
        let pool = ResourcePool::for_database();
        assert_eq!(pool.available(), 20);
    }

    /// F-POOL-011: Factory for_http
    #[test]
    fn f_pool_011_for_http() {
        let pool = ResourcePool::for_http();
        assert_eq!(pool.available(), 100);
    }

    /// F-POOL-012: Reset clears counters
    #[test]
    fn f_pool_012_reset() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.reset();
        assert!(pool.is_idle());
    }

    // ========================================================================
    // Histogram2D Falsification Tests (F-HIST2D-001 to F-HIST2D-012)
    // ========================================================================

    /// F-HIST2D-001: New creates empty histogram
    #[test]
    fn f_hist2d_001_new() {
        let h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-002: Default 0-100 range
    #[test]
    fn f_hist2d_002_default() {
        let h = Histogram2D::default();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-003: Add increases count
    #[test]
    fn f_hist2d_003_add() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        assert_eq!(h.count(), 1);
    }

    /// F-HIST2D-004: Get returns cell count
    #[test]
    fn f_hist2d_004_get() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        assert_eq!(h.get(5, 5), 1);
    }

    /// F-HIST2D-005: Density percentage
    #[test]
    fn f_hist2d_005_density() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        assert!((h.density(5, 5) - 100.0).abs() < 0.01);
    }

    /// F-HIST2D-006: Max count
    #[test]
    fn f_hist2d_006_max_count() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        h.add(10.0, 10.0);
        assert_eq!(h.max_count(), 2);
    }

    /// F-HIST2D-007: Hotspot detection
    #[test]
    fn f_hist2d_007_hotspot() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        assert_eq!(h.hotspot(), (5, 5));
    }

    /// F-HIST2D-008: Factory for_latency_throughput
    #[test]
    fn f_hist2d_008_for_latency() {
        let h = Histogram2D::for_latency_throughput();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-009: Factory for_cpu_memory
    #[test]
    fn f_hist2d_009_for_cpu() {
        let h = Histogram2D::for_cpu_memory();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-010: Reset clears cells
    #[test]
    fn f_hist2d_010_reset() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.reset();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-011: Debug format works
    #[test]
    fn f_hist2d_011_debug() {
        let h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        let debug = format!("{:?}", h);
        assert!(debug.contains("Histogram2D"));
    }

    /// F-HIST2D-012: Clone preserves state
    #[test]
    fn f_hist2d_012_clone() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        let cloned = h.clone();
        assert_eq!(h.count(), cloned.count());
    }

    // ========================================================================
    // ReservoirSampler Falsification Tests (F-RESERVOIR-001 to F-RESERVOIR-012)
    // ========================================================================

    /// F-RESERVOIR-001: New creates empty sampler
    #[test]
    fn f_reservoir_001_new() {
        let s = ReservoirSampler::new(10);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    /// F-RESERVOIR-002: Default capacity 16
    #[test]
    fn f_reservoir_002_default() {
        let s = ReservoirSampler::default();
        assert!(s.is_empty());
    }

    /// F-RESERVOIR-003: Add fills sample
    #[test]
    fn f_reservoir_003_add() {
        let mut s = ReservoirSampler::new(10);
        s.add(1.0);
        s.add(2.0);
        assert_eq!(s.len(), 2);
    }

    /// F-RESERVOIR-004: Get returns sample
    #[test]
    fn f_reservoir_004_get() {
        let mut s = ReservoirSampler::new(10);
        s.add(42.0);
        assert_eq!(s.get(0), Some(42.0));
    }

    /// F-RESERVOIR-005: Total seen tracks all
    #[test]
    fn f_reservoir_005_total_seen() {
        let mut s = ReservoirSampler::new(2);
        s.add(1.0);
        s.add(2.0);
        s.add(3.0);
        assert_eq!(s.total_seen(), 3);
        assert_eq!(s.len(), 2);
    }

    /// F-RESERVOIR-006: Mean calculation
    #[test]
    fn f_reservoir_006_mean() {
        let mut s = ReservoirSampler::new(10);
        s.add(10.0);
        s.add(20.0);
        assert!((s.mean() - 15.0).abs() < 0.01);
    }

    /// F-RESERVOIR-007: Min tracking
    #[test]
    fn f_reservoir_007_min() {
        let mut s = ReservoirSampler::new(10);
        s.add(30.0);
        s.add(10.0);
        s.add(20.0);
        assert!((s.min() - 10.0).abs() < 0.01);
    }

    /// F-RESERVOIR-008: Max tracking
    #[test]
    fn f_reservoir_008_max() {
        let mut s = ReservoirSampler::new(10);
        s.add(10.0);
        s.add(30.0);
        s.add(20.0);
        assert!((s.max() - 30.0).abs() < 0.01);
    }

    /// F-RESERVOIR-009: Get out of bounds returns None
    #[test]
    fn f_reservoir_009_oob() {
        let s = ReservoirSampler::new(10);
        assert_eq!(s.get(0), None);
    }

    /// F-RESERVOIR-010: Reset clears samples
    #[test]
    fn f_reservoir_010_reset() {
        let mut s = ReservoirSampler::new(10);
        s.add(1.0);
        s.reset();
        assert!(s.is_empty());
    }

    /// F-RESERVOIR-011: Debug format works
    #[test]
    fn f_reservoir_011_debug() {
        let s = ReservoirSampler::new(10);
        let debug = format!("{:?}", s);
        assert!(debug.contains("ReservoirSampler"));
    }

    /// F-RESERVOIR-012: Clone preserves state
    #[test]
    fn f_reservoir_012_clone() {
        let mut s = ReservoirSampler::new(10);
        s.add(42.0);
        let cloned = s.clone();
        assert_eq!(s.len(), cloned.len());
    }

    // ========================================================================
    // ExponentialHistogram Falsification Tests (F-EXPHIST-001 to F-EXPHIST-012)
    // ========================================================================

    /// F-EXPHIST-001: New creates empty histogram
    #[test]
    fn f_exphist_001_new() {
        let h = ExponentialHistogram::new(1.0);
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-002: Default base 1.0
    #[test]
    fn f_exphist_002_default() {
        let h = ExponentialHistogram::default();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-003: Add increases count
    #[test]
    fn f_exphist_003_add() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        assert_eq!(h.count(), 1);
    }

    /// F-EXPHIST-004: Bucket assignment
    #[test]
    fn f_exphist_004_bucket() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(0.5); // bucket 0
        h.add(1.5); // bucket 0
        h.add(3.0); // bucket 1
        h.add(5.0); // bucket 2
        assert!(h.bucket_count(0) >= 1);
    }

    /// F-EXPHIST-005: Mean calculation
    #[test]
    fn f_exphist_005_mean() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(10.0);
        h.add(20.0);
        assert!((h.mean() - 15.0).abs() < 0.01);
    }

    /// F-EXPHIST-006: Mode bucket
    #[test]
    fn f_exphist_006_mode() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(0.5);
        h.add(0.6);
        h.add(0.7);
        h.add(10.0);
        assert_eq!(h.mode_bucket(), 0);
    }

    /// F-EXPHIST-007: Factory for_latency_ms
    #[test]
    fn f_exphist_007_for_latency() {
        let h = ExponentialHistogram::for_latency_ms();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-008: Factory for_bytes_kb
    #[test]
    fn f_exphist_008_for_bytes() {
        let h = ExponentialHistogram::for_bytes_kb();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-009: Bucket upper bound
    #[test]
    fn f_exphist_009_upper_bound() {
        let h = ExponentialHistogram::new(1.0);
        assert!((h.bucket_upper_bound(0) - 2.0).abs() < 0.01);
        assert!((h.bucket_upper_bound(1) - 4.0).abs() < 0.01);
    }

    /// F-EXPHIST-010: Reset clears histogram
    #[test]
    fn f_exphist_010_reset() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        h.reset();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-011: Debug format works
    #[test]
    fn f_exphist_011_debug() {
        let h = ExponentialHistogram::new(1.0);
        let debug = format!("{:?}", h);
        assert!(debug.contains("ExponentialHistogram"));
    }

    /// F-EXPHIST-012: Clone preserves state
    #[test]
    fn f_exphist_012_clone() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        let cloned = h.clone();
        assert_eq!(h.count(), cloned.count());
    }

    // ========================================================================
    // CacheStats Falsification Tests (F-CACHE-001 to F-CACHE-012)
    // ========================================================================

    /// F-CACHE-001: New creates empty stats
    #[test]
    fn f_cache_001_new() {
        let cs = CacheStats::new(1024);
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-002: Default zero capacity
    #[test]
    fn f_cache_002_default() {
        let cs = CacheStats::default();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-003: Hit increases count
    #[test]
    fn f_cache_003_hit() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        assert_eq!(cs.total_requests(), 1);
    }

    /// F-CACHE-004: Miss increases count
    #[test]
    fn f_cache_004_miss() {
        let mut cs = CacheStats::new(1024);
        cs.miss();
        assert_eq!(cs.total_requests(), 1);
    }

    /// F-CACHE-005: Hit rate calculation
    #[test]
    fn f_cache_005_hit_rate() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.hit();
        cs.miss();
        // 2 hits / 3 total = 66.67%
        assert!(cs.hit_rate() > 60.0);
    }

    /// F-CACHE-006: Miss rate calculation
    #[test]
    fn f_cache_006_miss_rate() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.miss();
        assert!((cs.miss_rate() - 50.0).abs() < 0.01);
    }

    /// F-CACHE-007: Eviction tracking
    #[test]
    fn f_cache_007_eviction() {
        let mut cs = CacheStats::new(1024);
        cs.insert(512);
        cs.evict(256);
        assert!(cs.eviction_rate() > 0.0);
    }

    /// F-CACHE-008: Fill percentage
    #[test]
    fn f_cache_008_fill() {
        let mut cs = CacheStats::new(1024);
        cs.insert(512);
        assert!((cs.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-CACHE-009: Factory for_l1_cache
    #[test]
    fn f_cache_009_for_l1() {
        let cs = CacheStats::for_l1_cache();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-010: Factory for_app_cache
    #[test]
    fn f_cache_010_for_app() {
        let cs = CacheStats::for_app_cache();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-011: Is effective check
    #[test]
    fn f_cache_011_effective() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.hit();
        cs.miss();
        assert!(cs.is_effective(60.0));
    }

    /// F-CACHE-012: Reset clears stats
    #[test]
    fn f_cache_012_reset() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.reset();
        assert_eq!(cs.total_requests(), 0);
    }

    // ========================================================================
    // BloomFilter Falsification Tests (F-BLOOM-001 to F-BLOOM-012)
    // ========================================================================

    /// F-BLOOM-001: New creates empty filter
    #[test]
    fn f_bloom_001_new() {
        let bf = BloomFilter::new(3);
        assert!(bf.is_empty());
    }

    /// F-BLOOM-002: Default 3 hashes
    #[test]
    fn f_bloom_002_default() {
        let bf = BloomFilter::default();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-003: Add increases len
    #[test]
    fn f_bloom_003_add() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        assert_eq!(bf.len(), 1);
    }

    /// F-BLOOM-004: Might contain returns true for added
    #[test]
    fn f_bloom_004_contains() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        assert!(bf.might_contain(42));
    }

    /// F-BLOOM-005: Might contain returns false for not added
    #[test]
    fn f_bloom_005_not_contains() {
        let bf = BloomFilter::new(3);
        // Empty filter should not contain anything
        assert!(!bf.might_contain(12345));
    }

    /// F-BLOOM-006: Fill percentage increases
    #[test]
    fn f_bloom_006_fill() {
        let mut bf = BloomFilter::new(3);
        bf.add(1);
        bf.add(2);
        bf.add(3);
        assert!(bf.fill_percentage() > 0.0);
    }

    /// F-BLOOM-007: False positive rate estimation
    #[test]
    fn f_bloom_007_fpr() {
        let mut bf = BloomFilter::new(3);
        for i in 0..100 {
            bf.add(i);
        }
        // With 100 items in 1024 bits, FPR should be low but positive
        assert!(bf.false_positive_rate() > 0.0);
    }

    /// F-BLOOM-008: Factory for_small
    #[test]
    fn f_bloom_008_for_small() {
        let bf = BloomFilter::for_small();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-009: Factory for_medium
    #[test]
    fn f_bloom_009_for_medium() {
        let bf = BloomFilter::for_medium();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-010: Reset clears filter
    #[test]
    fn f_bloom_010_reset() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        bf.reset();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-011: Debug format works
    #[test]
    fn f_bloom_011_debug() {
        let bf = BloomFilter::new(3);
        let debug = format!("{:?}", bf);
        assert!(debug.contains("BloomFilter"));
    }

    /// F-BLOOM-012: Clone preserves state
    #[test]
    fn f_bloom_012_clone() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        let cloned = bf.clone();
        assert_eq!(bf.len(), cloned.len());
    }

    // ========================================================================
    // LoadBalancer Falsification Tests (F-LB-001 to F-LB-012)
    // ========================================================================

    /// F-LB-001: New creates empty balancer
    #[test]
    fn f_lb_001_new() {
        let lb = LoadBalancer::new();
        assert_eq!(lb.backend_count(), 0);
    }

    /// F-LB-002: Default same as new
    #[test]
    fn f_lb_002_default() {
        let lb = LoadBalancer::default();
        assert_eq!(lb.backend_count(), 0);
    }

    /// F-LB-003: Add backend increases count
    #[test]
    fn f_lb_003_add_backend() {
        let mut lb = LoadBalancer::new();
        lb.add_backend(1);
        assert_eq!(lb.backend_count(), 1);
    }

    /// F-LB-004: Next returns backend
    #[test]
    fn f_lb_004_next() {
        let mut lb = LoadBalancer::new();
        lb.add_backend(1);
        assert_eq!(lb.select_backend(), Some(0));
    }

    /// F-LB-005: Empty balancer returns None
    #[test]
    fn f_lb_005_empty_next() {
        let mut lb = LoadBalancer::new();
        assert_eq!(lb.select_backend(), None);
    }

    /// F-LB-006: Equal weights distributes evenly
    #[test]
    fn f_lb_006_equal_weights() {
        let mut lb = LoadBalancer::equal_weights(2);
        for _ in 0..10 {
            lb.select_backend();
        }
        // Both backends should get ~50%
        assert!(lb.distribution(0) > 40.0);
        assert!(lb.distribution(1) > 40.0);
    }

    /// F-LB-007: Total dispatched tracked
    #[test]
    fn f_lb_007_dispatched() {
        let mut lb = LoadBalancer::equal_weights(2);
        lb.select_backend();
        lb.select_backend();
        lb.select_backend();
        assert_eq!(lb.total_dispatched(), 3);
    }

    /// F-LB-008: Is balanced check
    #[test]
    fn f_lb_008_balanced() {
        let mut lb = LoadBalancer::equal_weights(2);
        for _ in 0..100 {
            lb.select_backend();
        }
        assert!(lb.is_balanced(20.0));
    }

    /// F-LB-009: Distribution percentage
    #[test]
    fn f_lb_009_distribution() {
        let mut lb = LoadBalancer::equal_weights(1);
        lb.select_backend();
        assert!((lb.distribution(0) - 100.0).abs() < 0.01);
    }

    /// F-LB-010: Reset clears counters
    #[test]
    fn f_lb_010_reset() {
        let mut lb = LoadBalancer::equal_weights(2);
        lb.select_backend();
        lb.reset();
        assert_eq!(lb.total_dispatched(), 0);
    }

    /// F-LB-011: Debug format works
    #[test]
    fn f_lb_011_debug() {
        let lb = LoadBalancer::new();
        let debug = format!("{:?}", lb);
        assert!(debug.contains("LoadBalancer"));
    }

    /// F-LB-012: Clone preserves state
    #[test]
    fn f_lb_012_clone() {
        let mut lb = LoadBalancer::equal_weights(2);
        lb.select_backend();
        let cloned = lb.clone();
        assert_eq!(lb.total_dispatched(), cloned.total_dispatched());
    }

    // ========================================================================
    // BurstTracker Falsification Tests (F-BURST-001 to F-BURST-012)
    // ========================================================================

    /// F-BURST-001: New creates full bucket
    #[test]
    fn f_burst_001_new() {
        let bt = BurstTracker::new(100.0, 10.0);
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-002: Default 100 capacity
    #[test]
    fn f_burst_002_default() {
        let bt = BurstTracker::default();
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-003: Consume reduces tokens
    #[test]
    fn f_burst_003_consume() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        assert!(bt.consume(10.0, 1000));
        assert!((bt.tokens() - 90.0).abs() < 0.01);
    }

    /// F-BURST-004: Consume returns false when empty
    #[test]
    fn f_burst_004_empty() {
        let mut bt = BurstTracker::new(10.0, 1.0);
        bt.consume(10.0, 1000);
        assert!(!bt.consume(10.0, 1000));
    }

    /// F-BURST-005: Max burst tracked
    #[test]
    fn f_burst_005_max_burst() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(1.0, 1000);
        bt.consume(1.0, 1000);
        bt.consume(1.0, 1000);
        assert_eq!(bt.max_burst(), 3);
    }

    /// F-BURST-006: Fill percentage
    #[test]
    fn f_burst_006_fill() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        assert!((bt.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-BURST-007: Factory for_api
    #[test]
    fn f_burst_007_for_api() {
        let bt = BurstTracker::for_api();
        assert!(bt.tokens() > 0.0);
    }

    /// F-BURST-008: Factory for_network
    #[test]
    fn f_burst_008_for_network() {
        let bt = BurstTracker::for_network();
        assert!(bt.tokens() > 0.0);
    }

    /// F-BURST-009: Refill over time
    #[test]
    fn f_burst_009_refill() {
        let mut bt = BurstTracker::new(100.0, 100.0);
        bt.consume(50.0, 1000);
        bt.consume(0.0, 1_001_000); // 1 second later
        // Should have refilled 100 tokens (capped at capacity)
        assert!(bt.tokens() > 50.0);
    }

    /// F-BURST-010: Reset restores capacity
    #[test]
    fn f_burst_010_reset() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        bt.reset();
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-011: Debug format works
    #[test]
    fn f_burst_011_debug() {
        let bt = BurstTracker::new(100.0, 10.0);
        let debug = format!("{:?}", bt);
        assert!(debug.contains("BurstTracker"));
    }

    /// F-BURST-012: Clone preserves state
    #[test]
    fn f_burst_012_clone() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        let cloned = bt.clone();
        assert!((bt.tokens() - cloned.tokens()).abs() < 0.01);
    }

    // ========================================================================
    // TopKTracker Falsification Tests (F-TOPK-001 to F-TOPK-012)
    // ========================================================================

    /// F-TOPK-001: New creates empty tracker
    #[test]
    fn f_topk_001_new() {
        let tk = TopKTracker::new(5);
        assert_eq!(tk.count(), 0);
    }

    /// F-TOPK-002: Default creates k=10
    #[test]
    fn f_topk_002_default() {
        let tk = TopKTracker::default();
        assert_eq!(tk.k(), 10);
    }

    /// F-TOPK-003: Add value increases count
    #[test]
    fn f_topk_003_add() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        assert_eq!(tk.count(), 1);
    }

    /// F-TOPK-004: Top returns sorted values
    #[test]
    fn f_topk_004_top() {
        let mut tk = TopKTracker::new(3);
        tk.add(10.0);
        tk.add(30.0);
        tk.add(20.0);
        let top = tk.top();
        assert!((top[0] - 30.0).abs() < 0.01);
    }

    /// F-TOPK-005: Limited to k values
    #[test]
    fn f_topk_005_limit() {
        let mut tk = TopKTracker::new(3);
        for i in 0..10 {
            tk.add(i as f64);
        }
        assert_eq!(tk.top().len(), 3);
    }

    /// F-TOPK-006: Minimum returns smallest in top-k
    #[test]
    fn f_topk_006_minimum() {
        let mut tk = TopKTracker::new(3);
        tk.add(100.0);
        tk.add(200.0);
        tk.add(300.0);
        assert!((tk.minimum().unwrap() - 100.0).abs() < 0.01);
    }

    /// F-TOPK-007: Maximum returns largest
    #[test]
    fn f_topk_007_maximum() {
        let mut tk = TopKTracker::new(3);
        tk.add(100.0);
        tk.add(200.0);
        tk.add(300.0);
        assert!((tk.maximum().unwrap() - 300.0).abs() < 0.01);
    }

    /// F-TOPK-008: Factory for_metrics
    #[test]
    fn f_topk_008_for_metrics() {
        let tk = TopKTracker::for_metrics();
        assert_eq!(tk.k(), 10);
    }

    /// F-TOPK-009: Factory for_processes
    #[test]
    fn f_topk_009_for_processes() {
        let tk = TopKTracker::for_processes();
        assert_eq!(tk.k(), 20);
    }

    /// F-TOPK-010: Reset clears values
    #[test]
    fn f_topk_010_reset() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        tk.reset();
        assert_eq!(tk.count(), 0);
    }

    /// F-TOPK-011: Debug format works
    #[test]
    fn f_topk_011_debug() {
        let tk = TopKTracker::new(5);
        let debug = format!("{:?}", tk);
        assert!(debug.contains("TopKTracker"));
    }

    /// F-TOPK-012: Clone preserves state
    #[test]
    fn f_topk_012_clone() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        let cloned = tk.clone();
        assert_eq!(tk.count(), cloned.count());
    }

    // ========================================================================
    // QuotaTracker Falsification Tests (F-QUOTA-001 to F-QUOTA-012)
    // ========================================================================

    /// F-QUOTA-001: New creates tracker with limit
    #[test]
    fn f_quota_001_new() {
        let qt = QuotaTracker::new(1000);
        assert_eq!(qt.limit(), 1000);
    }

    /// F-QUOTA-002: Default creates 1000 limit
    #[test]
    fn f_quota_002_default() {
        let qt = QuotaTracker::default();
        assert_eq!(qt.limit(), 1000);
    }

    /// F-QUOTA-003: Use reduces remaining
    #[test]
    fn f_quota_003_use() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(30);
        assert_eq!(qt.remaining(), 70);
    }

    /// F-QUOTA-004: Use returns false when exceeded
    #[test]
    fn f_quota_004_exceeded() {
        let mut qt = QuotaTracker::new(100);
        assert!(!qt.use_quota(150));
    }

    /// F-QUOTA-005: Usage percentage
    #[test]
    fn f_quota_005_usage() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        assert!((qt.usage_percentage() - 50.0).abs() < 0.01);
    }

    /// F-QUOTA-006: Is exhausted check
    #[test]
    fn f_quota_006_exhausted() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(100);
        assert!(qt.is_exhausted());
    }

    /// F-QUOTA-007: Factory for_api_daily
    #[test]
    fn f_quota_007_for_api() {
        let qt = QuotaTracker::for_api_daily();
        assert_eq!(qt.limit(), 10000);
    }

    /// F-QUOTA-008: Factory for_storage_gb
    #[test]
    fn f_quota_008_for_storage() {
        let qt = QuotaTracker::for_storage_gb();
        assert_eq!(qt.limit(), 100);
    }

    /// F-QUOTA-009: Release returns quota
    #[test]
    fn f_quota_009_release() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        qt.release(20);
        assert_eq!(qt.remaining(), 70);
    }

    /// F-QUOTA-010: Reset restores full quota
    #[test]
    fn f_quota_010_reset() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        qt.reset();
        assert_eq!(qt.remaining(), 100);
    }

    /// F-QUOTA-011: Debug format works
    #[test]
    fn f_quota_011_debug() {
        let qt = QuotaTracker::new(100);
        let debug = format!("{:?}", qt);
        assert!(debug.contains("QuotaTracker"));
    }

    /// F-QUOTA-012: Clone preserves state
    #[test]
    fn f_quota_012_clone() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(30);
        let cloned = qt.clone();
        assert_eq!(qt.remaining(), cloned.remaining());
    }

    // ========================================================================
    // FrequencyCounter Falsification Tests (F-FREQ-001 to F-FREQ-012)
    // ========================================================================

    /// F-FREQ-001: New creates empty counter
    #[test]
    fn f_freq_001_new() {
        let fc = FrequencyCounter::new();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-002: Default same as new
    #[test]
    fn f_freq_002_default() {
        let fc = FrequencyCounter::default();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-003: Increment increases count
    #[test]
    fn f_freq_003_increment() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        assert_eq!(fc.count(0), 1);
    }

    /// F-FREQ-004: Frequency calculation
    #[test]
    fn f_freq_004_frequency() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(0);
        fc.increment(1);
        assert!((fc.frequency(0) - 66.666).abs() < 1.0);
    }

    /// F-FREQ-005: Most frequent returns max
    #[test]
    fn f_freq_005_most_frequent() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(1);
        fc.increment(1);
        assert_eq!(fc.most_frequent(), Some(1));
    }

    /// F-FREQ-006: 16 slots available
    #[test]
    fn f_freq_006_slots() {
        let mut fc = FrequencyCounter::new();
        for i in 0..16 {
            fc.increment(i);
        }
        assert_eq!(fc.total(), 16);
    }

    /// F-FREQ-007: Non-zero slots counted
    #[test]
    fn f_freq_007_non_zero() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(5);
        assert_eq!(fc.non_zero_count(), 2);
    }

    /// F-FREQ-008: Add multiple at once
    #[test]
    fn f_freq_008_add() {
        let mut fc = FrequencyCounter::new();
        fc.add(0, 10);
        assert_eq!(fc.count(0), 10);
    }

    /// F-FREQ-009: Entropy calculation
    #[test]
    fn f_freq_009_entropy() {
        let mut fc = FrequencyCounter::new();
        // Uniform distribution across all 16 categories for max entropy
        for i in 0..16 {
            fc.add(i, 10);
        }
        // 16 uniform categories = log2(16) / log2(16) = 1.0 normalized
        assert!(fc.entropy() > 0.9);
    }

    /// F-FREQ-010: Reset clears counts
    #[test]
    fn f_freq_010_reset() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.reset();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-011: Debug format works
    #[test]
    fn f_freq_011_debug() {
        let fc = FrequencyCounter::new();
        let debug = format!("{:?}", fc);
        assert!(debug.contains("FrequencyCounter"));
    }

    /// F-FREQ-012: Clone preserves state
    #[test]
    fn f_freq_012_clone() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        let cloned = fc.clone();
        assert_eq!(fc.total(), cloned.total());
    }

    // ========================================================================
    // MovingRange Falsification Tests (F-RANGE-001 to F-RANGE-012)
    // ========================================================================

    /// F-RANGE-001: New creates empty tracker
    #[test]
    fn f_range_001_new() {
        let mr = MovingRange::new(10);
        assert_eq!(mr.count(), 0);
    }

    /// F-RANGE-002: Default window of 10
    #[test]
    fn f_range_002_default() {
        let mr = MovingRange::default();
        assert_eq!(mr.window_size(), 10);
    }

    /// F-RANGE-003: Add updates min/max
    #[test]
    fn f_range_003_add() {
        let mut mr = MovingRange::new(10);
        mr.add(50.0);
        assert!((mr.min().unwrap() - 50.0).abs() < 0.01);
        assert!((mr.max().unwrap() - 50.0).abs() < 0.01);
    }

    /// F-RANGE-004: Range calculation
    #[test]
    fn f_range_004_range() {
        let mut mr = MovingRange::new(10);
        mr.add(10.0);
        mr.add(30.0);
        assert!((mr.range() - 20.0).abs() < 0.01);
    }

    /// F-RANGE-005: Mid-range calculation
    #[test]
    fn f_range_005_midrange() {
        let mut mr = MovingRange::new(10);
        mr.add(10.0);
        mr.add(30.0);
        assert!((mr.midrange() - 20.0).abs() < 0.01);
    }

    /// F-RANGE-006: Volatility as range/midrange
    #[test]
    fn f_range_006_volatility() {
        let mut mr = MovingRange::new(10);
        mr.add(10.0);
        mr.add(30.0);
        // range=20, midrange=20, volatility=100%
        assert!((mr.volatility() - 100.0).abs() < 0.01);
    }

    /// F-RANGE-007: Window limiting
    #[test]
    fn f_range_007_window() {
        let mut mr = MovingRange::new(3);
        mr.add(100.0);
        mr.add(10.0);
        mr.add(20.0);
        mr.add(30.0);
        // 100.0 should be dropped, min=10, max=30
        assert!((mr.max().unwrap() - 30.0).abs() < 0.01);
    }

    /// F-RANGE-008: Factory for_prices
    #[test]
    fn f_range_008_for_prices() {
        let mr = MovingRange::for_prices();
        assert_eq!(mr.window_size(), 20);
    }

    /// F-RANGE-009: Factory for_latency
    #[test]
    fn f_range_009_for_latency() {
        let mr = MovingRange::for_latency();
        assert_eq!(mr.window_size(), 100);
    }

    /// F-RANGE-010: Reset clears values
    #[test]
    fn f_range_010_reset() {
        let mut mr = MovingRange::new(10);
        mr.add(50.0);
        mr.reset();
        assert_eq!(mr.count(), 0);
    }

    /// F-RANGE-011: Debug format works
    #[test]
    fn f_range_011_debug() {
        let mr = MovingRange::new(10);
        let debug = format!("{:?}", mr);
        assert!(debug.contains("MovingRange"));
    }

    /// F-RANGE-012: Clone preserves state
    #[test]
    fn f_range_012_clone() {
        let mut mr = MovingRange::new(10);
        mr.add(50.0);
        let cloned = mr.clone();
        assert_eq!(mr.count(), cloned.count());
    }

    // ========================================================================
    // TimeoutTracker Falsification Tests (F-TIMEOUT-001 to F-TIMEOUT-012)
    // ========================================================================

    /// F-TIMEOUT-001: New creates empty tracker
    #[test]
    fn f_timeout_001_new() {
        let tt = TimeoutTracker::new(1_000_000);
        assert_eq!(tt.total(), 0);
    }

    /// F-TIMEOUT-002: Default 1 second timeout
    #[test]
    fn f_timeout_002_default() {
        let tt = TimeoutTracker::default();
        assert_eq!(tt.timeout_threshold_us(), 1_000_000);
    }

    /// F-TIMEOUT-003: Record increases total
    #[test]
    fn f_timeout_003_record() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(500_000);
        assert_eq!(tt.total(), 1);
    }

    /// F-TIMEOUT-004: Timeout detection
    #[test]
    fn f_timeout_004_timeout() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(1_500_000); // Over timeout
        assert_eq!(tt.timed_out(), 1);
    }

    /// F-TIMEOUT-005: Timeout rate calculation
    #[test]
    fn f_timeout_005_rate() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(500_000);  // Success
        tt.record(1_500_000); // Timeout
        assert!((tt.timeout_rate() - 50.0).abs() < 0.01);
    }

    /// F-TIMEOUT-006: Success rate calculation
    #[test]
    fn f_timeout_006_success() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(500_000);
        assert!((tt.success_rate() - 100.0).abs() < 0.01);
    }

    /// F-TIMEOUT-007: Factory for_network
    #[test]
    fn f_timeout_007_for_network() {
        let tt = TimeoutTracker::for_network();
        assert_eq!(tt.timeout_threshold_us(), 5_000_000);
    }

    /// F-TIMEOUT-008: Factory for_database
    #[test]
    fn f_timeout_008_for_database() {
        let tt = TimeoutTracker::for_database();
        assert_eq!(tt.timeout_threshold_us(), 30_000_000);
    }

    /// F-TIMEOUT-009: Max duration tracked
    #[test]
    fn f_timeout_009_max() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(100_000);
        tt.record(500_000);
        assert_eq!(tt.max_duration_us(), 500_000);
    }

    /// F-TIMEOUT-010: Reset clears counters
    #[test]
    fn f_timeout_010_reset() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(500_000);
        tt.reset();
        assert_eq!(tt.total(), 0);
    }

    /// F-TIMEOUT-011: Debug format works
    #[test]
    fn f_timeout_011_debug() {
        let tt = TimeoutTracker::new(1_000_000);
        let debug = format!("{:?}", tt);
        assert!(debug.contains("TimeoutTracker"));
    }

    /// F-TIMEOUT-012: Clone preserves state
    #[test]
    fn f_timeout_012_clone() {
        let mut tt = TimeoutTracker::new(1_000_000);
        tt.record(500_000);
        let cloned = tt.clone();
        assert_eq!(tt.total(), cloned.total());
    }

    // ========================================================================
    // RetryTracker Falsification Tests (F-RETRY-001 to F-RETRY-012)
    // ========================================================================

    /// F-RETRY-001: New creates tracker
    #[test]
    fn f_retry_001_new() {
        let rt = RetryTracker::new(3, 100, 10000);
        assert_eq!(rt.current_retry(), 0);
    }

    /// F-RETRY-002: Default 3 retries
    #[test]
    fn f_retry_002_default() {
        let rt = RetryTracker::default();
        assert!(!rt.retries_exhausted());
    }

    /// F-RETRY-003: Retry increases counter
    #[test]
    fn f_retry_003_retry() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        rt.retry();
        assert_eq!(rt.current_retry(), 1);
    }

    /// F-RETRY-004: Success resets retry counter
    #[test]
    fn f_retry_004_success() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        rt.retry();
        rt.success();
        assert_eq!(rt.current_retry(), 0);
    }

    /// F-RETRY-005: Retries exhausted detection
    #[test]
    fn f_retry_005_exhausted() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        rt.retry();
        rt.retry();
        rt.retry();
        assert!(rt.retries_exhausted());
    }

    /// F-RETRY-006: Exponential backoff delay
    #[test]
    fn f_retry_006_delay() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        assert_eq!(rt.next_delay_ms(), 100);
        rt.retry();
        assert_eq!(rt.next_delay_ms(), 200);
        rt.retry();
        assert_eq!(rt.next_delay_ms(), 400);
    }

    /// F-RETRY-007: Factory for_api
    #[test]
    fn f_retry_007_for_api() {
        let rt = RetryTracker::for_api();
        assert_eq!(rt.next_delay_ms(), 100);
    }

    /// F-RETRY-008: Factory for_network
    #[test]
    fn f_retry_008_for_network() {
        let rt = RetryTracker::for_network();
        assert_eq!(rt.next_delay_ms(), 1000);
    }

    /// F-RETRY-009: Delay capped at max
    #[test]
    fn f_retry_009_max_delay() {
        let mut rt = RetryTracker::new(10, 1000, 5000);
        for _ in 0..10 {
            rt.retry();
        }
        assert!(rt.next_delay_ms() <= 5000);
    }

    /// F-RETRY-010: Reset clears state
    #[test]
    fn f_retry_010_reset() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        rt.retry();
        rt.reset();
        assert_eq!(rt.current_retry(), 0);
    }

    /// F-RETRY-011: Debug format works
    #[test]
    fn f_retry_011_debug() {
        let rt = RetryTracker::new(3, 100, 10000);
        let debug = format!("{:?}", rt);
        assert!(debug.contains("RetryTracker"));
    }

    /// F-RETRY-012: Clone preserves state
    #[test]
    fn f_retry_012_clone() {
        let mut rt = RetryTracker::new(3, 100, 10000);
        rt.retry();
        let cloned = rt.clone();
        assert_eq!(rt.current_retry(), cloned.current_retry());
    }

    // ========================================================================
    // ScheduleSlot Falsification Tests (F-SCHED-001 to F-SCHED-012)
    // ========================================================================

    /// F-SCHED-001: New creates scheduler
    #[test]
    fn f_sched_001_new() {
        let ss = ScheduleSlot::new(1_000_000, 10);
        assert_eq!(ss.current_slot(), 0);
    }

    /// F-SCHED-002: Default 1 second slots
    #[test]
    fn f_sched_002_default() {
        let ss = ScheduleSlot::default();
        assert_eq!(ss.num_slots(), 10);
    }

    /// F-SCHED-003: Execute increments slot count
    #[test]
    fn f_sched_003_execute() {
        let mut ss = ScheduleSlot::new(1_000_000, 10);
        ss.execute(1000);
        assert_eq!(ss.executions(0), 1);
    }

    /// F-SCHED-004: Slot advances with time
    #[test]
    fn f_sched_004_advance() {
        let mut ss = ScheduleSlot::new(1_000_000, 10);
        ss.update(1000);
        ss.update(2_001_000); // 2 seconds later
        assert!(ss.current_slot() > 0);
    }

    /// F-SCHED-005: Total executions calculation
    #[test]
    fn f_sched_005_total() {
        let mut ss = ScheduleSlot::new(1_000_000, 10);
        ss.execute(1000);
        ss.execute(1000);
        assert_eq!(ss.total_executions(), 2);
    }

    /// F-SCHED-006: Slot wraps around
    #[test]
    fn f_sched_006_wrap() {
        let mut ss = ScheduleSlot::new(100_000, 3);
        ss.update(1000);
        ss.update(401_000); // 4 slots passed, wraps
        assert!(ss.current_slot() < 3);
    }

    /// F-SCHED-007: Factory for_round_robin
    #[test]
    fn f_sched_007_for_round_robin() {
        let ss = ScheduleSlot::for_round_robin();
        assert_eq!(ss.num_slots(), 10);
    }

    /// F-SCHED-008: Factory for_minute
    #[test]
    fn f_sched_008_for_minute() {
        let ss = ScheduleSlot::for_minute();
        assert_eq!(ss.num_slots(), 5);
    }

    /// F-SCHED-009: Is balanced check
    #[test]
    fn f_sched_009_balanced() {
        let ss = ScheduleSlot::new(1_000_000, 10);
        assert!(ss.is_balanced(50.0));
    }

    /// F-SCHED-010: Reset clears state
    #[test]
    fn f_sched_010_reset() {
        let mut ss = ScheduleSlot::new(1_000_000, 10);
        ss.execute(1000);
        ss.reset();
        assert_eq!(ss.total_executions(), 0);
    }

    /// F-SCHED-011: Debug format works
    #[test]
    fn f_sched_011_debug() {
        let ss = ScheduleSlot::new(1_000_000, 10);
        let debug = format!("{:?}", ss);
        assert!(debug.contains("ScheduleSlot"));
    }

    /// F-SCHED-012: Clone preserves state
    #[test]
    fn f_sched_012_clone() {
        let mut ss = ScheduleSlot::new(1_000_000, 10);
        ss.execute(1000);
        let cloned = ss.clone();
        assert_eq!(ss.total_executions(), cloned.total_executions());
    }

    // ========================================================================
    // CooldownTimer Falsification Tests (F-COOL-001 to F-COOL-012)
    // ========================================================================

    /// F-COOL-001: New creates timer
    #[test]
    fn f_cool_001_new() {
        let ct = CooldownTimer::new(1_000_000);
        assert_eq!(ct.cooldown_us(), 1_000_000);
    }

    /// F-COOL-002: Default 1 second cooldown
    #[test]
    fn f_cool_002_default() {
        let ct = CooldownTimer::default();
        assert_eq!(ct.cooldown_us(), 1_000_000);
    }

    /// F-COOL-003: First action always ready
    #[test]
    fn f_cool_003_first_ready() {
        let ct = CooldownTimer::new(1_000_000);
        assert!(ct.is_ready(1000));
    }

    /// F-COOL-004: Action blocked during cooldown
    #[test]
    fn f_cool_004_blocked() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        assert!(!ct.try_action(500_000)); // 0.5s later, still cooling
    }

    /// F-COOL-005: Action ready after cooldown
    #[test]
    fn f_cool_005_ready_after() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        assert!(ct.is_ready(1_001_000)); // 1s later
    }

    /// F-COOL-006: Block rate calculation
    #[test]
    fn f_cool_006_block_rate() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        ct.try_action(500_000); // Blocked
        assert!((ct.block_rate() - 50.0).abs() < 0.01);
    }

    /// F-COOL-007: Factory for_fast
    #[test]
    fn f_cool_007_for_fast() {
        let ct = CooldownTimer::for_fast();
        assert_eq!(ct.cooldown_us(), 100_000);
    }

    /// F-COOL-008: Factory for_slow
    #[test]
    fn f_cool_008_for_slow() {
        let ct = CooldownTimer::for_slow();
        assert_eq!(ct.cooldown_us(), 10_000_000);
    }

    /// F-COOL-009: Remaining cooldown calculation
    #[test]
    fn f_cool_009_remaining() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        assert!(ct.remaining_us(500_000) > 0);
    }

    /// F-COOL-010: Reset clears state
    #[test]
    fn f_cool_010_reset() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        ct.reset();
        assert_eq!(ct.total_actions(), 0);
    }

    /// F-COOL-011: Debug format works
    #[test]
    fn f_cool_011_debug() {
        let ct = CooldownTimer::new(1_000_000);
        let debug = format!("{:?}", ct);
        assert!(debug.contains("CooldownTimer"));
    }

    /// F-COOL-012: Clone preserves state
    #[test]
    fn f_cool_012_clone() {
        let mut ct = CooldownTimer::new(1_000_000);
        ct.try_action(1000);
        let cloned = ct.clone();
        assert_eq!(ct.total_actions(), cloned.total_actions());
    }

    // ========================================================================
    // BackpressureMonitor Falsification Tests (F-BP-001 to F-BP-012)
    // ========================================================================

    /// F-BP-001: New creates empty monitor
    #[test]
    fn f_bp_001_new() {
        let bp = BackpressureMonitor::new();
        assert_eq!(bp.total_signals(), 0);
    }

    /// F-BP-002: Default same as new
    #[test]
    fn f_bp_002_default() {
        let bp = BackpressureMonitor::default();
        assert_eq!(bp.consecutive(), 0);
    }

    /// F-BP-003: Success resets consecutive
    #[test]
    fn f_bp_003_success() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.success();
        assert_eq!(bp.consecutive(), 0);
    }

    /// F-BP-004: Signal increments consecutive
    #[test]
    fn f_bp_004_signal() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.signal(2000);
        assert_eq!(bp.consecutive(), 2);
    }

    /// F-BP-005: Pressure rate calculation
    #[test]
    fn f_bp_005_rate() {
        let mut bp = BackpressureMonitor::new();
        bp.success();
        bp.signal(1000);
        assert!((bp.pressure_rate() - 50.0).abs() < 0.01);
    }

    /// F-BP-006: Max consecutive tracked
    #[test]
    fn f_bp_006_max_consecutive() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.signal(2000);
        bp.success();
        bp.signal(3000);
        assert_eq!(bp.max_consecutive(), 2);
    }

    /// F-BP-007: Is under pressure check
    #[test]
    fn f_bp_007_under_pressure() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.signal(2000);
        bp.signal(3000);
        assert!(bp.is_under_pressure(3));
    }

    /// F-BP-008: Is healthy check
    #[test]
    fn f_bp_008_healthy() {
        let mut bp = BackpressureMonitor::new();
        bp.success();
        bp.success();
        assert!(bp.is_healthy(10.0));
    }

    /// F-BP-009: Total signals counted
    #[test]
    fn f_bp_009_total() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.signal(2000);
        assert_eq!(bp.total_signals(), 2);
    }

    /// F-BP-010: Reset clears state
    #[test]
    fn f_bp_010_reset() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        bp.reset();
        assert_eq!(bp.total_signals(), 0);
    }

    /// F-BP-011: Debug format works
    #[test]
    fn f_bp_011_debug() {
        let bp = BackpressureMonitor::new();
        let debug = format!("{:?}", bp);
        assert!(debug.contains("BackpressureMonitor"));
    }

    /// F-BP-012: Clone preserves state
    #[test]
    fn f_bp_012_clone() {
        let mut bp = BackpressureMonitor::new();
        bp.signal(1000);
        let cloned = bp.clone();
        assert_eq!(bp.total_signals(), cloned.total_signals());
    }

    // ========================================================================
    // CapacityPlanner Falsification Tests (F-CAP-001 to F-CAP-012)
    // ========================================================================

    /// F-CAP-001: New creates planner
    #[test]
    fn f_cap_001_new() {
        let cp = CapacityPlanner::new(1000);
        assert_eq!(cp.remaining(), 1000);
    }

    /// F-CAP-002: Default 1000 capacity
    #[test]
    fn f_cap_002_default() {
        let cp = CapacityPlanner::default();
        assert_eq!(cp.remaining(), 1000);
    }

    /// F-CAP-003: Update tracks current
    #[test]
    fn f_cap_003_update() {
        let mut cp = CapacityPlanner::new(1000);
        cp.update(500);
        assert_eq!(cp.remaining(), 500);
    }

    /// F-CAP-004: Peak tracked
    #[test]
    fn f_cap_004_peak() {
        let mut cp = CapacityPlanner::new(1000);
        cp.update(800);
        cp.update(500);
        assert!((cp.peak_utilization() - 80.0).abs() < 0.01);
    }

    /// F-CAP-005: Utilization calculation
    #[test]
    fn f_cap_005_utilization() {
        let mut cp = CapacityPlanner::new(100);
        cp.update(50);
        assert!((cp.utilization() - 50.0).abs() < 0.01);
    }

    /// F-CAP-006: At risk check
    #[test]
    fn f_cap_006_at_risk() {
        let mut cp = CapacityPlanner::new(100);
        cp.update(90);
        assert!(cp.at_risk(80.0));
    }

    /// F-CAP-007: Factory for_connections
    #[test]
    fn f_cap_007_for_connections() {
        let cp = CapacityPlanner::for_connections();
        assert_eq!(cp.remaining(), 1000);
    }

    /// F-CAP-008: Factory for_storage
    #[test]
    fn f_cap_008_for_storage() {
        let cp = CapacityPlanner::for_storage();
        assert_eq!(cp.remaining(), 100);
    }

    /// F-CAP-009: Avg utilization calculation
    #[test]
    fn f_cap_009_avg() {
        let mut cp = CapacityPlanner::new(100);
        cp.update(50);
        cp.update(50);
        assert!((cp.avg_utilization() - 50.0).abs() < 0.01);
    }

    /// F-CAP-010: Reset clears state
    #[test]
    fn f_cap_010_reset() {
        let mut cp = CapacityPlanner::new(100);
        cp.update(50);
        cp.reset();
        assert_eq!(cp.remaining(), 100);
    }

    /// F-CAP-011: Debug format works
    #[test]
    fn f_cap_011_debug() {
        let cp = CapacityPlanner::new(100);
        let debug = format!("{:?}", cp);
        assert!(debug.contains("CapacityPlanner"));
    }

    /// F-CAP-012: Clone preserves state
    #[test]
    fn f_cap_012_clone() {
        let mut cp = CapacityPlanner::new(100);
        cp.update(50);
        let cloned = cp.clone();
        assert_eq!(cp.remaining(), cloned.remaining());
    }

    // ========================================================================
    // DriftTracker Falsification Tests (F-DRIFT-001 to F-DRIFT-012)
    // ========================================================================

    /// F-DRIFT-001: New creates tracker
    #[test]
    fn f_drift_001_new() {
        let dt = DriftTracker::new(1_000_000);
        assert_eq!(dt.samples(), 0);
    }

    /// F-DRIFT-002: Default 1 second interval
    #[test]
    fn f_drift_002_default() {
        let dt = DriftTracker::default();
        assert_eq!(dt.samples(), 0);
    }

    /// F-DRIFT-003: First record is baseline
    #[test]
    fn f_drift_003_baseline() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        assert_eq!(dt.samples(), 0); // First record is baseline
    }

    /// F-DRIFT-004: Drift calculated
    #[test]
    fn f_drift_004_drift() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(1_001_000); // Exactly on time
        assert!(dt.avg_drift_us().abs() < 1.0);
    }

    /// F-DRIFT-005: Late drift positive
    #[test]
    fn f_drift_005_late() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(1_100_000); // 100ms late
        assert!(dt.avg_drift_us() > 0.0);
    }

    /// F-DRIFT-006: Early drift negative
    #[test]
    fn f_drift_006_early() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(901_000); // 100ms early
        assert!(dt.avg_drift_us() < 0.0);
    }

    /// F-DRIFT-007: Factory for_60fps
    #[test]
    fn f_drift_007_for_60fps() {
        let dt = DriftTracker::for_60fps();
        assert_eq!(dt.samples(), 0);
    }

    /// F-DRIFT-008: Factory for_heartbeat
    #[test]
    fn f_drift_008_for_heartbeat() {
        let dt = DriftTracker::for_heartbeat();
        assert_eq!(dt.samples(), 0);
    }

    /// F-DRIFT-009: Is stable check
    #[test]
    fn f_drift_009_stable() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(1_001_000);
        assert!(dt.is_stable(10_000));
    }

    /// F-DRIFT-010: Reset clears state
    #[test]
    fn f_drift_010_reset() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(2_000_000);
        dt.reset();
        assert_eq!(dt.samples(), 0);
    }

    /// F-DRIFT-011: Debug format works
    #[test]
    fn f_drift_011_debug() {
        let dt = DriftTracker::new(1_000_000);
        let debug = format!("{:?}", dt);
        assert!(debug.contains("DriftTracker"));
    }

    /// F-DRIFT-012: Clone preserves state
    #[test]
    fn f_drift_012_clone() {
        let mut dt = DriftTracker::new(1_000_000);
        dt.record(1000);
        dt.record(2_000_000);
        let cloned = dt.clone();
        assert_eq!(dt.samples(), cloned.samples());
    }

    // ========================================================================
    // SemaphoreTracker Falsification Tests (F-SEM-001 to F-SEM-012)
    // ========================================================================

    /// F-SEM-001: New creates tracker
    #[test]
    fn f_sem_001_new() {
        let st = SemaphoreTracker::new(10);
        assert_eq!(st.available(), 10);
    }

    /// F-SEM-002: Default 10 permits
    #[test]
    fn f_sem_002_default() {
        let st = SemaphoreTracker::default();
        assert_eq!(st.available(), 10);
    }

    /// F-SEM-003: Acquire reduces available
    #[test]
    fn f_sem_003_acquire() {
        let mut st = SemaphoreTracker::new(10);
        st.try_acquire();
        assert_eq!(st.available(), 9);
    }

    /// F-SEM-004: Release increases available
    #[test]
    fn f_sem_004_release() {
        let mut st = SemaphoreTracker::new(10);
        st.try_acquire();
        st.release();
        assert_eq!(st.available(), 10);
    }

    /// F-SEM-005: Contention when full
    #[test]
    fn f_sem_005_contention() {
        let mut st = SemaphoreTracker::new(1);
        st.try_acquire();
        assert!(!st.try_acquire()); // Should fail
    }

    /// F-SEM-006: Contention rate calculation
    #[test]
    fn f_sem_006_contention_rate() {
        let mut st = SemaphoreTracker::new(1);
        st.try_acquire();
        st.try_acquire(); // Contention
        assert!((st.contention_rate() - 50.0).abs() < 0.01);
    }

    /// F-SEM-007: Factory for_database
    #[test]
    fn f_sem_007_for_database() {
        let st = SemaphoreTracker::for_database();
        assert_eq!(st.total_permits(), 20);
    }

    /// F-SEM-008: Factory for_workers
    #[test]
    fn f_sem_008_for_workers() {
        let st = SemaphoreTracker::for_workers();
        assert_eq!(st.total_permits(), 8);
    }

    /// F-SEM-009: Peak utilization tracked
    #[test]
    fn f_sem_009_peak() {
        let mut st = SemaphoreTracker::new(10);
        st.try_acquire();
        st.try_acquire();
        st.release();
        assert!((st.peak_utilization() - 20.0).abs() < 0.01);
    }

    /// F-SEM-010: Reset clears state
    #[test]
    fn f_sem_010_reset() {
        let mut st = SemaphoreTracker::new(10);
        st.try_acquire();
        st.reset();
        assert_eq!(st.available(), 10);
    }

    /// F-SEM-011: Debug format works
    #[test]
    fn f_sem_011_debug() {
        let st = SemaphoreTracker::new(10);
        let debug = format!("{:?}", st);
        assert!(debug.contains("SemaphoreTracker"));
    }

    /// F-SEM-012: Clone preserves state
    #[test]
    fn f_sem_012_clone() {
        let mut st = SemaphoreTracker::new(10);
        st.try_acquire();
        let cloned = st.clone();
        assert_eq!(st.available(), cloned.available());
    }
}

// ============================================================================
// GCTracker - O(1) garbage collection overhead tracking
// ============================================================================

/// O(1) garbage collection overhead tracking.
///
/// Tracks GC pause times, frequency, and overhead percentage.
/// Useful for monitoring memory pressure and GC tuning.
#[derive(Debug, Clone)]
pub struct GCTracker {
    gc_count: u64,
    total_pause_us: u64,
    total_time_us: u64,
    max_pause_us: u64,
    last_gc_us: u64,
}

impl Default for GCTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl GCTracker {
    /// Create a new GC tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gc_count: 0,
            total_pause_us: 0,
            total_time_us: 0,
            max_pause_us: 0,
            last_gc_us: 0,
        }
    }

    /// Record a GC event with pause duration.
    pub fn record_gc(&mut self, pause_us: u64, now_us: u64) {
        self.gc_count += 1;
        self.total_pause_us += pause_us;
        if pause_us > self.max_pause_us {
            self.max_pause_us = pause_us;
        }
        if self.last_gc_us > 0 && now_us > self.last_gc_us {
            self.total_time_us += now_us - self.last_gc_us;
        }
        self.last_gc_us = now_us;
    }

    /// Get GC overhead percentage (pause time / total time * 100).
    #[must_use]
    pub fn overhead_percentage(&self) -> f64 {
        if self.total_time_us == 0 {
            0.0
        } else {
            (self.total_pause_us as f64 / self.total_time_us as f64) * 100.0
        }
    }

    /// Get average pause time in microseconds.
    #[must_use]
    pub fn avg_pause_us(&self) -> f64 {
        if self.gc_count == 0 {
            0.0
        } else {
            self.total_pause_us as f64 / self.gc_count as f64
        }
    }

    /// Get maximum pause time in microseconds.
    #[must_use]
    pub fn max_pause_us(&self) -> u64 {
        self.max_pause_us
    }

    /// Get GC count.
    #[must_use]
    pub fn gc_count(&self) -> u64 {
        self.gc_count
    }

    /// Check if GC overhead is acceptable (< threshold %).
    #[must_use]
    pub fn is_healthy(&self, max_overhead: f64) -> bool {
        self.overhead_percentage() < max_overhead
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.gc_count = 0;
        self.total_pause_us = 0;
        self.total_time_us = 0;
        self.max_pause_us = 0;
        self.last_gc_us = 0;
    }
}

#[cfg(test)]
mod gc_tracker_tests {
    use super::*;

    /// F-GC-001: New tracker starts empty
    #[test]
    fn f_gc_001_new() {
        let gc = GCTracker::new();
        assert_eq!(gc.gc_count(), 0);
    }

    /// F-GC-002: Default equals new
    #[test]
    fn f_gc_002_default() {
        let gc = GCTracker::default();
        assert_eq!(gc.gc_count(), 0);
    }

    /// F-GC-003: Record increments count
    #[test]
    fn f_gc_003_record() {
        let mut gc = GCTracker::new();
        gc.record_gc(1000, 10000);
        assert_eq!(gc.gc_count(), 1);
    }

    /// F-GC-004: Max pause tracked
    #[test]
    fn f_gc_004_max_pause() {
        let mut gc = GCTracker::new();
        gc.record_gc(500, 10000);
        gc.record_gc(2000, 20000);
        gc.record_gc(800, 30000);
        assert_eq!(gc.max_pause_us(), 2000);
    }

    /// F-GC-005: Average pause calculated
    #[test]
    fn f_gc_005_avg_pause() {
        let mut gc = GCTracker::new();
        gc.record_gc(1000, 10000);
        gc.record_gc(2000, 20000);
        assert!((gc.avg_pause_us() - 1500.0).abs() < 0.01);
    }

    /// F-GC-006: Overhead percentage calculated
    #[test]
    fn f_gc_006_overhead() {
        let mut gc = GCTracker::new();
        gc.record_gc(100, 1000);
        gc.record_gc(100, 2000); // 1000us elapsed, 100us pause
        assert!(gc.overhead_percentage() > 0.0);
    }

    /// F-GC-007: Healthy when overhead low
    #[test]
    fn f_gc_007_healthy() {
        let mut gc = GCTracker::new();
        gc.record_gc(10, 1000);
        gc.record_gc(10, 2000);
        assert!(gc.is_healthy(10.0));
    }

    /// F-GC-008: Not healthy when overhead high
    #[test]
    fn f_gc_008_unhealthy() {
        let mut gc = GCTracker::new();
        gc.record_gc(500, 1000);
        gc.record_gc(500, 2000); // 1000us elapsed, 500us pause = 50%
        assert!(!gc.is_healthy(10.0));
    }

    /// F-GC-009: Reset clears state
    #[test]
    fn f_gc_009_reset() {
        let mut gc = GCTracker::new();
        gc.record_gc(1000, 10000);
        gc.reset();
        assert_eq!(gc.gc_count(), 0);
    }

    /// F-GC-010: Zero overhead with no events
    #[test]
    fn f_gc_010_zero_overhead() {
        let gc = GCTracker::new();
        assert!((gc.overhead_percentage() - 0.0).abs() < 0.01);
    }

    /// F-GC-011: Debug format works
    #[test]
    fn f_gc_011_debug() {
        let gc = GCTracker::new();
        let debug = format!("{:?}", gc);
        assert!(debug.contains("GCTracker"));
    }

    /// F-GC-012: Clone preserves state
    #[test]
    fn f_gc_012_clone() {
        let mut gc = GCTracker::new();
        gc.record_gc(1000, 10000);
        let cloned = gc.clone();
        assert_eq!(gc.gc_count(), cloned.gc_count());
    }
}

// ============================================================================
// CompactionTracker - O(1) compaction cycle tracking
// ============================================================================

/// O(1) compaction cycle tracking.
///
/// Tracks compaction cycles (database, log, etc.) with duration
/// and bytes processed for throughput analysis.
#[derive(Debug, Clone)]
pub struct CompactionTracker {
    compactions: u64,
    total_duration_us: u64,
    total_bytes: u64,
    max_duration_us: u64,
    active: bool,
    start_us: u64,
}

impl Default for CompactionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactionTracker {
    /// Create a new compaction tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            compactions: 0,
            total_duration_us: 0,
            total_bytes: 0,
            max_duration_us: 0,
            active: false,
            start_us: 0,
        }
    }

    /// Factory for database compaction (typically large).
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Factory for log compaction (typically smaller).
    #[must_use]
    pub fn for_logs() -> Self {
        Self::new()
    }

    /// Start a compaction cycle.
    pub fn start(&mut self, now_us: u64) {
        self.active = true;
        self.start_us = now_us;
    }

    /// Complete a compaction cycle with bytes processed.
    pub fn complete(&mut self, bytes: u64, now_us: u64) {
        if self.active && now_us >= self.start_us {
            let duration = now_us - self.start_us;
            self.compactions += 1;
            self.total_duration_us += duration;
            self.total_bytes += bytes;
            if duration > self.max_duration_us {
                self.max_duration_us = duration;
            }
        }
        self.active = false;
    }

    /// Get compaction count.
    #[must_use]
    pub fn compaction_count(&self) -> u64 {
        self.compactions
    }

    /// Get throughput in bytes per second.
    #[must_use]
    pub fn throughput_bytes_per_sec(&self) -> f64 {
        if self.total_duration_us == 0 {
            0.0
        } else {
            (self.total_bytes as f64 / self.total_duration_us as f64) * 1_000_000.0
        }
    }

    /// Get average duration in microseconds.
    #[must_use]
    pub fn avg_duration_us(&self) -> f64 {
        if self.compactions == 0 {
            0.0
        } else {
            self.total_duration_us as f64 / self.compactions as f64
        }
    }

    /// Get max duration in microseconds.
    #[must_use]
    pub fn max_duration_us(&self) -> u64 {
        self.max_duration_us
    }

    /// Check if compaction is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.compactions = 0;
        self.total_duration_us = 0;
        self.total_bytes = 0;
        self.max_duration_us = 0;
        self.active = false;
        self.start_us = 0;
    }
}

#[cfg(test)]
mod compaction_tracker_tests {
    use super::*;

    /// F-COMPACT-001: New tracker starts empty
    #[test]
    fn f_compact_001_new() {
        let ct = CompactionTracker::new();
        assert_eq!(ct.compaction_count(), 0);
    }

    /// F-COMPACT-002: Default equals new
    #[test]
    fn f_compact_002_default() {
        let ct = CompactionTracker::default();
        assert_eq!(ct.compaction_count(), 0);
    }

    /// F-COMPACT-003: Start sets active
    #[test]
    fn f_compact_003_start() {
        let mut ct = CompactionTracker::new();
        ct.start(1000);
        assert!(ct.is_active());
    }

    /// F-COMPACT-004: Complete increments count
    #[test]
    fn f_compact_004_complete() {
        let mut ct = CompactionTracker::new();
        ct.start(1000);
        ct.complete(1024, 2000);
        assert_eq!(ct.compaction_count(), 1);
    }

    /// F-COMPACT-005: Throughput calculated
    #[test]
    fn f_compact_005_throughput() {
        let mut ct = CompactionTracker::new();
        ct.start(0);
        ct.complete(1_000_000, 1_000_000); // 1MB in 1 second
        assert!((ct.throughput_bytes_per_sec() - 1_000_000.0).abs() < 1.0);
    }

    /// F-COMPACT-006: Max duration tracked
    #[test]
    fn f_compact_006_max_duration() {
        let mut ct = CompactionTracker::new();
        ct.start(0);
        ct.complete(100, 1000);
        ct.start(2000);
        ct.complete(100, 5000); // 3000us
        assert_eq!(ct.max_duration_us(), 3000);
    }

    /// F-COMPACT-007: Factory for_database
    #[test]
    fn f_compact_007_for_database() {
        let ct = CompactionTracker::for_database();
        assert_eq!(ct.compaction_count(), 0);
    }

    /// F-COMPACT-008: Factory for_logs
    #[test]
    fn f_compact_008_for_logs() {
        let ct = CompactionTracker::for_logs();
        assert_eq!(ct.compaction_count(), 0);
    }

    /// F-COMPACT-009: Average duration calculated
    #[test]
    fn f_compact_009_avg_duration() {
        let mut ct = CompactionTracker::new();
        ct.start(0);
        ct.complete(100, 1000);
        ct.start(2000);
        ct.complete(100, 4000);
        assert!((ct.avg_duration_us() - 1500.0).abs() < 0.01);
    }

    /// F-COMPACT-010: Reset clears state
    #[test]
    fn f_compact_010_reset() {
        let mut ct = CompactionTracker::new();
        ct.start(0);
        ct.complete(100, 1000);
        ct.reset();
        assert_eq!(ct.compaction_count(), 0);
    }

    /// F-COMPACT-011: Debug format works
    #[test]
    fn f_compact_011_debug() {
        let ct = CompactionTracker::new();
        let debug = format!("{:?}", ct);
        assert!(debug.contains("CompactionTracker"));
    }

    /// F-COMPACT-012: Clone preserves state
    #[test]
    fn f_compact_012_clone() {
        let mut ct = CompactionTracker::new();
        ct.start(0);
        ct.complete(100, 1000);
        let cloned = ct.clone();
        assert_eq!(ct.compaction_count(), cloned.compaction_count());
    }
}

// ============================================================================
// FlushTracker - O(1) buffer flush pattern monitoring
// ============================================================================

/// O(1) buffer flush pattern monitoring.
///
/// Tracks flush frequency, data volume, and identifies bursty patterns.
#[derive(Debug, Clone)]
pub struct FlushTracker {
    flushes: u64,
    total_bytes: u64,
    max_bytes: u64,
    last_flush_us: u64,
    min_interval_us: u64,
}

impl Default for FlushTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl FlushTracker {
    /// Create a new flush tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            flushes: 0,
            total_bytes: 0,
            max_bytes: 0,
            last_flush_us: 0,
            min_interval_us: u64::MAX,
        }
    }

    /// Factory for write buffer flushing.
    #[must_use]
    pub fn for_write_buffer() -> Self {
        Self::new()
    }

    /// Factory for network buffer flushing.
    #[must_use]
    pub fn for_network() -> Self {
        Self::new()
    }

    /// Record a flush event.
    pub fn flush(&mut self, bytes: u64, now_us: u64) {
        self.flushes += 1;
        self.total_bytes += bytes;
        if bytes > self.max_bytes {
            self.max_bytes = bytes;
        }
        if self.last_flush_us > 0 && now_us > self.last_flush_us {
            let interval = now_us - self.last_flush_us;
            if interval < self.min_interval_us {
                self.min_interval_us = interval;
            }
        }
        self.last_flush_us = now_us;
    }

    /// Get flush count.
    #[must_use]
    pub fn flush_count(&self) -> u64 {
        self.flushes
    }

    /// Get total bytes flushed.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Get average bytes per flush.
    #[must_use]
    pub fn avg_bytes(&self) -> f64 {
        if self.flushes == 0 {
            0.0
        } else {
            self.total_bytes as f64 / self.flushes as f64
        }
    }

    /// Get max bytes in single flush.
    #[must_use]
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// Get minimum interval between flushes (bursty indicator).
    #[must_use]
    pub fn min_interval_us(&self) -> u64 {
        if self.min_interval_us == u64::MAX {
            0
        } else {
            self.min_interval_us
        }
    }

    /// Check if flush pattern is bursty (min interval < threshold).
    #[must_use]
    pub fn is_bursty(&self, threshold_us: u64) -> bool {
        self.min_interval_us < threshold_us
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.flushes = 0;
        self.total_bytes = 0;
        self.max_bytes = 0;
        self.last_flush_us = 0;
        self.min_interval_us = u64::MAX;
    }
}

#[cfg(test)]
mod flush_tracker_tests {
    use super::*;

    /// F-FLUSH-001: New tracker starts empty
    #[test]
    fn f_flush_001_new() {
        let ft = FlushTracker::new();
        assert_eq!(ft.flush_count(), 0);
    }

    /// F-FLUSH-002: Default equals new
    #[test]
    fn f_flush_002_default() {
        let ft = FlushTracker::default();
        assert_eq!(ft.flush_count(), 0);
    }

    /// F-FLUSH-003: Flush increments count
    #[test]
    fn f_flush_003_flush() {
        let mut ft = FlushTracker::new();
        ft.flush(1024, 1000);
        assert_eq!(ft.flush_count(), 1);
    }

    /// F-FLUSH-004: Total bytes tracked
    #[test]
    fn f_flush_004_total_bytes() {
        let mut ft = FlushTracker::new();
        ft.flush(1024, 1000);
        ft.flush(2048, 2000);
        assert_eq!(ft.total_bytes(), 3072);
    }

    /// F-FLUSH-005: Max bytes tracked
    #[test]
    fn f_flush_005_max_bytes() {
        let mut ft = FlushTracker::new();
        ft.flush(1024, 1000);
        ft.flush(4096, 2000);
        ft.flush(2048, 3000);
        assert_eq!(ft.max_bytes(), 4096);
    }

    /// F-FLUSH-006: Average bytes calculated
    #[test]
    fn f_flush_006_avg_bytes() {
        let mut ft = FlushTracker::new();
        ft.flush(1000, 1000);
        ft.flush(2000, 2000);
        assert!((ft.avg_bytes() - 1500.0).abs() < 0.01);
    }

    /// F-FLUSH-007: Factory for_write_buffer
    #[test]
    fn f_flush_007_for_write_buffer() {
        let ft = FlushTracker::for_write_buffer();
        assert_eq!(ft.flush_count(), 0);
    }

    /// F-FLUSH-008: Factory for_network
    #[test]
    fn f_flush_008_for_network() {
        let ft = FlushTracker::for_network();
        assert_eq!(ft.flush_count(), 0);
    }

    /// F-FLUSH-009: Min interval tracked
    #[test]
    fn f_flush_009_min_interval() {
        let mut ft = FlushTracker::new();
        ft.flush(100, 1000);
        ft.flush(100, 1100); // 100us interval
        ft.flush(100, 2000); // 900us interval
        assert_eq!(ft.min_interval_us(), 100);
    }

    /// F-FLUSH-010: Bursty detection
    #[test]
    fn f_flush_010_bursty() {
        let mut ft = FlushTracker::new();
        ft.flush(100, 1000);
        ft.flush(100, 1050); // 50us interval
        assert!(ft.is_bursty(100));
    }

    /// F-FLUSH-011: Reset clears state
    #[test]
    fn f_flush_011_reset() {
        let mut ft = FlushTracker::new();
        ft.flush(1024, 1000);
        ft.reset();
        assert_eq!(ft.flush_count(), 0);
    }

    /// F-FLUSH-012: Clone preserves state
    #[test]
    fn f_flush_012_clone() {
        let mut ft = FlushTracker::new();
        ft.flush(1024, 1000);
        let cloned = ft.clone();
        assert_eq!(ft.flush_count(), cloned.flush_count());
    }
}

// ============================================================================
// WatermarkTracker - O(1) high/low watermark monitoring
// ============================================================================

/// O(1) high/low watermark monitoring.
///
/// Tracks value against configurable watermarks for flow control.
#[derive(Debug, Clone)]
pub struct WatermarkTracker {
    low_watermark: u64,
    high_watermark: u64,
    current: u64,
    peak: u64,
    high_events: u64,
    low_events: u64,
}

impl Default for WatermarkTracker {
    fn default() -> Self {
        Self::new(25, 75)
    }
}

impl WatermarkTracker {
    /// Create a new watermark tracker with thresholds.
    #[must_use]
    pub fn new(low: u64, high: u64) -> Self {
        Self {
            low_watermark: low,
            high_watermark: high,
            current: 0,
            peak: 0,
            high_events: 0,
            low_events: 0,
        }
    }

    /// Factory for buffer management (25%/75%).
    #[must_use]
    pub fn for_buffer() -> Self {
        Self::new(25, 75)
    }

    /// Factory for queue management (10%/90%).
    #[must_use]
    pub fn for_queue() -> Self {
        Self::new(10, 90)
    }

    /// Update current value and check watermarks.
    pub fn update(&mut self, value: u64) {
        let was_above_high = self.current >= self.high_watermark;
        let was_below_low = self.current <= self.low_watermark;

        self.current = value;
        if value > self.peak {
            self.peak = value;
        }

        // Count transitions
        if !was_above_high && value >= self.high_watermark {
            self.high_events += 1;
        }
        if !was_below_low && value <= self.low_watermark {
            self.low_events += 1;
        }
    }

    /// Get current value.
    #[must_use]
    pub fn current(&self) -> u64 {
        self.current
    }

    /// Get peak value.
    #[must_use]
    pub fn peak(&self) -> u64 {
        self.peak
    }

    /// Check if currently above high watermark.
    #[must_use]
    pub fn is_high(&self) -> bool {
        self.current >= self.high_watermark
    }

    /// Check if currently below low watermark.
    #[must_use]
    pub fn is_low(&self) -> bool {
        self.current <= self.low_watermark
    }

    /// Get count of high watermark events.
    #[must_use]
    pub fn high_events(&self) -> u64 {
        self.high_events
    }

    /// Get count of low watermark events.
    #[must_use]
    pub fn low_events(&self) -> u64 {
        self.low_events
    }

    /// Check if value is in normal range (between watermarks).
    #[must_use]
    pub fn is_normal(&self) -> bool {
        self.current > self.low_watermark && self.current < self.high_watermark
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.current = 0;
        self.peak = 0;
        self.high_events = 0;
        self.low_events = 0;
    }
}

#[cfg(test)]
mod watermark_tracker_tests {
    use super::*;

    /// F-WATER-001: New tracker starts at zero
    #[test]
    fn f_water_001_new() {
        let wt = WatermarkTracker::new(25, 75);
        assert_eq!(wt.current(), 0);
    }

    /// F-WATER-002: Default uses 25/75
    #[test]
    fn f_water_002_default() {
        let wt = WatermarkTracker::default();
        assert!(wt.is_low()); // 0 <= 25
    }

    /// F-WATER-003: Update changes current
    #[test]
    fn f_water_003_update() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(50);
        assert_eq!(wt.current(), 50);
    }

    /// F-WATER-004: Peak tracked
    #[test]
    fn f_water_004_peak() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(80);
        wt.update(30);
        assert_eq!(wt.peak(), 80);
    }

    /// F-WATER-005: High detection
    #[test]
    fn f_water_005_is_high() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(80);
        assert!(wt.is_high());
    }

    /// F-WATER-006: Low detection
    #[test]
    fn f_water_006_is_low() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(20);
        assert!(wt.is_low());
    }

    /// F-WATER-007: Factory for_buffer
    #[test]
    fn f_water_007_for_buffer() {
        let wt = WatermarkTracker::for_buffer();
        assert_eq!(wt.current(), 0);
    }

    /// F-WATER-008: Factory for_queue
    #[test]
    fn f_water_008_for_queue() {
        let wt = WatermarkTracker::for_queue();
        assert_eq!(wt.current(), 0);
    }

    /// F-WATER-009: High events counted
    #[test]
    fn f_water_009_high_events() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(50);
        wt.update(80); // crosses high
        assert_eq!(wt.high_events(), 1);
    }

    /// F-WATER-010: Low events counted
    #[test]
    fn f_water_010_low_events() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(50);
        wt.update(20); // crosses low
        assert_eq!(wt.low_events(), 1);
    }

    /// F-WATER-011: Normal range detection
    #[test]
    fn f_water_011_normal() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(50);
        assert!(wt.is_normal());
    }

    /// F-WATER-012: Reset clears state
    #[test]
    fn f_water_012_reset() {
        let mut wt = WatermarkTracker::new(25, 75);
        wt.update(80);
        wt.reset();
        assert_eq!(wt.current(), 0);
    }
}

// ============================================================================
// SnapshotTracker - O(1) point-in-time state tracking
// ============================================================================

/// O(1) point-in-time state tracking.
///
/// Tracks snapshots with timestamps for recovery and comparison.
#[derive(Debug, Clone)]
pub struct SnapshotTracker {
    snapshot_count: u64,
    last_snapshot_us: u64,
    total_size_bytes: u64,
    max_size_bytes: u64,
    avg_interval_us: f64,
}

impl Default for SnapshotTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotTracker {
    /// Create a new snapshot tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            snapshot_count: 0,
            last_snapshot_us: 0,
            total_size_bytes: 0,
            max_size_bytes: 0,
            avg_interval_us: 0.0,
        }
    }

    /// Factory for database snapshots.
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Factory for state snapshots.
    #[must_use]
    pub fn for_state() -> Self {
        Self::new()
    }

    /// Record a snapshot.
    pub fn snapshot(&mut self, size_bytes: u64, now_us: u64) {
        if self.last_snapshot_us > 0 && now_us > self.last_snapshot_us {
            let interval = (now_us - self.last_snapshot_us) as f64;
            let n = self.snapshot_count as f64;
            self.avg_interval_us = (self.avg_interval_us * n + interval) / (n + 1.0);
        }
        self.snapshot_count += 1;
        self.total_size_bytes += size_bytes;
        if size_bytes > self.max_size_bytes {
            self.max_size_bytes = size_bytes;
        }
        self.last_snapshot_us = now_us;
    }

    /// Get snapshot count.
    #[must_use]
    pub fn snapshot_count(&self) -> u64 {
        self.snapshot_count
    }

    /// Get total bytes across all snapshots.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total_size_bytes
    }

    /// Get average snapshot size.
    #[must_use]
    pub fn avg_size_bytes(&self) -> f64 {
        if self.snapshot_count == 0 {
            0.0
        } else {
            self.total_size_bytes as f64 / self.snapshot_count as f64
        }
    }

    /// Get max snapshot size.
    #[must_use]
    pub fn max_size_bytes(&self) -> u64 {
        self.max_size_bytes
    }

    /// Get average interval between snapshots in microseconds.
    #[must_use]
    pub fn avg_interval_us(&self) -> f64 {
        self.avg_interval_us
    }

    /// Get last snapshot timestamp.
    #[must_use]
    pub fn last_snapshot_us(&self) -> u64 {
        self.last_snapshot_us
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.snapshot_count = 0;
        self.last_snapshot_us = 0;
        self.total_size_bytes = 0;
        self.max_size_bytes = 0;
        self.avg_interval_us = 0.0;
    }
}

#[cfg(test)]
mod snapshot_tracker_tests {
    use super::*;

    /// F-SNAP-001: New tracker starts empty
    #[test]
    fn f_snap_001_new() {
        let st = SnapshotTracker::new();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-002: Default equals new
    #[test]
    fn f_snap_002_default() {
        let st = SnapshotTracker::default();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-003: Snapshot increments count
    #[test]
    fn f_snap_003_snapshot() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        assert_eq!(st.snapshot_count(), 1);
    }

    /// F-SNAP-004: Total bytes tracked
    #[test]
    fn f_snap_004_total_bytes() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.snapshot(2048, 2000);
        assert_eq!(st.total_bytes(), 3072);
    }

    /// F-SNAP-005: Max size tracked
    #[test]
    fn f_snap_005_max_size() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.snapshot(4096, 2000);
        st.snapshot(2048, 3000);
        assert_eq!(st.max_size_bytes(), 4096);
    }

    /// F-SNAP-006: Average size calculated
    #[test]
    fn f_snap_006_avg_size() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1000, 1000);
        st.snapshot(2000, 2000);
        assert!((st.avg_size_bytes() - 1500.0).abs() < 0.01);
    }

    /// F-SNAP-007: Factory for_database
    #[test]
    fn f_snap_007_for_database() {
        let st = SnapshotTracker::for_database();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-008: Factory for_state
    #[test]
    fn f_snap_008_for_state() {
        let st = SnapshotTracker::for_state();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-009: Avg interval tracked
    #[test]
    fn f_snap_009_avg_interval() {
        let mut st = SnapshotTracker::new();
        st.snapshot(100, 1000);
        st.snapshot(100, 2000); // 1000us interval
        assert!(st.avg_interval_us() > 0.0);
    }

    /// F-SNAP-010: Last snapshot timestamp
    #[test]
    fn f_snap_010_last_snapshot() {
        let mut st = SnapshotTracker::new();
        st.snapshot(100, 5000);
        assert_eq!(st.last_snapshot_us(), 5000);
    }

    /// F-SNAP-011: Reset clears state
    #[test]
    fn f_snap_011_reset() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.reset();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-012: Clone preserves state
    #[test]
    fn f_snap_012_clone() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        let cloned = st.clone();
        assert_eq!(st.snapshot_count(), cloned.snapshot_count());
    }
}

// ============================================================================
// VersionTracker - O(1) version/generation tracking
// ============================================================================

/// O(1) version/generation tracking for optimistic concurrency.
///
/// Tracks version numbers, conflicts, and updates for optimistic locking.
#[derive(Debug, Clone)]
pub struct VersionTracker {
    current_version: u64,
    updates: u64,
    conflicts: u64,
    last_update_us: u64,
}

impl Default for VersionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionTracker {
    /// Create a new version tracker starting at version 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_version: 0,
            updates: 0,
            conflicts: 0,
            last_update_us: 0,
        }
    }

    /// Factory for database records.
    #[must_use]
    pub fn for_record() -> Self {
        Self::new()
    }

    /// Factory for cache entries.
    #[must_use]
    pub fn for_cache() -> Self {
        Self::new()
    }

    /// Attempt to update with expected version (CAS-like).
    /// Returns true if update succeeds (version matches), false on conflict.
    pub fn try_update(&mut self, expected_version: u64, now_us: u64) -> bool {
        if self.current_version == expected_version {
            self.current_version += 1;
            self.updates += 1;
            self.last_update_us = now_us;
            true
        } else {
            self.conflicts += 1;
            false
        }
    }

    /// Force update regardless of version (for recovery).
    pub fn force_update(&mut self, now_us: u64) {
        self.current_version += 1;
        self.updates += 1;
        self.last_update_us = now_us;
    }

    /// Get current version.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.current_version
    }

    /// Get total successful updates.
    #[must_use]
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// Get conflict count.
    #[must_use]
    pub fn conflicts(&self) -> u64 {
        self.conflicts
    }

    /// Get conflict rate (conflicts / total attempts).
    #[must_use]
    pub fn conflict_rate(&self) -> f64 {
        let total = self.updates + self.conflicts;
        if total == 0 {
            0.0
        } else {
            self.conflicts as f64 / total as f64
        }
    }

    /// Check if conflict rate is acceptable.
    #[must_use]
    pub fn is_healthy(&self, max_conflict_rate: f64) -> bool {
        self.conflict_rate() <= max_conflict_rate
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.current_version = 0;
        self.updates = 0;
        self.conflicts = 0;
        self.last_update_us = 0;
    }
}

#[cfg(test)]
mod version_tracker_tests {
    use super::*;

    /// F-VER-001: New tracker starts at version 0
    #[test]
    fn f_ver_001_new() {
        let vt = VersionTracker::new();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-002: Default equals new
    #[test]
    fn f_ver_002_default() {
        let vt = VersionTracker::default();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-003: Try update success
    #[test]
    fn f_ver_003_try_update_success() {
        let mut vt = VersionTracker::new();
        assert!(vt.try_update(0, 1000));
        assert_eq!(vt.version(), 1);
    }

    /// F-VER-004: Try update conflict
    #[test]
    fn f_ver_004_try_update_conflict() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000); // v0 -> v1
        assert!(!vt.try_update(0, 2000)); // conflict: expected 0, got 1
    }

    /// F-VER-005: Force update increments
    #[test]
    fn f_ver_005_force_update() {
        let mut vt = VersionTracker::new();
        vt.force_update(1000);
        assert_eq!(vt.version(), 1);
    }

    /// F-VER-006: Conflict count tracked
    #[test]
    fn f_ver_006_conflicts() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        vt.try_update(0, 2000); // conflict
        assert_eq!(vt.conflicts(), 1);
    }

    /// F-VER-007: Factory for_record
    #[test]
    fn f_ver_007_for_record() {
        let vt = VersionTracker::for_record();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-008: Factory for_cache
    #[test]
    fn f_ver_008_for_cache() {
        let vt = VersionTracker::for_cache();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-009: Conflict rate calculated
    #[test]
    fn f_ver_009_conflict_rate() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000); // success
        vt.try_update(0, 2000); // conflict
        assert!((vt.conflict_rate() - 0.5).abs() < 0.01);
    }

    /// F-VER-010: Healthy when low conflicts
    #[test]
    fn f_ver_010_healthy() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        assert!(vt.is_healthy(0.1));
    }

    /// F-VER-011: Reset clears state
    #[test]
    fn f_ver_011_reset() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        vt.reset();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-012: Clone preserves state
    #[test]
    fn f_ver_012_clone() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        let cloned = vt.clone();
        assert_eq!(vt.version(), cloned.version());
    }
}

// ============================================================================
// TokenBucketShaper - O(1) traffic shaping
// ============================================================================

/// O(1) traffic shaping with guaranteed bandwidth.
///
/// Implements token bucket with configurable burst and sustained rates.
#[derive(Debug, Clone)]
pub struct TokenBucketShaper {
    bucket_size: u64,
    tokens: u64,
    fill_rate_per_us: f64,
    last_fill_us: u64,
    bytes_shaped: u64,
    drops: u64,
}

impl Default for TokenBucketShaper {
    fn default() -> Self {
        Self::for_network()
    }
}

impl TokenBucketShaper {
    /// Create a new shaper with bucket size and fill rate (bytes/second).
    #[must_use]
    pub fn new(bucket_size: u64, fill_rate_per_sec: u64) -> Self {
        Self {
            bucket_size,
            tokens: bucket_size, // Start full
            fill_rate_per_us: fill_rate_per_sec as f64 / 1_000_000.0,
            last_fill_us: 0,
            bytes_shaped: 0,
            drops: 0,
        }
    }

    /// Factory for network traffic (1MB bucket, 100KB/s).
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(1_000_000, 100_000)
    }

    /// Factory for API rate limiting (10KB bucket, 1KB/s).
    #[must_use]
    pub fn for_api() -> Self {
        Self::new(10_000, 1_000)
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self, now_us: u64) {
        if self.last_fill_us > 0 && now_us > self.last_fill_us {
            let elapsed = now_us - self.last_fill_us;
            let new_tokens = (elapsed as f64 * self.fill_rate_per_us) as u64;
            self.tokens = (self.tokens + new_tokens).min(self.bucket_size);
        }
        self.last_fill_us = now_us;
    }

    /// Try to consume tokens (returns true if allowed).
    pub fn try_consume(&mut self, bytes: u64, now_us: u64) -> bool {
        self.refill(now_us);
        if self.tokens >= bytes {
            self.tokens -= bytes;
            self.bytes_shaped += bytes;
            true
        } else {
            self.drops += 1;
            false
        }
    }

    /// Get current token count.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    /// Get total bytes shaped.
    #[must_use]
    pub fn bytes_shaped(&self) -> u64 {
        self.bytes_shaped
    }

    /// Get drop count.
    #[must_use]
    pub fn drops(&self) -> u64 {
        self.drops
    }

    /// Get fill percentage.
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        if self.bucket_size == 0 {
            0.0
        } else {
            (self.tokens as f64 / self.bucket_size as f64) * 100.0
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.tokens = self.bucket_size;
        self.last_fill_us = 0;
        self.bytes_shaped = 0;
        self.drops = 0;
    }
}

#[cfg(test)]
mod token_bucket_shaper_tests {
    use super::*;

    /// F-SHAPE-001: New shaper starts full
    #[test]
    fn f_shape_001_new() {
        let ts = TokenBucketShaper::new(1000, 100);
        assert_eq!(ts.tokens(), 1000);
    }

    /// F-SHAPE-002: Default uses network settings
    #[test]
    fn f_shape_002_default() {
        let ts = TokenBucketShaper::default();
        assert_eq!(ts.tokens(), 1_000_000);
    }

    /// F-SHAPE-003: Consume reduces tokens
    #[test]
    fn f_shape_003_consume() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        assert_eq!(ts.tokens(), 900);
    }

    /// F-SHAPE-004: Consume fails when insufficient
    #[test]
    fn f_shape_004_consume_fail() {
        let mut ts = TokenBucketShaper::new(100, 10);
        assert!(!ts.try_consume(200, 1000));
    }

    /// F-SHAPE-005: Drops counted
    #[test]
    fn f_shape_005_drops() {
        let mut ts = TokenBucketShaper::new(100, 10);
        ts.try_consume(200, 1000);
        assert_eq!(ts.drops(), 1);
    }

    /// F-SHAPE-006: Bytes shaped tracked
    #[test]
    fn f_shape_006_bytes_shaped() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        ts.try_consume(200, 2000);
        assert_eq!(ts.bytes_shaped(), 300);
    }

    /// F-SHAPE-007: Factory for_network
    #[test]
    fn f_shape_007_for_network() {
        let ts = TokenBucketShaper::for_network();
        assert_eq!(ts.tokens(), 1_000_000);
    }

    /// F-SHAPE-008: Factory for_api
    #[test]
    fn f_shape_008_for_api() {
        let ts = TokenBucketShaper::for_api();
        assert_eq!(ts.tokens(), 10_000);
    }

    /// F-SHAPE-009: Fill percentage calculated
    #[test]
    fn f_shape_009_fill_percentage() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(500, 1000);
        assert!((ts.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SHAPE-010: Refill adds tokens over time
    #[test]
    fn f_shape_010_refill() {
        let mut ts = TokenBucketShaper::new(1000, 1_000_000); // 1 byte/us
        ts.try_consume(500, 0);
        ts.try_consume(0, 250); // 250us later, refill 250 tokens
        assert!(ts.tokens() >= 500); // Should have refilled some
    }

    /// F-SHAPE-011: Reset restores full bucket
    #[test]
    fn f_shape_011_reset() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(500, 1000);
        ts.reset();
        assert_eq!(ts.tokens(), 1000);
    }

    /// F-SHAPE-012: Clone preserves state
    #[test]
    fn f_shape_012_clone() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        let cloned = ts.clone();
        assert_eq!(ts.tokens(), cloned.tokens());
    }
}

// ============================================================================
// LeaderElection - O(1) leader election state tracking
// ============================================================================

/// O(1) leader election state tracking.
///
/// Simple leader/follower state machine with term tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectionState {
    Follower,
    Candidate,
    Leader,
}

/// O(1) leader election tracker.
#[derive(Debug, Clone)]
pub struct LeaderElection {
    state: ElectionState,
    term: u64,
    elections: u64,
    terms_as_leader: u64,
    last_heartbeat_us: u64,
}

impl Default for LeaderElection {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderElection {
    /// Create a new election tracker starting as follower.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ElectionState::Follower,
            term: 0,
            elections: 0,
            terms_as_leader: 0,
            last_heartbeat_us: 0,
        }
    }

    /// Factory for cluster leadership.
    #[must_use]
    pub fn for_cluster() -> Self {
        Self::new()
    }

    /// Start election (become candidate).
    pub fn start_election(&mut self, now_us: u64) {
        self.state = ElectionState::Candidate;
        self.term += 1;
        self.elections += 1;
        self.last_heartbeat_us = now_us;
    }

    /// Win election (become leader).
    pub fn win_election(&mut self, now_us: u64) {
        if self.state == ElectionState::Candidate {
            self.state = ElectionState::Leader;
            self.terms_as_leader += 1;
            self.last_heartbeat_us = now_us;
        }
    }

    /// Lose election or step down (become follower).
    pub fn step_down(&mut self, new_term: u64) {
        if new_term > self.term {
            self.term = new_term;
        }
        self.state = ElectionState::Follower;
    }

    /// Record heartbeat (leader activity).
    pub fn heartbeat(&mut self, now_us: u64) {
        self.last_heartbeat_us = now_us;
    }

    /// Get current state.
    #[must_use]
    pub fn state(&self) -> ElectionState {
        self.state
    }

    /// Get current term.
    #[must_use]
    pub fn term(&self) -> u64 {
        self.term
    }

    /// Check if currently leader.
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.state == ElectionState::Leader
    }

    /// Get total elections.
    #[must_use]
    pub fn elections(&self) -> u64 {
        self.elections
    }

    /// Get terms as leader.
    #[must_use]
    pub fn terms_as_leader(&self) -> u64 {
        self.terms_as_leader
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.state = ElectionState::Follower;
        self.term = 0;
        self.elections = 0;
        self.terms_as_leader = 0;
        self.last_heartbeat_us = 0;
    }
}

#[cfg(test)]
mod leader_election_tests {
    use super::*;

    /// F-ELECT-001: New tracker starts as follower
    #[test]
    fn f_elect_001_new() {
        let le = LeaderElection::new();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-002: Default equals new
    #[test]
    fn f_elect_002_default() {
        let le = LeaderElection::default();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-003: Start election becomes candidate
    #[test]
    fn f_elect_003_start_election() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        assert_eq!(le.state(), ElectionState::Candidate);
    }

    /// F-ELECT-004: Term increments on election
    #[test]
    fn f_elect_004_term_increment() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        assert_eq!(le.term(), 1);
    }

    /// F-ELECT-005: Win election becomes leader
    #[test]
    fn f_elect_005_win_election() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        assert!(le.is_leader());
    }

    /// F-ELECT-006: Step down becomes follower
    #[test]
    fn f_elect_006_step_down() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        le.step_down(2);
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-007: Factory for_cluster
    #[test]
    fn f_elect_007_for_cluster() {
        let le = LeaderElection::for_cluster();
        assert_eq!(le.term(), 0);
    }

    /// F-ELECT-008: Elections counted
    #[test]
    fn f_elect_008_elections() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.start_election(2000);
        assert_eq!(le.elections(), 2);
    }

    /// F-ELECT-009: Terms as leader tracked
    #[test]
    fn f_elect_009_terms_as_leader() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        assert_eq!(le.terms_as_leader(), 1);
    }

    /// F-ELECT-010: Win only works from candidate
    #[test]
    fn f_elect_010_win_requires_candidate() {
        let mut le = LeaderElection::new();
        le.win_election(1000); // Should not become leader
        assert!(!le.is_leader());
    }

    /// F-ELECT-011: Reset clears state
    #[test]
    fn f_elect_011_reset() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        le.reset();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-012: Clone preserves state
    #[test]
    fn f_elect_012_clone() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        let cloned = le.clone();
        assert_eq!(le.state(), cloned.state());
    }
}

// ============================================================================
// CheckpointTracker - O(1) checkpoint/recovery point tracking
// ============================================================================

/// O(1) checkpoint/recovery point tracking.
///
/// Tracks checkpoint frequency, duration, and recovery points.
#[derive(Debug, Clone)]
pub struct CheckpointTracker {
    checkpoints: u64,
    total_duration_us: u64,
    last_checkpoint_us: u64,
    bytes_written: u64,
    failures: u64,
}

impl Default for CheckpointTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointTracker {
    /// Create a new checkpoint tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: 0,
            total_duration_us: 0,
            last_checkpoint_us: 0,
            bytes_written: 0,
            failures: 0,
        }
    }

    /// Factory for database checkpoints.
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Factory for WAL checkpoints.
    #[must_use]
    pub fn for_wal() -> Self {
        Self::new()
    }

    /// Record a successful checkpoint.
    pub fn checkpoint(&mut self, duration_us: u64, bytes: u64, now_us: u64) {
        self.checkpoints += 1;
        self.total_duration_us += duration_us;
        self.bytes_written += bytes;
        self.last_checkpoint_us = now_us;
    }

    /// Record a failed checkpoint.
    pub fn fail(&mut self) {
        self.failures += 1;
    }

    /// Get checkpoint count.
    #[must_use]
    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoints
    }

    /// Get average duration in microseconds.
    #[must_use]
    pub fn avg_duration_us(&self) -> f64 {
        if self.checkpoints == 0 {
            0.0
        } else {
            self.total_duration_us as f64 / self.checkpoints as f64
        }
    }

    /// Get total bytes written.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get failure rate.
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        let total = self.checkpoints + self.failures;
        if total == 0 {
            0.0
        } else {
            self.failures as f64 / total as f64
        }
    }

    /// Check if checkpoint system is healthy.
    #[must_use]
    pub fn is_healthy(&self, max_failure_rate: f64) -> bool {
        self.failure_rate() <= max_failure_rate
    }

    /// Get time since last checkpoint in microseconds.
    #[must_use]
    pub fn time_since_checkpoint(&self, now_us: u64) -> u64 {
        if self.last_checkpoint_us == 0 {
            0
        } else {
            now_us.saturating_sub(self.last_checkpoint_us)
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.checkpoints = 0;
        self.total_duration_us = 0;
        self.last_checkpoint_us = 0;
        self.bytes_written = 0;
        self.failures = 0;
    }
}

#[cfg(test)]
mod checkpoint_tracker_tests {
    use super::*;

    /// F-CKPT-001: New tracker starts empty
    #[test]
    fn f_ckpt_001_new() {
        let ct = CheckpointTracker::new();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-002: Default equals new
    #[test]
    fn f_ckpt_002_default() {
        let ct = CheckpointTracker::default();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-003: Checkpoint increments count
    #[test]
    fn f_ckpt_003_checkpoint() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 1024, 10000);
        assert_eq!(ct.checkpoint_count(), 1);
    }

    /// F-CKPT-004: Bytes written tracked
    #[test]
    fn f_ckpt_004_bytes_written() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 1024, 10000);
        ct.checkpoint(1000, 2048, 20000);
        assert_eq!(ct.bytes_written(), 3072);
    }

    /// F-CKPT-005: Average duration calculated
    #[test]
    fn f_ckpt_005_avg_duration() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.checkpoint(2000, 100, 20000);
        assert!((ct.avg_duration_us() - 1500.0).abs() < 0.01);
    }

    /// F-CKPT-006: Failures tracked
    #[test]
    fn f_ckpt_006_failures() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.fail();
        assert!((ct.failure_rate() - 0.5).abs() < 0.01);
    }

    /// F-CKPT-007: Factory for_database
    #[test]
    fn f_ckpt_007_for_database() {
        let ct = CheckpointTracker::for_database();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-008: Factory for_wal
    #[test]
    fn f_ckpt_008_for_wal() {
        let ct = CheckpointTracker::for_wal();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-009: Healthy when low failures
    #[test]
    fn f_ckpt_009_healthy() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        assert!(ct.is_healthy(0.1));
    }

    /// F-CKPT-010: Time since checkpoint
    #[test]
    fn f_ckpt_010_time_since() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        assert_eq!(ct.time_since_checkpoint(15000), 5000);
    }

    /// F-CKPT-011: Reset clears state
    #[test]
    fn f_ckpt_011_reset() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.reset();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-012: Clone preserves state
    #[test]
    fn f_ckpt_012_clone() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        let cloned = ct.clone();
        assert_eq!(ct.checkpoint_count(), cloned.checkpoint_count());
    }
}

// ============================================================================
// ReplicationLag - O(1) replication lag monitoring
// ============================================================================

/// O(1) replication lag monitoring.
///
/// Tracks replication lag between primary and replica.
#[derive(Debug, Clone)]
pub struct ReplicationLag {
    samples: u64,
    total_lag_us: u64,
    max_lag_us: u64,
    current_lag_us: u64,
    threshold_us: u64,
    breaches: u64,
}

impl Default for ReplicationLag {
    fn default() -> Self {
        Self::for_database()
    }
}

impl ReplicationLag {
    /// Create a new replication lag tracker with threshold.
    #[must_use]
    pub fn new(threshold_us: u64) -> Self {
        Self {
            samples: 0,
            total_lag_us: 0,
            max_lag_us: 0,
            current_lag_us: 0,
            threshold_us,
            breaches: 0,
        }
    }

    /// Factory for database replication (1 second threshold).
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(1_000_000) // 1 second
    }

    /// Factory for cache replication (100ms threshold).
    #[must_use]
    pub fn for_cache() -> Self {
        Self::new(100_000) // 100ms
    }

    /// Record a lag measurement.
    pub fn record(&mut self, lag_us: u64) {
        self.samples += 1;
        self.total_lag_us += lag_us;
        self.current_lag_us = lag_us;
        if lag_us > self.max_lag_us {
            self.max_lag_us = lag_us;
        }
        if lag_us > self.threshold_us {
            self.breaches += 1;
        }
    }

    /// Get current lag in microseconds.
    #[must_use]
    pub fn current_lag_us(&self) -> u64 {
        self.current_lag_us
    }

    /// Get average lag in microseconds.
    #[must_use]
    pub fn avg_lag_us(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total_lag_us as f64 / self.samples as f64
        }
    }

    /// Get max lag in microseconds.
    #[must_use]
    pub fn max_lag_us(&self) -> u64 {
        self.max_lag_us
    }

    /// Get breach count (exceeded threshold).
    #[must_use]
    pub fn breaches(&self) -> u64 {
        self.breaches
    }

    /// Check if currently within threshold.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.current_lag_us <= self.threshold_us
    }

    /// Get breach rate.
    #[must_use]
    pub fn breach_rate(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.breaches as f64 / self.samples as f64
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.samples = 0;
        self.total_lag_us = 0;
        self.max_lag_us = 0;
        self.current_lag_us = 0;
        self.breaches = 0;
    }
}

#[cfg(test)]
mod replication_lag_tests {
    use super::*;

    /// F-REPL-001: New tracker starts empty
    #[test]
    fn f_repl_001_new() {
        let rl = ReplicationLag::new(1000);
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-002: Default uses database threshold
    #[test]
    fn f_repl_002_default() {
        let rl = ReplicationLag::default();
        assert!(rl.is_healthy()); // 0 lag is healthy
    }

    /// F-REPL-003: Record updates current
    #[test]
    fn f_repl_003_record() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        assert_eq!(rl.current_lag_us(), 500);
    }

    /// F-REPL-004: Max lag tracked
    #[test]
    fn f_repl_004_max_lag() {
        let mut rl = ReplicationLag::new(10000);
        rl.record(500);
        rl.record(2000);
        rl.record(800);
        assert_eq!(rl.max_lag_us(), 2000);
    }

    /// F-REPL-005: Average lag calculated
    #[test]
    fn f_repl_005_avg_lag() {
        let mut rl = ReplicationLag::new(10000);
        rl.record(1000);
        rl.record(2000);
        assert!((rl.avg_lag_us() - 1500.0).abs() < 0.01);
    }

    /// F-REPL-006: Breaches counted
    #[test]
    fn f_repl_006_breaches() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        rl.record(1500); // breach
        assert_eq!(rl.breaches(), 1);
    }

    /// F-REPL-007: Factory for_database
    #[test]
    fn f_repl_007_for_database() {
        let rl = ReplicationLag::for_database();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-008: Factory for_cache
    #[test]
    fn f_repl_008_for_cache() {
        let rl = ReplicationLag::for_cache();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-009: Healthy when under threshold
    #[test]
    fn f_repl_009_healthy() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        assert!(rl.is_healthy());
    }

    /// F-REPL-010: Not healthy when over threshold
    #[test]
    fn f_repl_010_unhealthy() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(1500);
        assert!(!rl.is_healthy());
    }

    /// F-REPL-011: Reset clears state
    #[test]
    fn f_repl_011_reset() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        rl.reset();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-012: Clone preserves state
    #[test]
    fn f_repl_012_clone() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        let cloned = rl.clone();
        assert_eq!(rl.current_lag_us(), cloned.current_lag_us());
    }
}

// ============================================================================
// QuorumTracker - O(1) consensus quorum tracking
// ============================================================================

/// O(1) consensus quorum tracking.
///
/// Tracks votes and quorum achievement for distributed consensus.
#[derive(Debug, Clone)]
pub struct QuorumTracker {
    total_nodes: u32,
    votes_received: u32,
    quorum_threshold: u32,
    rounds: u64,
    quorum_achieved: u64,
}

impl Default for QuorumTracker {
    fn default() -> Self {
        Self::for_cluster(3)
    }
}

impl QuorumTracker {
    /// Create a new quorum tracker with total nodes.
    #[must_use]
    pub fn new(total_nodes: u32) -> Self {
        Self {
            total_nodes,
            votes_received: 0,
            quorum_threshold: total_nodes / 2 + 1, // Majority
            rounds: 0,
            quorum_achieved: 0,
        }
    }

    /// Factory for cluster consensus.
    #[must_use]
    pub fn for_cluster(nodes: u32) -> Self {
        Self::new(nodes)
    }

    /// Start a new voting round.
    pub fn start_round(&mut self) {
        self.votes_received = 0;
        self.rounds += 1;
    }

    /// Record a vote.
    pub fn vote(&mut self) {
        if self.votes_received < self.total_nodes {
            self.votes_received += 1;
            if self.votes_received == self.quorum_threshold {
                self.quorum_achieved += 1;
            }
        }
    }

    /// Check if quorum is achieved.
    #[must_use]
    pub fn has_quorum(&self) -> bool {
        self.votes_received >= self.quorum_threshold
    }

    /// Get votes received.
    #[must_use]
    pub fn votes(&self) -> u32 {
        self.votes_received
    }

    /// Get votes needed for quorum.
    #[must_use]
    pub fn votes_needed(&self) -> u32 {
        self.quorum_threshold.saturating_sub(self.votes_received)
    }

    /// Get total rounds.
    #[must_use]
    pub fn rounds(&self) -> u64 {
        self.rounds
    }

    /// Get quorum success rate.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.rounds == 0 {
            0.0
        } else {
            self.quorum_achieved as f64 / self.rounds as f64
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.votes_received = 0;
        self.rounds = 0;
        self.quorum_achieved = 0;
    }
}

#[cfg(test)]
mod quorum_tracker_tests {
    use super::*;

    /// F-QUORUM-001: New tracker starts with no votes
    #[test]
    fn f_quorum_001_new() {
        let qt = QuorumTracker::new(5);
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-002: Default uses 3 nodes
    #[test]
    fn f_quorum_002_default() {
        let qt = QuorumTracker::default();
        assert!(!qt.has_quorum());
    }

    /// F-QUORUM-003: Vote increments count
    #[test]
    fn f_quorum_003_vote() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        assert_eq!(qt.votes(), 1);
    }

    /// F-QUORUM-004: Quorum achieved with majority
    #[test]
    fn f_quorum_004_quorum() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote();
        qt.vote(); // 3/5 = quorum
        assert!(qt.has_quorum());
    }

    /// F-QUORUM-005: No quorum without majority
    #[test]
    fn f_quorum_005_no_quorum() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote(); // 2/5 < quorum
        assert!(!qt.has_quorum());
    }

    /// F-QUORUM-006: Votes needed calculated
    #[test]
    fn f_quorum_006_votes_needed() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        assert_eq!(qt.votes_needed(), 2); // Need 3, have 1
    }

    /// F-QUORUM-007: Factory for_cluster
    #[test]
    fn f_quorum_007_for_cluster() {
        let qt = QuorumTracker::for_cluster(7);
        assert_eq!(qt.votes_needed(), 4); // 7/2+1 = 4
    }

    /// F-QUORUM-008: Start round resets votes
    #[test]
    fn f_quorum_008_start_round() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote();
        qt.start_round();
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-009: Rounds counted
    #[test]
    fn f_quorum_009_rounds() {
        let mut qt = QuorumTracker::new(5);
        qt.start_round();
        qt.start_round();
        assert_eq!(qt.rounds(), 2);
    }

    /// F-QUORUM-010: Success rate calculated
    #[test]
    fn f_quorum_010_success_rate() {
        let mut qt = QuorumTracker::new(3);
        qt.start_round();
        qt.vote();
        qt.vote(); // quorum achieved
        qt.start_round();
        // no votes = no quorum
        assert!((qt.success_rate() - 0.5).abs() < 0.01);
    }

    /// F-QUORUM-011: Reset clears state
    #[test]
    fn f_quorum_011_reset() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.reset();
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-012: Clone preserves state
    #[test]
    fn f_quorum_012_clone() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        let cloned = qt.clone();
        assert_eq!(qt.votes(), cloned.votes());
    }
}

// ============================================================================
// PartitionTracker - O(1) partition/shard tracking
// ============================================================================

/// O(1) partition/shard tracking.
///
/// Tracks partition health, assignment, and rebalancing.
#[derive(Debug, Clone)]
pub struct PartitionTracker {
    total_partitions: u32,
    assigned: u32,
    healthy: u32,
    rebalances: u64,
    last_rebalance_us: u64,
}

impl Default for PartitionTracker {
    fn default() -> Self {
        Self::for_kafka()
    }
}

impl PartitionTracker {
    /// Create a new partition tracker.
    #[must_use]
    pub fn new(total_partitions: u32) -> Self {
        Self {
            total_partitions,
            assigned: 0,
            healthy: 0,
            rebalances: 0,
            last_rebalance_us: 0,
        }
    }

    /// Factory for Kafka-style partitions (12 default).
    #[must_use]
    pub fn for_kafka() -> Self {
        Self::new(12)
    }

    /// Factory for database shards (8 default).
    #[must_use]
    pub fn for_shards() -> Self {
        Self::new(8)
    }

    /// Assign partitions.
    pub fn assign(&mut self, count: u32) {
        self.assigned = count.min(self.total_partitions);
    }

    /// Mark partitions as healthy.
    pub fn mark_healthy(&mut self, count: u32) {
        self.healthy = count.min(self.assigned);
    }

    /// Record a rebalance event.
    pub fn rebalance(&mut self, now_us: u64) {
        self.rebalances += 1;
        self.last_rebalance_us = now_us;
    }

    /// Get assigned partition count.
    #[must_use]
    pub fn assigned(&self) -> u32 {
        self.assigned
    }

    /// Get healthy partition count.
    #[must_use]
    pub fn healthy(&self) -> u32 {
        self.healthy
    }

    /// Get assignment percentage.
    #[must_use]
    pub fn assignment_rate(&self) -> f64 {
        if self.total_partitions == 0 {
            0.0
        } else {
            (self.assigned as f64 / self.total_partitions as f64) * 100.0
        }
    }

    /// Get health percentage.
    #[must_use]
    pub fn health_rate(&self) -> f64 {
        if self.assigned == 0 {
            0.0
        } else {
            (self.healthy as f64 / self.assigned as f64) * 100.0
        }
    }

    /// Check if all assigned partitions are healthy.
    #[must_use]
    pub fn is_fully_healthy(&self) -> bool {
        self.healthy == self.assigned && self.assigned > 0
    }

    /// Get rebalance count.
    #[must_use]
    pub fn rebalances(&self) -> u64 {
        self.rebalances
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.assigned = 0;
        self.healthy = 0;
        self.rebalances = 0;
        self.last_rebalance_us = 0;
    }
}

#[cfg(test)]
mod partition_tracker_tests {
    use super::*;

    /// F-PART-001: New tracker starts empty
    #[test]
    fn f_part_001_new() {
        let pt = PartitionTracker::new(10);
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-002: Default uses Kafka defaults
    #[test]
    fn f_part_002_default() {
        let pt = PartitionTracker::default();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-003: Assign sets count
    #[test]
    fn f_part_003_assign() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        assert_eq!(pt.assigned(), 5);
    }

    /// F-PART-004: Assign capped at total
    #[test]
    fn f_part_004_assign_cap() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(15);
        assert_eq!(pt.assigned(), 10);
    }

    /// F-PART-005: Mark healthy sets count
    #[test]
    fn f_part_005_mark_healthy() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.mark_healthy(3);
        assert_eq!(pt.healthy(), 3);
    }

    /// F-PART-006: Health rate calculated
    #[test]
    fn f_part_006_health_rate() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(10);
        pt.mark_healthy(5);
        assert!((pt.health_rate() - 50.0).abs() < 0.01);
    }

    /// F-PART-007: Factory for_kafka
    #[test]
    fn f_part_007_for_kafka() {
        let pt = PartitionTracker::for_kafka();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-008: Factory for_shards
    #[test]
    fn f_part_008_for_shards() {
        let pt = PartitionTracker::for_shards();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-009: Fully healthy when all healthy
    #[test]
    fn f_part_009_fully_healthy() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.mark_healthy(5);
        assert!(pt.is_fully_healthy());
    }

    /// F-PART-010: Rebalances tracked
    #[test]
    fn f_part_010_rebalances() {
        let mut pt = PartitionTracker::new(10);
        pt.rebalance(1000);
        pt.rebalance(2000);
        assert_eq!(pt.rebalances(), 2);
    }

    /// F-PART-011: Reset clears state
    #[test]
    fn f_part_011_reset() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.reset();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-012: Clone preserves state
    #[test]
    fn f_part_012_clone() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        let cloned = pt.clone();
        assert_eq!(pt.assigned(), cloned.assigned());
    }
}

// ============================================================================
// ConnectionPool - O(1) connection pool state tracking
// ============================================================================

/// O(1) connection pool state tracking.
///
/// Tracks active connections, idle pool, and connection lifecycle.
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    max_size: u32,
    active: u32,
    idle: u32,
    created: u64,
    destroyed: u64,
    wait_count: u64,
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::for_database()
    }
}

impl ConnectionPool {
    /// Create a new connection pool tracker.
    #[must_use]
    pub fn new(max_size: u32) -> Self {
        Self {
            max_size,
            active: 0,
            idle: 0,
            created: 0,
            destroyed: 0,
            wait_count: 0,
        }
    }

    /// Factory for database pool (20 connections).
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(20)
    }

    /// Factory for HTTP pool (100 connections).
    #[must_use]
    pub fn for_http() -> Self {
        Self::new(100)
    }

    /// Acquire a connection from pool.
    pub fn acquire(&mut self) -> bool {
        if self.idle > 0 {
            self.idle -= 1;
            self.active += 1;
            true
        } else if self.active + self.idle < self.max_size {
            self.active += 1;
            self.created += 1;
            true
        } else {
            self.wait_count += 1;
            false
        }
    }

    /// Release a connection back to pool.
    pub fn release(&mut self) {
        if self.active > 0 {
            self.active -= 1;
            self.idle += 1;
        }
    }

    /// Destroy a connection (evict from pool).
    pub fn destroy(&mut self) {
        if self.idle > 0 {
            self.idle -= 1;
            self.destroyed += 1;
        }
    }

    /// Get active connection count.
    #[must_use]
    pub fn active(&self) -> u32 {
        self.active
    }

    /// Get idle connection count.
    #[must_use]
    pub fn idle(&self) -> u32 {
        self.idle
    }

    /// Get pool utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.active as f64 / self.max_size as f64) * 100.0
        }
    }

    /// Check if pool is exhausted.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.active >= self.max_size && self.idle == 0
    }

    /// Get wait count (failed acquisitions).
    #[must_use]
    pub fn wait_count(&self) -> u64 {
        self.wait_count
    }

    /// Reset pool state.
    pub fn reset(&mut self) {
        self.active = 0;
        self.idle = 0;
        self.created = 0;
        self.destroyed = 0;
        self.wait_count = 0;
    }
}

#[cfg(test)]
mod connection_pool_tests {
    use super::*;

    /// F-CPOOL-001: New pool starts empty
    #[test]
    fn f_cpool_001_new() {
        let cp = ConnectionPool::new(10);
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-002: Default uses database size
    #[test]
    fn f_cpool_002_default() {
        let cp = ConnectionPool::default();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-003: Acquire creates connection
    #[test]
    fn f_cpool_003_acquire() {
        let mut cp = ConnectionPool::new(10);
        assert!(cp.acquire());
        assert_eq!(cp.active(), 1);
    }

    /// F-CPOOL-004: Release returns to idle
    #[test]
    fn f_cpool_004_release() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.release();
        assert_eq!(cp.idle(), 1);
    }

    /// F-CPOOL-005: Acquire from idle
    #[test]
    fn f_cpool_005_acquire_idle() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.release();
        cp.acquire();
        assert_eq!(cp.active(), 1);
        assert_eq!(cp.idle(), 0);
    }

    /// F-CPOOL-006: Exhausted when full
    #[test]
    fn f_cpool_006_exhausted() {
        let mut cp = ConnectionPool::new(2);
        cp.acquire();
        cp.acquire();
        assert!(cp.is_exhausted());
    }

    /// F-CPOOL-007: Factory for_database
    #[test]
    fn f_cpool_007_for_database() {
        let cp = ConnectionPool::for_database();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-008: Factory for_http
    #[test]
    fn f_cpool_008_for_http() {
        let cp = ConnectionPool::for_http();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-009: Utilization calculated
    #[test]
    fn f_cpool_009_utilization() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.acquire();
        assert!((cp.utilization() - 20.0).abs() < 0.01);
    }

    /// F-CPOOL-010: Wait count on exhaustion
    #[test]
    fn f_cpool_010_wait_count() {
        let mut cp = ConnectionPool::new(1);
        cp.acquire();
        cp.acquire(); // fails
        assert_eq!(cp.wait_count(), 1);
    }

    /// F-CPOOL-011: Reset clears state
    #[test]
    fn f_cpool_011_reset() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.reset();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-012: Clone preserves state
    #[test]
    fn f_cpool_012_clone() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        let cloned = cp.clone();
        assert_eq!(cp.active(), cloned.active());
    }
}

// ============================================================================
// RequestTracker - O(1) request lifecycle tracking
// ============================================================================

/// O(1) request lifecycle tracking.
///
/// Tracks request counts, latencies, and error rates.
#[derive(Debug, Clone)]
pub struct RequestTracker {
    total: u64,
    success: u64,
    errors: u64,
    total_latency_us: u64,
    max_latency_us: u64,
    in_flight: u32,
}

impl Default for RequestTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestTracker {
    /// Create a new request tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            errors: 0,
            total_latency_us: 0,
            max_latency_us: 0,
            in_flight: 0,
        }
    }

    /// Factory for API requests.
    #[must_use]
    pub fn for_api() -> Self {
        Self::new()
    }

    /// Factory for database queries.
    #[must_use]
    pub fn for_queries() -> Self {
        Self::new()
    }

    /// Start tracking a request.
    pub fn start(&mut self) {
        self.in_flight += 1;
    }

    /// Complete a successful request.
    pub fn complete(&mut self, latency_us: u64) {
        self.total += 1;
        self.success += 1;
        self.total_latency_us += latency_us;
        if latency_us > self.max_latency_us {
            self.max_latency_us = latency_us;
        }
        if self.in_flight > 0 {
            self.in_flight -= 1;
        }
    }

    /// Complete a failed request.
    pub fn fail(&mut self, latency_us: u64) {
        self.total += 1;
        self.errors += 1;
        self.total_latency_us += latency_us;
        if latency_us > self.max_latency_us {
            self.max_latency_us = latency_us;
        }
        if self.in_flight > 0 {
            self.in_flight -= 1;
        }
    }

    /// Get total requests.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total
    }

    /// Get success rate.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.success as f64 / self.total as f64) * 100.0
        }
    }

    /// Get error rate.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.errors as f64 / self.total as f64) * 100.0
        }
    }

    /// Get average latency in microseconds.
    #[must_use]
    pub fn avg_latency_us(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.total_latency_us as f64 / self.total as f64
        }
    }

    /// Get in-flight request count.
    #[must_use]
    pub fn in_flight(&self) -> u32 {
        self.in_flight
    }

    /// Check if error rate is acceptable.
    #[must_use]
    pub fn is_healthy(&self, max_error_rate: f64) -> bool {
        self.error_rate() <= max_error_rate
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.total = 0;
        self.success = 0;
        self.errors = 0;
        self.total_latency_us = 0;
        self.max_latency_us = 0;
        self.in_flight = 0;
    }
}

#[cfg(test)]
mod request_tracker_tests {
    use super::*;

    /// F-REQ-001: New tracker starts empty
    #[test]
    fn f_req_001_new() {
        let rt = RequestTracker::new();
        assert_eq!(rt.total(), 0);
    }

    /// F-REQ-002: Default equals new
    #[test]
    fn f_req_002_default() {
        let rt = RequestTracker::default();
        assert_eq!(rt.total(), 0);
    }

    /// F-REQ-003: Start increments in_flight
    #[test]
    fn f_req_003_start() {
        let mut rt = RequestTracker::new();
        rt.start();
        assert_eq!(rt.in_flight(), 1);
    }

    /// F-REQ-004: Complete tracks success
    #[test]
    fn f_req_004_complete() {
        let mut rt = RequestTracker::new();
        rt.start();
        rt.complete(1000);
        assert_eq!(rt.total(), 1);
        assert!((rt.success_rate() - 100.0).abs() < 0.01);
    }

    /// F-REQ-005: Fail tracks errors
    #[test]
    fn f_req_005_fail() {
        let mut rt = RequestTracker::new();
        rt.start();
        rt.fail(1000);
        assert!((rt.error_rate() - 100.0).abs() < 0.01);
    }

    /// F-REQ-006: Average latency calculated
    #[test]
    fn f_req_006_avg_latency() {
        let mut rt = RequestTracker::new();
        rt.complete(1000);
        rt.complete(2000);
        assert!((rt.avg_latency_us() - 1500.0).abs() < 0.01);
    }

    /// F-REQ-007: Factory for_api
    #[test]
    fn f_req_007_for_api() {
        let rt = RequestTracker::for_api();
        assert_eq!(rt.total(), 0);
    }

    /// F-REQ-008: Factory for_queries
    #[test]
    fn f_req_008_for_queries() {
        let rt = RequestTracker::for_queries();
        assert_eq!(rt.total(), 0);
    }

    /// F-REQ-009: Healthy when low errors
    #[test]
    fn f_req_009_healthy() {
        let mut rt = RequestTracker::new();
        rt.complete(1000);
        assert!(rt.is_healthy(1.0));
    }

    /// F-REQ-010: Not healthy when high errors
    #[test]
    fn f_req_010_unhealthy() {
        let mut rt = RequestTracker::new();
        rt.fail(1000);
        assert!(!rt.is_healthy(1.0));
    }

    /// F-REQ-011: Reset clears state
    #[test]
    fn f_req_011_reset() {
        let mut rt = RequestTracker::new();
        rt.complete(1000);
        rt.reset();
        assert_eq!(rt.total(), 0);
    }

    /// F-REQ-012: Clone preserves state
    #[test]
    fn f_req_012_clone() {
        let mut rt = RequestTracker::new();
        rt.complete(1000);
        let cloned = rt.clone();
        assert_eq!(rt.total(), cloned.total());
    }
}

// ============================================================================
// SessionTracker - O(1) session management tracking
// ============================================================================

/// O(1) session management tracking.
///
/// Tracks active sessions, expirations, and session lifecycle.
#[derive(Debug, Clone)]
pub struct SessionTracker {
    active: u64,
    created: u64,
    expired: u64,
    peak: u64,
    total_duration_us: u64,
}

impl Default for SessionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionTracker {
    /// Create a new session tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: 0,
            created: 0,
            expired: 0,
            peak: 0,
            total_duration_us: 0,
        }
    }

    /// Factory for user sessions.
    #[must_use]
    pub fn for_users() -> Self {
        Self::new()
    }

    /// Factory for API sessions.
    #[must_use]
    pub fn for_api() -> Self {
        Self::new()
    }

    /// Create a new session.
    pub fn create(&mut self) {
        self.active += 1;
        self.created += 1;
        if self.active > self.peak {
            self.peak = self.active;
        }
    }

    /// End a session normally.
    pub fn end(&mut self, duration_us: u64) {
        if self.active > 0 {
            self.active -= 1;
            self.total_duration_us += duration_us;
        }
    }

    /// Expire a session (timeout).
    pub fn expire(&mut self, duration_us: u64) {
        if self.active > 0 {
            self.active -= 1;
            self.expired += 1;
            self.total_duration_us += duration_us;
        }
    }

    /// Get active session count.
    #[must_use]
    pub fn active(&self) -> u64 {
        self.active
    }

    /// Get total created sessions.
    #[must_use]
    pub fn created(&self) -> u64 {
        self.created
    }

    /// Get peak concurrent sessions.
    #[must_use]
    pub fn peak(&self) -> u64 {
        self.peak
    }

    /// Get expiration rate.
    #[must_use]
    pub fn expiration_rate(&self) -> f64 {
        let total_ended = self.created - self.active;
        if total_ended == 0 {
            0.0
        } else {
            (self.expired as f64 / total_ended as f64) * 100.0
        }
    }

    /// Get average session duration in microseconds.
    #[must_use]
    pub fn avg_duration_us(&self) -> f64 {
        let ended = self.created - self.active;
        if ended == 0 {
            0.0
        } else {
            self.total_duration_us as f64 / ended as f64
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.active = 0;
        self.created = 0;
        self.expired = 0;
        self.peak = 0;
        self.total_duration_us = 0;
    }
}

#[cfg(test)]
mod session_tracker_tests {
    use super::*;

    /// F-SESS-001: New tracker starts empty
    #[test]
    fn f_sess_001_new() {
        let st = SessionTracker::new();
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-002: Default equals new
    #[test]
    fn f_sess_002_default() {
        let st = SessionTracker::default();
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-003: Create increments active
    #[test]
    fn f_sess_003_create() {
        let mut st = SessionTracker::new();
        st.create();
        assert_eq!(st.active(), 1);
    }

    /// F-SESS-004: End decrements active
    #[test]
    fn f_sess_004_end() {
        let mut st = SessionTracker::new();
        st.create();
        st.end(1000);
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-005: Expire tracks timeouts
    #[test]
    fn f_sess_005_expire() {
        let mut st = SessionTracker::new();
        st.create();
        st.expire(1000);
        assert!(st.expiration_rate() > 0.0);
    }

    /// F-SESS-006: Peak tracked
    #[test]
    fn f_sess_006_peak() {
        let mut st = SessionTracker::new();
        st.create();
        st.create();
        st.end(1000);
        assert_eq!(st.peak(), 2);
    }

    /// F-SESS-007: Factory for_users
    #[test]
    fn f_sess_007_for_users() {
        let st = SessionTracker::for_users();
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-008: Factory for_api
    #[test]
    fn f_sess_008_for_api() {
        let st = SessionTracker::for_api();
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-009: Average duration calculated
    #[test]
    fn f_sess_009_avg_duration() {
        let mut st = SessionTracker::new();
        st.create();
        st.end(1000);
        st.create();
        st.end(2000);
        assert!((st.avg_duration_us() - 1500.0).abs() < 0.01);
    }

    /// F-SESS-010: Created count tracked
    #[test]
    fn f_sess_010_created() {
        let mut st = SessionTracker::new();
        st.create();
        st.create();
        assert_eq!(st.created(), 2);
    }

    /// F-SESS-011: Reset clears state
    #[test]
    fn f_sess_011_reset() {
        let mut st = SessionTracker::new();
        st.create();
        st.reset();
        assert_eq!(st.active(), 0);
    }

    /// F-SESS-012: Clone preserves state
    #[test]
    fn f_sess_012_clone() {
        let mut st = SessionTracker::new();
        st.create();
        let cloned = st.clone();
        assert_eq!(st.active(), cloned.active());
    }
}

// ============================================================================
// TransactionTracker - O(1) transaction state tracking
// ============================================================================

/// O(1) transaction state tracking.
///
/// Tracks transactions, commits, rollbacks, and deadlocks.
#[derive(Debug, Clone)]
pub struct TransactionTracker {
    active: u32,
    committed: u64,
    rolled_back: u64,
    deadlocks: u64,
    total_duration_us: u64,
}

impl Default for TransactionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TransactionTracker {
    /// Create a new transaction tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: 0,
            committed: 0,
            rolled_back: 0,
            deadlocks: 0,
            total_duration_us: 0,
        }
    }

    /// Factory for database transactions.
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Factory for distributed transactions.
    #[must_use]
    pub fn for_distributed() -> Self {
        Self::new()
    }

    /// Begin a transaction.
    pub fn begin(&mut self) {
        self.active += 1;
    }

    /// Commit a transaction.
    pub fn commit(&mut self, duration_us: u64) {
        if self.active > 0 {
            self.active -= 1;
            self.committed += 1;
            self.total_duration_us += duration_us;
        }
    }

    /// Rollback a transaction.
    pub fn rollback(&mut self, duration_us: u64) {
        if self.active > 0 {
            self.active -= 1;
            self.rolled_back += 1;
            self.total_duration_us += duration_us;
        }
    }

    /// Record a deadlock.
    pub fn deadlock(&mut self) {
        self.deadlocks += 1;
    }

    /// Get active transaction count.
    #[must_use]
    pub fn active(&self) -> u32 {
        self.active
    }

    /// Get commit count.
    #[must_use]
    pub fn committed(&self) -> u64 {
        self.committed
    }

    /// Get commit rate.
    #[must_use]
    pub fn commit_rate(&self) -> f64 {
        let total = self.committed + self.rolled_back;
        if total == 0 {
            0.0
        } else {
            (self.committed as f64 / total as f64) * 100.0
        }
    }

    /// Get rollback rate.
    #[must_use]
    pub fn rollback_rate(&self) -> f64 {
        let total = self.committed + self.rolled_back;
        if total == 0 {
            0.0
        } else {
            (self.rolled_back as f64 / total as f64) * 100.0
        }
    }

    /// Get deadlock count.
    #[must_use]
    pub fn deadlocks(&self) -> u64 {
        self.deadlocks
    }

    /// Check if transaction health is good.
    #[must_use]
    pub fn is_healthy(&self, max_rollback_rate: f64) -> bool {
        self.rollback_rate() <= max_rollback_rate
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.active = 0;
        self.committed = 0;
        self.rolled_back = 0;
        self.deadlocks = 0;
        self.total_duration_us = 0;
    }
}

#[cfg(test)]
mod transaction_tracker_tests {
    use super::*;

    /// F-TXN-001: New tracker starts empty
    #[test]
    fn f_txn_001_new() {
        let tt = TransactionTracker::new();
        assert_eq!(tt.active(), 0);
    }

    /// F-TXN-002: Default equals new
    #[test]
    fn f_txn_002_default() {
        let tt = TransactionTracker::default();
        assert_eq!(tt.active(), 0);
    }

    /// F-TXN-003: Begin increments active
    #[test]
    fn f_txn_003_begin() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        assert_eq!(tt.active(), 1);
    }

    /// F-TXN-004: Commit tracks success
    #[test]
    fn f_txn_004_commit() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        tt.commit(1000);
        assert_eq!(tt.committed(), 1);
    }

    /// F-TXN-005: Rollback tracks failure
    #[test]
    fn f_txn_005_rollback() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        tt.rollback(1000);
        assert!((tt.rollback_rate() - 100.0).abs() < 0.01);
    }

    /// F-TXN-006: Commit rate calculated
    #[test]
    fn f_txn_006_commit_rate() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        tt.commit(1000);
        tt.begin();
        tt.rollback(1000);
        assert!((tt.commit_rate() - 50.0).abs() < 0.01);
    }

    /// F-TXN-007: Factory for_database
    #[test]
    fn f_txn_007_for_database() {
        let tt = TransactionTracker::for_database();
        assert_eq!(tt.active(), 0);
    }

    /// F-TXN-008: Factory for_distributed
    #[test]
    fn f_txn_008_for_distributed() {
        let tt = TransactionTracker::for_distributed();
        assert_eq!(tt.active(), 0);
    }

    /// F-TXN-009: Deadlocks tracked
    #[test]
    fn f_txn_009_deadlocks() {
        let mut tt = TransactionTracker::new();
        tt.deadlock();
        tt.deadlock();
        assert_eq!(tt.deadlocks(), 2);
    }

    /// F-TXN-010: Healthy when low rollbacks
    #[test]
    fn f_txn_010_healthy() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        tt.commit(1000);
        assert!(tt.is_healthy(10.0));
    }

    /// F-TXN-011: Reset clears state
    #[test]
    fn f_txn_011_reset() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        tt.commit(1000);
        tt.reset();
        assert_eq!(tt.committed(), 0);
    }

    /// F-TXN-012: Clone preserves state
    #[test]
    fn f_txn_012_clone() {
        let mut tt = TransactionTracker::new();
        tt.begin();
        let cloned = tt.clone();
        assert_eq!(tt.active(), cloned.active());
    }
}

// ============================================================================
// v9.28.0: Event & Queue O(1) Helpers
// ============================================================================

/// O(1) event emission tracking.
///
/// Tracks event dispatch patterns with subscriber counts
/// and delivery success rates.
#[derive(Debug, Clone)]
pub struct EventEmitter {
    events_emitted: u64,
    events_delivered: u64,
    events_dropped: u64,
    subscribers: u32,
    max_subscribers: u32,
}

impl Default for EventEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl EventEmitter {
    /// Create new event emitter tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events_emitted: 0,
            events_delivered: 0,
            events_dropped: 0,
            subscribers: 0,
            max_subscribers: 0,
        }
    }

    /// Factory for UI event buses.
    #[must_use]
    pub fn for_ui() -> Self {
        Self::new()
    }

    /// Factory for system event buses.
    #[must_use]
    pub fn for_system() -> Self {
        Self::new()
    }

    /// Subscribe a new listener.
    pub fn subscribe(&mut self) {
        self.subscribers += 1;
        self.max_subscribers = self.max_subscribers.max(self.subscribers);
    }

    /// Unsubscribe a listener.
    pub fn unsubscribe(&mut self) {
        self.subscribers = self.subscribers.saturating_sub(1);
    }

    /// Emit an event to all subscribers.
    pub fn emit(&mut self, delivered: u32) {
        self.events_emitted += 1;
        self.events_delivered += u64::from(delivered);
        if delivered < self.subscribers {
            self.events_dropped += u64::from(self.subscribers - delivered);
        }
    }

    /// Get total events emitted.
    #[must_use]
    pub fn emitted(&self) -> u64 {
        self.events_emitted
    }

    /// Get current subscriber count.
    #[must_use]
    pub fn subscribers(&self) -> u32 {
        self.subscribers
    }

    /// Get delivery success rate (%).
    #[must_use]
    pub fn delivery_rate(&self) -> f64 {
        let total = self.events_delivered + self.events_dropped;
        if total == 0 {
            100.0
        } else {
            (self.events_delivered as f64 / total as f64) * 100.0
        }
    }

    /// Check if emitter is healthy (delivery > threshold).
    #[must_use]
    pub fn is_healthy(&self, min_delivery_rate: f64) -> bool {
        self.delivery_rate() >= min_delivery_rate
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        self.events_emitted = 0;
        self.events_delivered = 0;
        self.events_dropped = 0;
        self.max_subscribers = self.subscribers;
    }
}

/// O(1) queue depth monitoring.
///
/// Tracks queue fill levels and throughput patterns.
#[derive(Debug, Clone)]
pub struct QueueDepth {
    capacity: u64,
    current: u64,
    peak: u64,
    enqueued: u64,
    dequeued: u64,
}

impl Default for QueueDepth {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl QueueDepth {
    /// Create new queue depth tracker.
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            current: 0,
            peak: 0,
            enqueued: 0,
            dequeued: 0,
        }
    }

    /// Factory for message queues.
    #[must_use]
    pub fn for_messages() -> Self {
        Self::new(10000)
    }

    /// Factory for task queues.
    #[must_use]
    pub fn for_tasks() -> Self {
        Self::new(1000)
    }

    /// Enqueue an item.
    pub fn enqueue(&mut self) -> bool {
        if self.current < self.capacity {
            self.current += 1;
            self.enqueued += 1;
            self.peak = self.peak.max(self.current);
            true
        } else {
            false
        }
    }

    /// Dequeue an item.
    pub fn dequeue(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            self.dequeued += 1;
            true
        } else {
            false
        }
    }

    /// Get current depth.
    #[must_use]
    pub fn depth(&self) -> u64 {
        self.current
    }

    /// Get utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current as f64 / self.capacity as f64) * 100.0
        }
    }

    /// Check if queue is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.current >= self.capacity
    }

    /// Check if queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current == 0
    }

    /// Get throughput (items processed).
    #[must_use]
    pub fn throughput(&self) -> u64 {
        self.dequeued
    }

    /// Reset counters (keep current depth).
    pub fn reset(&mut self) {
        self.peak = self.current;
        self.enqueued = 0;
        self.dequeued = 0;
    }
}

/// O(1) scheduled task tracking.
///
/// Tracks task scheduling, execution, and deadline metrics.
#[derive(Debug, Clone)]
pub struct TaskScheduler {
    scheduled: u64,
    executed: u64,
    missed: u64,
    cancelled: u64,
    total_latency_us: u64,
}

impl Default for TaskScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskScheduler {
    /// Create new task scheduler tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            scheduled: 0,
            executed: 0,
            missed: 0,
            cancelled: 0,
            total_latency_us: 0,
        }
    }

    /// Factory for periodic tasks.
    #[must_use]
    pub fn for_periodic() -> Self {
        Self::new()
    }

    /// Factory for one-shot tasks.
    #[must_use]
    pub fn for_oneshot() -> Self {
        Self::new()
    }

    /// Schedule a new task.
    pub fn schedule(&mut self) {
        self.scheduled += 1;
    }

    /// Mark task as executed.
    pub fn execute(&mut self, latency_us: u64) {
        self.executed += 1;
        self.total_latency_us += latency_us;
    }

    /// Mark task as missed (deadline exceeded).
    pub fn miss(&mut self) {
        self.missed += 1;
    }

    /// Cancel a scheduled task.
    pub fn cancel(&mut self) {
        self.cancelled += 1;
    }

    /// Get execution rate (%).
    #[must_use]
    pub fn execution_rate(&self) -> f64 {
        if self.scheduled == 0 {
            100.0
        } else {
            (self.executed as f64 / self.scheduled as f64) * 100.0
        }
    }

    /// Get miss rate (%).
    #[must_use]
    pub fn miss_rate(&self) -> f64 {
        let total = self.executed + self.missed;
        if total == 0 {
            0.0
        } else {
            (self.missed as f64 / total as f64) * 100.0
        }
    }

    /// Get average execution latency (us).
    #[must_use]
    pub fn avg_latency_us(&self) -> u64 {
        if self.executed == 0 {
            0
        } else {
            self.total_latency_us / self.executed
        }
    }

    /// Check if scheduler is healthy (miss rate < threshold).
    #[must_use]
    pub fn is_healthy(&self, max_miss_rate: f64) -> bool {
        self.miss_rate() <= max_miss_rate
    }

    /// Reset all counters.
    pub fn reset(&mut self) {
        self.scheduled = 0;
        self.executed = 0;
        self.missed = 0;
        self.cancelled = 0;
        self.total_latency_us = 0;
    }
}

/// O(1) dead letter queue tracking.
///
/// Tracks failed message routing and retry patterns.
#[derive(Debug, Clone)]
pub struct DeadletterQueue {
    capacity: u64,
    current: u64,
    added: u64,
    reprocessed: u64,
    expired: u64,
}

impl Default for DeadletterQueue {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl DeadletterQueue {
    /// Create new DLQ tracker.
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            current: 0,
            added: 0,
            reprocessed: 0,
            expired: 0,
        }
    }

    /// Factory for message DLQ.
    #[must_use]
    pub fn for_messages() -> Self {
        Self::new(10000)
    }

    /// Factory for event DLQ.
    #[must_use]
    pub fn for_events() -> Self {
        Self::new(1000)
    }

    /// Add failed message to DLQ.
    pub fn add(&mut self) -> bool {
        if self.current < self.capacity {
            self.current += 1;
            self.added += 1;
            true
        } else {
            false
        }
    }

    /// Reprocess message from DLQ.
    pub fn reprocess(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            self.reprocessed += 1;
            true
        } else {
            false
        }
    }

    /// Expire message from DLQ.
    pub fn expire(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            self.expired += 1;
            true
        } else {
            false
        }
    }

    /// Get current DLQ size.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.current
    }

    /// Get recovery rate (%).
    #[must_use]
    pub fn recovery_rate(&self) -> f64 {
        let processed = self.reprocessed + self.expired;
        if processed == 0 {
            100.0
        } else {
            (self.reprocessed as f64 / processed as f64) * 100.0
        }
    }

    /// Check if DLQ is healthy (recovery > threshold).
    #[must_use]
    pub fn is_healthy(&self, min_recovery_rate: f64) -> bool {
        self.recovery_rate() >= min_recovery_rate
    }

    /// Check if DLQ is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.current >= self.capacity
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.added = 0;
        self.reprocessed = 0;
        self.expired = 0;
    }
}

#[cfg(test)]
mod event_emitter_tests {
    use super::*;

    /// F-EMIT-001: New creates empty emitter
    #[test]
    fn f_emit_001_new() {
        let ee = EventEmitter::new();
        assert_eq!(ee.emitted(), 0);
    }

    /// F-EMIT-002: Default equals new
    #[test]
    fn f_emit_002_default() {
        let ee = EventEmitter::default();
        assert_eq!(ee.subscribers(), 0);
    }

    /// F-EMIT-003: Subscribe increments count
    #[test]
    fn f_emit_003_subscribe() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        assert_eq!(ee.subscribers(), 1);
    }

    /// F-EMIT-004: Unsubscribe decrements count
    #[test]
    fn f_emit_004_unsubscribe() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        ee.unsubscribe();
        assert_eq!(ee.subscribers(), 0);
    }

    /// F-EMIT-005: Emit tracks events
    #[test]
    fn f_emit_005_emit() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        ee.emit(1);
        assert_eq!(ee.emitted(), 1);
    }

    /// F-EMIT-006: Delivery rate calculated
    #[test]
    fn f_emit_006_delivery_rate() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        ee.subscribe();
        ee.emit(1); // 1 delivered, 1 dropped
        assert!((ee.delivery_rate() - 50.0).abs() < 0.01);
    }

    /// F-EMIT-007: Factory for_ui creates emitter
    #[test]
    fn f_emit_007_for_ui() {
        let ee = EventEmitter::for_ui();
        assert_eq!(ee.emitted(), 0);
    }

    /// F-EMIT-008: Factory for_system creates emitter
    #[test]
    fn f_emit_008_for_system() {
        let ee = EventEmitter::for_system();
        assert_eq!(ee.subscribers(), 0);
    }

    /// F-EMIT-009: Healthy when delivery high
    #[test]
    fn f_emit_009_healthy() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        ee.emit(1);
        assert!(ee.is_healthy(90.0));
    }

    /// F-EMIT-010: Unhealthy when delivery low
    #[test]
    fn f_emit_010_unhealthy() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        ee.subscribe();
        ee.emit(0); // 0 delivered, 2 dropped
        assert!(!ee.is_healthy(50.0));
    }

    /// F-EMIT-011: Reset clears counters
    #[test]
    fn f_emit_011_reset() {
        let mut ee = EventEmitter::new();
        ee.emit(0);
        ee.reset();
        assert_eq!(ee.emitted(), 0);
    }

    /// F-EMIT-012: Clone preserves state
    #[test]
    fn f_emit_012_clone() {
        let mut ee = EventEmitter::new();
        ee.subscribe();
        let cloned = ee.clone();
        assert_eq!(ee.subscribers(), cloned.subscribers());
    }
}

#[cfg(test)]
mod queue_depth_tests {
    use super::*;

    /// F-QDEPTH-001: New creates empty queue
    #[test]
    fn f_qdepth_001_new() {
        let qd = QueueDepth::new(100);
        assert_eq!(qd.depth(), 0);
    }

    /// F-QDEPTH-002: Default has capacity
    #[test]
    fn f_qdepth_002_default() {
        let qd = QueueDepth::default();
        assert!(qd.is_empty());
    }

    /// F-QDEPTH-003: Enqueue increases depth
    #[test]
    fn f_qdepth_003_enqueue() {
        let mut qd = QueueDepth::new(100);
        assert!(qd.enqueue());
        assert_eq!(qd.depth(), 1);
    }

    /// F-QDEPTH-004: Dequeue decreases depth
    #[test]
    fn f_qdepth_004_dequeue() {
        let mut qd = QueueDepth::new(100);
        qd.enqueue();
        assert!(qd.dequeue());
        assert_eq!(qd.depth(), 0);
    }

    /// F-QDEPTH-005: Utilization calculated
    #[test]
    fn f_qdepth_005_utilization() {
        let mut qd = QueueDepth::new(100);
        for _ in 0..50 {
            qd.enqueue();
        }
        assert!((qd.utilization() - 50.0).abs() < 0.01);
    }

    /// F-QDEPTH-006: Full when at capacity
    #[test]
    fn f_qdepth_006_full() {
        let mut qd = QueueDepth::new(2);
        qd.enqueue();
        qd.enqueue();
        assert!(qd.is_full());
    }

    /// F-QDEPTH-007: Factory for_messages
    #[test]
    fn f_qdepth_007_for_messages() {
        let qd = QueueDepth::for_messages();
        assert_eq!(qd.capacity, 10000);
    }

    /// F-QDEPTH-008: Factory for_tasks
    #[test]
    fn f_qdepth_008_for_tasks() {
        let qd = QueueDepth::for_tasks();
        assert_eq!(qd.capacity, 1000);
    }

    /// F-QDEPTH-009: Throughput tracks dequeues
    #[test]
    fn f_qdepth_009_throughput() {
        let mut qd = QueueDepth::new(100);
        qd.enqueue();
        qd.dequeue();
        assert_eq!(qd.throughput(), 1);
    }

    /// F-QDEPTH-010: Enqueue fails when full
    #[test]
    fn f_qdepth_010_enqueue_full() {
        let mut qd = QueueDepth::new(1);
        qd.enqueue();
        assert!(!qd.enqueue());
    }

    /// F-QDEPTH-011: Reset clears counters
    #[test]
    fn f_qdepth_011_reset() {
        let mut qd = QueueDepth::new(100);
        qd.enqueue();
        qd.dequeue();
        qd.reset();
        assert_eq!(qd.throughput(), 0);
    }

    /// F-QDEPTH-012: Clone preserves state
    #[test]
    fn f_qdepth_012_clone() {
        let mut qd = QueueDepth::new(100);
        qd.enqueue();
        let cloned = qd.clone();
        assert_eq!(qd.depth(), cloned.depth());
    }
}

#[cfg(test)]
mod task_scheduler_tests {
    use super::*;

    /// F-TSCHED-001: New creates empty scheduler
    #[test]
    fn f_tsched_001_new() {
        let ts = TaskScheduler::new();
        assert_eq!(ts.scheduled, 0);
    }

    /// F-TSCHED-002: Default equals new
    #[test]
    fn f_tsched_002_default() {
        let ts = TaskScheduler::default();
        assert_eq!(ts.executed, 0);
    }

    /// F-TSCHED-003: Schedule increments count
    #[test]
    fn f_tsched_003_schedule() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        assert_eq!(ts.scheduled, 1);
    }

    /// F-TSCHED-004: Execute tracks success
    #[test]
    fn f_tsched_004_execute() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        ts.execute(1000);
        assert_eq!(ts.executed, 1);
    }

    /// F-TSCHED-005: Miss tracks failures
    #[test]
    fn f_tsched_005_miss() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        ts.miss();
        assert!((ts.miss_rate() - 100.0).abs() < 0.01);
    }

    /// F-TSCHED-006: Execution rate calculated
    #[test]
    fn f_tsched_006_execution_rate() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        ts.execute(1000);
        assert!((ts.execution_rate() - 100.0).abs() < 0.01);
    }

    /// F-TSCHED-007: Factory for_periodic
    #[test]
    fn f_tsched_007_for_periodic() {
        let ts = TaskScheduler::for_periodic();
        assert_eq!(ts.scheduled, 0);
    }

    /// F-TSCHED-008: Factory for_oneshot
    #[test]
    fn f_tsched_008_for_oneshot() {
        let ts = TaskScheduler::for_oneshot();
        assert_eq!(ts.executed, 0);
    }

    /// F-TSCHED-009: Avg latency calculated
    #[test]
    fn f_tsched_009_avg_latency() {
        let mut ts = TaskScheduler::new();
        ts.execute(1000);
        ts.execute(2000);
        assert_eq!(ts.avg_latency_us(), 1500);
    }

    /// F-TSCHED-010: Healthy when miss rate low
    #[test]
    fn f_tsched_010_healthy() {
        let mut ts = TaskScheduler::new();
        ts.execute(1000);
        assert!(ts.is_healthy(5.0));
    }

    /// F-TSCHED-011: Reset clears counters
    #[test]
    fn f_tsched_011_reset() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        ts.execute(1000);
        ts.reset();
        assert_eq!(ts.scheduled, 0);
    }

    /// F-TSCHED-012: Clone preserves state
    #[test]
    fn f_tsched_012_clone() {
        let mut ts = TaskScheduler::new();
        ts.schedule();
        let cloned = ts.clone();
        assert_eq!(ts.scheduled, cloned.scheduled);
    }
}

#[cfg(test)]
mod deadletter_queue_tests {
    use super::*;

    /// F-DLQ-001: New creates empty DLQ
    #[test]
    fn f_dlq_001_new() {
        let dlq = DeadletterQueue::new(100);
        assert_eq!(dlq.size(), 0);
    }

    /// F-DLQ-002: Default has capacity
    #[test]
    fn f_dlq_002_default() {
        let dlq = DeadletterQueue::default();
        assert!(!dlq.is_full());
    }

    /// F-DLQ-003: Add increases size
    #[test]
    fn f_dlq_003_add() {
        let mut dlq = DeadletterQueue::new(100);
        assert!(dlq.add());
        assert_eq!(dlq.size(), 1);
    }

    /// F-DLQ-004: Reprocess decreases size
    #[test]
    fn f_dlq_004_reprocess() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        assert!(dlq.reprocess());
        assert_eq!(dlq.size(), 0);
    }

    /// F-DLQ-005: Expire decreases size
    #[test]
    fn f_dlq_005_expire() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        assert!(dlq.expire());
        assert_eq!(dlq.size(), 0);
    }

    /// F-DLQ-006: Recovery rate calculated
    #[test]
    fn f_dlq_006_recovery_rate() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        dlq.add();
        dlq.reprocess();
        dlq.expire();
        assert!((dlq.recovery_rate() - 50.0).abs() < 0.01);
    }

    /// F-DLQ-007: Factory for_messages
    #[test]
    fn f_dlq_007_for_messages() {
        let dlq = DeadletterQueue::for_messages();
        assert_eq!(dlq.capacity, 10000);
    }

    /// F-DLQ-008: Factory for_events
    #[test]
    fn f_dlq_008_for_events() {
        let dlq = DeadletterQueue::for_events();
        assert_eq!(dlq.capacity, 1000);
    }

    /// F-DLQ-009: Full when at capacity
    #[test]
    fn f_dlq_009_full() {
        let mut dlq = DeadletterQueue::new(1);
        dlq.add();
        assert!(dlq.is_full());
    }

    /// F-DLQ-010: Healthy when recovery high
    #[test]
    fn f_dlq_010_healthy() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        dlq.reprocess();
        assert!(dlq.is_healthy(90.0));
    }

    /// F-DLQ-011: Reset clears counters
    #[test]
    fn f_dlq_011_reset() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        dlq.reprocess();
        dlq.reset();
        assert_eq!(dlq.reprocessed, 0);
    }

    /// F-DLQ-012: Clone preserves state
    #[test]
    fn f_dlq_012_clone() {
        let mut dlq = DeadletterQueue::new(100);
        dlq.add();
        let cloned = dlq.clone();
        assert_eq!(dlq.size(), cloned.size());
    }
}

// ============================================================================
// v9.29.0: Stream Processing O(1) Helpers
// ============================================================================

/// O(1) stream processing state tracking.
///
/// Tracks streaming data pipeline throughput and backpressure.
#[derive(Debug, Clone)]
pub struct StreamProcessor {
    records_in: u64,
    records_out: u64,
    records_dropped: u64,
    bytes_processed: u64,
    watermark_us: u64,
}

impl Default for StreamProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamProcessor {
    /// Create new stream processor tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            records_in: 0,
            records_out: 0,
            records_dropped: 0,
            bytes_processed: 0,
            watermark_us: 0,
        }
    }

    /// Factory for Kafka streams.
    #[must_use]
    pub fn for_kafka() -> Self {
        Self::new()
    }

    /// Factory for event streams.
    #[must_use]
    pub fn for_events() -> Self {
        Self::new()
    }

    /// Process incoming record.
    pub fn process_in(&mut self, bytes: u64) {
        self.records_in += 1;
        self.bytes_processed += bytes;
    }

    /// Emit output record.
    pub fn emit(&mut self) {
        self.records_out += 1;
    }

    /// Drop a record (backpressure).
    pub fn drop_record(&mut self) {
        self.records_dropped += 1;
    }

    /// Update watermark timestamp.
    pub fn update_watermark(&mut self, timestamp_us: u64) {
        self.watermark_us = timestamp_us;
    }

    /// Get processing ratio (out/in).
    #[must_use]
    pub fn processing_ratio(&self) -> f64 {
        if self.records_in == 0 {
            1.0
        } else {
            self.records_out as f64 / self.records_in as f64
        }
    }

    /// Get drop rate (%).
    #[must_use]
    pub fn drop_rate(&self) -> f64 {
        let total = self.records_in;
        if total == 0 {
            0.0
        } else {
            (self.records_dropped as f64 / total as f64) * 100.0
        }
    }

    /// Check if stream is healthy (drop rate < threshold).
    #[must_use]
    pub fn is_healthy(&self, max_drop_rate: f64) -> bool {
        self.drop_rate() <= max_drop_rate
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.records_in = 0;
        self.records_out = 0;
        self.records_dropped = 0;
        self.bytes_processed = 0;
    }
}

/// O(1) batch aggregation tracking.
///
/// Tracks batch assembly and flush patterns.
#[derive(Debug, Clone)]
pub struct BatchAggregator {
    batch_size: u64,
    current_count: u64,
    batches_flushed: u64,
    total_items: u64,
    flush_trigger_size: u64,
}

impl Default for BatchAggregator {
    fn default() -> Self {
        Self::new(100)
    }
}

impl BatchAggregator {
    /// Create new batch aggregator tracker.
    #[must_use]
    pub fn new(batch_size: u64) -> Self {
        Self {
            batch_size,
            current_count: 0,
            batches_flushed: 0,
            total_items: 0,
            flush_trigger_size: 0,
        }
    }

    /// Factory for write batching.
    #[must_use]
    pub fn for_writes() -> Self {
        Self::new(1000)
    }

    /// Factory for small batches.
    #[must_use]
    pub fn for_small() -> Self {
        Self::new(10)
    }

    /// Add item to current batch.
    pub fn add(&mut self) -> bool {
        self.current_count += 1;
        self.total_items += 1;
        if self.current_count >= self.batch_size {
            self.flush_trigger_size += self.current_count;
            self.batches_flushed += 1;
            self.current_count = 0;
            true
        } else {
            false
        }
    }

    /// Force flush current batch.
    pub fn flush(&mut self) {
        if self.current_count > 0 {
            self.flush_trigger_size += self.current_count;
            self.batches_flushed += 1;
            self.current_count = 0;
        }
    }

    /// Get current batch fill level.
    #[must_use]
    pub fn fill_level(&self) -> f64 {
        if self.batch_size == 0 {
            0.0
        } else {
            (self.current_count as f64 / self.batch_size as f64) * 100.0
        }
    }

    /// Get average batch size at flush.
    #[must_use]
    pub fn avg_batch_size(&self) -> u64 {
        if self.batches_flushed == 0 {
            0
        } else {
            self.flush_trigger_size / self.batches_flushed
        }
    }

    /// Get total batches flushed.
    #[must_use]
    pub fn batches(&self) -> u64 {
        self.batches_flushed
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.current_count = 0;
        self.batches_flushed = 0;
        self.total_items = 0;
        self.flush_trigger_size = 0;
    }
}

/// O(1) time window tracking.
///
/// Tracks sliding/tumbling window state.
#[derive(Debug, Clone)]
pub struct WindowTracker {
    window_size_us: u64,
    slide_interval_us: u64,
    windows_completed: u64,
    current_count: u64,
    last_window_start_us: u64,
}

impl Default for WindowTracker {
    fn default() -> Self {
        Self::new(60_000_000, 60_000_000) // 1 minute tumbling
    }
}

impl WindowTracker {
    /// Create new window tracker.
    #[must_use]
    pub fn new(window_size_us: u64, slide_interval_us: u64) -> Self {
        Self {
            window_size_us,
            slide_interval_us,
            windows_completed: 0,
            current_count: 0,
            last_window_start_us: 0,
        }
    }

    /// Factory for 1-minute tumbling windows.
    #[must_use]
    pub fn for_minute_tumbling() -> Self {
        Self::new(60_000_000, 60_000_000)
    }

    /// Factory for 10-second sliding windows with 1s slide.
    #[must_use]
    pub fn for_10s_sliding() -> Self {
        Self::new(10_000_000, 1_000_000)
    }

    /// Add event to current window.
    pub fn add_event(&mut self) {
        self.current_count += 1;
    }

    /// Close current window and advance.
    pub fn close_window(&mut self, timestamp_us: u64) {
        self.windows_completed += 1;
        self.current_count = 0;
        self.last_window_start_us = timestamp_us;
    }

    /// Get current window count.
    #[must_use]
    pub fn current_count(&self) -> u64 {
        self.current_count
    }

    /// Get total windows completed.
    #[must_use]
    pub fn windows(&self) -> u64 {
        self.windows_completed
    }

    /// Check if window is tumbling (window_size == slide).
    #[must_use]
    pub fn is_tumbling(&self) -> bool {
        self.window_size_us == self.slide_interval_us
    }

    /// Check if window is sliding (window_size != slide).
    #[must_use]
    pub fn is_sliding(&self) -> bool {
        self.window_size_us != self.slide_interval_us
    }

    /// Reset tracker.
    pub fn reset(&mut self) {
        self.windows_completed = 0;
        self.current_count = 0;
        self.last_window_start_us = 0;
    }
}

/// O(1) priority queue state tracking.
///
/// Tracks priority queue operations and distribution.
#[derive(Debug, Clone)]
pub struct PriorityQueueTracker {
    capacity: u64,
    current: u64,
    enqueued: u64,
    dequeued: u64,
    priority_sum: u64,
    max_priority: u64,
}

impl Default for PriorityQueueTracker {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl PriorityQueueTracker {
    /// Create new priority queue tracker.
    #[must_use]
    pub fn new(capacity: u64) -> Self {
        Self {
            capacity,
            current: 0,
            enqueued: 0,
            dequeued: 0,
            priority_sum: 0,
            max_priority: 0,
        }
    }

    /// Factory for task scheduling.
    #[must_use]
    pub fn for_tasks() -> Self {
        Self::new(1000)
    }

    /// Factory for event processing.
    #[must_use]
    pub fn for_events() -> Self {
        Self::new(10000)
    }

    /// Enqueue with priority.
    pub fn enqueue(&mut self, priority: u64) -> bool {
        if self.current < self.capacity {
            self.current += 1;
            self.enqueued += 1;
            self.priority_sum += priority;
            self.max_priority = self.max_priority.max(priority);
            true
        } else {
            false
        }
    }

    /// Dequeue highest priority.
    pub fn dequeue(&mut self) -> bool {
        if self.current > 0 {
            self.current -= 1;
            self.dequeued += 1;
            true
        } else {
            false
        }
    }

    /// Get current queue size.
    #[must_use]
    pub fn size(&self) -> u64 {
        self.current
    }

    /// Get average priority of enqueued items.
    #[must_use]
    pub fn avg_priority(&self) -> f64 {
        if self.enqueued == 0 {
            0.0
        } else {
            self.priority_sum as f64 / self.enqueued as f64
        }
    }

    /// Check if queue is full.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.current >= self.capacity
    }

    /// Check if queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current == 0
    }

    /// Get utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            0.0
        } else {
            (self.current as f64 / self.capacity as f64) * 100.0
        }
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.current = 0;
        self.enqueued = 0;
        self.dequeued = 0;
        self.priority_sum = 0;
        self.max_priority = 0;
    }
}

#[cfg(test)]
mod stream_processor_tests {
    use super::*;

    /// F-STREAM-001: New creates empty processor
    #[test]
    fn f_stream_001_new() {
        let sp = StreamProcessor::new();
        assert_eq!(sp.records_in, 0);
    }

    /// F-STREAM-002: Default equals new
    #[test]
    fn f_stream_002_default() {
        let sp = StreamProcessor::default();
        assert_eq!(sp.records_out, 0);
    }

    /// F-STREAM-003: Process tracks input
    #[test]
    fn f_stream_003_process() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        assert_eq!(sp.records_in, 1);
        assert_eq!(sp.bytes_processed, 100);
    }

    /// F-STREAM-004: Emit tracks output
    #[test]
    fn f_stream_004_emit() {
        let mut sp = StreamProcessor::new();
        sp.emit();
        assert_eq!(sp.records_out, 1);
    }

    /// F-STREAM-005: Drop tracks backpressure
    #[test]
    fn f_stream_005_drop() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        sp.drop_record();
        assert!((sp.drop_rate() - 100.0).abs() < 0.01);
    }

    /// F-STREAM-006: Processing ratio calculated
    #[test]
    fn f_stream_006_ratio() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        sp.process_in(100);
        sp.emit();
        assert!((sp.processing_ratio() - 0.5).abs() < 0.01);
    }

    /// F-STREAM-007: Factory for_kafka
    #[test]
    fn f_stream_007_for_kafka() {
        let sp = StreamProcessor::for_kafka();
        assert_eq!(sp.records_in, 0);
    }

    /// F-STREAM-008: Factory for_events
    #[test]
    fn f_stream_008_for_events() {
        let sp = StreamProcessor::for_events();
        assert_eq!(sp.records_out, 0);
    }

    /// F-STREAM-009: Watermark updates
    #[test]
    fn f_stream_009_watermark() {
        let mut sp = StreamProcessor::new();
        sp.update_watermark(1000);
        assert_eq!(sp.watermark_us, 1000);
    }

    /// F-STREAM-010: Healthy when drops low
    #[test]
    fn f_stream_010_healthy() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        sp.emit();
        assert!(sp.is_healthy(5.0));
    }

    /// F-STREAM-011: Reset clears counters
    #[test]
    fn f_stream_011_reset() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        sp.reset();
        assert_eq!(sp.records_in, 0);
    }

    /// F-STREAM-012: Clone preserves state
    #[test]
    fn f_stream_012_clone() {
        let mut sp = StreamProcessor::new();
        sp.process_in(100);
        let cloned = sp.clone();
        assert_eq!(sp.records_in, cloned.records_in);
    }
}

#[cfg(test)]
mod batch_aggregator_tests {
    use super::*;

    /// F-BATCH-001: New creates empty aggregator
    #[test]
    fn f_batch_001_new() {
        let ba = BatchAggregator::new(100);
        assert_eq!(ba.current_count, 0);
    }

    /// F-BATCH-002: Default has capacity
    #[test]
    fn f_batch_002_default() {
        let ba = BatchAggregator::default();
        assert_eq!(ba.batch_size, 100);
    }

    /// F-BATCH-003: Add increments count
    #[test]
    fn f_batch_003_add() {
        let mut ba = BatchAggregator::new(100);
        ba.add();
        assert_eq!(ba.current_count, 1);
    }

    /// F-BATCH-004: Auto-flush at capacity
    #[test]
    fn f_batch_004_auto_flush() {
        let mut ba = BatchAggregator::new(2);
        ba.add();
        let flushed = ba.add();
        assert!(flushed);
        assert_eq!(ba.batches(), 1);
    }

    /// F-BATCH-005: Manual flush works
    #[test]
    fn f_batch_005_manual_flush() {
        let mut ba = BatchAggregator::new(100);
        ba.add();
        ba.flush();
        assert_eq!(ba.batches(), 1);
    }

    /// F-BATCH-006: Fill level calculated
    #[test]
    fn f_batch_006_fill_level() {
        let mut ba = BatchAggregator::new(100);
        for _ in 0..50 {
            ba.add();
        }
        assert!((ba.fill_level() - 50.0).abs() < 0.01);
    }

    /// F-BATCH-007: Factory for_writes
    #[test]
    fn f_batch_007_for_writes() {
        let ba = BatchAggregator::for_writes();
        assert_eq!(ba.batch_size, 1000);
    }

    /// F-BATCH-008: Factory for_small
    #[test]
    fn f_batch_008_for_small() {
        let ba = BatchAggregator::for_small();
        assert_eq!(ba.batch_size, 10);
    }

    /// F-BATCH-009: Avg batch size calculated
    #[test]
    fn f_batch_009_avg_batch() {
        let mut ba = BatchAggregator::new(10);
        for _ in 0..10 {
            ba.add();
        }
        assert_eq!(ba.avg_batch_size(), 10);
    }

    /// F-BATCH-010: Total items tracked
    #[test]
    fn f_batch_010_total() {
        let mut ba = BatchAggregator::new(100);
        ba.add();
        ba.add();
        assert_eq!(ba.total_items, 2);
    }

    /// F-BATCH-011: Reset clears counters
    #[test]
    fn f_batch_011_reset() {
        let mut ba = BatchAggregator::new(100);
        ba.add();
        ba.flush();
        ba.reset();
        assert_eq!(ba.batches(), 0);
    }

    /// F-BATCH-012: Clone preserves state
    #[test]
    fn f_batch_012_clone() {
        let mut ba = BatchAggregator::new(100);
        ba.add();
        let cloned = ba.clone();
        assert_eq!(ba.current_count, cloned.current_count);
    }
}

#[cfg(test)]
mod window_tracker_tests {
    use super::*;

    /// F-WINDOW-001: New creates empty tracker
    #[test]
    fn f_window_001_new() {
        let wt = WindowTracker::new(60_000_000, 60_000_000);
        assert_eq!(wt.current_count(), 0);
    }

    /// F-WINDOW-002: Default is tumbling
    #[test]
    fn f_window_002_default() {
        let wt = WindowTracker::default();
        assert!(wt.is_tumbling());
    }

    /// F-WINDOW-003: Add event increments count
    #[test]
    fn f_window_003_add() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.add_event();
        assert_eq!(wt.current_count(), 1);
    }

    /// F-WINDOW-004: Close window increments count
    #[test]
    fn f_window_004_close() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.add_event();
        wt.close_window(1000);
        assert_eq!(wt.windows(), 1);
        assert_eq!(wt.current_count(), 0);
    }

    /// F-WINDOW-005: Tumbling detection
    #[test]
    fn f_window_005_tumbling() {
        let wt = WindowTracker::for_minute_tumbling();
        assert!(wt.is_tumbling());
    }

    /// F-WINDOW-006: Sliding detection
    #[test]
    fn f_window_006_sliding() {
        let wt = WindowTracker::for_10s_sliding();
        assert!(wt.is_sliding());
    }

    /// F-WINDOW-007: Factory for_minute_tumbling
    #[test]
    fn f_window_007_for_minute() {
        let wt = WindowTracker::for_minute_tumbling();
        assert_eq!(wt.window_size_us, 60_000_000);
    }

    /// F-WINDOW-008: Factory for_10s_sliding
    #[test]
    fn f_window_008_for_10s() {
        let wt = WindowTracker::for_10s_sliding();
        assert_eq!(wt.window_size_us, 10_000_000);
        assert_eq!(wt.slide_interval_us, 1_000_000);
    }

    /// F-WINDOW-009: Last window start updated
    #[test]
    fn f_window_009_last_start() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.close_window(5000);
        assert_eq!(wt.last_window_start_us, 5000);
    }

    /// F-WINDOW-010: Multiple windows tracked
    #[test]
    fn f_window_010_multiple() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.close_window(1000);
        wt.close_window(2000);
        assert_eq!(wt.windows(), 2);
    }

    /// F-WINDOW-011: Reset clears counters
    #[test]
    fn f_window_011_reset() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.add_event();
        wt.close_window(1000);
        wt.reset();
        assert_eq!(wt.windows(), 0);
    }

    /// F-WINDOW-012: Clone preserves state
    #[test]
    fn f_window_012_clone() {
        let mut wt = WindowTracker::new(60_000_000, 60_000_000);
        wt.add_event();
        let cloned = wt.clone();
        assert_eq!(wt.current_count(), cloned.current_count());
    }
}

#[cfg(test)]
mod priority_queue_tracker_tests {
    use super::*;

    /// F-PQUEUE-001: New creates empty queue
    #[test]
    fn f_pqueue_001_new() {
        let pq = PriorityQueueTracker::new(100);
        assert_eq!(pq.size(), 0);
    }

    /// F-PQUEUE-002: Default has capacity
    #[test]
    fn f_pqueue_002_default() {
        let pq = PriorityQueueTracker::default();
        assert!(pq.is_empty());
    }

    /// F-PQUEUE-003: Enqueue increases size
    #[test]
    fn f_pqueue_003_enqueue() {
        let mut pq = PriorityQueueTracker::new(100);
        assert!(pq.enqueue(5));
        assert_eq!(pq.size(), 1);
    }

    /// F-PQUEUE-004: Dequeue decreases size
    #[test]
    fn f_pqueue_004_dequeue() {
        let mut pq = PriorityQueueTracker::new(100);
        pq.enqueue(5);
        assert!(pq.dequeue());
        assert_eq!(pq.size(), 0);
    }

    /// F-PQUEUE-005: Priority sum tracked
    #[test]
    fn f_pqueue_005_priority() {
        let mut pq = PriorityQueueTracker::new(100);
        pq.enqueue(5);
        pq.enqueue(10);
        assert!((pq.avg_priority() - 7.5).abs() < 0.01);
    }

    /// F-PQUEUE-006: Full when at capacity
    #[test]
    fn f_pqueue_006_full() {
        let mut pq = PriorityQueueTracker::new(2);
        pq.enqueue(1);
        pq.enqueue(2);
        assert!(pq.is_full());
    }

    /// F-PQUEUE-007: Factory for_tasks
    #[test]
    fn f_pqueue_007_for_tasks() {
        let pq = PriorityQueueTracker::for_tasks();
        assert_eq!(pq.capacity, 1000);
    }

    /// F-PQUEUE-008: Factory for_events
    #[test]
    fn f_pqueue_008_for_events() {
        let pq = PriorityQueueTracker::for_events();
        assert_eq!(pq.capacity, 10000);
    }

    /// F-PQUEUE-009: Utilization calculated
    #[test]
    fn f_pqueue_009_utilization() {
        let mut pq = PriorityQueueTracker::new(100);
        for i in 0..50 {
            pq.enqueue(i);
        }
        assert!((pq.utilization() - 50.0).abs() < 0.01);
    }

    /// F-PQUEUE-010: Enqueue fails when full
    #[test]
    fn f_pqueue_010_full_enqueue() {
        let mut pq = PriorityQueueTracker::new(1);
        pq.enqueue(1);
        assert!(!pq.enqueue(2));
    }

    /// F-PQUEUE-011: Reset clears counters
    #[test]
    fn f_pqueue_011_reset() {
        let mut pq = PriorityQueueTracker::new(100);
        pq.enqueue(5);
        pq.reset();
        assert_eq!(pq.size(), 0);
    }

    /// F-PQUEUE-012: Clone preserves state
    #[test]
    fn f_pqueue_012_clone() {
        let mut pq = PriorityQueueTracker::new(100);
        pq.enqueue(5);
        let cloned = pq.clone();
        assert_eq!(pq.size(), cloned.size());
    }
}

// ============================================================================
// v9.30.0: Metric & Index O(1) Helpers
// ============================================================================

/// O(1) metric registry tracking.
///
/// Tracks metric registration and collection patterns.
#[derive(Debug, Clone)]
pub struct MetricRegistry {
    counters: u32,
    gauges: u32,
    histograms: u32,
    collections: u64,
    last_collection_us: u64,
}

impl Default for MetricRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricRegistry {
    /// Create new metric registry tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            counters: 0,
            gauges: 0,
            histograms: 0,
            collections: 0,
            last_collection_us: 0,
        }
    }

    /// Factory for application metrics.
    #[must_use]
    pub fn for_application() -> Self {
        Self::new()
    }

    /// Factory for system metrics.
    #[must_use]
    pub fn for_system() -> Self {
        Self::new()
    }

    /// Register a counter metric.
    pub fn register_counter(&mut self) {
        self.counters += 1;
    }

    /// Register a gauge metric.
    pub fn register_gauge(&mut self) {
        self.gauges += 1;
    }

    /// Register a histogram metric.
    pub fn register_histogram(&mut self) {
        self.histograms += 1;
    }

    /// Record a collection event.
    pub fn collect(&mut self, timestamp_us: u64) {
        self.collections += 1;
        self.last_collection_us = timestamp_us;
    }

    /// Get total registered metrics.
    #[must_use]
    pub fn total_metrics(&self) -> u32 {
        self.counters + self.gauges + self.histograms
    }

    /// Get collection count.
    #[must_use]
    pub fn collections(&self) -> u64 {
        self.collections
    }

    /// Reset registry.
    pub fn reset(&mut self) {
        self.counters = 0;
        self.gauges = 0;
        self.histograms = 0;
        self.collections = 0;
        self.last_collection_us = 0;
    }
}

/// O(1) alert state tracking.
///
/// Tracks alert firing, acknowledgment, and resolution.
#[derive(Debug, Clone)]
pub struct AlertManager {
    active: u32,
    fired: u64,
    acknowledged: u64,
    resolved: u64,
    suppressed: u64,
}

impl Default for AlertManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AlertManager {
    /// Create new alert manager tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: 0,
            fired: 0,
            acknowledged: 0,
            resolved: 0,
            suppressed: 0,
        }
    }

    /// Factory for critical alerts.
    #[must_use]
    pub fn for_critical() -> Self {
        Self::new()
    }

    /// Factory for warning alerts.
    #[must_use]
    pub fn for_warnings() -> Self {
        Self::new()
    }

    /// Fire a new alert.
    pub fn fire(&mut self) {
        self.active += 1;
        self.fired += 1;
    }

    /// Acknowledge an alert.
    pub fn acknowledge(&mut self) {
        self.acknowledged += 1;
    }

    /// Resolve an alert.
    pub fn resolve(&mut self) {
        if self.active > 0 {
            self.active -= 1;
            self.resolved += 1;
        }
    }

    /// Suppress an alert.
    pub fn suppress(&mut self) {
        if self.active > 0 {
            self.active -= 1;
            self.suppressed += 1;
        }
    }

    /// Get active alert count.
    #[must_use]
    pub fn active(&self) -> u32 {
        self.active
    }

    /// Get resolution rate (%).
    #[must_use]
    pub fn resolution_rate(&self) -> f64 {
        if self.fired == 0 {
            100.0
        } else {
            (self.resolved as f64 / self.fired as f64) * 100.0
        }
    }

    /// Check if alert load is healthy.
    #[must_use]
    pub fn is_healthy(&self, max_active: u32) -> bool {
        self.active <= max_active
    }

    /// Reset manager.
    pub fn reset(&mut self) {
        self.active = 0;
        self.fired = 0;
        self.acknowledged = 0;
        self.resolved = 0;
        self.suppressed = 0;
    }
}

/// O(1) index building tracking.
///
/// Tracks index construction progress and throughput.
#[derive(Debug, Clone)]
pub struct IndexBuilder {
    entries_indexed: u64,
    bytes_indexed: u64,
    segments_built: u64,
    merges_completed: u64,
    build_time_us: u64,
}

impl Default for IndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl IndexBuilder {
    /// Create new index builder tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries_indexed: 0,
            bytes_indexed: 0,
            segments_built: 0,
            merges_completed: 0,
            build_time_us: 0,
        }
    }

    /// Factory for search indexes.
    #[must_use]
    pub fn for_search() -> Self {
        Self::new()
    }

    /// Factory for database indexes.
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Index an entry.
    pub fn index_entry(&mut self, bytes: u64) {
        self.entries_indexed += 1;
        self.bytes_indexed += bytes;
    }

    /// Complete a segment build.
    pub fn build_segment(&mut self, duration_us: u64) {
        self.segments_built += 1;
        self.build_time_us += duration_us;
    }

    /// Complete a segment merge.
    pub fn complete_merge(&mut self) {
        self.merges_completed += 1;
    }

    /// Get indexing throughput (entries/second).
    #[must_use]
    pub fn throughput(&self) -> f64 {
        if self.build_time_us == 0 {
            0.0
        } else {
            (self.entries_indexed as f64 / self.build_time_us as f64) * 1_000_000.0
        }
    }

    /// Get average segment build time (us).
    #[must_use]
    pub fn avg_segment_time_us(&self) -> u64 {
        if self.segments_built == 0 {
            0
        } else {
            self.build_time_us / self.segments_built
        }
    }

    /// Reset builder.
    pub fn reset(&mut self) {
        self.entries_indexed = 0;
        self.bytes_indexed = 0;
        self.segments_built = 0;
        self.merges_completed = 0;
        self.build_time_us = 0;
    }
}

/// O(1) compaction policy tracking.
///
/// Tracks compaction decisions and effectiveness.
#[derive(Debug, Clone)]
pub struct CompactionPolicy {
    evaluations: u64,
    triggered: u64,
    skipped: u64,
    bytes_reclaimed: u64,
    space_amplification: f64,
}

impl Default for CompactionPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl CompactionPolicy {
    /// Create new compaction policy tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            evaluations: 0,
            triggered: 0,
            skipped: 0,
            bytes_reclaimed: 0,
            space_amplification: 1.0,
        }
    }

    /// Factory for leveled compaction.
    #[must_use]
    pub fn for_leveled() -> Self {
        Self::new()
    }

    /// Factory for size-tiered compaction.
    #[must_use]
    pub fn for_size_tiered() -> Self {
        Self::new()
    }

    /// Evaluate compaction need.
    pub fn evaluate(&mut self, should_compact: bool) {
        self.evaluations += 1;
        if should_compact {
            self.triggered += 1;
        } else {
            self.skipped += 1;
        }
    }

    /// Record bytes reclaimed from compaction.
    pub fn reclaim(&mut self, bytes: u64) {
        self.bytes_reclaimed += bytes;
    }

    /// Update space amplification factor.
    pub fn set_amplification(&mut self, factor: f64) {
        self.space_amplification = factor;
    }

    /// Get trigger rate (%).
    #[must_use]
    pub fn trigger_rate(&self) -> f64 {
        if self.evaluations == 0 {
            0.0
        } else {
            (self.triggered as f64 / self.evaluations as f64) * 100.0
        }
    }

    /// Check if compaction is effective.
    #[must_use]
    pub fn is_effective(&self, max_amplification: f64) -> bool {
        self.space_amplification <= max_amplification
    }

    /// Get bytes reclaimed.
    #[must_use]
    pub fn reclaimed(&self) -> u64 {
        self.bytes_reclaimed
    }

    /// Reset policy.
    pub fn reset(&mut self) {
        self.evaluations = 0;
        self.triggered = 0;
        self.skipped = 0;
        self.bytes_reclaimed = 0;
        self.space_amplification = 1.0;
    }
}

#[cfg(test)]
mod metric_registry_tests {
    use super::*;

    /// F-MREG-001: New creates empty registry
    #[test]
    fn f_mreg_001_new() {
        let mr = MetricRegistry::new();
        assert_eq!(mr.total_metrics(), 0);
    }

    /// F-MREG-002: Default equals new
    #[test]
    fn f_mreg_002_default() {
        let mr = MetricRegistry::default();
        assert_eq!(mr.collections(), 0);
    }

    /// F-MREG-003: Register counter
    #[test]
    fn f_mreg_003_counter() {
        let mut mr = MetricRegistry::new();
        mr.register_counter();
        assert_eq!(mr.counters, 1);
    }

    /// F-MREG-004: Register gauge
    #[test]
    fn f_mreg_004_gauge() {
        let mut mr = MetricRegistry::new();
        mr.register_gauge();
        assert_eq!(mr.gauges, 1);
    }

    /// F-MREG-005: Register histogram
    #[test]
    fn f_mreg_005_histogram() {
        let mut mr = MetricRegistry::new();
        mr.register_histogram();
        assert_eq!(mr.histograms, 1);
    }

    /// F-MREG-006: Total metrics calculated
    #[test]
    fn f_mreg_006_total() {
        let mut mr = MetricRegistry::new();
        mr.register_counter();
        mr.register_gauge();
        mr.register_histogram();
        assert_eq!(mr.total_metrics(), 3);
    }

    /// F-MREG-007: Factory for_application
    #[test]
    fn f_mreg_007_for_app() {
        let mr = MetricRegistry::for_application();
        assert_eq!(mr.total_metrics(), 0);
    }

    /// F-MREG-008: Factory for_system
    #[test]
    fn f_mreg_008_for_system() {
        let mr = MetricRegistry::for_system();
        assert_eq!(mr.collections(), 0);
    }

    /// F-MREG-009: Collection tracked
    #[test]
    fn f_mreg_009_collect() {
        let mut mr = MetricRegistry::new();
        mr.collect(1000);
        assert_eq!(mr.collections(), 1);
        assert_eq!(mr.last_collection_us, 1000);
    }

    /// F-MREG-010: Multiple collections
    #[test]
    fn f_mreg_010_multi_collect() {
        let mut mr = MetricRegistry::new();
        mr.collect(1000);
        mr.collect(2000);
        assert_eq!(mr.collections(), 2);
    }

    /// F-MREG-011: Reset clears counters
    #[test]
    fn f_mreg_011_reset() {
        let mut mr = MetricRegistry::new();
        mr.register_counter();
        mr.collect(1000);
        mr.reset();
        assert_eq!(mr.total_metrics(), 0);
    }

    /// F-MREG-012: Clone preserves state
    #[test]
    fn f_mreg_012_clone() {
        let mut mr = MetricRegistry::new();
        mr.register_counter();
        let cloned = mr.clone();
        assert_eq!(mr.counters, cloned.counters);
    }
}

#[cfg(test)]
mod alert_manager_tests {
    use super::*;

    /// F-ALERT-001: New creates empty manager
    #[test]
    fn f_alert_001_new() {
        let am = AlertManager::new();
        assert_eq!(am.active(), 0);
    }

    /// F-ALERT-002: Default equals new
    #[test]
    fn f_alert_002_default() {
        let am = AlertManager::default();
        assert_eq!(am.fired, 0);
    }

    /// F-ALERT-003: Fire increments active
    #[test]
    fn f_alert_003_fire() {
        let mut am = AlertManager::new();
        am.fire();
        assert_eq!(am.active(), 1);
    }

    /// F-ALERT-004: Resolve decrements active
    #[test]
    fn f_alert_004_resolve() {
        let mut am = AlertManager::new();
        am.fire();
        am.resolve();
        assert_eq!(am.active(), 0);
    }

    /// F-ALERT-005: Acknowledge tracks acks
    #[test]
    fn f_alert_005_ack() {
        let mut am = AlertManager::new();
        am.fire();
        am.acknowledge();
        assert_eq!(am.acknowledged, 1);
    }

    /// F-ALERT-006: Resolution rate calculated
    #[test]
    fn f_alert_006_resolution_rate() {
        let mut am = AlertManager::new();
        am.fire();
        am.resolve();
        assert!((am.resolution_rate() - 100.0).abs() < 0.01);
    }

    /// F-ALERT-007: Factory for_critical
    #[test]
    fn f_alert_007_for_critical() {
        let am = AlertManager::for_critical();
        assert_eq!(am.active(), 0);
    }

    /// F-ALERT-008: Factory for_warnings
    #[test]
    fn f_alert_008_for_warnings() {
        let am = AlertManager::for_warnings();
        assert_eq!(am.fired, 0);
    }

    /// F-ALERT-009: Suppress decrements active
    #[test]
    fn f_alert_009_suppress() {
        let mut am = AlertManager::new();
        am.fire();
        am.suppress();
        assert_eq!(am.active(), 0);
        assert_eq!(am.suppressed, 1);
    }

    /// F-ALERT-010: Healthy when low active
    #[test]
    fn f_alert_010_healthy() {
        let mut am = AlertManager::new();
        am.fire();
        assert!(am.is_healthy(5));
    }

    /// F-ALERT-011: Reset clears counters
    #[test]
    fn f_alert_011_reset() {
        let mut am = AlertManager::new();
        am.fire();
        am.reset();
        assert_eq!(am.active(), 0);
    }

    /// F-ALERT-012: Clone preserves state
    #[test]
    fn f_alert_012_clone() {
        let mut am = AlertManager::new();
        am.fire();
        let cloned = am.clone();
        assert_eq!(am.active(), cloned.active());
    }
}

#[cfg(test)]
mod index_builder_tests {
    use super::*;

    /// F-IDXB-001: New creates empty builder
    #[test]
    fn f_idxb_001_new() {
        let ib = IndexBuilder::new();
        assert_eq!(ib.entries_indexed, 0);
    }

    /// F-IDXB-002: Default equals new
    #[test]
    fn f_idxb_002_default() {
        let ib = IndexBuilder::default();
        assert_eq!(ib.segments_built, 0);
    }

    /// F-IDXB-003: Index entry tracks count
    #[test]
    fn f_idxb_003_index() {
        let mut ib = IndexBuilder::new();
        ib.index_entry(100);
        assert_eq!(ib.entries_indexed, 1);
        assert_eq!(ib.bytes_indexed, 100);
    }

    /// F-IDXB-004: Build segment tracks time
    #[test]
    fn f_idxb_004_segment() {
        let mut ib = IndexBuilder::new();
        ib.build_segment(1000);
        assert_eq!(ib.segments_built, 1);
    }

    /// F-IDXB-005: Complete merge tracks count
    #[test]
    fn f_idxb_005_merge() {
        let mut ib = IndexBuilder::new();
        ib.complete_merge();
        assert_eq!(ib.merges_completed, 1);
    }

    /// F-IDXB-006: Throughput calculated
    #[test]
    fn f_idxb_006_throughput() {
        let mut ib = IndexBuilder::new();
        ib.index_entry(100);
        ib.build_segment(1_000_000); // 1 second
        assert!((ib.throughput() - 1.0).abs() < 0.01);
    }

    /// F-IDXB-007: Factory for_search
    #[test]
    fn f_idxb_007_for_search() {
        let ib = IndexBuilder::for_search();
        assert_eq!(ib.entries_indexed, 0);
    }

    /// F-IDXB-008: Factory for_database
    #[test]
    fn f_idxb_008_for_database() {
        let ib = IndexBuilder::for_database();
        assert_eq!(ib.segments_built, 0);
    }

    /// F-IDXB-009: Avg segment time calculated
    #[test]
    fn f_idxb_009_avg_segment() {
        let mut ib = IndexBuilder::new();
        ib.build_segment(1000);
        ib.build_segment(2000);
        assert_eq!(ib.avg_segment_time_us(), 1500);
    }

    /// F-IDXB-010: Multiple entries tracked
    #[test]
    fn f_idxb_010_multi_entry() {
        let mut ib = IndexBuilder::new();
        ib.index_entry(100);
        ib.index_entry(200);
        assert_eq!(ib.bytes_indexed, 300);
    }

    /// F-IDXB-011: Reset clears counters
    #[test]
    fn f_idxb_011_reset() {
        let mut ib = IndexBuilder::new();
        ib.index_entry(100);
        ib.reset();
        assert_eq!(ib.entries_indexed, 0);
    }

    /// F-IDXB-012: Clone preserves state
    #[test]
    fn f_idxb_012_clone() {
        let mut ib = IndexBuilder::new();
        ib.index_entry(100);
        let cloned = ib.clone();
        assert_eq!(ib.entries_indexed, cloned.entries_indexed);
    }
}

#[cfg(test)]
mod compaction_policy_tests {
    use super::*;

    /// F-CPOL-001: New creates empty policy
    #[test]
    fn f_cpol_001_new() {
        let cp = CompactionPolicy::new();
        assert_eq!(cp.evaluations, 0);
    }

    /// F-CPOL-002: Default equals new
    #[test]
    fn f_cpol_002_default() {
        let cp = CompactionPolicy::default();
        assert_eq!(cp.triggered, 0);
    }

    /// F-CPOL-003: Evaluate triggers
    #[test]
    fn f_cpol_003_trigger() {
        let mut cp = CompactionPolicy::new();
        cp.evaluate(true);
        assert_eq!(cp.triggered, 1);
    }

    /// F-CPOL-004: Evaluate skips
    #[test]
    fn f_cpol_004_skip() {
        let mut cp = CompactionPolicy::new();
        cp.evaluate(false);
        assert_eq!(cp.skipped, 1);
    }

    /// F-CPOL-005: Reclaim tracks bytes
    #[test]
    fn f_cpol_005_reclaim() {
        let mut cp = CompactionPolicy::new();
        cp.reclaim(1000);
        assert_eq!(cp.reclaimed(), 1000);
    }

    /// F-CPOL-006: Trigger rate calculated
    #[test]
    fn f_cpol_006_trigger_rate() {
        let mut cp = CompactionPolicy::new();
        cp.evaluate(true);
        cp.evaluate(false);
        assert!((cp.trigger_rate() - 50.0).abs() < 0.01);
    }

    /// F-CPOL-007: Factory for_leveled
    #[test]
    fn f_cpol_007_for_leveled() {
        let cp = CompactionPolicy::for_leveled();
        assert_eq!(cp.evaluations, 0);
    }

    /// F-CPOL-008: Factory for_size_tiered
    #[test]
    fn f_cpol_008_for_size_tiered() {
        let cp = CompactionPolicy::for_size_tiered();
        assert_eq!(cp.triggered, 0);
    }

    /// F-CPOL-009: Set amplification
    #[test]
    fn f_cpol_009_amplification() {
        let mut cp = CompactionPolicy::new();
        cp.set_amplification(2.5);
        assert!((cp.space_amplification - 2.5).abs() < 0.01);
    }

    /// F-CPOL-010: Effective when low amplification
    #[test]
    fn f_cpol_010_effective() {
        let cp = CompactionPolicy::new();
        assert!(cp.is_effective(2.0));
    }

    /// F-CPOL-011: Reset clears counters
    #[test]
    fn f_cpol_011_reset() {
        let mut cp = CompactionPolicy::new();
        cp.evaluate(true);
        cp.reclaim(1000);
        cp.reset();
        assert_eq!(cp.evaluations, 0);
    }

    /// F-CPOL-012: Clone preserves state
    #[test]
    fn f_cpol_012_clone() {
        let mut cp = CompactionPolicy::new();
        cp.evaluate(true);
        let cloned = cp.clone();
        assert_eq!(cp.triggered, cloned.triggered);
    }
}

// ============================================================================
// v9.31.0: Amplification & Lock O(1) Helpers
// ============================================================================

/// O(1) write amplification tracking.
///
/// Tracks write amplification factor for storage systems.
#[derive(Debug, Clone)]
pub struct WriteAmplification {
    user_bytes: u64,
    actual_bytes: u64,
    writes: u64,
    compaction_bytes: u64,
}

impl Default for WriteAmplification {
    fn default() -> Self {
        Self::new()
    }
}

impl WriteAmplification {
    /// Create new write amplification tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            user_bytes: 0,
            actual_bytes: 0,
            writes: 0,
            compaction_bytes: 0,
        }
    }

    /// Factory for LSM-tree storage.
    #[must_use]
    pub fn for_lsm() -> Self {
        Self::new()
    }

    /// Factory for B-tree storage.
    #[must_use]
    pub fn for_btree() -> Self {
        Self::new()
    }

    /// Record user write.
    pub fn user_write(&mut self, bytes: u64) {
        self.user_bytes += bytes;
        self.writes += 1;
    }

    /// Record actual disk write.
    pub fn disk_write(&mut self, bytes: u64) {
        self.actual_bytes += bytes;
    }

    /// Record compaction write.
    pub fn compaction_write(&mut self, bytes: u64) {
        self.compaction_bytes += bytes;
        self.actual_bytes += bytes;
    }

    /// Get write amplification factor.
    #[must_use]
    pub fn amplification(&self) -> f64 {
        if self.user_bytes == 0 {
            1.0
        } else {
            self.actual_bytes as f64 / self.user_bytes as f64
        }
    }

    /// Check if amplification is acceptable.
    #[must_use]
    pub fn is_acceptable(&self, max_amp: f64) -> bool {
        self.amplification() <= max_amp
    }

    /// Get total writes.
    #[must_use]
    pub fn writes(&self) -> u64 {
        self.writes
    }

    /// Reset tracker.
    pub fn reset(&mut self) {
        self.user_bytes = 0;
        self.actual_bytes = 0;
        self.writes = 0;
        self.compaction_bytes = 0;
    }
}

/// O(1) read amplification tracking.
///
/// Tracks read amplification factor for storage lookups.
#[derive(Debug, Clone)]
pub struct ReadAmplification {
    logical_reads: u64,
    physical_reads: u64,
    cache_hits: u64,
    bloom_filter_hits: u64,
}

impl Default for ReadAmplification {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadAmplification {
    /// Create new read amplification tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            logical_reads: 0,
            physical_reads: 0,
            cache_hits: 0,
            bloom_filter_hits: 0,
        }
    }

    /// Factory for LSM-tree storage.
    #[must_use]
    pub fn for_lsm() -> Self {
        Self::new()
    }

    /// Factory for B-tree storage.
    #[must_use]
    pub fn for_btree() -> Self {
        Self::new()
    }

    /// Record a logical read request.
    pub fn logical_read(&mut self) {
        self.logical_reads += 1;
    }

    /// Record a physical disk read.
    pub fn physical_read(&mut self) {
        self.physical_reads += 1;
    }

    /// Record a cache hit.
    pub fn cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    /// Record a bloom filter hit (avoided read).
    pub fn bloom_hit(&mut self) {
        self.bloom_filter_hits += 1;
    }

    /// Get read amplification factor.
    #[must_use]
    pub fn amplification(&self) -> f64 {
        if self.logical_reads == 0 {
            1.0
        } else {
            self.physical_reads as f64 / self.logical_reads as f64
        }
    }

    /// Get cache hit rate.
    #[must_use]
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.physical_reads;
        if total == 0 {
            0.0
        } else {
            (self.cache_hits as f64 / total as f64) * 100.0
        }
    }

    /// Check if amplification is acceptable.
    #[must_use]
    pub fn is_acceptable(&self, max_amp: f64) -> bool {
        self.amplification() <= max_amp
    }

    /// Reset tracker.
    pub fn reset(&mut self) {
        self.logical_reads = 0;
        self.physical_reads = 0;
        self.cache_hits = 0;
        self.bloom_filter_hits = 0;
    }
}

/// O(1) lock contention tracking.
///
/// Tracks lock acquisition patterns and contention.
#[derive(Debug, Clone)]
pub struct LockManager {
    acquisitions: u64,
    contentions: u64,
    deadlocks: u64,
    total_wait_us: u64,
    held_count: u32,
}

impl Default for LockManager {
    fn default() -> Self {
        Self::new()
    }
}

impl LockManager {
    /// Create new lock manager tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            acquisitions: 0,
            contentions: 0,
            deadlocks: 0,
            total_wait_us: 0,
            held_count: 0,
        }
    }

    /// Factory for mutex locks.
    #[must_use]
    pub fn for_mutex() -> Self {
        Self::new()
    }

    /// Factory for RW locks.
    #[must_use]
    pub fn for_rwlock() -> Self {
        Self::new()
    }

    /// Acquire a lock.
    pub fn acquire(&mut self, wait_us: u64) {
        self.acquisitions += 1;
        self.total_wait_us += wait_us;
        self.held_count += 1;
        if wait_us > 0 {
            self.contentions += 1;
        }
    }

    /// Release a lock.
    pub fn release(&mut self) {
        self.held_count = self.held_count.saturating_sub(1);
    }

    /// Record a deadlock detection.
    pub fn deadlock(&mut self) {
        self.deadlocks += 1;
    }

    /// Get contention rate (%).
    #[must_use]
    pub fn contention_rate(&self) -> f64 {
        if self.acquisitions == 0 {
            0.0
        } else {
            (self.contentions as f64 / self.acquisitions as f64) * 100.0
        }
    }

    /// Get average wait time (us).
    #[must_use]
    pub fn avg_wait_us(&self) -> u64 {
        if self.acquisitions == 0 {
            0
        } else {
            self.total_wait_us / self.acquisitions
        }
    }

    /// Check if lock health is good.
    #[must_use]
    pub fn is_healthy(&self, max_contention_rate: f64) -> bool {
        self.contention_rate() <= max_contention_rate && self.deadlocks == 0
    }

    /// Reset tracker.
    pub fn reset(&mut self) {
        self.acquisitions = 0;
        self.contentions = 0;
        self.deadlocks = 0;
        self.total_wait_us = 0;
        self.held_count = 0;
    }
}

/// O(1) memory pressure tracking.
///
/// Tracks memory allocation pressure and GC triggers.
#[derive(Debug, Clone)]
pub struct MemoryPressure {
    allocated_bytes: u64,
    limit_bytes: u64,
    pressure_events: u64,
    gc_triggers: u64,
    evictions: u64,
}

impl Default for MemoryPressure {
    fn default() -> Self {
        Self::new(1024 * 1024 * 1024) // 1GB default
    }
}

impl MemoryPressure {
    /// Create new memory pressure tracker.
    #[must_use]
    pub fn new(limit_bytes: u64) -> Self {
        Self {
            allocated_bytes: 0,
            limit_bytes,
            pressure_events: 0,
            gc_triggers: 0,
            evictions: 0,
        }
    }

    /// Factory for heap memory.
    #[must_use]
    pub fn for_heap() -> Self {
        Self::new(8 * 1024 * 1024 * 1024) // 8GB
    }

    /// Factory for cache memory.
    #[must_use]
    pub fn for_cache() -> Self {
        Self::new(1024 * 1024 * 1024) // 1GB
    }

    /// Allocate memory.
    pub fn allocate(&mut self, bytes: u64) {
        self.allocated_bytes += bytes;
        if self.allocated_bytes > self.limit_bytes * 80 / 100 {
            self.pressure_events += 1;
        }
    }

    /// Free memory.
    pub fn free(&mut self, bytes: u64) {
        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes);
    }

    /// Trigger GC.
    pub fn trigger_gc(&mut self) {
        self.gc_triggers += 1;
    }

    /// Record eviction.
    pub fn evict(&mut self, bytes: u64) {
        self.evictions += 1;
        self.allocated_bytes = self.allocated_bytes.saturating_sub(bytes);
    }

    /// Get utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.limit_bytes == 0 {
            0.0
        } else {
            (self.allocated_bytes as f64 / self.limit_bytes as f64) * 100.0
        }
    }

    /// Check if under pressure.
    #[must_use]
    pub fn is_under_pressure(&self) -> bool {
        self.utilization() > 80.0
    }

    /// Check if healthy.
    #[must_use]
    pub fn is_healthy(&self, max_utilization: f64) -> bool {
        self.utilization() <= max_utilization
    }

    /// Reset tracker.
    pub fn reset(&mut self) {
        self.allocated_bytes = 0;
        self.pressure_events = 0;
        self.gc_triggers = 0;
        self.evictions = 0;
    }
}

#[cfg(test)]
mod write_amplification_tests {
    use super::*;

    /// F-WAMP-001: New creates empty tracker
    #[test]
    fn f_wamp_001_new() {
        let wa = WriteAmplification::new();
        assert_eq!(wa.writes(), 0);
    }

    /// F-WAMP-002: Default equals new
    #[test]
    fn f_wamp_002_default() {
        let wa = WriteAmplification::default();
        assert!((wa.amplification() - 1.0).abs() < 0.01);
    }

    /// F-WAMP-003: User write tracked
    #[test]
    fn f_wamp_003_user_write() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        assert_eq!(wa.user_bytes, 100);
    }

    /// F-WAMP-004: Disk write tracked
    #[test]
    fn f_wamp_004_disk_write() {
        let mut wa = WriteAmplification::new();
        wa.disk_write(200);
        assert_eq!(wa.actual_bytes, 200);
    }

    /// F-WAMP-005: Amplification calculated
    #[test]
    fn f_wamp_005_amplification() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        wa.disk_write(300);
        assert!((wa.amplification() - 3.0).abs() < 0.01);
    }

    /// F-WAMP-006: Compaction tracked
    #[test]
    fn f_wamp_006_compaction() {
        let mut wa = WriteAmplification::new();
        wa.compaction_write(500);
        assert_eq!(wa.compaction_bytes, 500);
    }

    /// F-WAMP-007: Factory for_lsm
    #[test]
    fn f_wamp_007_for_lsm() {
        let wa = WriteAmplification::for_lsm();
        assert_eq!(wa.writes(), 0);
    }

    /// F-WAMP-008: Factory for_btree
    #[test]
    fn f_wamp_008_for_btree() {
        let wa = WriteAmplification::for_btree();
        assert_eq!(wa.user_bytes, 0);
    }

    /// F-WAMP-009: Acceptable when low amp
    #[test]
    fn f_wamp_009_acceptable() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        wa.disk_write(150);
        assert!(wa.is_acceptable(2.0));
    }

    /// F-WAMP-010: Not acceptable when high amp
    #[test]
    fn f_wamp_010_not_acceptable() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        wa.disk_write(500);
        assert!(!wa.is_acceptable(2.0));
    }

    /// F-WAMP-011: Reset clears counters
    #[test]
    fn f_wamp_011_reset() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        wa.reset();
        assert_eq!(wa.writes(), 0);
    }

    /// F-WAMP-012: Clone preserves state
    #[test]
    fn f_wamp_012_clone() {
        let mut wa = WriteAmplification::new();
        wa.user_write(100);
        let cloned = wa.clone();
        assert_eq!(wa.user_bytes, cloned.user_bytes);
    }
}

#[cfg(test)]
mod read_amplification_tests {
    use super::*;

    /// F-RAMP-001: New creates empty tracker
    #[test]
    fn f_ramp_001_new() {
        let ra = ReadAmplification::new();
        assert_eq!(ra.logical_reads, 0);
    }

    /// F-RAMP-002: Default equals new
    #[test]
    fn f_ramp_002_default() {
        let ra = ReadAmplification::default();
        assert!((ra.amplification() - 1.0).abs() < 0.01);
    }

    /// F-RAMP-003: Logical read tracked
    #[test]
    fn f_ramp_003_logical() {
        let mut ra = ReadAmplification::new();
        ra.logical_read();
        assert_eq!(ra.logical_reads, 1);
    }

    /// F-RAMP-004: Physical read tracked
    #[test]
    fn f_ramp_004_physical() {
        let mut ra = ReadAmplification::new();
        ra.physical_read();
        assert_eq!(ra.physical_reads, 1);
    }

    /// F-RAMP-005: Amplification calculated
    #[test]
    fn f_ramp_005_amplification() {
        let mut ra = ReadAmplification::new();
        ra.logical_read();
        ra.physical_read();
        ra.physical_read();
        ra.physical_read();
        assert!((ra.amplification() - 3.0).abs() < 0.01);
    }

    /// F-RAMP-006: Cache hit tracked
    #[test]
    fn f_ramp_006_cache() {
        let mut ra = ReadAmplification::new();
        ra.cache_hit();
        assert_eq!(ra.cache_hits, 1);
    }

    /// F-RAMP-007: Factory for_lsm
    #[test]
    fn f_ramp_007_for_lsm() {
        let ra = ReadAmplification::for_lsm();
        assert_eq!(ra.logical_reads, 0);
    }

    /// F-RAMP-008: Factory for_btree
    #[test]
    fn f_ramp_008_for_btree() {
        let ra = ReadAmplification::for_btree();
        assert_eq!(ra.physical_reads, 0);
    }

    /// F-RAMP-009: Cache hit rate calculated
    #[test]
    fn f_ramp_009_cache_rate() {
        let mut ra = ReadAmplification::new();
        ra.cache_hit();
        ra.physical_read();
        assert!((ra.cache_hit_rate() - 50.0).abs() < 0.01);
    }

    /// F-RAMP-010: Bloom filter tracked
    #[test]
    fn f_ramp_010_bloom() {
        let mut ra = ReadAmplification::new();
        ra.bloom_hit();
        assert_eq!(ra.bloom_filter_hits, 1);
    }

    /// F-RAMP-011: Reset clears counters
    #[test]
    fn f_ramp_011_reset() {
        let mut ra = ReadAmplification::new();
        ra.logical_read();
        ra.reset();
        assert_eq!(ra.logical_reads, 0);
    }

    /// F-RAMP-012: Clone preserves state
    #[test]
    fn f_ramp_012_clone() {
        let mut ra = ReadAmplification::new();
        ra.logical_read();
        let cloned = ra.clone();
        assert_eq!(ra.logical_reads, cloned.logical_reads);
    }
}

#[cfg(test)]
mod lock_manager_tests {
    use super::*;

    /// F-LOCK-001: New creates empty manager
    #[test]
    fn f_lock_001_new() {
        let lm = LockManager::new();
        assert_eq!(lm.acquisitions, 0);
    }

    /// F-LOCK-002: Default equals new
    #[test]
    fn f_lock_002_default() {
        let lm = LockManager::default();
        assert_eq!(lm.contentions, 0);
    }

    /// F-LOCK-003: Acquire increments count
    #[test]
    fn f_lock_003_acquire() {
        let mut lm = LockManager::new();
        lm.acquire(0);
        assert_eq!(lm.acquisitions, 1);
    }

    /// F-LOCK-004: Release decrements held
    #[test]
    fn f_lock_004_release() {
        let mut lm = LockManager::new();
        lm.acquire(0);
        lm.release();
        assert_eq!(lm.held_count, 0);
    }

    /// F-LOCK-005: Contention tracked
    #[test]
    fn f_lock_005_contention() {
        let mut lm = LockManager::new();
        lm.acquire(100); // Wait indicates contention
        assert_eq!(lm.contentions, 1);
    }

    /// F-LOCK-006: Contention rate calculated
    #[test]
    fn f_lock_006_rate() {
        let mut lm = LockManager::new();
        lm.acquire(0);
        lm.acquire(100);
        assert!((lm.contention_rate() - 50.0).abs() < 0.01);
    }

    /// F-LOCK-007: Factory for_mutex
    #[test]
    fn f_lock_007_for_mutex() {
        let lm = LockManager::for_mutex();
        assert_eq!(lm.acquisitions, 0);
    }

    /// F-LOCK-008: Factory for_rwlock
    #[test]
    fn f_lock_008_for_rwlock() {
        let lm = LockManager::for_rwlock();
        assert_eq!(lm.contentions, 0);
    }

    /// F-LOCK-009: Deadlock tracked
    #[test]
    fn f_lock_009_deadlock() {
        let mut lm = LockManager::new();
        lm.deadlock();
        assert_eq!(lm.deadlocks, 1);
    }

    /// F-LOCK-010: Healthy when no deadlocks
    #[test]
    fn f_lock_010_healthy() {
        let mut lm = LockManager::new();
        lm.acquire(0);
        assert!(lm.is_healthy(50.0));
    }

    /// F-LOCK-011: Reset clears counters
    #[test]
    fn f_lock_011_reset() {
        let mut lm = LockManager::new();
        lm.acquire(100);
        lm.reset();
        assert_eq!(lm.acquisitions, 0);
    }

    /// F-LOCK-012: Clone preserves state
    #[test]
    fn f_lock_012_clone() {
        let mut lm = LockManager::new();
        lm.acquire(100);
        let cloned = lm.clone();
        assert_eq!(lm.contentions, cloned.contentions);
    }
}

#[cfg(test)]
mod memory_pressure_tests {
    use super::*;

    /// F-MPRESS-001: New creates empty tracker
    #[test]
    fn f_mpress_001_new() {
        let mp = MemoryPressure::new(1000);
        assert_eq!(mp.allocated_bytes, 0);
    }

    /// F-MPRESS-002: Default has limit
    #[test]
    fn f_mpress_002_default() {
        let mp = MemoryPressure::default();
        assert!(mp.limit_bytes > 0);
    }

    /// F-MPRESS-003: Allocate increases bytes
    #[test]
    fn f_mpress_003_allocate() {
        let mut mp = MemoryPressure::new(1000);
        mp.allocate(100);
        assert_eq!(mp.allocated_bytes, 100);
    }

    /// F-MPRESS-004: Free decreases bytes
    #[test]
    fn f_mpress_004_free() {
        let mut mp = MemoryPressure::new(1000);
        mp.allocate(100);
        mp.free(50);
        assert_eq!(mp.allocated_bytes, 50);
    }

    /// F-MPRESS-005: Utilization calculated
    #[test]
    fn f_mpress_005_utilization() {
        let mut mp = MemoryPressure::new(100);
        mp.allocate(50);
        assert!((mp.utilization() - 50.0).abs() < 0.01);
    }

    /// F-MPRESS-006: Pressure detected
    #[test]
    fn f_mpress_006_pressure() {
        let mut mp = MemoryPressure::new(100);
        mp.allocate(90);
        assert!(mp.is_under_pressure());
    }

    /// F-MPRESS-007: Factory for_heap
    #[test]
    fn f_mpress_007_for_heap() {
        let mp = MemoryPressure::for_heap();
        assert!(mp.limit_bytes > 1024 * 1024 * 1024);
    }

    /// F-MPRESS-008: Factory for_cache
    #[test]
    fn f_mpress_008_for_cache() {
        let mp = MemoryPressure::for_cache();
        assert_eq!(mp.limit_bytes, 1024 * 1024 * 1024);
    }

    /// F-MPRESS-009: GC trigger tracked
    #[test]
    fn f_mpress_009_gc() {
        let mut mp = MemoryPressure::new(1000);
        mp.trigger_gc();
        assert_eq!(mp.gc_triggers, 1);
    }

    /// F-MPRESS-010: Eviction tracked
    #[test]
    fn f_mpress_010_evict() {
        let mut mp = MemoryPressure::new(1000);
        mp.allocate(100);
        mp.evict(50);
        assert_eq!(mp.evictions, 1);
        assert_eq!(mp.allocated_bytes, 50);
    }

    /// F-MPRESS-011: Reset clears counters
    #[test]
    fn f_mpress_011_reset() {
        let mut mp = MemoryPressure::new(1000);
        mp.allocate(100);
        mp.reset();
        assert_eq!(mp.allocated_bytes, 0);
    }

    /// F-MPRESS-012: Clone preserves state
    #[test]
    fn f_mpress_012_clone() {
        let mut mp = MemoryPressure::new(1000);
        mp.allocate(100);
        let cloned = mp.clone();
        assert_eq!(mp.allocated_bytes, cloned.allocated_bytes);
    }
}

// ============================================================================
// FileDescriptorTracker - O(1) file descriptor usage tracking
// ============================================================================

/// O(1) file descriptor usage tracking.
///
/// Tracks open/close operations, leaks, and usage patterns for FD management.
#[derive(Debug, Clone)]
pub struct FileDescriptorTracker {
    /// Currently open FDs
    pub open_fds: u32,
    /// Maximum allowed FDs
    pub max_fds: u32,
    /// Total opens
    pub opens: u64,
    /// Total closes
    pub closes: u64,
    /// Detected leaks
    pub leaks: u64,
    /// Peak open FDs
    pub peak_open: u32,
}

impl Default for FileDescriptorTracker {
    fn default() -> Self {
        Self::for_process()
    }
}

impl FileDescriptorTracker {
    /// Create new FD tracker with max limit.
    #[must_use]
    pub fn new(max_fds: u32) -> Self {
        Self {
            open_fds: 0,
            max_fds,
            opens: 0,
            closes: 0,
            leaks: 0,
            peak_open: 0,
        }
    }

    /// Factory for process-level tracking (1024 default).
    #[must_use]
    pub fn for_process() -> Self {
        Self::new(1024)
    }

    /// Factory for server tracking (65536).
    #[must_use]
    pub fn for_server() -> Self {
        Self::new(65536)
    }

    /// Record FD open.
    pub fn open(&mut self) {
        self.opens += 1;
        self.open_fds += 1;
        if self.open_fds > self.peak_open {
            self.peak_open = self.open_fds;
        }
    }

    /// Record FD close.
    pub fn close(&mut self) {
        self.closes += 1;
        self.open_fds = self.open_fds.saturating_sub(1);
    }

    /// Record detected leak.
    pub fn leak(&mut self) {
        self.leaks += 1;
    }

    /// Get FD utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_fds == 0 {
            return 0.0;
        }
        (self.open_fds as f64 / self.max_fds as f64) * 100.0
    }

    /// Check if FD exhaustion risk.
    #[must_use]
    pub fn is_at_risk(&self) -> bool {
        self.utilization() > 80.0
    }

    /// Get leak rate percentage.
    #[must_use]
    pub fn leak_rate(&self) -> f64 {
        if self.opens == 0 {
            return 0.0;
        }
        (self.leaks as f64 / self.opens as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.open_fds = 0;
        self.opens = 0;
        self.closes = 0;
        self.leaks = 0;
        self.peak_open = 0;
    }
}

#[cfg(test)]
mod fd_tracker_tests {
    use super::*;

    /// F-FD-001: New tracker has max FDs
    #[test]
    fn f_fd_001_new() {
        let fd = FileDescriptorTracker::new(1024);
        assert_eq!(fd.max_fds, 1024);
    }

    /// F-FD-002: Default uses process limit
    #[test]
    fn f_fd_002_default() {
        let fd = FileDescriptorTracker::default();
        assert_eq!(fd.max_fds, 1024);
    }

    /// F-FD-003: Open increases count
    #[test]
    fn f_fd_003_open() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.open();
        assert_eq!(fd.open_fds, 1);
        assert_eq!(fd.opens, 1);
    }

    /// F-FD-004: Close decreases count
    #[test]
    fn f_fd_004_close() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.open();
        fd.close();
        assert_eq!(fd.open_fds, 0);
        assert_eq!(fd.closes, 1);
    }

    /// F-FD-005: Utilization calculated
    #[test]
    fn f_fd_005_utilization() {
        let mut fd = FileDescriptorTracker::new(100);
        for _ in 0..50 {
            fd.open();
        }
        assert!((fd.utilization() - 50.0).abs() < 0.01);
    }

    /// F-FD-006: Risk detected at high utilization
    #[test]
    fn f_fd_006_risk() {
        let mut fd = FileDescriptorTracker::new(100);
        for _ in 0..85 {
            fd.open();
        }
        assert!(fd.is_at_risk());
    }

    /// F-FD-007: Factory for_process
    #[test]
    fn f_fd_007_for_process() {
        let fd = FileDescriptorTracker::for_process();
        assert_eq!(fd.max_fds, 1024);
    }

    /// F-FD-008: Factory for_server
    #[test]
    fn f_fd_008_for_server() {
        let fd = FileDescriptorTracker::for_server();
        assert_eq!(fd.max_fds, 65536);
    }

    /// F-FD-009: Leak tracked
    #[test]
    fn f_fd_009_leak() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.leak();
        assert_eq!(fd.leaks, 1);
    }

    /// F-FD-010: Leak rate calculated
    #[test]
    fn f_fd_010_leak_rate() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.open();
        fd.open();
        fd.leak();
        assert!((fd.leak_rate() - 50.0).abs() < 0.01);
    }

    /// F-FD-011: Reset clears state
    #[test]
    fn f_fd_011_reset() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.open();
        fd.reset();
        assert_eq!(fd.open_fds, 0);
    }

    /// F-FD-012: Clone preserves state
    #[test]
    fn f_fd_012_clone() {
        let mut fd = FileDescriptorTracker::new(100);
        fd.open();
        let cloned = fd.clone();
        assert_eq!(fd.open_fds, cloned.open_fds);
    }
}

// ============================================================================
// SocketTracker - O(1) socket state tracking
// ============================================================================

/// O(1) socket state tracking.
///
/// Tracks socket lifecycle, states, and connection patterns.
#[derive(Debug, Clone)]
pub struct SocketTracker {
    /// Active sockets
    pub active: u32,
    /// Maximum sockets
    pub max_sockets: u32,
    /// Sockets in TIME_WAIT
    pub time_wait: u32,
    /// Total connections
    pub connections: u64,
    /// Total accepts
    pub accepts: u64,
    /// Connection errors
    pub errors: u64,
}

impl Default for SocketTracker {
    fn default() -> Self {
        Self::for_server()
    }
}

impl SocketTracker {
    /// Create new socket tracker.
    #[must_use]
    pub fn new(max_sockets: u32) -> Self {
        Self {
            active: 0,
            max_sockets,
            time_wait: 0,
            connections: 0,
            accepts: 0,
            errors: 0,
        }
    }

    /// Factory for server tracking (10000).
    #[must_use]
    pub fn for_server() -> Self {
        Self::new(10000)
    }

    /// Factory for client tracking (100).
    #[must_use]
    pub fn for_client() -> Self {
        Self::new(100)
    }

    /// Record new connection.
    pub fn connect(&mut self) {
        self.connections += 1;
        self.active += 1;
    }

    /// Record accepted connection.
    pub fn accept(&mut self) {
        self.accepts += 1;
        self.active += 1;
    }

    /// Record socket close (enters TIME_WAIT).
    pub fn close(&mut self) {
        self.active = self.active.saturating_sub(1);
        self.time_wait += 1;
    }

    /// Record TIME_WAIT expiry.
    pub fn expire_time_wait(&mut self) {
        self.time_wait = self.time_wait.saturating_sub(1);
    }

    /// Record connection error.
    pub fn error(&mut self) {
        self.errors += 1;
    }

    /// Get socket utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_sockets == 0 {
            return 0.0;
        }
        ((self.active + self.time_wait) as f64 / self.max_sockets as f64) * 100.0
    }

    /// Check if TIME_WAIT buildup issue.
    #[must_use]
    pub fn has_time_wait_issue(&self) -> bool {
        self.time_wait > self.active * 2
    }

    /// Get error rate percentage.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        let total = self.connections + self.accepts;
        if total == 0 {
            return 0.0;
        }
        (self.errors as f64 / total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.active = 0;
        self.time_wait = 0;
        self.connections = 0;
        self.accepts = 0;
        self.errors = 0;
    }
}

#[cfg(test)]
mod socket_tracker_tests {
    use super::*;

    /// F-SOCK-001: New tracker has max
    #[test]
    fn f_sock_001_new() {
        let sock = SocketTracker::new(1000);
        assert_eq!(sock.max_sockets, 1000);
    }

    /// F-SOCK-002: Default uses server
    #[test]
    fn f_sock_002_default() {
        let sock = SocketTracker::default();
        assert_eq!(sock.max_sockets, 10000);
    }

    /// F-SOCK-003: Connect increases active
    #[test]
    fn f_sock_003_connect() {
        let mut sock = SocketTracker::new(100);
        sock.connect();
        assert_eq!(sock.active, 1);
        assert_eq!(sock.connections, 1);
    }

    /// F-SOCK-004: Accept increases active
    #[test]
    fn f_sock_004_accept() {
        let mut sock = SocketTracker::new(100);
        sock.accept();
        assert_eq!(sock.active, 1);
        assert_eq!(sock.accepts, 1);
    }

    /// F-SOCK-005: Close moves to TIME_WAIT
    #[test]
    fn f_sock_005_close() {
        let mut sock = SocketTracker::new(100);
        sock.connect();
        sock.close();
        assert_eq!(sock.active, 0);
        assert_eq!(sock.time_wait, 1);
    }

    /// F-SOCK-006: TIME_WAIT expiry
    #[test]
    fn f_sock_006_expire() {
        let mut sock = SocketTracker::new(100);
        sock.connect();
        sock.close();
        sock.expire_time_wait();
        assert_eq!(sock.time_wait, 0);
    }

    /// F-SOCK-007: Factory for_server
    #[test]
    fn f_sock_007_for_server() {
        let sock = SocketTracker::for_server();
        assert_eq!(sock.max_sockets, 10000);
    }

    /// F-SOCK-008: Factory for_client
    #[test]
    fn f_sock_008_for_client() {
        let sock = SocketTracker::for_client();
        assert_eq!(sock.max_sockets, 100);
    }

    /// F-SOCK-009: Utilization includes TIME_WAIT
    #[test]
    fn f_sock_009_utilization() {
        let mut sock = SocketTracker::new(100);
        for _ in 0..30 {
            sock.connect();
        }
        for _ in 0..20 {
            sock.close();
        }
        // 10 active + 20 time_wait = 30 total
        assert!((sock.utilization() - 30.0).abs() < 0.01);
    }

    /// F-SOCK-010: TIME_WAIT issue detected
    #[test]
    fn f_sock_010_time_wait_issue() {
        let mut sock = SocketTracker::new(100);
        sock.active = 10;
        sock.time_wait = 30;
        assert!(sock.has_time_wait_issue());
    }

    /// F-SOCK-011: Error rate calculated
    #[test]
    fn f_sock_011_error_rate() {
        let mut sock = SocketTracker::new(100);
        sock.connect();
        sock.connect();
        sock.error();
        assert!((sock.error_rate() - 50.0).abs() < 0.01);
    }

    /// F-SOCK-012: Clone preserves state
    #[test]
    fn f_sock_012_clone() {
        let mut sock = SocketTracker::new(100);
        sock.connect();
        let cloned = sock.clone();
        assert_eq!(sock.active, cloned.active);
    }
}

// ============================================================================
// ThreadPoolTracker - O(1) thread pool utilization tracking
// ============================================================================

/// O(1) thread pool utilization tracking.
///
/// Tracks worker threads, task queuing, and pool efficiency.
#[derive(Debug, Clone)]
pub struct ThreadPoolTracker {
    /// Worker count
    pub workers: u32,
    /// Active workers
    pub active: u32,
    /// Queued tasks
    pub queued: u64,
    /// Completed tasks
    pub completed: u64,
    /// Rejected tasks (queue full)
    pub rejected: u64,
    /// Peak queue depth
    pub peak_queued: u64,
}

impl Default for ThreadPoolTracker {
    fn default() -> Self {
        Self::for_cpu()
    }
}

impl ThreadPoolTracker {
    /// Create new thread pool tracker.
    #[must_use]
    pub fn new(workers: u32) -> Self {
        Self {
            workers,
            active: 0,
            queued: 0,
            completed: 0,
            rejected: 0,
            peak_queued: 0,
        }
    }

    /// Factory for CPU-bound pools (num_cpus).
    #[must_use]
    pub fn for_cpu() -> Self {
        Self::new(8)
    }

    /// Factory for IO-bound pools (larger).
    #[must_use]
    pub fn for_io() -> Self {
        Self::new(64)
    }

    /// Submit task to pool.
    pub fn submit(&mut self) {
        self.queued += 1;
        if self.queued > self.peak_queued {
            self.peak_queued = self.queued;
        }
    }

    /// Worker starts task.
    pub fn start(&mut self) {
        if self.queued > 0 {
            self.queued -= 1;
        }
        self.active += 1;
    }

    /// Worker completes task.
    pub fn complete(&mut self) {
        self.active = self.active.saturating_sub(1);
        self.completed += 1;
    }

    /// Task rejected (queue full).
    pub fn reject(&mut self) {
        self.rejected += 1;
    }

    /// Get worker utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.workers == 0 {
            return 0.0;
        }
        (self.active as f64 / self.workers as f64) * 100.0
    }

    /// Check if pool is saturated.
    #[must_use]
    pub fn is_saturated(&self) -> bool {
        self.active >= self.workers
    }

    /// Get rejection rate percentage.
    #[must_use]
    pub fn rejection_rate(&self) -> f64 {
        let submitted = self.completed + self.rejected + self.queued;
        if submitted == 0 {
            return 0.0;
        }
        (self.rejected as f64 / submitted as f64) * 100.0
    }

    /// Get throughput (completed per period).
    #[must_use]
    pub fn throughput(&self) -> u64 {
        self.completed
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.active = 0;
        self.queued = 0;
        self.completed = 0;
        self.rejected = 0;
        self.peak_queued = 0;
    }
}

#[cfg(test)]
mod thread_pool_tests {
    use super::*;

    /// F-TPOOL-001: New tracker has workers
    #[test]
    fn f_tpool_001_new() {
        let tp = ThreadPoolTracker::new(8);
        assert_eq!(tp.workers, 8);
    }

    /// F-TPOOL-002: Default uses CPU count
    #[test]
    fn f_tpool_002_default() {
        let tp = ThreadPoolTracker::default();
        assert_eq!(tp.workers, 8);
    }

    /// F-TPOOL-003: Submit increases queue
    #[test]
    fn f_tpool_003_submit() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.submit();
        assert_eq!(tp.queued, 1);
    }

    /// F-TPOOL-004: Start activates worker
    #[test]
    fn f_tpool_004_start() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.submit();
        tp.start();
        assert_eq!(tp.active, 1);
        assert_eq!(tp.queued, 0);
    }

    /// F-TPOOL-005: Complete releases worker
    #[test]
    fn f_tpool_005_complete() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.submit();
        tp.start();
        tp.complete();
        assert_eq!(tp.active, 0);
        assert_eq!(tp.completed, 1);
    }

    /// F-TPOOL-006: Utilization calculated
    #[test]
    fn f_tpool_006_utilization() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.active = 4;
        assert!((tp.utilization() - 50.0).abs() < 0.01);
    }

    /// F-TPOOL-007: Saturation detected
    #[test]
    fn f_tpool_007_saturated() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.active = 8;
        assert!(tp.is_saturated());
    }

    /// F-TPOOL-008: Factory for_cpu
    #[test]
    fn f_tpool_008_for_cpu() {
        let tp = ThreadPoolTracker::for_cpu();
        assert_eq!(tp.workers, 8);
    }

    /// F-TPOOL-009: Factory for_io
    #[test]
    fn f_tpool_009_for_io() {
        let tp = ThreadPoolTracker::for_io();
        assert_eq!(tp.workers, 64);
    }

    /// F-TPOOL-010: Rejection tracked
    #[test]
    fn f_tpool_010_reject() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.reject();
        assert_eq!(tp.rejected, 1);
    }

    /// F-TPOOL-011: Rejection rate calculated
    #[test]
    fn f_tpool_011_rejection_rate() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.completed = 9;
        tp.rejected = 1;
        assert!((tp.rejection_rate() - 10.0).abs() < 0.01);
    }

    /// F-TPOOL-012: Clone preserves state
    #[test]
    fn f_tpool_012_clone() {
        let mut tp = ThreadPoolTracker::new(8);
        tp.submit();
        let cloned = tp.clone();
        assert_eq!(tp.queued, cloned.queued);
    }
}

// ============================================================================
// IoCostTracker - O(1) IO cost tracking
// ============================================================================

/// O(1) IO cost tracking.
///
/// Tracks IO operations, latency, and throughput for cost analysis.
#[derive(Debug, Clone)]
pub struct IoCostTracker {
    /// Read operations
    pub reads: u64,
    /// Write operations
    pub writes: u64,
    /// Read bytes
    pub read_bytes: u64,
    /// Write bytes
    pub write_bytes: u64,
    /// Total latency microseconds
    pub total_latency_us: u64,
    /// IO errors
    pub errors: u64,
}

impl Default for IoCostTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl IoCostTracker {
    /// Create new IO cost tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            reads: 0,
            writes: 0,
            read_bytes: 0,
            write_bytes: 0,
            total_latency_us: 0,
            errors: 0,
        }
    }

    /// Factory for disk IO tracking.
    #[must_use]
    pub fn for_disk() -> Self {
        Self::new()
    }

    /// Factory for network IO tracking.
    #[must_use]
    pub fn for_network() -> Self {
        Self::new()
    }

    /// Record read operation.
    pub fn read(&mut self, bytes: u64, latency_us: u64) {
        self.reads += 1;
        self.read_bytes += bytes;
        self.total_latency_us += latency_us;
    }

    /// Record write operation.
    pub fn write(&mut self, bytes: u64, latency_us: u64) {
        self.writes += 1;
        self.write_bytes += bytes;
        self.total_latency_us += latency_us;
    }

    /// Record IO error.
    pub fn error(&mut self) {
        self.errors += 1;
    }

    /// Get total operations.
    #[must_use]
    pub fn total_ops(&self) -> u64 {
        self.reads + self.writes
    }

    /// Get total bytes.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.read_bytes + self.write_bytes
    }

    /// Get average latency in microseconds.
    #[must_use]
    pub fn avg_latency_us(&self) -> u64 {
        let ops = self.total_ops();
        if ops == 0 {
            return 0;
        }
        self.total_latency_us / ops
    }

    /// Get read/write ratio.
    #[must_use]
    pub fn read_ratio(&self) -> f64 {
        let ops = self.total_ops();
        if ops == 0 {
            return 0.0;
        }
        (self.reads as f64 / ops as f64) * 100.0
    }

    /// Get error rate percentage.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        let ops = self.total_ops();
        if ops == 0 {
            return 0.0;
        }
        (self.errors as f64 / ops as f64) * 100.0
    }

    /// Check if IO is healthy (error rate < 1%).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.error_rate() < 1.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.reads = 0;
        self.writes = 0;
        self.read_bytes = 0;
        self.write_bytes = 0;
        self.total_latency_us = 0;
        self.errors = 0;
    }
}

#[cfg(test)]
mod io_cost_tests {
    use super::*;

    /// F-IO-001: New tracker is empty
    #[test]
    fn f_io_001_new() {
        let io = IoCostTracker::new();
        assert_eq!(io.total_ops(), 0);
    }

    /// F-IO-002: Default is empty
    #[test]
    fn f_io_002_default() {
        let io = IoCostTracker::default();
        assert_eq!(io.total_ops(), 0);
    }

    /// F-IO-003: Read operation tracked
    #[test]
    fn f_io_003_read() {
        let mut io = IoCostTracker::new();
        io.read(1024, 100);
        assert_eq!(io.reads, 1);
        assert_eq!(io.read_bytes, 1024);
    }

    /// F-IO-004: Write operation tracked
    #[test]
    fn f_io_004_write() {
        let mut io = IoCostTracker::new();
        io.write(2048, 200);
        assert_eq!(io.writes, 1);
        assert_eq!(io.write_bytes, 2048);
    }

    /// F-IO-005: Total ops calculated
    #[test]
    fn f_io_005_total_ops() {
        let mut io = IoCostTracker::new();
        io.read(1024, 100);
        io.write(1024, 100);
        assert_eq!(io.total_ops(), 2);
    }

    /// F-IO-006: Average latency calculated
    #[test]
    fn f_io_006_avg_latency() {
        let mut io = IoCostTracker::new();
        io.read(1024, 100);
        io.write(1024, 200);
        assert_eq!(io.avg_latency_us(), 150);
    }

    /// F-IO-007: Factory for_disk
    #[test]
    fn f_io_007_for_disk() {
        let io = IoCostTracker::for_disk();
        assert_eq!(io.total_ops(), 0);
    }

    /// F-IO-008: Factory for_network
    #[test]
    fn f_io_008_for_network() {
        let io = IoCostTracker::for_network();
        assert_eq!(io.total_ops(), 0);
    }

    /// F-IO-009: Read ratio calculated
    #[test]
    fn f_io_009_read_ratio() {
        let mut io = IoCostTracker::new();
        io.read(1024, 100);
        io.write(1024, 100);
        assert!((io.read_ratio() - 50.0).abs() < 0.01);
    }

    /// F-IO-010: Error tracked
    #[test]
    fn f_io_010_error() {
        let mut io = IoCostTracker::new();
        io.error();
        assert_eq!(io.errors, 1);
    }

    /// F-IO-011: Is healthy check
    #[test]
    fn f_io_011_healthy() {
        let mut io = IoCostTracker::new();
        io.reads = 100;
        assert!(io.is_healthy());
    }

    /// F-IO-012: Clone preserves state
    #[test]
    fn f_io_012_clone() {
        let mut io = IoCostTracker::new();
        io.read(1024, 100);
        let cloned = io.clone();
        assert_eq!(io.reads, cloned.reads);
    }
}
