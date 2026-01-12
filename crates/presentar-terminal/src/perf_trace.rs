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
use std::time::{Duration, Instant};

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
}
