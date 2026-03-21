    fn f_netdev_003_rx() {
        let mut netdev = NetDevTracker::new();
        netdev.rx(1500);
        assert_eq!(netdev.rx_packets, 1);
        assert_eq!(netdev.rx_bytes, 1500);
    }

    /// F-NETDEV-004: TX tracked
    #[test]
    fn f_netdev_004_tx() {
        let mut netdev = NetDevTracker::new();
        netdev.tx(1000);
        assert_eq!(netdev.tx_packets, 1);
        assert_eq!(netdev.tx_bytes, 1000);
    }

    /// F-NETDEV-005: RX error tracked
    #[test]
    fn f_netdev_005_rx_error() {
        let mut netdev = NetDevTracker::new();
        netdev.rx_error();
        assert_eq!(netdev.rx_errors, 1);
    }

    /// F-NETDEV-006: TX error tracked
    #[test]
    fn f_netdev_006_tx_error() {
        let mut netdev = NetDevTracker::new();
        netdev.tx_error();
        assert_eq!(netdev.tx_errors, 1);
    }

    /// F-NETDEV-007: Total packets
    #[test]
    fn f_netdev_007_total_packets() {
        let mut netdev = NetDevTracker::new();
        netdev.rx(1500);
        netdev.tx(1000);
        assert_eq!(netdev.total_packets(), 2);
    }

    /// F-NETDEV-008: Total bytes
    #[test]
    fn f_netdev_008_total_bytes() {
        let mut netdev = NetDevTracker::new();
        netdev.rx(1500);
        netdev.tx(1000);
        assert_eq!(netdev.total_bytes(), 2500);
    }

    /// F-NETDEV-009: Factory for_eth
    #[test]
    fn f_netdev_009_eth() {
        let netdev = NetDevTracker::for_eth();
        assert_eq!(netdev.rx_packets, 0);
    }

    /// F-NETDEV-010: Factory for_lo
    #[test]
    fn f_netdev_010_lo() {
        let netdev = NetDevTracker::for_lo();
        assert_eq!(netdev.rx_packets, 0);
    }

    /// F-NETDEV-011: Reset clears counters
    #[test]
    fn f_netdev_011_reset() {
        let mut netdev = NetDevTracker::new();
        netdev.rx(1500);
        netdev.reset();
        assert_eq!(netdev.rx_packets, 0);
    }

    /// F-NETDEV-012: Clone preserves state
    #[test]
    fn f_netdev_012_clone() {
        let mut netdev = NetDevTracker::new();
        netdev.rx(1500);
        let cloned = netdev;
        assert_eq!(netdev.rx_packets, cloned.rx_packets);
    }
}

// ============================================================================
// v9.43.0: Timer Subsystem Helpers
// ============================================================================

/// O(1) timer tracker.
///
/// Tracks Linux timer subsystem operations including timer starts,
/// cancellations, expirations, and callback latencies.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::TimerTracker;
///
/// let mut timer = TimerTracker::new();
/// timer.start();
/// timer.expire();
/// assert_eq!(timer.starts, 1);
/// assert_eq!(timer.expirations, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimerTracker {
    /// Timer starts.
    pub starts: u64,
    /// Timer cancellations.
    pub cancels: u64,
    /// Timer expirations.
    pub expirations: u64,
    /// Callback invocations.
    pub callbacks: u64,
    /// Active timers.
    pub active: u64,
    /// Peak active timers.
    pub peak_active: u64,
}

impl TimerTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            starts: 0,
            cancels: 0,
            expirations: 0,
            callbacks: 0,
            active: 0,
            peak_active: 0,
        }
    }

    /// Factory for softirq timer.
    #[must_use]
    pub const fn for_softirq() -> Self {
        Self::new()
    }

    /// Factory for workqueue timer.
    #[must_use]
    pub const fn for_workqueue() -> Self {
        Self::new()
    }

    /// Record timer start.
    pub fn start(&mut self) {
        self.starts += 1;
        self.active += 1;
        if self.active > self.peak_active {
            self.peak_active = self.active;
        }
    }

    /// Record timer cancel.
    pub fn cancel(&mut self) {
        self.cancels += 1;
        self.active = self.active.saturating_sub(1);
    }

    /// Record timer expiration.
    pub fn expire(&mut self) {
        self.expirations += 1;
        self.active = self.active.saturating_sub(1);
    }

    /// Record callback invocation.
    pub fn callback(&mut self) {
        self.callbacks += 1;
    }

    /// Get cancel rate.
    #[must_use]
    pub fn cancel_rate(&self) -> f64 {
        if self.starts == 0 {
            return 0.0;
        }
        (self.cancels as f64) / (self.starts as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.starts = 0;
        self.cancels = 0;
        self.expirations = 0;
        self.callbacks = 0;
    }
}

#[cfg(test)]
mod timer_tests {
    use super::*;

    /// F-TIMER-001: New tracker is empty
    #[test]
    fn f_timer_001_new() {
        let timer = TimerTracker::new();
        assert_eq!(timer.starts, 0);
    }

    /// F-TIMER-002: Default is empty
    #[test]
    fn f_timer_002_default() {
        let timer = TimerTracker::default();
        assert_eq!(timer.starts, 0);
    }

    /// F-TIMER-003: Start tracked
    #[test]
    fn f_timer_003_start() {
        let mut timer = TimerTracker::new();
        timer.start();
        assert_eq!(timer.starts, 1);
        assert_eq!(timer.active, 1);
    }

    /// F-TIMER-004: Cancel tracked
    #[test]
    fn f_timer_004_cancel() {
        let mut timer = TimerTracker::new();
        timer.start();
        timer.cancel();
        assert_eq!(timer.cancels, 1);
        assert_eq!(timer.active, 0);
    }

    /// F-TIMER-005: Expire tracked
    #[test]
    fn f_timer_005_expire() {
        let mut timer = TimerTracker::new();
        timer.start();
        timer.expire();
        assert_eq!(timer.expirations, 1);
    }

    /// F-TIMER-006: Callback tracked
    #[test]
    fn f_timer_006_callback() {
        let mut timer = TimerTracker::new();
        timer.callback();
        assert_eq!(timer.callbacks, 1);
    }

    /// F-TIMER-007: Peak active tracked
    #[test]
    fn f_timer_007_peak() {
        let mut timer = TimerTracker::new();
        timer.start();
        timer.start();
        timer.expire();
        assert_eq!(timer.peak_active, 2);
    }

    /// F-TIMER-008: Cancel rate
    #[test]
    fn f_timer_008_cancel_rate() {
        let mut timer = TimerTracker::new();
        timer.start();
        timer.start();
        timer.cancel();
        assert!((timer.cancel_rate() - 0.5).abs() < 0.01);
    }

    /// F-TIMER-009: Factory for_softirq
    #[test]
    fn f_timer_009_softirq() {
        let timer = TimerTracker::for_softirq();
        assert_eq!(timer.starts, 0);
    }

    /// F-TIMER-010: Factory for_workqueue
    #[test]
    fn f_timer_010_workqueue() {
        let timer = TimerTracker::for_workqueue();
        assert_eq!(timer.starts, 0);
    }

    /// F-TIMER-011: Reset clears counters
    #[test]
    fn f_timer_011_reset() {
        let mut timer = TimerTracker::new();
        timer.start();
        timer.reset();
        assert_eq!(timer.starts, 0);
    }

    /// F-TIMER-012: Clone preserves state
    #[test]
    fn f_timer_012_clone() {
        let mut timer = TimerTracker::new();
        timer.start();
        let cloned = timer;
        assert_eq!(timer.starts, cloned.starts);
    }
}

/// O(1) high-resolution timer tracker.
///
/// Tracks Linux hrtimer subsystem operations for nanosecond-precision
/// timing including starts, expirations, and drift measurements.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::HrTimerTracker;
///
/// let mut hrt = HrTimerTracker::new();
/// hrt.start();
/// hrt.expire_ns(1000);
/// assert_eq!(hrt.starts, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HrTimerTracker {
    /// Timer starts.
    pub starts: u64,
    /// Timer expirations.
    pub expirations: u64,
    /// Timer restarts.
    pub restarts: u64,
    /// Total latency (ns).
    pub total_latency_ns: u64,
    /// Max latency seen (ns).
    pub max_latency_ns: u64,
    /// Active hrtimers.
    pub active: u64,
}

impl HrTimerTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            starts: 0,
            expirations: 0,
            restarts: 0,
            total_latency_ns: 0,
            max_latency_ns: 0,
            active: 0,
        }
    }

    /// Factory for monotonic clock.
    #[must_use]
    pub const fn for_monotonic() -> Self {
        Self::new()
    }

    /// Factory for realtime clock.
    #[must_use]
    pub const fn for_realtime() -> Self {
        Self::new()
    }

    /// Record timer start.
    pub fn start(&mut self) {
        self.starts += 1;
        self.active += 1;
    }

    /// Record timer expiration with latency.
    pub fn expire_ns(&mut self, latency_ns: u64) {
        self.expirations += 1;
        self.active = self.active.saturating_sub(1);
        self.total_latency_ns += latency_ns;
        if latency_ns > self.max_latency_ns {
            self.max_latency_ns = latency_ns;
        }
    }

    /// Record timer restart.
    pub fn restart(&mut self) {
        self.restarts += 1;
    }

    /// Get average latency (ns).
    #[must_use]
    pub fn avg_latency_ns(&self) -> u64 {
        if self.expirations == 0 {
            return 0;
        }
        self.total_latency_ns / self.expirations
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.starts = 0;
        self.expirations = 0;
        self.restarts = 0;
        self.total_latency_ns = 0;
    }
}

#[cfg(test)]
mod hrtimer_tests {
    use super::*;

    /// F-HRT-001: New tracker is empty
    #[test]
    fn f_hrt_001_new() {
        let hrt = HrTimerTracker::new();
        assert_eq!(hrt.starts, 0);
    }

    /// F-HRT-002: Default is empty
    #[test]
    fn f_hrt_002_default() {
        let hrt = HrTimerTracker::default();
        assert_eq!(hrt.starts, 0);
    }

    /// F-HRT-003: Start tracked
    #[test]
    fn f_hrt_003_start() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        assert_eq!(hrt.starts, 1);
        assert_eq!(hrt.active, 1);
    }

    /// F-HRT-004: Expire tracked
    #[test]
    fn f_hrt_004_expire() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        hrt.expire_ns(1000);
        assert_eq!(hrt.expirations, 1);
        assert_eq!(hrt.total_latency_ns, 1000);
    }

    /// F-HRT-005: Restart tracked
    #[test]
    fn f_hrt_005_restart() {
        let mut hrt = HrTimerTracker::new();
        hrt.restart();
        assert_eq!(hrt.restarts, 1);
    }

    /// F-HRT-006: Max latency tracked
    #[test]
    fn f_hrt_006_max_latency() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        hrt.expire_ns(500);
        hrt.start();
        hrt.expire_ns(1500);
        assert_eq!(hrt.max_latency_ns, 1500);
    }

    /// F-HRT-007: Avg latency
    #[test]
    fn f_hrt_007_avg_latency() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        hrt.expire_ns(1000);
        hrt.start();
        hrt.expire_ns(2000);
        assert_eq!(hrt.avg_latency_ns(), 1500);
    }

    /// F-HRT-008: Active tracking
    #[test]
    fn f_hrt_008_active() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        hrt.start();
        hrt.expire_ns(100);
        assert_eq!(hrt.active, 1);
    }

    /// F-HRT-009: Factory for_monotonic
    #[test]
    fn f_hrt_009_monotonic() {
        let hrt = HrTimerTracker::for_monotonic();
        assert_eq!(hrt.starts, 0);
    }

    /// F-HRT-010: Factory for_realtime
    #[test]
    fn f_hrt_010_realtime() {
        let hrt = HrTimerTracker::for_realtime();
        assert_eq!(hrt.starts, 0);
    }

    /// F-HRT-011: Reset clears counters
    #[test]
    fn f_hrt_011_reset() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        hrt.reset();
        assert_eq!(hrt.starts, 0);
    }

    /// F-HRT-012: Clone preserves state
    #[test]
    fn f_hrt_012_clone() {
        let mut hrt = HrTimerTracker::new();
        hrt.start();
        let cloned = hrt;
        assert_eq!(hrt.starts, cloned.starts);
    }
}

/// O(1) clock source tracker.
///
/// Tracks clock source operations including reads, frequency adjustments,
/// and NTP synchronization status.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::ClockTracker;
///
/// let mut clock = ClockTracker::new();
/// clock.read();
/// clock.adjust(100);
/// assert_eq!(clock.reads, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClockTracker {
    /// Clock reads.
    pub reads: u64,
    /// Frequency adjustments.
    pub adjustments: u64,
    /// Total adjustment (ppb).
    pub total_adj_ppb: i64,
    /// NTP syncs.
    pub ntp_syncs: u64,
    /// Clock wraps.
    pub wraps: u64,
    /// Unstable events.
    pub unstable_events: u64,
}

impl ClockTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reads: 0,
            adjustments: 0,
            total_adj_ppb: 0,
            ntp_syncs: 0,
            wraps: 0,
            unstable_events: 0,
        }
    }

    /// Factory for TSC clock.
    #[must_use]
    pub const fn for_tsc() -> Self {
        Self::new()
    }

    /// Factory for HPET clock.
    #[must_use]
    pub const fn for_hpet() -> Self {
        Self::new()
    }

    /// Record clock read.
    pub fn read(&mut self) {
        self.reads += 1;
    }

    /// Record frequency adjustment (ppb).
    pub fn adjust(&mut self, ppb: i64) {
        self.adjustments += 1;
        self.total_adj_ppb += ppb;
    }

    /// Record NTP sync.
    pub fn ntp_sync(&mut self) {
        self.ntp_syncs += 1;
    }

    /// Record clock wrap.
    pub fn wrap(&mut self) {
        self.wraps += 1;
    }

    /// Record unstable event.
    pub fn unstable(&mut self) {
        self.unstable_events += 1;
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.reads = 0;
        self.adjustments = 0;
        self.total_adj_ppb = 0;
        self.ntp_syncs = 0;
        self.wraps = 0;
        self.unstable_events = 0;
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    /// F-CLOCK-001: New tracker is empty
    #[test]
    fn f_clock_001_new() {
        let clock = ClockTracker::new();
        assert_eq!(clock.reads, 0);
    }

    /// F-CLOCK-002: Default is empty
    #[test]
    fn f_clock_002_default() {
        let clock = ClockTracker::default();
        assert_eq!(clock.reads, 0);
    }

    /// F-CLOCK-003: Read tracked
    #[test]
    fn f_clock_003_read() {
        let mut clock = ClockTracker::new();
        clock.read();
        assert_eq!(clock.reads, 1);
    }

    /// F-CLOCK-004: Adjust tracked
    #[test]
    fn f_clock_004_adjust() {
        let mut clock = ClockTracker::new();
        clock.adjust(100);
        assert_eq!(clock.adjustments, 1);
        assert_eq!(clock.total_adj_ppb, 100);
    }

    /// F-CLOCK-005: NTP sync tracked
    #[test]
    fn f_clock_005_ntp() {
        let mut clock = ClockTracker::new();
        clock.ntp_sync();
        assert_eq!(clock.ntp_syncs, 1);
    }

    /// F-CLOCK-006: Wrap tracked
    #[test]
    fn f_clock_006_wrap() {
        let mut clock = ClockTracker::new();
        clock.wrap();
        assert_eq!(clock.wraps, 1);
    }

    /// F-CLOCK-007: Unstable tracked
    #[test]
    fn f_clock_007_unstable() {
        let mut clock = ClockTracker::new();
        clock.unstable();
        assert_eq!(clock.unstable_events, 1);
    }

    /// F-CLOCK-008: Negative adjustment
    #[test]
    fn f_clock_008_negative_adj() {
        let mut clock = ClockTracker::new();
        clock.adjust(-50);
        assert_eq!(clock.total_adj_ppb, -50);
    }

    /// F-CLOCK-009: Factory for_tsc
    #[test]
    fn f_clock_009_tsc() {
        let clock = ClockTracker::for_tsc();
        assert_eq!(clock.reads, 0);
    }

    /// F-CLOCK-010: Factory for_hpet
    #[test]
    fn f_clock_010_hpet() {
        let clock = ClockTracker::for_hpet();
        assert_eq!(clock.reads, 0);
    }

    /// F-CLOCK-011: Reset clears counters
    #[test]
    fn f_clock_011_reset() {
        let mut clock = ClockTracker::new();
        clock.read();
        clock.reset();
        assert_eq!(clock.reads, 0);
    }

    /// F-CLOCK-012: Clone preserves state
    #[test]
    fn f_clock_012_clone() {
        let mut clock = ClockTracker::new();
        clock.read();
        let cloned = clock;
        assert_eq!(clock.reads, cloned.reads);
    }
}

/// O(1) timekeeping tracker.
///
/// Tracks Linux timekeeping subsystem operations including clock updates,
/// leap second handling, and time synchronization metrics.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::TimeKeepingTracker;
///
/// let mut tk = TimeKeepingTracker::new();
/// tk.update();
/// tk.leap_second();
/// assert_eq!(tk.updates, 1);
/// assert_eq!(tk.leap_seconds, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TimeKeepingTracker {
    /// Time updates.
    pub updates: u64,
    /// Leap seconds handled.
    pub leap_seconds: u64,
    /// Time jumps (large adjustments).
    pub time_jumps: u64,
    /// Suspend/resume cycles.
    pub suspend_cycles: u64,
    /// Clock source switches.
    pub clock_switches: u64,
    /// Error corrections.
    pub corrections: u64,
}

impl TimeKeepingTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            updates: 0,
            leap_seconds: 0,
            time_jumps: 0,
            suspend_cycles: 0,
            clock_switches: 0,
            corrections: 0,
        }
    }

    /// Factory for system time.
    #[must_use]
    pub const fn for_system() -> Self {
        Self::new()
    }

    /// Factory for boot time.
    #[must_use]
    pub const fn for_boot() -> Self {
        Self::new()
    }

    /// Record time update.
    pub fn update(&mut self) {
        self.updates += 1;
    }

    /// Record leap second.
    pub fn leap_second(&mut self) {
        self.leap_seconds += 1;
    }

    /// Record time jump.
    pub fn time_jump(&mut self) {
        self.time_jumps += 1;
    }

    /// Record suspend/resume cycle.
    pub fn suspend_resume(&mut self) {
        self.suspend_cycles += 1;
    }

    /// Record clock source switch.
    pub fn clock_switch(&mut self) {
        self.clock_switches += 1;
    }

    /// Record error correction.
    pub fn correction(&mut self) {
        self.corrections += 1;
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.updates = 0;
        self.leap_seconds = 0;
        self.time_jumps = 0;
        self.suspend_cycles = 0;
        self.clock_switches = 0;
        self.corrections = 0;
    }
}

#[cfg(test)]
mod timekeeping_tests {
    use super::*;

    /// F-TK-001: New tracker is empty
    #[test]
    fn f_tk_001_new() {
        let tk = TimeKeepingTracker::new();
        assert_eq!(tk.updates, 0);
    }

    /// F-TK-002: Default is empty
    #[test]
    fn f_tk_002_default() {
        let tk = TimeKeepingTracker::default();
        assert_eq!(tk.updates, 0);
    }

    /// F-TK-003: Update tracked
    #[test]
    fn f_tk_003_update() {
        let mut tk = TimeKeepingTracker::new();
        tk.update();
        assert_eq!(tk.updates, 1);
    }

    /// F-TK-004: Leap second tracked
    #[test]
    fn f_tk_004_leap() {
        let mut tk = TimeKeepingTracker::new();
        tk.leap_second();
        assert_eq!(tk.leap_seconds, 1);
    }

    /// F-TK-005: Time jump tracked
    #[test]
    fn f_tk_005_jump() {
        let mut tk = TimeKeepingTracker::new();
        tk.time_jump();
        assert_eq!(tk.time_jumps, 1);
    }

    /// F-TK-006: Suspend tracked
    #[test]
    fn f_tk_006_suspend() {
        let mut tk = TimeKeepingTracker::new();
        tk.suspend_resume();
        assert_eq!(tk.suspend_cycles, 1);
    }

    /// F-TK-007: Clock switch tracked
    #[test]
    fn f_tk_007_switch() {
        let mut tk = TimeKeepingTracker::new();
        tk.clock_switch();
        assert_eq!(tk.clock_switches, 1);
    }

    /// F-TK-008: Correction tracked
    #[test]
    fn f_tk_008_correction() {
        let mut tk = TimeKeepingTracker::new();
        tk.correction();
        assert_eq!(tk.corrections, 1);
    }

    /// F-TK-009: Factory for_system
    #[test]
    fn f_tk_009_system() {
        let tk = TimeKeepingTracker::for_system();
        assert_eq!(tk.updates, 0);
    }

    /// F-TK-010: Factory for_boot
    #[test]
    fn f_tk_010_boot() {
        let tk = TimeKeepingTracker::for_boot();
        assert_eq!(tk.updates, 0);
    }

    /// F-TK-011: Reset clears counters
    #[test]
    fn f_tk_011_reset() {
        let mut tk = TimeKeepingTracker::new();
        tk.update();
        tk.reset();
        assert_eq!(tk.updates, 0);
    }

    /// F-TK-012: Clone preserves state
    #[test]
    fn f_tk_012_clone() {
        let mut tk = TimeKeepingTracker::new();
        tk.update();
        let cloned = tk;
        assert_eq!(tk.updates, cloned.updates);
    }
}

// ============================================================================
// v9.44.0: I/O Path Helpers
// ============================================================================

/// O(1) AIO (Async I/O) tracker.
///
/// Tracks Linux kernel AIO operations including submissions, completions,
/// and queue depths for asynchronous I/O.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::AioTracker;
///
/// let mut aio = AioTracker::new();
/// aio.submit(4);
/// aio.complete(2);
/// assert_eq!(aio.submissions, 4);
/// assert_eq!(aio.completions, 2);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AioTracker {
    /// Submissions.
    pub submissions: u64,
    /// Completions.
    pub completions: u64,
    /// Cancellations.
    pub cancels: u64,
    /// Bytes transferred.
    pub bytes: u64,
    /// Current queue depth.
    pub queue_depth: u32,
    /// Peak queue depth.
    pub peak_queue_depth: u32,
}

impl AioTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            submissions: 0,
            completions: 0,
            cancels: 0,
            bytes: 0,
            queue_depth: 0,
            peak_queue_depth: 0,
        }
    }

    /// Factory for read operations.
    #[must_use]
    pub const fn for_read() -> Self {
        Self::new()
    }

    /// Factory for write operations.
    #[must_use]
    pub const fn for_write() -> Self {
        Self::new()
    }

    /// Record submissions.
    pub fn submit(&mut self, count: u64) {
        self.submissions += count;
        self.queue_depth = self.queue_depth.saturating_add(count as u32);
        if self.queue_depth > self.peak_queue_depth {
            self.peak_queue_depth = self.queue_depth;
        }
    }

    /// Record completions.
    pub fn complete(&mut self, count: u64) {
        self.completions += count;
        self.queue_depth = self.queue_depth.saturating_sub(count as u32);
    }

    /// Record cancellation.
    pub fn cancel(&mut self) {
        self.cancels += 1;
        self.queue_depth = self.queue_depth.saturating_sub(1);
    }

    /// Record bytes transferred.
    pub fn transfer(&mut self, bytes: u64) {
        self.bytes += bytes;
    }

    /// Get pending operations.
    #[must_use]
    pub fn pending(&self) -> u64 {
        self.submissions
            .saturating_sub(self.completions + self.cancels)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.submissions = 0;
        self.completions = 0;
        self.cancels = 0;
        self.bytes = 0;
        self.queue_depth = 0;
    }
}

#[cfg(test)]
mod aio_tests {
    use super::*;

    /// F-AIO-001: New tracker is empty
    #[test]
    fn f_aio_001_new() {
        let aio = AioTracker::new();
        assert_eq!(aio.submissions, 0);
    }

    /// F-AIO-002: Default is empty
    #[test]
    fn f_aio_002_default() {
        let aio = AioTracker::default();
        assert_eq!(aio.submissions, 0);
    }

    /// F-AIO-003: Submit tracked
    #[test]
    fn f_aio_003_submit() {
        let mut aio = AioTracker::new();
        aio.submit(4);
        assert_eq!(aio.submissions, 4);
        assert_eq!(aio.queue_depth, 4);
    }

    /// F-AIO-004: Complete tracked
    #[test]
    fn f_aio_004_complete() {
        let mut aio = AioTracker::new();
        aio.submit(4);
        aio.complete(2);
        assert_eq!(aio.completions, 2);
        assert_eq!(aio.queue_depth, 2);
    }

    /// F-AIO-005: Cancel tracked
    #[test]
    fn f_aio_005_cancel() {
        let mut aio = AioTracker::new();
        aio.submit(4);
        aio.cancel();
        assert_eq!(aio.cancels, 1);
    }

    /// F-AIO-006: Bytes tracked
    #[test]
    fn f_aio_006_bytes() {
        let mut aio = AioTracker::new();
        aio.transfer(4096);
        assert_eq!(aio.bytes, 4096);
    }

    /// F-AIO-007: Peak queue depth
    #[test]
    fn f_aio_007_peak() {
        let mut aio = AioTracker::new();
        aio.submit(10);
        aio.complete(5);
        aio.submit(2);
        assert_eq!(aio.peak_queue_depth, 10);
    }

    /// F-AIO-008: Pending operations
    #[test]
    fn f_aio_008_pending() {
        let mut aio = AioTracker::new();
        aio.submit(10);
        aio.complete(3);
        aio.cancel();
        assert_eq!(aio.pending(), 6);
    }

    /// F-AIO-009: Factory for_read
    #[test]
    fn f_aio_009_read() {
        let aio = AioTracker::for_read();
        assert_eq!(aio.submissions, 0);
    }

    /// F-AIO-010: Factory for_write
    #[test]
    fn f_aio_010_write() {
        let aio = AioTracker::for_write();
        assert_eq!(aio.submissions, 0);
    }

    /// F-AIO-011: Reset clears counters
    #[test]
    fn f_aio_011_reset() {
        let mut aio = AioTracker::new();
        aio.submit(4);
        aio.reset();
        assert_eq!(aio.submissions, 0);
    }

    /// F-AIO-012: Clone preserves state
    #[test]
    fn f_aio_012_clone() {
        let mut aio = AioTracker::new();
        aio.submit(4);
        let cloned = aio;
        assert_eq!(aio.submissions, cloned.submissions);
    }
}

/// O(1) direct I/O tracker.
///
/// Tracks direct I/O operations bypassing page cache including
/// alignment issues, fallbacks, and performance metrics.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::DirectIoTracker;
///
/// let mut dio = DirectIoTracker::new();
/// dio.read(4096);
/// dio.write(8192);
/// assert_eq!(dio.reads, 1);
/// assert_eq!(dio.writes, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DirectIoTracker {
    /// Direct reads.
    pub reads: u64,
    /// Direct writes.
    pub writes: u64,
    /// Bytes read.
    pub bytes_read: u64,
    /// Bytes written.
    pub bytes_written: u64,
    /// Alignment failures.
    pub alignment_fails: u64,
    /// Fallbacks to buffered.
    pub fallbacks: u64,
}

impl DirectIoTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            reads: 0,
            writes: 0,
            bytes_read: 0,
            bytes_written: 0,
            alignment_fails: 0,
            fallbacks: 0,
        }
    }

    /// Factory for ext4.
    #[must_use]
    pub const fn for_ext4() -> Self {
        Self::new()
    }

    /// Factory for xfs.
    #[must_use]
    pub const fn for_xfs() -> Self {
        Self::new()
    }

    /// Record direct read.
    pub fn read(&mut self, bytes: u64) {
        self.reads += 1;
        self.bytes_read += bytes;
    }

    /// Record direct write.
    pub fn write(&mut self, bytes: u64) {
        self.writes += 1;
        self.bytes_written += bytes;
    }

    /// Record alignment failure.
    pub fn alignment_fail(&mut self) {
        self.alignment_fails += 1;
    }

    /// Record fallback to buffered.
    pub fn fallback(&mut self) {
        self.fallbacks += 1;
    }

    /// Get total bytes.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.bytes_read + self.bytes_written
    }

    /// Get total operations.
    #[must_use]
    pub fn total_ops(&self) -> u64 {
        self.reads + self.writes
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.reads = 0;
        self.writes = 0;
        self.bytes_read = 0;
        self.bytes_written = 0;
        self.alignment_fails = 0;
        self.fallbacks = 0;
    }
}

#[cfg(test)]
mod dio_tests {
    use super::*;

    /// F-DIO-001: New tracker is empty
    #[test]
    fn f_dio_001_new() {
        let dio = DirectIoTracker::new();
        assert_eq!(dio.reads, 0);
    }

    /// F-DIO-002: Default is empty
    #[test]
    fn f_dio_002_default() {
        let dio = DirectIoTracker::default();
        assert_eq!(dio.reads, 0);
    }

    /// F-DIO-003: Read tracked
    #[test]
    fn f_dio_003_read() {
        let mut dio = DirectIoTracker::new();
        dio.read(4096);
        assert_eq!(dio.reads, 1);
        assert_eq!(dio.bytes_read, 4096);
    }

    /// F-DIO-004: Write tracked
    #[test]
    fn f_dio_004_write() {
        let mut dio = DirectIoTracker::new();
        dio.write(8192);
        assert_eq!(dio.writes, 1);
        assert_eq!(dio.bytes_written, 8192);
    }

    /// F-DIO-005: Alignment fail tracked
    #[test]
    fn f_dio_005_alignment() {
        let mut dio = DirectIoTracker::new();
        dio.alignment_fail();
        assert_eq!(dio.alignment_fails, 1);
    }

    /// F-DIO-006: Fallback tracked
    #[test]
    fn f_dio_006_fallback() {
        let mut dio = DirectIoTracker::new();
        dio.fallback();
        assert_eq!(dio.fallbacks, 1);
    }

    /// F-DIO-007: Total bytes
    #[test]
    fn f_dio_007_total_bytes() {
        let mut dio = DirectIoTracker::new();
        dio.read(1000);
        dio.write(2000);
        assert_eq!(dio.total_bytes(), 3000);
    }

    /// F-DIO-008: Total ops
    #[test]
    fn f_dio_008_total_ops() {
        let mut dio = DirectIoTracker::new();
        dio.read(1000);
        dio.write(2000);
        assert_eq!(dio.total_ops(), 2);
    }

    /// F-DIO-009: Factory for_ext4
    #[test]
    fn f_dio_009_ext4() {
        let dio = DirectIoTracker::for_ext4();
        assert_eq!(dio.reads, 0);
    }

    /// F-DIO-010: Factory for_xfs
    #[test]
    fn f_dio_010_xfs() {
        let dio = DirectIoTracker::for_xfs();
        assert_eq!(dio.reads, 0);
    }

    /// F-DIO-011: Reset clears counters
    #[test]
    fn f_dio_011_reset() {
        let mut dio = DirectIoTracker::new();
        dio.read(4096);
        dio.reset();
        assert_eq!(dio.reads, 0);
    }

    /// F-DIO-012: Clone preserves state
    #[test]
    fn f_dio_012_clone() {
        let mut dio = DirectIoTracker::new();
        dio.read(4096);
        let cloned = dio;
        assert_eq!(dio.reads, cloned.reads);
    }
}

/// O(1) buffered I/O tracker.
///
/// Tracks buffered I/O operations through page cache including
/// reads, writes, cache hits/misses, and writeback operations.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::BufferedIoTracker;
///
/// let mut bio = BufferedIoTracker::new();
/// bio.read_hit(4096);
/// bio.write(8192);
/// assert_eq!(bio.cache_hits, 1);
/// assert_eq!(bio.writes, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BufferedIoTracker {
    /// Cache hits.
    pub cache_hits: u64,
    /// Cache misses.
    pub cache_misses: u64,
    /// Writes (dirty pages).
    pub writes: u64,
    /// Writeback completions.
    pub writebacks: u64,
    /// Bytes read (from cache).
    pub bytes_read: u64,
    /// Bytes written (dirty).
    pub bytes_written: u64,
}

impl BufferedIoTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cache_hits: 0,
            cache_misses: 0,
            writes: 0,
            writebacks: 0,
            bytes_read: 0,
            bytes_written: 0,
        }
    }

    /// Factory for read-heavy workload.
    #[must_use]
    pub const fn for_read_heavy() -> Self {
        Self::new()
    }

    /// Factory for write-heavy workload.
    #[must_use]
    pub const fn for_write_heavy() -> Self {
        Self::new()
    }

    /// Record cache hit read.
    pub fn read_hit(&mut self, bytes: u64) {
        self.cache_hits += 1;
        self.bytes_read += bytes;
    }

    /// Record cache miss read.
    pub fn read_miss(&mut self, bytes: u64) {
        self.cache_misses += 1;
        self.bytes_read += bytes;
    }

    /// Record write (dirty page).
    pub fn write(&mut self, bytes: u64) {
        self.writes += 1;
        self.bytes_written += bytes;
    }

    /// Record writeback completion.
    pub fn writeback(&mut self) {
        self.writebacks += 1;
    }

    /// Get hit rate percentage.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            return 0.0;
        }
        (self.cache_hits as f64) / (total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.cache_hits = 0;
        self.cache_misses = 0;
        self.writes = 0;
        self.writebacks = 0;
        self.bytes_read = 0;
        self.bytes_written = 0;
    }
}

#[cfg(test)]
mod buffered_io_tests {
    use super::*;

    /// F-BIO-001: New tracker is empty
    #[test]
    fn f_bio_001_new() {
        let bio = BufferedIoTracker::new();
        assert_eq!(bio.cache_hits, 0);
    }

    /// F-BIO-002: Default is empty
    #[test]
    fn f_bio_002_default() {
        let bio = BufferedIoTracker::default();
        assert_eq!(bio.cache_hits, 0);
    }

    /// F-BIO-003: Read hit tracked
    #[test]
    fn f_bio_003_hit() {
        let mut bio = BufferedIoTracker::new();
        bio.read_hit(4096);
        assert_eq!(bio.cache_hits, 1);
        assert_eq!(bio.bytes_read, 4096);
    }

    /// F-BIO-004: Read miss tracked
    #[test]
    fn f_bio_004_miss() {
        let mut bio = BufferedIoTracker::new();
        bio.read_miss(4096);
        assert_eq!(bio.cache_misses, 1);
    }

    /// F-BIO-005: Write tracked
    #[test]
    fn f_bio_005_write() {
        let mut bio = BufferedIoTracker::new();
        bio.write(8192);
        assert_eq!(bio.writes, 1);
        assert_eq!(bio.bytes_written, 8192);
    }

    /// F-BIO-006: Writeback tracked
    #[test]
    fn f_bio_006_writeback() {
        let mut bio = BufferedIoTracker::new();
        bio.writeback();
        assert_eq!(bio.writebacks, 1);
    }

    /// F-BIO-007: Hit rate
    #[test]
    fn f_bio_007_hit_rate() {
        let mut bio = BufferedIoTracker::new();
        bio.read_hit(1000);
        bio.read_miss(1000);
        assert!((bio.hit_rate() - 50.0).abs() < 0.01);
    }

    /// F-BIO-008: Total bytes
    #[test]
    fn f_bio_008_total_bytes() {
        let mut bio = BufferedIoTracker::new();
        bio.read_hit(1000);
        bio.write(2000);
        assert_eq!(bio.bytes_read + bio.bytes_written, 3000);
    }

    /// F-BIO-009: Factory for_read_heavy
    #[test]
    fn f_bio_009_read_heavy() {
        let bio = BufferedIoTracker::for_read_heavy();
        assert_eq!(bio.cache_hits, 0);
    }

    /// F-BIO-010: Factory for_write_heavy
    #[test]
    fn f_bio_010_write_heavy() {
        let bio = BufferedIoTracker::for_write_heavy();
        assert_eq!(bio.cache_hits, 0);
    }

    /// F-BIO-011: Reset clears counters
    #[test]
    fn f_bio_011_reset() {
        let mut bio = BufferedIoTracker::new();
        bio.read_hit(4096);
        bio.reset();
        assert_eq!(bio.cache_hits, 0);
    }

    /// F-BIO-012: Clone preserves state
    #[test]
    fn f_bio_012_clone() {
        let mut bio = BufferedIoTracker::new();
        bio.read_hit(4096);
        let cloned = bio;
        assert_eq!(bio.cache_hits, cloned.cache_hits);
    }
}

/// O(1) splice/sendfile tracker.
///
/// Tracks zero-copy I/O operations using splice, sendfile, and
/// copy_file_range syscalls for efficient data movement.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::SpliceTracker;
///
/// let mut splice = SpliceTracker::new();
/// splice.splice(1024 * 1024);
/// splice.sendfile(512 * 1024);
/// assert_eq!(splice.splices, 1);
/// assert_eq!(splice.sendfiles, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpliceTracker {
    /// Splice operations.
    pub splices: u64,
    /// Sendfile operations.
    pub sendfiles: u64,
    /// copy_file_range operations.
    pub copy_ranges: u64,
    /// Bytes via splice.
    pub splice_bytes: u64,
    /// Bytes via sendfile.
    pub sendfile_bytes: u64,
    /// Fallbacks to read/write.
    pub fallbacks: u64,
}

impl SpliceTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            splices: 0,
            sendfiles: 0,
            copy_ranges: 0,
            splice_bytes: 0,
            sendfile_bytes: 0,
            fallbacks: 0,
        }
    }

    /// Factory for pipe operations.
    #[must_use]
    pub const fn for_pipe() -> Self {
        Self::new()
    }

    /// Factory for socket operations.
    #[must_use]
    pub const fn for_socket() -> Self {
        Self::new()
    }

    /// Record splice operation.
    pub fn splice(&mut self, bytes: u64) {
        self.splices += 1;
        self.splice_bytes += bytes;
    }

    /// Record sendfile operation.
    pub fn sendfile(&mut self, bytes: u64) {
        self.sendfiles += 1;
        self.sendfile_bytes += bytes;
    }

    /// Record copy_file_range operation.
    pub fn copy_range(&mut self) {
        self.copy_ranges += 1;
    }

    /// Record fallback.
    pub fn fallback(&mut self) {
        self.fallbacks += 1;
    }

    /// Get total zero-copy bytes.
    #[must_use]
    pub fn total_zero_copy_bytes(&self) -> u64 {
        self.splice_bytes + self.sendfile_bytes
    }

    /// Get total operations.
    #[must_use]
    pub fn total_ops(&self) -> u64 {
        self.splices + self.sendfiles + self.copy_ranges
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.splices = 0;
        self.sendfiles = 0;
        self.copy_ranges = 0;
        self.splice_bytes = 0;
        self.sendfile_bytes = 0;
        self.fallbacks = 0;
    }
}

#[cfg(test)]
mod splice_tests {
    use super::*;

    /// F-SPLICE-001: New tracker is empty
    #[test]
    fn f_splice_001_new() {
        let splice = SpliceTracker::new();
        assert_eq!(splice.splices, 0);
    }

    /// F-SPLICE-002: Default is empty
    #[test]
    fn f_splice_002_default() {
        let splice = SpliceTracker::default();
        assert_eq!(splice.splices, 0);
    }

    /// F-SPLICE-003: Splice tracked
    #[test]
    fn f_splice_003_splice() {
        let mut splice = SpliceTracker::new();
        splice.splice(1024);
        assert_eq!(splice.splices, 1);
        assert_eq!(splice.splice_bytes, 1024);
    }

    /// F-SPLICE-004: Sendfile tracked
    #[test]
    fn f_splice_004_sendfile() {
        let mut splice = SpliceTracker::new();
        splice.sendfile(2048);
        assert_eq!(splice.sendfiles, 1);
        assert_eq!(splice.sendfile_bytes, 2048);
    }

    /// F-SPLICE-005: Copy range tracked
    #[test]
    fn f_splice_005_copy_range() {
        let mut splice = SpliceTracker::new();
        splice.copy_range();
        assert_eq!(splice.copy_ranges, 1);
    }

    /// F-SPLICE-006: Fallback tracked
    #[test]
    fn f_splice_006_fallback() {
        let mut splice = SpliceTracker::new();
        splice.fallback();
        assert_eq!(splice.fallbacks, 1);
    }

    /// F-SPLICE-007: Total zero-copy bytes
    #[test]
    fn f_splice_007_total_bytes() {
        let mut splice = SpliceTracker::new();
        splice.splice(1000);
        splice.sendfile(2000);
        assert_eq!(splice.total_zero_copy_bytes(), 3000);
    }

    /// F-SPLICE-008: Total ops
    #[test]
    fn f_splice_008_total_ops() {
        let mut splice = SpliceTracker::new();
        splice.splice(1000);
        splice.sendfile(1000);
        splice.copy_range();
        assert_eq!(splice.total_ops(), 3);
    }

    /// F-SPLICE-009: Factory for_pipe
    #[test]
    fn f_splice_009_pipe() {
        let splice = SpliceTracker::for_pipe();
        assert_eq!(splice.splices, 0);
    }

    /// F-SPLICE-010: Factory for_socket
    #[test]
    fn f_splice_010_socket() {
        let splice = SpliceTracker::for_socket();
        assert_eq!(splice.splices, 0);
    }

    /// F-SPLICE-011: Reset clears counters
    #[test]
    fn f_splice_011_reset() {
        let mut splice = SpliceTracker::new();
        splice.splice(1024);
        splice.reset();
        assert_eq!(splice.splices, 0);
    }

    /// F-SPLICE-012: Clone preserves state
    #[test]
    fn f_splice_012_clone() {
        let mut splice = SpliceTracker::new();
        splice.splice(1024);
        let cloned = splice;
        assert_eq!(splice.splices, cloned.splices);
    }
}

// ============================================================================
// v9.45.0: Process Accounting O(1) Helpers
// ============================================================================

define_tracker! {
    /// Task accounting tracker - per-task CPU and scheduling statistics.
    ///
    /// O(1) tracking of task-level CPU, I/O, and memory accounting
    /// from /proc/[pid]/stat and taskstats.
    pub struct TaskAccountingTracker {
        /// User CPU time (clock ticks)
        pub utime: u64,
        /// System CPU time (clock ticks)
        pub stime: u64,
        /// Children user CPU time
        pub cutime: u64,
        /// Children system CPU time
        pub cstime: u64,
        /// Voluntary context switches
        pub voluntary_ctxt_switches: u64,
        /// Involuntary context switches
        pub nonvoluntary_ctxt_switches: u64,
    }
}

impl TaskAccountingTracker {
    /// Factory: Create for process stats
    #[inline]
    #[must_use]
    pub fn for_proc(utime: u64, stime: u64) -> Self {
        Self {
            utime,
            stime,
            ..Self::new()
        }
    }

    /// Record user time increment
    #[inline]
    pub fn add_utime(&mut self, ticks: u64) {
        self.utime = self.utime.saturating_add(ticks);
    }

    /// Record system time increment
    #[inline]
    pub fn add_stime(&mut self, ticks: u64) {
        self.stime = self.stime.saturating_add(ticks);
    }

    /// Record voluntary context switch
    #[inline]
    pub fn voluntary_switch(&mut self) {
        self.voluntary_ctxt_switches = self.voluntary_ctxt_switches.saturating_add(1);
    }

    /// Record involuntary context switch
    #[inline]
    pub fn involuntary_switch(&mut self) {
        self.nonvoluntary_ctxt_switches = self.nonvoluntary_ctxt_switches.saturating_add(1);
    }

    /// Total CPU time (user + system)
    #[inline]
    #[must_use]
    pub const fn total_cpu(&self) -> u64 {
        self.utime + self.stime
    }

    /// Total context switches
    #[inline]
    #[must_use]
    pub const fn total_switches(&self) -> u64 {
        self.voluntary_ctxt_switches + self.nonvoluntary_ctxt_switches
    }
}

define_tracker! {
    /// I/O accounting tracker - per-task I/O statistics.
    ///
    /// O(1) tracking of task I/O from /proc/[pid]/io.
    pub struct IoAccountingTracker {
        /// Bytes read (rchar)
        pub read_bytes: u64,
        /// Bytes written (wchar)
        pub write_bytes: u64,
        /// Read syscalls
        pub read_syscalls: u64,
        /// Write syscalls
        pub write_syscalls: u64,
        /// Actual disk read bytes
        pub disk_read_bytes: u64,
        /// Actual disk write bytes
        pub disk_write_bytes: u64,
    }
}

impl IoAccountingTracker {
    /// Factory: Create from /proc/[pid]/io stats
    #[inline]
    #[must_use]
    pub fn for_proc_io(read_bytes: u64, write_bytes: u64) -> Self {
        Self {
            read_bytes,
            write_bytes,
            ..Self::new()
        }
    }

    /// Record read operation
    #[inline]
    pub fn read(&mut self, bytes: u64) {
        self.read_bytes = self.read_bytes.saturating_add(bytes);
        self.read_syscalls = self.read_syscalls.saturating_add(1);
    }

    /// Record write operation
    #[inline]
    pub fn write(&mut self, bytes: u64) {
        self.write_bytes = self.write_bytes.saturating_add(bytes);
        self.write_syscalls = self.write_syscalls.saturating_add(1);
    }

    /// Record disk read
    #[inline]
    pub fn disk_read(&mut self, bytes: u64) {
        self.disk_read_bytes = self.disk_read_bytes.saturating_add(bytes);
    }

    /// Record disk write
    #[inline]
    pub fn disk_write(&mut self, bytes: u64) {
        self.disk_write_bytes = self.disk_write_bytes.saturating_add(bytes);
    }

    /// Total I/O bytes
    #[inline]
    #[must_use]
    pub const fn total_bytes(&self) -> u64 {
        self.read_bytes + self.write_bytes
    }

    /// Total syscalls
    #[inline]
    #[must_use]
