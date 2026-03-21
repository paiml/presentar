    #[test]
    fn f_integrity_006_verify_fail() {
        let mut tracker = IntegrityTracker::new();
        tracker.verify(false);
        assert_eq!(tracker.failed, 1);
    }

    /// F-INTEGRITY-007: Appraise increments
    #[test]
    fn f_integrity_007_appraise() {
        let mut tracker = IntegrityTracker::new();
        tracker.appraise();
        assert_eq!(tracker.appraisals, 1);
    }

    /// F-INTEGRITY-008: Signature validation increments
    #[test]
    fn f_integrity_008_signature() {
        let mut tracker = IntegrityTracker::new();
        tracker.validate_sig();
        assert_eq!(tracker.signatures, 1);
    }

    /// F-INTEGRITY-009: Violation increments
    #[test]
    fn f_integrity_009_violation() {
        let mut tracker = IntegrityTracker::new();
        tracker.violation();
        assert_eq!(tracker.violations, 1);
    }

    /// F-INTEGRITY-010: Success rate calculates
    #[test]
    fn f_integrity_010_success_rate() {
        let mut tracker = IntegrityTracker::new();
        tracker.verify(true);
        tracker.verify(false);
        let rate = tracker.success_rate();
        assert!((rate - 50.0).abs() < 0.1);
    }

    /// F-INTEGRITY-011: Reset clears counters
    #[test]
    fn f_integrity_011_reset() {
        let mut tracker = IntegrityTracker::new();
        tracker.measure();
        tracker.reset();
        assert_eq!(tracker.measurements, 0);
    }

    /// F-INTEGRITY-012: Clone preserves state
    #[test]
    fn f_integrity_012_clone() {
        let mut tracker = IntegrityTracker::new();
        tracker.measure();
        let cloned = tracker;
        assert_eq!(tracker.measurements, cloned.measurements);
    }
}
