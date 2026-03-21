// HeapFragmentationTracker - O(1) heap fragmentation tracking
// ============================================================================

/// O(1) heap fragmentation tracking.
///
/// Tracks heap allocations and fragmentation patterns.
#[derive(Debug, Clone)]
pub struct HeapFragmentationTracker {
    /// Total allocated bytes
    pub allocated: u64,
    /// Total freed bytes
    pub freed: u64,
    /// Allocation count
    pub allocations: u64,
    /// Free count
    pub frees: u64,
    /// Peak allocated
    pub peak_allocated: u64,
    /// Fragmentation events
    pub fragmentation_events: u64,
}

impl Default for HeapFragmentationTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl HeapFragmentationTracker {
    /// Create new heap fragmentation tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            allocated: 0,
            freed: 0,
            allocations: 0,
            frees: 0,
            peak_allocated: 0,
            fragmentation_events: 0,
        }
    }

    /// Factory for jemalloc tracking.
    #[must_use]
    pub fn for_jemalloc() -> Self {
        Self::new()
    }

    /// Factory for system allocator tracking.
    #[must_use]
    pub fn for_system() -> Self {
        Self::new()
    }

    /// Record allocation.
    pub fn allocate(&mut self, bytes: u64) {
        self.allocated += bytes;
        self.allocations += 1;
        let current = self.allocated.saturating_sub(self.freed);
        if current > self.peak_allocated {
            self.peak_allocated = current;
        }
    }

    /// Record free.
    pub fn free(&mut self, bytes: u64) {
        self.freed += bytes;
        self.frees += 1;
    }

    /// Record fragmentation event.
    pub fn fragment(&mut self) {
        self.fragmentation_events += 1;
    }

    /// Get current memory in use.
    #[must_use]
    pub fn in_use(&self) -> u64 {
        self.allocated.saturating_sub(self.freed)
    }

    /// Get fragmentation rate.
    #[must_use]
    pub fn fragmentation_rate(&self) -> f64 {
        if self.allocations == 0 {
            return 0.0;
        }
        (self.fragmentation_events as f64 / self.allocations as f64) * 100.0
    }

    /// Check if fragmentation is excessive (>5%).
    #[must_use]
    pub fn is_fragmented(&self) -> bool {
        self.fragmentation_rate() > 5.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocated = 0;
        self.freed = 0;
        self.allocations = 0;
        self.frees = 0;
        self.peak_allocated = 0;
        self.fragmentation_events = 0;
    }
}

#[cfg(test)]
mod heap_frag_tests {
    use super::*;

    /// F-HEAP-001: New tracker is empty
    #[test]
    fn f_heap_001_new() {
        let hf = HeapFragmentationTracker::new();
        assert_eq!(hf.in_use(), 0);
    }

    /// F-HEAP-002: Default is empty
    #[test]
    fn f_heap_002_default() {
        let hf = HeapFragmentationTracker::default();
        assert_eq!(hf.in_use(), 0);
    }

    /// F-HEAP-003: Allocate increases bytes
    #[test]
    fn f_heap_003_allocate() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocate(1024);
        assert_eq!(hf.allocated, 1024);
        assert_eq!(hf.allocations, 1);
    }

    /// F-HEAP-004: Free tracks bytes
    #[test]
    fn f_heap_004_free() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocate(1024);
        hf.free(512);
        assert_eq!(hf.in_use(), 512);
    }

    /// F-HEAP-005: Peak tracked
    #[test]
    fn f_heap_005_peak() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocate(1024);
        hf.free(512);
        hf.allocate(256);
        assert_eq!(hf.peak_allocated, 1024);
    }

    /// F-HEAP-006: Fragmentation tracked
    #[test]
    fn f_heap_006_fragment() {
        let mut hf = HeapFragmentationTracker::new();
        hf.fragment();
        assert_eq!(hf.fragmentation_events, 1);
    }

    /// F-HEAP-007: Factory for_jemalloc
    #[test]
    fn f_heap_007_for_jemalloc() {
        let hf = HeapFragmentationTracker::for_jemalloc();
        assert_eq!(hf.in_use(), 0);
    }

    /// F-HEAP-008: Factory for_system
    #[test]
    fn f_heap_008_for_system() {
        let hf = HeapFragmentationTracker::for_system();
        assert_eq!(hf.in_use(), 0);
    }

    /// F-HEAP-009: Fragmentation rate calculated
    #[test]
    fn f_heap_009_frag_rate() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocations = 100;
        hf.fragmentation_events = 10;
        assert!((hf.fragmentation_rate() - 10.0).abs() < 0.01);
    }

    /// F-HEAP-010: Is fragmented check
    #[test]
    fn f_heap_010_is_fragmented() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocations = 100;
        hf.fragmentation_events = 10;
        assert!(hf.is_fragmented());
    }

    /// F-HEAP-011: Reset clears state
    #[test]
    fn f_heap_011_reset() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocate(1024);
        hf.reset();
        assert_eq!(hf.in_use(), 0);
    }

    /// F-HEAP-012: Clone preserves state
    #[test]
    fn f_heap_012_clone() {
        let mut hf = HeapFragmentationTracker::new();
        hf.allocate(1024);
        let cloned = hf.clone();
        assert_eq!(hf.allocated, cloned.allocated);
    }
}

// ============================================================================
// StackDepthTracker - O(1) stack depth tracking
// ============================================================================

/// O(1) stack depth tracking.
///
/// Tracks call stack depth for recursion analysis.
#[derive(Debug, Clone)]
pub struct StackDepthTracker {
    /// Current depth
    pub depth: u32,
    /// Peak depth
    pub peak_depth: u32,
    /// Total calls
    pub calls: u64,
    /// Stack overflow warnings
    pub warnings: u64,
    /// Warning threshold
    pub threshold: u32,
}

impl Default for StackDepthTracker {
    fn default() -> Self {
        Self::for_default()
    }
}

impl StackDepthTracker {
    /// Create new stack depth tracker with threshold.
    #[must_use]
    pub fn new(threshold: u32) -> Self {
        Self {
            depth: 0,
            peak_depth: 0,
            calls: 0,
            warnings: 0,
            threshold,
        }
    }

    /// Factory for default tracking (100 depth).
    #[must_use]
    pub fn for_default() -> Self {
        Self::new(100)
    }

    /// Factory for deep recursion tracking (1000 depth).
    #[must_use]
    pub fn for_deep() -> Self {
        Self::new(1000)
    }

    /// Record function entry.
    pub fn enter(&mut self) {
        self.depth += 1;
        self.calls += 1;
        if self.depth > self.peak_depth {
            self.peak_depth = self.depth;
        }
        if self.depth > self.threshold {
            self.warnings += 1;
        }
    }

    /// Record function exit.
    pub fn exit(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// Get current depth.
    #[must_use]
    pub fn current(&self) -> u32 {
        self.depth
    }

    /// Get depth utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.threshold == 0 {
            return 0.0;
        }
        (self.depth as f64 / self.threshold as f64) * 100.0
    }

    /// Check if approaching threshold.
    #[must_use]
    pub fn is_at_risk(&self) -> bool {
        self.utilization() > 80.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.depth = 0;
        self.peak_depth = 0;
        self.calls = 0;
        self.warnings = 0;
    }
}

#[cfg(test)]
mod stack_depth_tests {
    use super::*;

    /// F-STACK-001: New tracker has threshold
    #[test]
    fn f_stack_001_new() {
        let sd = StackDepthTracker::new(100);
        assert_eq!(sd.threshold, 100);
    }

    /// F-STACK-002: Default uses 100
    #[test]
    fn f_stack_002_default() {
        let sd = StackDepthTracker::default();
        assert_eq!(sd.threshold, 100);
    }

    /// F-STACK-003: Enter increases depth
    #[test]
    fn f_stack_003_enter() {
        let mut sd = StackDepthTracker::new(100);
        sd.enter();
        assert_eq!(sd.depth, 1);
        assert_eq!(sd.calls, 1);
    }

    /// F-STACK-004: Exit decreases depth
    #[test]
    fn f_stack_004_exit() {
        let mut sd = StackDepthTracker::new(100);
        sd.enter();
        sd.exit();
        assert_eq!(sd.depth, 0);
    }

    /// F-STACK-005: Peak tracked
    #[test]
    fn f_stack_005_peak() {
        let mut sd = StackDepthTracker::new(100);
        sd.enter();
        sd.enter();
        sd.exit();
        assert_eq!(sd.peak_depth, 2);
    }

    /// F-STACK-006: Warning on threshold
    #[test]
    fn f_stack_006_warning() {
        let mut sd = StackDepthTracker::new(2);
        sd.enter();
        sd.enter();
        sd.enter();
        assert_eq!(sd.warnings, 1);
    }

    /// F-STACK-007: Factory for_default
    #[test]
    fn f_stack_007_for_default() {
        let sd = StackDepthTracker::for_default();
        assert_eq!(sd.threshold, 100);
    }

    /// F-STACK-008: Factory for_deep
    #[test]
    fn f_stack_008_for_deep() {
        let sd = StackDepthTracker::for_deep();
        assert_eq!(sd.threshold, 1000);
    }

    /// F-STACK-009: Utilization calculated
    #[test]
    fn f_stack_009_utilization() {
        let mut sd = StackDepthTracker::new(100);
        for _ in 0..50 {
            sd.enter();
        }
        assert!((sd.utilization() - 50.0).abs() < 0.01);
    }

    /// F-STACK-010: Is at risk check
    #[test]
    fn f_stack_010_at_risk() {
        let mut sd = StackDepthTracker::new(100);
        for _ in 0..85 {
            sd.enter();
        }
        assert!(sd.is_at_risk());
    }

    /// F-STACK-011: Reset clears state
    #[test]
    fn f_stack_011_reset() {
        let mut sd = StackDepthTracker::new(100);
        sd.enter();
        sd.reset();
        assert_eq!(sd.depth, 0);
    }

    /// F-STACK-012: Clone preserves state
    #[test]
    fn f_stack_012_clone() {
        let mut sd = StackDepthTracker::new(100);
        sd.enter();
        let cloned = sd.clone();
        assert_eq!(sd.depth, cloned.depth);
    }
}

// ============================================================================
// SyscallTracker - O(1) syscall tracking
// ============================================================================

/// O(1) syscall frequency tracking.
///
/// Tracks syscall counts and latency.
#[derive(Debug, Clone)]
pub struct SyscallTracker {
    /// Total syscalls
    pub total: u64,
    /// Read syscalls
    pub reads: u64,
    /// Write syscalls
    pub writes: u64,
    /// Other syscalls
    pub other: u64,
    /// Total latency microseconds
    pub total_latency_us: u64,
    /// Errors
    pub errors: u64,
}

impl Default for SyscallTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SyscallTracker {
    /// Create new syscall tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            reads: 0,
            writes: 0,
            other: 0,
            total_latency_us: 0,
            errors: 0,
        }
    }

    /// Factory for IO-heavy tracking.
    #[must_use]
    pub fn for_io() -> Self {
        Self::new()
    }

    /// Factory for general tracking.
    #[must_use]
    pub fn for_general() -> Self {
        Self::new()
    }

    /// Record read syscall.
    pub fn read(&mut self, latency_us: u64) {
        self.total += 1;
        self.reads += 1;
        self.total_latency_us += latency_us;
    }

    /// Record write syscall.
    pub fn write(&mut self, latency_us: u64) {
        self.total += 1;
        self.writes += 1;
        self.total_latency_us += latency_us;
    }

    /// Record other syscall.
    pub fn other(&mut self, latency_us: u64) {
        self.total += 1;
        self.other += 1;
        self.total_latency_us += latency_us;
    }

    /// Record syscall error.
    pub fn error(&mut self) {
        self.errors += 1;
    }

    /// Get average latency in microseconds.
    #[must_use]
    pub fn avg_latency_us(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        self.total_latency_us / self.total
    }

    /// Get IO percentage (reads + writes).
    #[must_use]
    pub fn io_percentage(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        ((self.reads + self.writes) as f64 / self.total as f64) * 100.0
    }

    /// Get error rate.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.errors as f64 / self.total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.total = 0;
        self.reads = 0;
        self.writes = 0;
        self.other = 0;
        self.total_latency_us = 0;
        self.errors = 0;
    }
}

#[cfg(test)]
mod syscall_tests {
    use super::*;

    /// F-SYSCALL-001: New tracker is empty
    #[test]
    fn f_syscall_001_new() {
        let sc = SyscallTracker::new();
        assert_eq!(sc.total, 0);
    }

    /// F-SYSCALL-002: Default is empty
    #[test]
    fn f_syscall_002_default() {
        let sc = SyscallTracker::default();
        assert_eq!(sc.total, 0);
    }

    /// F-SYSCALL-003: Read tracked
    #[test]
    fn f_syscall_003_read() {
        let mut sc = SyscallTracker::new();
        sc.read(100);
        assert_eq!(sc.reads, 1);
        assert_eq!(sc.total, 1);
    }

    /// F-SYSCALL-004: Write tracked
    #[test]
    fn f_syscall_004_write() {
        let mut sc = SyscallTracker::new();
        sc.write(100);
        assert_eq!(sc.writes, 1);
        assert_eq!(sc.total, 1);
    }

    /// F-SYSCALL-005: Other tracked
    #[test]
    fn f_syscall_005_other() {
        let mut sc = SyscallTracker::new();
        sc.other(100);
        assert_eq!(sc.other, 1);
        assert_eq!(sc.total, 1);
    }

    /// F-SYSCALL-006: Average latency calculated
    #[test]
    fn f_syscall_006_avg_latency() {
        let mut sc = SyscallTracker::new();
        sc.read(100);
        sc.write(200);
        assert_eq!(sc.avg_latency_us(), 150);
    }

    /// F-SYSCALL-007: Factory for_io
    #[test]
    fn f_syscall_007_for_io() {
        let sc = SyscallTracker::for_io();
        assert_eq!(sc.total, 0);
    }

    /// F-SYSCALL-008: Factory for_general
    #[test]
    fn f_syscall_008_for_general() {
        let sc = SyscallTracker::for_general();
        assert_eq!(sc.total, 0);
    }

    /// F-SYSCALL-009: IO percentage calculated
    #[test]
    fn f_syscall_009_io_percentage() {
        let mut sc = SyscallTracker::new();
        sc.read(100);
        sc.write(100);
        sc.other(100);
        sc.other(100);
        assert!((sc.io_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SYSCALL-010: Error tracked
    #[test]
    fn f_syscall_010_error() {
        let mut sc = SyscallTracker::new();
        sc.error();
        assert_eq!(sc.errors, 1);
    }

    /// F-SYSCALL-011: Reset clears state
    #[test]
    fn f_syscall_011_reset() {
        let mut sc = SyscallTracker::new();
        sc.read(100);
        sc.reset();
        assert_eq!(sc.total, 0);
    }

    /// F-SYSCALL-012: Clone preserves state
    #[test]
    fn f_syscall_012_clone() {
        let mut sc = SyscallTracker::new();
        sc.read(100);
        let cloned = sc.clone();
        assert_eq!(sc.total, cloned.total);
    }
}

// ============================================================================
// SignalTracker - O(1) signal tracking
// ============================================================================

/// O(1) signal delivery tracking.
///
/// Tracks signal counts and handling.
#[derive(Debug, Clone)]
pub struct SignalTracker {
    /// Total signals received
    pub received: u64,
    /// Signals handled
    pub handled: u64,
    /// Signals ignored
    pub ignored: u64,
    /// Fatal signals (SIGKILL, SIGSEGV, etc.)
    pub fatal: u64,
    /// Last signal number
    pub last_signal: u32,
}

impl Default for SignalTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalTracker {
    /// Create new signal tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            received: 0,
            handled: 0,
            ignored: 0,
            fatal: 0,
            last_signal: 0,
        }
    }

    /// Factory for process tracking.
    #[must_use]
    pub fn for_process() -> Self {
        Self::new()
    }

    /// Factory for daemon tracking.
    #[must_use]
    pub fn for_daemon() -> Self {
        Self::new()
    }

    /// Record signal received and handled.
    pub fn handle(&mut self, signal: u32) {
        self.received += 1;
        self.handled += 1;
        self.last_signal = signal;
    }

    /// Record signal ignored.
    pub fn ignore(&mut self, signal: u32) {
        self.received += 1;
        self.ignored += 1;
        self.last_signal = signal;
    }

    /// Record fatal signal.
    pub fn fatal(&mut self, signal: u32) {
        self.received += 1;
        self.fatal += 1;
        self.last_signal = signal;
    }

    /// Get handling rate percentage.
    #[must_use]
    pub fn handling_rate(&self) -> f64 {
        if self.received == 0 {
            return 0.0;
        }
        (self.handled as f64 / self.received as f64) * 100.0
    }

    /// Check if process received fatal signal.
    #[must_use]
    pub fn has_fatal(&self) -> bool {
        self.fatal > 0
    }

    /// Get total signals.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.received
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.received = 0;
        self.handled = 0;
        self.ignored = 0;
        self.fatal = 0;
        self.last_signal = 0;
    }
}

#[cfg(test)]
mod signal_tests {
    use super::*;

    /// F-SIGNAL-001: New tracker is empty
    #[test]
    fn f_signal_001_new() {
        let sig = SignalTracker::new();
        assert_eq!(sig.total(), 0);
    }

    /// F-SIGNAL-002: Default is empty
    #[test]
    fn f_signal_002_default() {
        let sig = SignalTracker::default();
        assert_eq!(sig.total(), 0);
    }

    /// F-SIGNAL-003: Handle tracked
    #[test]
    fn f_signal_003_handle() {
        let mut sig = SignalTracker::new();
        sig.handle(15); // SIGTERM
        assert_eq!(sig.handled, 1);
        assert_eq!(sig.received, 1);
    }

    /// F-SIGNAL-004: Ignore tracked
    #[test]
    fn f_signal_004_ignore() {
        let mut sig = SignalTracker::new();
        sig.ignore(1); // SIGHUP
        assert_eq!(sig.ignored, 1);
        assert_eq!(sig.received, 1);
    }

    /// F-SIGNAL-005: Fatal tracked
    #[test]
    fn f_signal_005_fatal() {
        let mut sig = SignalTracker::new();
        sig.fatal(9); // SIGKILL
        assert_eq!(sig.fatal, 1);
        assert!(sig.has_fatal());
    }

    /// F-SIGNAL-006: Handling rate calculated
    #[test]
    fn f_signal_006_handling_rate() {
        let mut sig = SignalTracker::new();
        sig.handle(15);
        sig.ignore(1);
        assert!((sig.handling_rate() - 50.0).abs() < 0.01);
    }

    /// F-SIGNAL-007: Factory for_process
    #[test]
    fn f_signal_007_for_process() {
        let sig = SignalTracker::for_process();
        assert_eq!(sig.total(), 0);
    }

    /// F-SIGNAL-008: Factory for_daemon
    #[test]
    fn f_signal_008_for_daemon() {
        let sig = SignalTracker::for_daemon();
        assert_eq!(sig.total(), 0);
    }

    /// F-SIGNAL-009: Last signal tracked
    #[test]
    fn f_signal_009_last_signal() {
        let mut sig = SignalTracker::new();
        sig.handle(15);
        assert_eq!(sig.last_signal, 15);
    }

    /// F-SIGNAL-010: Has fatal check
    #[test]
    fn f_signal_010_has_fatal() {
        let mut sig = SignalTracker::new();
        sig.handle(15);
        assert!(!sig.has_fatal());
    }

    /// F-SIGNAL-011: Reset clears state
    #[test]
    fn f_signal_011_reset() {
        let mut sig = SignalTracker::new();
        sig.handle(15);
        sig.reset();
        assert_eq!(sig.total(), 0);
    }

    /// F-SIGNAL-012: Clone preserves state
    #[test]
    fn f_signal_012_clone() {
        let mut sig = SignalTracker::new();
        sig.handle(15);
        let cloned = sig.clone();
        assert_eq!(sig.received, cloned.received);
    }
}

// ============================================================================
// FutexTracker - O(1) futex wait/wake tracking (v9.35.0)
// ============================================================================

/// O(1) futex (fast userspace mutex) tracking.
///
/// Tracks futex wait/wake operations for synchronization analysis.
#[derive(Debug, Clone)]
pub struct FutexTracker {
    /// Wait operations
    pub waits: u64,
    /// Wake operations
    pub wakes: u64,
    /// Requeue operations
    pub requeues: u64,
    /// Timeouts
    pub timeouts: u64,
    /// Total wait time in microseconds
    pub total_wait_us: u64,
    /// Peak waiters at any time
    pub peak_waiters: u64,
}

impl Default for FutexTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl FutexTracker {
    /// Create new futex tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            waits: 0,
            wakes: 0,
            requeues: 0,
            timeouts: 0,
            total_wait_us: 0,
            peak_waiters: 0,
        }
    }

    /// Create for mutex workload.
    #[must_use]
    pub const fn for_mutex() -> Self {
        Self::new()
    }

    /// Create for condition variable workload.
    #[must_use]
    pub const fn for_condvar() -> Self {
        Self::new()
    }

    /// Record a wait operation.
    pub fn wait(&mut self, duration_us: u64) {
        self.waits += 1;
        self.total_wait_us += duration_us;
    }

    /// Record a wake operation.
    pub fn wake(&mut self, count: u64) {
        self.wakes += 1;
        if count > self.peak_waiters {
            self.peak_waiters = count;
        }
    }

    /// Record a requeue operation.
    pub fn requeue(&mut self) {
        self.requeues += 1;
    }

    /// Record a timeout.
    pub fn timeout(&mut self) {
        self.timeouts += 1;
    }

    /// Get average wait time.
    #[must_use]
    pub fn avg_wait_us(&self) -> u64 {
        if self.waits == 0 {
            return 0;
        }
        self.total_wait_us / self.waits
    }

    /// Get timeout rate.
    #[must_use]
    pub fn timeout_rate(&self) -> f64 {
        if self.waits == 0 {
            return 0.0;
        }
        (self.timeouts as f64 / self.waits as f64) * 100.0
    }

    /// Total operations.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.waits + self.wakes + self.requeues
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.waits = 0;
        self.wakes = 0;
        self.requeues = 0;
        self.timeouts = 0;
        self.total_wait_us = 0;
        self.peak_waiters = 0;
    }
}

#[cfg(test)]
mod futex_tests {
    use super::*;

    /// F-FUTEX-001: New tracker is empty
    #[test]
    fn f_futex_001_new() {
        let ft = FutexTracker::new();
        assert_eq!(ft.total(), 0);
    }

    /// F-FUTEX-002: Default is empty
    #[test]
    fn f_futex_002_default() {
        let ft = FutexTracker::default();
        assert_eq!(ft.total(), 0);
    }

    /// F-FUTEX-003: Wait tracked
    #[test]
    fn f_futex_003_wait() {
        let mut ft = FutexTracker::new();
        ft.wait(100);
        assert_eq!(ft.waits, 1);
        assert_eq!(ft.total_wait_us, 100);
    }

    /// F-FUTEX-004: Wake tracked
    #[test]
    fn f_futex_004_wake() {
        let mut ft = FutexTracker::new();
        ft.wake(5);
        assert_eq!(ft.wakes, 1);
        assert_eq!(ft.peak_waiters, 5);
    }

    /// F-FUTEX-005: Requeue tracked
    #[test]
    fn f_futex_005_requeue() {
        let mut ft = FutexTracker::new();
        ft.requeue();
        assert_eq!(ft.requeues, 1);
    }

    /// F-FUTEX-006: Timeout tracked
    #[test]
    fn f_futex_006_timeout() {
        let mut ft = FutexTracker::new();
        ft.timeout();
        assert_eq!(ft.timeouts, 1);
    }

    /// F-FUTEX-007: Average wait calculated
    #[test]
    fn f_futex_007_avg_wait() {
        let mut ft = FutexTracker::new();
        ft.wait(100);
        ft.wait(200);
        assert_eq!(ft.avg_wait_us(), 150);
    }

    /// F-FUTEX-008: Timeout rate calculated
    #[test]
    fn f_futex_008_timeout_rate() {
        let mut ft = FutexTracker::new();
        ft.wait(100);
        ft.timeout();
        ft.wait(100);
        assert!((ft.timeout_rate() - 50.0).abs() < 0.01);
    }

    /// F-FUTEX-009: Factory for_mutex
    #[test]
    fn f_futex_009_for_mutex() {
        let ft = FutexTracker::for_mutex();
        assert_eq!(ft.total(), 0);
    }

    /// F-FUTEX-010: Factory for_condvar
    #[test]
    fn f_futex_010_for_condvar() {
        let ft = FutexTracker::for_condvar();
        assert_eq!(ft.total(), 0);
    }

    /// F-FUTEX-011: Reset clears state
    #[test]
    fn f_futex_011_reset() {
        let mut ft = FutexTracker::new();
        ft.wait(100);
        ft.reset();
        assert_eq!(ft.total(), 0);
    }

    /// F-FUTEX-012: Clone preserves state
    #[test]
    fn f_futex_012_clone() {
        let mut ft = FutexTracker::new();
        ft.wait(100);
        let cloned = ft.clone();
        assert_eq!(ft.waits, cloned.waits);
    }
}

// ============================================================================
// EpollTracker - O(1) epoll event tracking (v9.35.0)
// ============================================================================

/// O(1) epoll event loop tracking.
///
/// Tracks epoll_wait calls and event processing efficiency.
#[derive(Debug, Clone)]
pub struct EpollTracker {
    /// Number of epoll_wait calls
    pub waits: u64,
    /// Total events returned
    pub events: u64,
    /// Empty waits (returned 0)
    pub empty_waits: u64,
    /// Timeouts
    pub timeouts: u64,
    /// Peak events per wait
    pub peak_events: u64,
    /// Total wait time in microseconds
    pub total_wait_us: u64,
}

impl Default for EpollTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl EpollTracker {
    /// Create new epoll tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            waits: 0,
            events: 0,
            empty_waits: 0,
            timeouts: 0,
            peak_events: 0,
            total_wait_us: 0,
        }
    }

    /// Create for network server.
    #[must_use]
    pub const fn for_network() -> Self {
        Self::new()
    }

    /// Create for file I/O.
    #[must_use]
    pub const fn for_file_io() -> Self {
        Self::new()
    }

    /// Record a wait returning events.
    pub fn wait(&mut self, event_count: u64, duration_us: u64) {
        self.waits += 1;
        self.events += event_count;
        self.total_wait_us += duration_us;
        if event_count == 0 {
            self.empty_waits += 1;
        }
        if event_count > self.peak_events {
            self.peak_events = event_count;
        }
    }

    /// Record a timeout.
    pub fn timeout(&mut self) {
        self.timeouts += 1;
    }

    /// Get average events per wait.
    #[must_use]
    pub fn avg_events_per_wait(&self) -> f64 {
        if self.waits == 0 {
            return 0.0;
        }
        self.events as f64 / self.waits as f64
    }

    /// Get empty wait rate.
    #[must_use]
    pub fn empty_rate(&self) -> f64 {
        if self.waits == 0 {
            return 0.0;
        }
        (self.empty_waits as f64 / self.waits as f64) * 100.0
    }

    /// Get average wait time.
    #[must_use]
    pub fn avg_wait_us(&self) -> u64 {
        if self.waits == 0 {
            return 0;
        }
        self.total_wait_us / self.waits
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.waits = 0;
        self.events = 0;
        self.empty_waits = 0;
        self.timeouts = 0;
        self.peak_events = 0;
        self.total_wait_us = 0;
    }
}

#[cfg(test)]
mod epoll_tests {
    use super::*;

    /// F-EPOLL-001: New tracker is empty
    #[test]
    fn f_epoll_001_new() {
        let ep = EpollTracker::new();
        assert_eq!(ep.waits, 0);
    }

    /// F-EPOLL-002: Default is empty
    #[test]
    fn f_epoll_002_default() {
        let ep = EpollTracker::default();
        assert_eq!(ep.waits, 0);
    }

    /// F-EPOLL-003: Wait with events tracked
    #[test]
    fn f_epoll_003_wait_events() {
        let mut ep = EpollTracker::new();
        ep.wait(5, 100);
        assert_eq!(ep.events, 5);
        assert_eq!(ep.waits, 1);
    }

    /// F-EPOLL-004: Empty wait tracked
    #[test]
    fn f_epoll_004_empty_wait() {
        let mut ep = EpollTracker::new();
        ep.wait(0, 100);
        assert_eq!(ep.empty_waits, 1);
    }

    /// F-EPOLL-005: Peak events tracked
    #[test]
    fn f_epoll_005_peak_events() {
        let mut ep = EpollTracker::new();
        ep.wait(5, 100);
        ep.wait(10, 100);
        ep.wait(3, 100);
        assert_eq!(ep.peak_events, 10);
    }

    /// F-EPOLL-006: Timeout tracked
    #[test]
    fn f_epoll_006_timeout() {
        let mut ep = EpollTracker::new();
        ep.timeout();
        assert_eq!(ep.timeouts, 1);
    }

    /// F-EPOLL-007: Average events calculated
    #[test]
    fn f_epoll_007_avg_events() {
        let mut ep = EpollTracker::new();
        ep.wait(5, 100);
        ep.wait(15, 100);
        assert!((ep.avg_events_per_wait() - 10.0).abs() < 0.01);
    }

    /// F-EPOLL-008: Empty rate calculated
    #[test]
    fn f_epoll_008_empty_rate() {
        let mut ep = EpollTracker::new();
        ep.wait(0, 100);
        ep.wait(5, 100);
        assert!((ep.empty_rate() - 50.0).abs() < 0.01);
    }

    /// F-EPOLL-009: Factory for_network
    #[test]
    fn f_epoll_009_for_network() {
        let ep = EpollTracker::for_network();
        assert_eq!(ep.waits, 0);
    }

    /// F-EPOLL-010: Factory for_file_io
    #[test]
    fn f_epoll_010_for_file_io() {
        let ep = EpollTracker::for_file_io();
        assert_eq!(ep.waits, 0);
    }

    /// F-EPOLL-011: Reset clears state
    #[test]
    fn f_epoll_011_reset() {
        let mut ep = EpollTracker::new();
        ep.wait(5, 100);
        ep.reset();
        assert_eq!(ep.waits, 0);
    }

    /// F-EPOLL-012: Clone preserves state
    #[test]
    fn f_epoll_012_clone() {
        let mut ep = EpollTracker::new();
        ep.wait(5, 100);
        let cloned = ep.clone();
        assert_eq!(ep.events, cloned.events);
    }
}

// ============================================================================
// MmapTracker - O(1) memory mapping tracking (v9.35.0)
// ============================================================================

/// O(1) memory mapping (mmap) tracking.
///
/// Tracks mmap/munmap operations and memory usage.
#[derive(Debug, Clone)]
pub struct MmapTracker {
    /// Active mappings
    pub active: u64,
    /// Total mmap calls
    pub maps: u64,
    /// Total munmap calls
    pub unmaps: u64,
    /// Total mapped bytes
    pub mapped_bytes: u64,
    /// Peak mapped bytes
    pub peak_mapped_bytes: u64,
    /// Failed mmap calls
    pub failures: u64,
}

impl Default for MmapTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MmapTracker {
    /// Create new mmap tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: 0,
            maps: 0,
            unmaps: 0,
            mapped_bytes: 0,
            peak_mapped_bytes: 0,
            failures: 0,
        }
    }

    /// Create for file-backed mappings.
    #[must_use]
    pub const fn for_file() -> Self {
        Self::new()
    }

    /// Create for anonymous mappings.
    #[must_use]
    pub const fn for_anonymous() -> Self {
        Self::new()
    }

    /// Record a mmap operation.
    pub fn map(&mut self, size: u64) {
        self.maps += 1;
        self.active += 1;
        self.mapped_bytes += size;
        if self.mapped_bytes > self.peak_mapped_bytes {
            self.peak_mapped_bytes = self.mapped_bytes;
        }
    }

    /// Record a munmap operation.
    pub fn unmap(&mut self, size: u64) {
        self.unmaps += 1;
        if self.active > 0 {
            self.active -= 1;
        }
        self.mapped_bytes = self.mapped_bytes.saturating_sub(size);
    }

    /// Record a failed mmap.
    pub fn failure(&mut self) {
        self.failures += 1;
    }

    /// Get failure rate.
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        let total = self.maps + self.failures;
        if total == 0 {
            return 0.0;
        }
        (self.failures as f64 / total as f64) * 100.0
    }

    /// Check for leak (more maps than unmaps).
    #[must_use]
    pub fn has_leak(&self) -> bool {
        self.maps > self.unmaps + 10 // Allow some tolerance
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.active = 0;
        self.maps = 0;
        self.unmaps = 0;
        self.mapped_bytes = 0;
        self.peak_mapped_bytes = 0;
        self.failures = 0;
    }
}

#[cfg(test)]
mod mmap_tests {
    use super::*;

    /// F-MMAP-001: New tracker is empty
    #[test]
    fn f_mmap_001_new() {
        let mm = MmapTracker::new();
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-002: Default is empty
    #[test]
    fn f_mmap_002_default() {
        let mm = MmapTracker::default();
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-003: Map tracked
    #[test]
    fn f_mmap_003_map() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        assert_eq!(mm.maps, 1);
        assert_eq!(mm.active, 1);
        assert_eq!(mm.mapped_bytes, 4096);
    }

    /// F-MMAP-004: Unmap tracked
    #[test]
    fn f_mmap_004_unmap() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        mm.unmap(4096);
        assert_eq!(mm.unmaps, 1);
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-005: Peak bytes tracked
    #[test]
    fn f_mmap_005_peak() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        mm.map(4096);
        mm.unmap(4096);
        assert_eq!(mm.peak_mapped_bytes, 8192);
    }

    /// F-MMAP-006: Failure tracked
    #[test]
    fn f_mmap_006_failure() {
        let mut mm = MmapTracker::new();
        mm.failure();
        assert_eq!(mm.failures, 1);
    }

    /// F-MMAP-007: Failure rate calculated
    #[test]
    fn f_mmap_007_failure_rate() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        mm.failure();
        assert!((mm.failure_rate() - 50.0).abs() < 0.01);
    }

    /// F-MMAP-008: Leak detection
    #[test]
    fn f_mmap_008_leak() {
        let mut mm = MmapTracker::new();
        for _ in 0..20 {
            mm.map(4096);
        }
        assert!(mm.has_leak());
    }

    /// F-MMAP-009: Factory for_file
    #[test]
    fn f_mmap_009_for_file() {
        let mm = MmapTracker::for_file();
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-010: Factory for_anonymous
    #[test]
    fn f_mmap_010_for_anonymous() {
        let mm = MmapTracker::for_anonymous();
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-011: Reset clears state
    #[test]
    fn f_mmap_011_reset() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        mm.reset();
        assert_eq!(mm.active, 0);
    }

    /// F-MMAP-012: Clone preserves state
    #[test]
    fn f_mmap_012_clone() {
        let mut mm = MmapTracker::new();
        mm.map(4096);
        let cloned = mm.clone();
        assert_eq!(mm.mapped_bytes, cloned.mapped_bytes);
    }
}

// ============================================================================
// CgroupTracker - O(1) cgroup resource tracking (v9.35.0)
// ============================================================================

/// O(1) cgroup (control group) resource tracking.
///
/// Tracks cgroup limits and usage for containerization.
#[derive(Debug, Clone)]
pub struct CgroupTracker {
    /// CPU shares allocated
    pub cpu_shares: u64,
    /// Memory limit bytes
    pub memory_limit: u64,
    /// Current memory usage
    pub memory_usage: u64,
    /// CPU throttle events
    pub cpu_throttled: u64,
    /// OOM events
    pub oom_events: u64,
    /// IO weight
    pub io_weight: u64,
}

impl Default for CgroupTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CgroupTracker {
    /// Create new cgroup tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cpu_shares: 1024, // Default shares
            memory_limit: 0,  // No limit
            memory_usage: 0,
            cpu_throttled: 0,
            oom_events: 0,
            io_weight: 100, // Default weight
        }
    }

    /// Create for container.
    #[must_use]
    pub const fn for_container() -> Self {
        Self {
            cpu_shares: 1024,
            memory_limit: 1024 * 1024 * 1024, // 1GB
            memory_usage: 0,
            cpu_throttled: 0,
            oom_events: 0,
            io_weight: 100,
        }
    }

    /// Create for service (systemd).
    #[must_use]
    pub const fn for_service() -> Self {
        Self::new()
    }

    /// Set CPU shares.
    pub fn set_cpu_shares(&mut self, shares: u64) {
        self.cpu_shares = shares;
    }

    /// Set memory limit.
    pub fn set_memory_limit(&mut self, limit: u64) {
        self.memory_limit = limit;
    }

    /// Update memory usage.
    pub fn update_memory(&mut self, usage: u64) {
        self.memory_usage = usage;
    }

    /// Record CPU throttle event.
    pub fn throttle(&mut self) {
        self.cpu_throttled += 1;
    }

    /// Record OOM event.
    pub fn oom(&mut self) {
        self.oom_events += 1;
    }

    /// Get memory utilization percentage.
    #[must_use]
    pub fn memory_utilization(&self) -> f64 {
        if self.memory_limit == 0 {
            return 0.0;
        }
        (self.memory_usage as f64 / self.memory_limit as f64) * 100.0
    }

    /// Check if memory is near limit (>90%).
    #[must_use]
    pub fn is_memory_pressure(&self) -> bool {
        self.memory_utilization() > 90.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.cpu_throttled = 0;
        self.oom_events = 0;
    }
}

#[cfg(test)]
mod cgroup_tests {
    use super::*;

    /// F-CGROUP-001: New tracker has defaults
    #[test]
    fn f_cgroup_001_new() {
        let cg = CgroupTracker::new();
        assert_eq!(cg.cpu_shares, 1024);
    }

    /// F-CGROUP-002: Default has defaults
    #[test]
    fn f_cgroup_002_default() {
        let cg = CgroupTracker::default();
        assert_eq!(cg.cpu_shares, 1024);
    }

    /// F-CGROUP-003: CPU shares settable
    #[test]
    fn f_cgroup_003_cpu_shares() {
        let mut cg = CgroupTracker::new();
        cg.set_cpu_shares(2048);
        assert_eq!(cg.cpu_shares, 2048);
    }

    /// F-CGROUP-004: Memory limit settable
    #[test]
    fn f_cgroup_004_memory_limit() {
        let mut cg = CgroupTracker::new();
        cg.set_memory_limit(1024 * 1024 * 1024);
        assert_eq!(cg.memory_limit, 1024 * 1024 * 1024);
    }

    /// F-CGROUP-005: Memory usage updated
    #[test]
    fn f_cgroup_005_memory_usage() {
        let mut cg = CgroupTracker::new();
        cg.update_memory(512 * 1024 * 1024);
        assert_eq!(cg.memory_usage, 512 * 1024 * 1024);
    }

    /// F-CGROUP-006: Throttle tracked
    #[test]
    fn f_cgroup_006_throttle() {
        let mut cg = CgroupTracker::new();
        cg.throttle();
        assert_eq!(cg.cpu_throttled, 1);
    }

    /// F-CGROUP-007: OOM tracked
    #[test]
    fn f_cgroup_007_oom() {
        let mut cg = CgroupTracker::new();
        cg.oom();
        assert_eq!(cg.oom_events, 1);
    }

    /// F-CGROUP-008: Memory utilization calculated
    #[test]
    fn f_cgroup_008_memory_util() {
        let mut cg = CgroupTracker::new();
        cg.set_memory_limit(1000);
        cg.update_memory(500);
        assert!((cg.memory_utilization() - 50.0).abs() < 0.01);
    }

    /// F-CGROUP-009: Memory pressure detected
    #[test]
    fn f_cgroup_009_memory_pressure() {
        let mut cg = CgroupTracker::new();
        cg.set_memory_limit(1000);
        cg.update_memory(950);
        assert!(cg.is_memory_pressure());
    }

    /// F-CGROUP-010: Factory for_container
    #[test]
    fn f_cgroup_010_for_container() {
        let cg = CgroupTracker::for_container();
        assert_eq!(cg.memory_limit, 1024 * 1024 * 1024);
    }

    /// F-CGROUP-011: Reset clears counters
    #[test]
    fn f_cgroup_011_reset() {
        let mut cg = CgroupTracker::new();
        cg.throttle();
        cg.oom();
        cg.reset();
        assert_eq!(cg.cpu_throttled, 0);
        assert_eq!(cg.oom_events, 0);
    }

    /// F-CGROUP-012: Clone preserves state
    #[test]
    fn f_cgroup_012_clone() {
        let mut cg = CgroupTracker::new();
        cg.throttle();
        let cloned = cg.clone();
        assert_eq!(cg.cpu_throttled, cloned.cpu_throttled);
    }
}

// ============================================================================
// NetfilterTracker - O(1) netfilter/iptables tracking (v9.36.0)
// ============================================================================

/// O(1) netfilter (iptables/nftables) packet tracking.
///
/// Tracks packet filtering decisions and rule matches.
#[derive(Debug, Clone)]
pub struct NetfilterTracker {
    /// Packets accepted
    pub accepted: u64,
    /// Packets dropped
    pub dropped: u64,
    /// Packets rejected
    pub rejected: u64,
    /// Packets NATed
    pub nated: u64,
    /// Rule matches
    pub rule_matches: u64,
    /// Connection tracking entries
    pub conntrack_entries: u64,
}

impl Default for NetfilterTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl NetfilterTracker {
    /// Create new netfilter tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            accepted: 0,
            dropped: 0,
            rejected: 0,
            nated: 0,
            rule_matches: 0,
            conntrack_entries: 0,
        }
    }

    /// Create for firewall workload.
    #[must_use]
    pub const fn for_firewall() -> Self {
        Self::new()
    }

    /// Create for NAT gateway.
    #[must_use]
    pub const fn for_nat() -> Self {
        Self::new()
    }

    /// Record accepted packet.
    pub fn accept(&mut self) {
        self.accepted += 1;
        self.rule_matches += 1;
    }

    /// Record dropped packet.
    pub fn record_drop(&mut self) {
        self.dropped += 1;
        self.rule_matches += 1;
    }

    /// Record rejected packet.
    pub fn reject(&mut self) {
        self.rejected += 1;
        self.rule_matches += 1;
    }

    /// Record NAT operation.
    pub fn nat(&mut self) {
        self.nated += 1;
    }

    /// Update conntrack entries.
    pub fn set_conntrack(&mut self, entries: u64) {
        self.conntrack_entries = entries;
    }

    /// Total packets processed.
    #[must_use]
    pub fn total_packets(&self) -> u64 {
        self.accepted + self.dropped + self.rejected
    }

    /// Get drop rate.
    #[must_use]
    pub fn drop_rate(&self) -> f64 {
        let total = self.total_packets();
        if total == 0 {
            return 0.0;
        }
        (self.dropped as f64 / total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.accepted = 0;
        self.dropped = 0;
        self.rejected = 0;
        self.nated = 0;
        self.rule_matches = 0;
    }
}

#[cfg(test)]
mod netfilter_tests {
    use super::*;

    /// F-NF-001: New tracker is empty
    #[test]
    fn f_nf_001_new() {
        let nf = NetfilterTracker::new();
        assert_eq!(nf.total_packets(), 0);
    }

    /// F-NF-002: Default is empty
    #[test]
    fn f_nf_002_default() {
        let nf = NetfilterTracker::default();
        assert_eq!(nf.total_packets(), 0);
    }

    /// F-NF-003: Accept tracked
    #[test]
    fn f_nf_003_accept() {
        let mut nf = NetfilterTracker::new();
        nf.accept();
        assert_eq!(nf.accepted, 1);
        assert_eq!(nf.rule_matches, 1);
    }

    /// F-NF-004: Drop tracked
    #[test]
    fn f_nf_004_drop() {
        let mut nf = NetfilterTracker::new();
        nf.record_drop();
        assert_eq!(nf.dropped, 1);
    }

    /// F-NF-005: Reject tracked
    #[test]
    fn f_nf_005_reject() {
        let mut nf = NetfilterTracker::new();
        nf.reject();
        assert_eq!(nf.rejected, 1);
    }

    /// F-NF-006: NAT tracked
    #[test]
    fn f_nf_006_nat() {
        let mut nf = NetfilterTracker::new();
        nf.nat();
        assert_eq!(nf.nated, 1);
    }

    /// F-NF-007: Drop rate calculated
    #[test]
    fn f_nf_007_drop_rate() {
        let mut nf = NetfilterTracker::new();
        nf.accept();
        nf.record_drop();
        assert!((nf.drop_rate() - 50.0).abs() < 0.01);
    }

    /// F-NF-008: Conntrack updated
    #[test]
    fn f_nf_008_conntrack() {
        let mut nf = NetfilterTracker::new();
        nf.set_conntrack(1000);
        assert_eq!(nf.conntrack_entries, 1000);
    }

    /// F-NF-009: Factory for_firewall
    #[test]
    fn f_nf_009_for_firewall() {
        let nf = NetfilterTracker::for_firewall();
        assert_eq!(nf.total_packets(), 0);
    }

    /// F-NF-010: Factory for_nat
    #[test]
    fn f_nf_010_for_nat() {
        let nf = NetfilterTracker::for_nat();
