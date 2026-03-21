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
        tt.record(500_000); // Success
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
