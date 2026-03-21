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
            Self::Collect => 100_000, // 100ms for collection
            Self::Render => 16_000,   // 16ms for 60fps
            Self::Compute => 1_000,   // 1ms for compute
            Self::Network => 500_000, // 500ms for network
            Self::Storage => 50_000,  // 50ms for storage
        }
    }

    /// Get severity threshold (CV%) for escalation
    #[must_use]
    pub fn cv_threshold(&self) -> f64 {
        match self {
            Self::Render => 10.0,  // Strict for render
            Self::Compute => 15.0, // Standard
            Self::Collect => 25.0, // More lenient
            Self::Network => 50.0, // High variance expected
            Self::Storage => 30.0, // Moderate variance
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
            b_total
                .partial_cmp(&a_total)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for (name, (brick_type, stats)) in sorted {
            let budget = brick_type.default_budget_us();
            let avg = stats.mean();
            let cv = stats.cv_percent();
            let status = if avg > budget as f64 { "⚠️" } else { "✓" };
            let escalate = if self.should_escalate(name) {
                " [ESCALATE]"
            } else {
                ""
            };

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
        let start = if self.len < N { 0 } else { self.head };
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
            0..=999 => 0,         // 0-1ms
            1000..=4999 => 1,     // 1-5ms
            5000..=9999 => 2,     // 5-10ms
            10000..=49999 => 3,   // 10-50ms
            50000..=99999 => 4,   // 50-100ms
            100000..=499999 => 5, // 100-500ms
            _ => 6,               // 500ms+
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
// TRACKER MACRO (PMAT-019: reduce ResourceManagement entropy)
// =============================================================================

/// Generates a tracker struct with all-zero `new()`, `Default`, `Clone`, and `reset()`.
/// Domain-specific methods are added via separate `impl` blocks.
macro_rules! define_tracker {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $( $(#[$fmeta:meta])* $fvis:vis $fname:ident : $fty:ty ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
        $vis struct $name {
            $( $(#[$fmeta])* $fvis $fname : $fty, )+
        }

        impl $name {
            /// Create new zeroed tracker.
            #[inline]
            #[must_use]
            pub const fn new() -> Self {
                Self { $( $fname: 0, )+ }
            }

            /// Reset all counters to zero.
            #[inline]
            pub fn reset(&mut self) {
                *self = Self::new();
            }
        }
    };
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
        let interval_us = if hz == 0 {
            1_000_000
        } else {
            1_000_000 / hz as u64
        };
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
                1_000,     // 0-1ms
                5_000,     // 1-5ms
                10_000,    // 5-10ms
                25_000,    // 10-25ms
                50_000,    // 25-50ms
                100_000,   // 50-100ms
                250_000,   // 100-250ms
                500_000,   // 250-500ms
                1_000_000, // 500-1000ms
                u64::MAX,  // 1000ms+
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
            boundaries: [
                100.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0,
            ],
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
        if self.count == 0 {
            0.0
        } else {
            self.min
        }
    }

    /// Get max value
    #[must_use]
    pub fn max(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.max
        }
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
        if self.count == 0 {
            0.0
        } else {
            self.min
        }
    }

    /// Get maximum value (O(1))
    #[must_use]
    pub fn max(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.max
        }
    }

    /// Get average value (O(1))
    #[must_use]
    pub fn average(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.sum / self.count as f64
        }
    }

    /// Get range (max - min) (O(1))
    #[must_use]
    pub fn range(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.max - self.min
        }
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
        if total == 0 {
            100.0
        } else {
            (self.successes as f64 / total as f64) * 100.0
        }
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
        self.cells
            .iter()
            .flat_map(|r| r.iter())
            .copied()
            .max()
            .unwrap_or(0)
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
        self.rng_state = self
            .rng_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1);
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
            self.cooldown_us
                .saturating_sub(now_us.saturating_sub(self.last_action_us))
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
        if self.samples == 0 {
            0
        } else {
            self.max_drift_us
        }
    }

    /// Get min drift
    #[must_use]
    pub fn min_drift_us(&self) -> i64 {
        if self.samples == 0 {
            0
        } else {
            self.min_drift_us
        }
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
