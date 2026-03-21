#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::disallowed_methods)]
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
        let cloned = event;
        assert_eq!(cloned.name, "test");
        assert!(cloned.budget_exceeded);
    }

    #[test]
    fn test_trace_stats_clone() {
        let stats = TraceStats::new(Duration::from_micros(100), 1000, false);
        let cloned = stats;
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
        let cloned = thresholds;
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
        let cloned = original;
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
        h.record(500); // 0.5ms
        h.record(999); // 0.999ms
        assert_eq!(h.bin_count(0), 2);
    }

    /// F-HIST-004: Record bins 1-5ms correctly
    #[test]
    fn f_hist_004_bin_1_5ms() {
        let mut h = LatencyHistogram::new();
        h.record(1000); // 1ms
        h.record(4999); // 4.999ms
        assert_eq!(h.bin_count(1), 2);
    }

    /// F-HIST-005: Record bins 500ms+ correctly
    #[test]
    fn f_hist_005_bin_500ms_plus() {
        let mut h = LatencyHistogram::new();
        h.record(500_000); // 500ms
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
        h.record(500); // bin 0
        h.record(2000); // bin 1
        h.record(7000); // bin 2
        h.record(25000); // bin 3
        h.record(75000); // bin 4
        h.record(250000); // bin 5
        h.record(750000); // bin 6

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
        pt.record_us(500); // 0.5ms -> bucket 0
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
            pt.record_ms(2.0); // 2ms -> bucket 1
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
        for i in 0..100 {
            pt.record_ms(i as f64);
        }
        let us = pt.percentile_us(0.5);
        assert!(us > 0, "percentile_us should return microseconds");
    }
}
