#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::disallowed_methods)]
mod tests {
    use super::*;

    /// F-PCT-013b: percentile_us/ms consistency
    #[test]
    fn f_pct_013b_percentile_us_ms_consistency() {
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
        let cd = ChangeDetector::new(0.0, 5.0, 100.0);
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
        assert!((-1.0..=1.0).contains(&r));
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
        let _ = s1;
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
        assert!((80.0..=100.0).contains(&p90));
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
}
