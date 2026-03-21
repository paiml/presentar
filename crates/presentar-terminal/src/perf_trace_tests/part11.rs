        irq.storage_irq(10);
        assert_eq!(irq.storage, 1);
    }

    /// F-IRQ-006: Handler time accumulated
    #[test]
    fn f_irq_006_handler() {
        let mut irq = IrqTracker::new();
        irq.timer_irq(100);
        irq.network_irq(200);
        assert_eq!(irq.handler_time_us, 300);
    }

    /// F-IRQ-007: Average handler time calculated
    #[test]
    fn f_irq_007_avg() {
        let mut irq = IrqTracker::new();
        irq.timer_irq(100);
        irq.network_irq(200);
        assert_eq!(irq.avg_handler_us(), 150);
    }

    /// F-IRQ-008: Peak rate tracked
    #[test]
    fn f_irq_008_peak() {
        let mut irq = IrqTracker::new();
        irq.update_rate(1000);
        irq.update_rate(500);
        assert_eq!(irq.peak_rate, 1000);
    }

    /// F-IRQ-009: Factory for_server
    #[test]
    fn f_irq_009_server() {
        let irq = IrqTracker::for_server();
        assert_eq!(irq.total, 0);
    }

    /// F-IRQ-010: Factory for_embedded
    #[test]
    fn f_irq_010_embedded() {
        let irq = IrqTracker::for_embedded();
        assert_eq!(irq.total, 0);
    }

    /// F-IRQ-011: Reset clears counters
    #[test]
    fn f_irq_011_reset() {
        let mut irq = IrqTracker::new();
        irq.timer_irq(10);
        irq.reset();
        assert_eq!(irq.total, 0);
    }

    /// F-IRQ-012: Clone preserves state
    #[test]
    fn f_irq_012_clone() {
        let mut irq = IrqTracker::new();
        irq.timer_irq(10);
        let cloned = irq.clone();
        assert_eq!(irq.timer, cloned.timer);
    }
}

// ============================================================================
// SoftirqTracker - O(1) softirq tracking (v9.38.0)
// ============================================================================

/// O(1) software interrupt (softirq) tracking.
///
/// Tracks NET_RX, NET_TX, BLOCK, TIMER softirqs.
#[derive(Debug, Clone)]
pub struct SoftirqTracker {
    /// Total softirqs
    pub total: u64,
    /// NET_RX softirqs
    pub net_rx: u64,
    /// NET_TX softirqs
    pub net_tx: u64,
    /// BLOCK softirqs
    pub block: u64,
    /// TIMER softirqs
    pub timer: u64,
    /// Total execution time (us)
    pub exec_time_us: u64,
}

impl Default for SoftirqTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftirqTracker {
    /// Create new softirq tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            total: 0,
            net_rx: 0,
            net_tx: 0,
            block: 0,
            timer: 0,
            exec_time_us: 0,
        }
    }

    /// Create for network-heavy workload.
    #[must_use]
    pub const fn for_network() -> Self {
        Self::new()
    }

    /// Create for storage-heavy workload.
    #[must_use]
    pub const fn for_storage() -> Self {
        Self::new()
    }

    /// Record NET_RX softirq.
    pub fn net_rx(&mut self, exec_us: u64) {
        self.total += 1;
        self.net_rx += 1;
        self.exec_time_us += exec_us;
    }

    /// Record NET_TX softirq.
    pub fn net_tx(&mut self, exec_us: u64) {
        self.total += 1;
        self.net_tx += 1;
        self.exec_time_us += exec_us;
    }

    /// Record BLOCK softirq.
    pub fn block(&mut self, exec_us: u64) {
        self.total += 1;
        self.block += 1;
        self.exec_time_us += exec_us;
    }

    /// Record TIMER softirq.
    pub fn timer(&mut self, exec_us: u64) {
        self.total += 1;
        self.timer += 1;
        self.exec_time_us += exec_us;
    }

    /// Get network percentage.
    #[must_use]
    pub fn network_percentage(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        ((self.net_rx + self.net_tx) as f64 / self.total as f64) * 100.0
    }

    /// Get average execution time.
    #[must_use]
    pub fn avg_exec_us(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        self.exec_time_us / self.total
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.total = 0;
        self.net_rx = 0;
        self.net_tx = 0;
        self.block = 0;
        self.timer = 0;
        self.exec_time_us = 0;
    }
}

#[cfg(test)]
mod softirq_tests {
    use super::*;

    /// F-SOFTIRQ-001: New tracker is empty
    #[test]
    fn f_softirq_001_new() {
        let si = SoftirqTracker::new();
        assert_eq!(si.total, 0);
    }

    /// F-SOFTIRQ-002: Default is empty
    #[test]
    fn f_softirq_002_default() {
        let si = SoftirqTracker::default();
        assert_eq!(si.total, 0);
    }

    /// F-SOFTIRQ-003: NET_RX tracked
    #[test]
    fn f_softirq_003_net_rx() {
        let mut si = SoftirqTracker::new();
        si.net_rx(10);
        assert_eq!(si.net_rx, 1);
        assert_eq!(si.total, 1);
    }

    /// F-SOFTIRQ-004: NET_TX tracked
    #[test]
    fn f_softirq_004_net_tx() {
        let mut si = SoftirqTracker::new();
        si.net_tx(10);
        assert_eq!(si.net_tx, 1);
    }

    /// F-SOFTIRQ-005: BLOCK tracked
    #[test]
    fn f_softirq_005_block() {
        let mut si = SoftirqTracker::new();
        si.block(10);
        assert_eq!(si.block, 1);
    }

    /// F-SOFTIRQ-006: TIMER tracked
    #[test]
    fn f_softirq_006_timer() {
        let mut si = SoftirqTracker::new();
        si.timer(10);
        assert_eq!(si.timer, 1);
    }

    /// F-SOFTIRQ-007: Exec time accumulated
    #[test]
    fn f_softirq_007_exec() {
        let mut si = SoftirqTracker::new();
        si.net_rx(100);
        si.block(200);
        assert_eq!(si.exec_time_us, 300);
    }

    /// F-SOFTIRQ-008: Network percentage calculated
    #[test]
    fn f_softirq_008_net_pct() {
        let mut si = SoftirqTracker::new();
        si.net_rx(10);
        si.net_tx(10);
        si.block(10);
        si.timer(10);
        assert!((si.network_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SOFTIRQ-009: Factory for_network
    #[test]
    fn f_softirq_009_network() {
        let si = SoftirqTracker::for_network();
        assert_eq!(si.total, 0);
    }

    /// F-SOFTIRQ-010: Factory for_storage
    #[test]
    fn f_softirq_010_storage() {
        let si = SoftirqTracker::for_storage();
        assert_eq!(si.total, 0);
    }

    /// F-SOFTIRQ-011: Reset clears counters
    #[test]
    fn f_softirq_011_reset() {
        let mut si = SoftirqTracker::new();
        si.net_rx(10);
        si.reset();
        assert_eq!(si.total, 0);
    }

    /// F-SOFTIRQ-012: Clone preserves state
    #[test]
    fn f_softirq_012_clone() {
        let mut si = SoftirqTracker::new();
        si.net_rx(10);
        let cloned = si.clone();
        assert_eq!(si.net_rx, cloned.net_rx);
    }
}

// ============================================================================
// WorkqueueTracker - O(1) kernel workqueue tracking (v9.38.0)
// ============================================================================

/// O(1) kernel workqueue tracking.
///
/// Tracks work items queued and executed.
#[derive(Debug, Clone)]
pub struct WorkqueueTracker {
    /// Work items queued
    pub queued: u64,
    /// Work items executed
    pub executed: u64,
    /// Work items cancelled
    pub cancelled: u64,
    /// Total execution time (us)
    pub exec_time_us: u64,
    /// Peak queue depth
    pub peak_depth: u64,
    /// Delayed work items
    pub delayed: u64,
}

impl Default for WorkqueueTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkqueueTracker {
    /// Create new workqueue tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            queued: 0,
            executed: 0,
            cancelled: 0,
            exec_time_us: 0,
            peak_depth: 0,
            delayed: 0,
        }
    }

    /// Create for system workqueue.
    #[must_use]
    pub const fn for_system() -> Self {
        Self::new()
    }

    /// Create for high-priority workqueue.
    #[must_use]
    pub const fn for_highpri() -> Self {
        Self::new()
    }

    /// Record work item queued.
    pub fn queue(&mut self) {
        self.queued += 1;
        let pending = self.queued.saturating_sub(self.executed);
        if pending > self.peak_depth {
            self.peak_depth = pending;
        }
    }

    /// Record work item executed.
    pub fn execute(&mut self, exec_us: u64) {
        self.executed += 1;
        self.exec_time_us += exec_us;
    }

    /// Record work item cancelled.
    pub fn cancel(&mut self) {
        self.cancelled += 1;
    }

    /// Record delayed work item.
    pub fn delay(&mut self) {
        self.delayed += 1;
    }

    /// Get pending work items.
    #[must_use]
    pub fn pending(&self) -> u64 {
        self.queued.saturating_sub(self.executed + self.cancelled)
    }

    /// Get average execution time.
    #[must_use]
    pub fn avg_exec_us(&self) -> u64 {
        if self.executed == 0 {
            return 0;
        }
        self.exec_time_us / self.executed
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.queued = 0;
        self.executed = 0;
        self.cancelled = 0;
        self.exec_time_us = 0;
        self.peak_depth = 0;
        self.delayed = 0;
    }
}

#[cfg(test)]
mod workqueue_tests {
    use super::*;

    /// F-WQ-001: New tracker is empty
    #[test]
    fn f_wq_001_new() {
        let wq = WorkqueueTracker::new();
        assert_eq!(wq.queued, 0);
    }

    /// F-WQ-002: Default is empty
    #[test]
    fn f_wq_002_default() {
        let wq = WorkqueueTracker::default();
        assert_eq!(wq.queued, 0);
    }

    /// F-WQ-003: Queue tracked
    #[test]
    fn f_wq_003_queue() {
        let mut wq = WorkqueueTracker::new();
        wq.queue();
        assert_eq!(wq.queued, 1);
    }

    /// F-WQ-004: Execute tracked
    #[test]
    fn f_wq_004_execute() {
        let mut wq = WorkqueueTracker::new();
        wq.execute(100);
        assert_eq!(wq.executed, 1);
        assert_eq!(wq.exec_time_us, 100);
    }

    /// F-WQ-005: Cancel tracked
    #[test]
    fn f_wq_005_cancel() {
        let mut wq = WorkqueueTracker::new();
        wq.cancel();
        assert_eq!(wq.cancelled, 1);
    }

    /// F-WQ-006: Delay tracked
    #[test]
    fn f_wq_006_delay() {
        let mut wq = WorkqueueTracker::new();
        wq.delay();
        assert_eq!(wq.delayed, 1);
    }

    /// F-WQ-007: Pending calculated
    #[test]
    fn f_wq_007_pending() {
        let mut wq = WorkqueueTracker::new();
        wq.queue();
        wq.queue();
        wq.execute(100);
        assert_eq!(wq.pending(), 1);
    }

    /// F-WQ-008: Peak depth tracked
    #[test]
    fn f_wq_008_peak() {
        let mut wq = WorkqueueTracker::new();
        wq.queue();
        wq.queue();
        wq.execute(100);
        wq.execute(100);
        assert_eq!(wq.peak_depth, 2);
    }

    /// F-WQ-009: Factory for_system
    #[test]
    fn f_wq_009_system() {
        let wq = WorkqueueTracker::for_system();
        assert_eq!(wq.queued, 0);
    }

    /// F-WQ-010: Factory for_highpri
    #[test]
    fn f_wq_010_highpri() {
        let wq = WorkqueueTracker::for_highpri();
        assert_eq!(wq.queued, 0);
    }

    /// F-WQ-011: Reset clears counters
    #[test]
    fn f_wq_011_reset() {
        let mut wq = WorkqueueTracker::new();
        wq.queue();
        wq.reset();
        assert_eq!(wq.queued, 0);
    }

    /// F-WQ-012: Clone preserves state
    #[test]
    fn f_wq_012_clone() {
        let mut wq = WorkqueueTracker::new();
        wq.queue();
        let cloned = wq.clone();
        assert_eq!(wq.queued, cloned.queued);
    }
}

// ============================================================================
// RcuTracker - O(1) RCU synchronization tracking (v9.39.0)
// ============================================================================

/// O(1) RCU (Read-Copy-Update) tracking.
///
/// Tracks RCU grace periods and callbacks.
#[derive(Debug, Clone)]
pub struct RcuTracker {
    /// Grace periods completed
    pub grace_periods: u64,
    /// Callbacks queued
    pub callbacks_queued: u64,
    /// Callbacks executed
    pub callbacks_executed: u64,
    /// Expedited grace periods
    pub expedited: u64,
    /// Total grace period duration (us)
    pub total_gp_duration_us: u64,
    /// Peak callbacks pending
    pub peak_callbacks: u64,
}

impl Default for RcuTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RcuTracker {
    /// Create new RCU tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            grace_periods: 0,
            callbacks_queued: 0,
            callbacks_executed: 0,
            expedited: 0,
            total_gp_duration_us: 0,
            peak_callbacks: 0,
        }
    }

    /// Create for kernel workload.
    #[must_use]
    pub const fn for_kernel() -> Self {
        Self::new()
    }

    /// Create for SRCU workload.
    #[must_use]
    pub const fn for_srcu() -> Self {
        Self::new()
    }

    /// Record grace period completion.
    pub fn grace_period(&mut self, duration_us: u64) {
        self.grace_periods += 1;
        self.total_gp_duration_us += duration_us;
    }

    /// Queue callback.
    pub fn queue_callback(&mut self) {
        self.callbacks_queued += 1;
        let pending = self
            .callbacks_queued
            .saturating_sub(self.callbacks_executed);
        if pending > self.peak_callbacks {
            self.peak_callbacks = pending;
        }
    }

    /// Execute callback.
    pub fn execute_callback(&mut self) {
        self.callbacks_executed += 1;
    }

    /// Record expedited grace period.
    pub fn expedite(&mut self) {
        self.expedited += 1;
    }

    /// Get average grace period duration.
    #[must_use]
    pub fn avg_gp_duration_us(&self) -> u64 {
        if self.grace_periods == 0 {
            return 0;
        }
        self.total_gp_duration_us / self.grace_periods
    }

    /// Get pending callbacks.
    #[must_use]
    pub fn pending_callbacks(&self) -> u64 {
        self.callbacks_queued
            .saturating_sub(self.callbacks_executed)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.grace_periods = 0;
        self.callbacks_queued = 0;
        self.callbacks_executed = 0;
        self.expedited = 0;
        self.total_gp_duration_us = 0;
        self.peak_callbacks = 0;
    }
}

#[cfg(test)]
mod rcu_tests {
    use super::*;

    /// F-RCU-001: New tracker is empty
    #[test]
    fn f_rcu_001_new() {
        let rcu = RcuTracker::new();
        assert_eq!(rcu.grace_periods, 0);
    }

    /// F-RCU-002: Default is empty
    #[test]
    fn f_rcu_002_default() {
        let rcu = RcuTracker::default();
        assert_eq!(rcu.grace_periods, 0);
    }

    /// F-RCU-003: Grace period tracked
    #[test]
    fn f_rcu_003_gp() {
        let mut rcu = RcuTracker::new();
        rcu.grace_period(100);
        assert_eq!(rcu.grace_periods, 1);
    }

    /// F-RCU-004: Callback queued
    #[test]
    fn f_rcu_004_queue() {
        let mut rcu = RcuTracker::new();
        rcu.queue_callback();
        assert_eq!(rcu.callbacks_queued, 1);
    }

    /// F-RCU-005: Callback executed
    #[test]
    fn f_rcu_005_execute() {
        let mut rcu = RcuTracker::new();
        rcu.execute_callback();
        assert_eq!(rcu.callbacks_executed, 1);
    }

    /// F-RCU-006: Expedited tracked
    #[test]
    fn f_rcu_006_expedite() {
        let mut rcu = RcuTracker::new();
        rcu.expedite();
        assert_eq!(rcu.expedited, 1);
    }

    /// F-RCU-007: Average GP duration
    #[test]
    fn f_rcu_007_avg_gp() {
        let mut rcu = RcuTracker::new();
        rcu.grace_period(100);
        rcu.grace_period(200);
        assert_eq!(rcu.avg_gp_duration_us(), 150);
    }

    /// F-RCU-008: Pending callbacks
    #[test]
    fn f_rcu_008_pending() {
        let mut rcu = RcuTracker::new();
        rcu.queue_callback();
        rcu.queue_callback();
        rcu.execute_callback();
        assert_eq!(rcu.pending_callbacks(), 1);
    }

    /// F-RCU-009: Factory for_kernel
    #[test]
    fn f_rcu_009_kernel() {
        let rcu = RcuTracker::for_kernel();
        assert_eq!(rcu.grace_periods, 0);
    }

    /// F-RCU-010: Factory for_srcu
    #[test]
    fn f_rcu_010_srcu() {
        let rcu = RcuTracker::for_srcu();
        assert_eq!(rcu.grace_periods, 0);
    }

    /// F-RCU-011: Reset clears counters
    #[test]
    fn f_rcu_011_reset() {
        let mut rcu = RcuTracker::new();
        rcu.grace_period(100);
        rcu.reset();
        assert_eq!(rcu.grace_periods, 0);
    }

    /// F-RCU-012: Clone preserves state
    #[test]
    fn f_rcu_012_clone() {
        let mut rcu = RcuTracker::new();
        rcu.grace_period(100);
        let cloned = rcu.clone();
        assert_eq!(rcu.grace_periods, cloned.grace_periods);
    }
}

// ============================================================================
// SlabTracker - O(1) slab allocator tracking (v9.39.0)
// ============================================================================

/// O(1) slab allocator tracking.
///
/// Tracks SLUB/SLAB cache allocations.
#[derive(Debug, Clone)]
pub struct SlabTracker {
    /// Objects allocated
    pub allocs: u64,
    /// Objects freed
    pub frees: u64,
    /// Cache misses (slow path)
    pub cache_misses: u64,
    /// Total objects in use
    pub objects_in_use: u64,
    /// Total memory used (bytes)
    pub memory_used: u64,
    /// Number of slabs
    pub slabs: u64,
}

impl Default for SlabTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl SlabTracker {
    /// Create new slab tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allocs: 0,
            frees: 0,
            cache_misses: 0,
            objects_in_use: 0,
            memory_used: 0,
            slabs: 0,
        }
    }

    /// Create for kmalloc cache.
    #[must_use]
    pub const fn for_kmalloc() -> Self {
        Self::new()
    }

    /// Create for specific cache.
    #[must_use]
    pub const fn for_cache() -> Self {
        Self::new()
    }

    /// Record allocation.
    pub fn alloc(&mut self, size: u64) {
        self.allocs += 1;
        self.objects_in_use += 1;
        self.memory_used += size;
    }

    /// Record free.
    pub fn free(&mut self, size: u64) {
        self.frees += 1;
        if self.objects_in_use > 0 {
            self.objects_in_use -= 1;
        }
        self.memory_used = self.memory_used.saturating_sub(size);
    }

    /// Record cache miss.
    pub fn cache_miss(&mut self) {
        self.cache_misses += 1;
    }

    /// Update slab count.
    pub fn set_slabs(&mut self, count: u64) {
        self.slabs = count;
    }

    /// Get cache hit rate.
    #[must_use]
    pub fn cache_hit_rate(&self) -> f64 {
        if self.allocs == 0 {
            return 100.0;
        }
        let hits = self.allocs.saturating_sub(self.cache_misses);
        (hits as f64 / self.allocs as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocs = 0;
        self.frees = 0;
        self.cache_misses = 0;
        self.objects_in_use = 0;
        self.memory_used = 0;
        self.slabs = 0;
    }
}

#[cfg(test)]
mod slab_tests {
    use super::*;

    /// F-SLAB-001: New tracker is empty
    #[test]
    fn f_slab_001_new() {
        let slab = SlabTracker::new();
        assert_eq!(slab.allocs, 0);
    }

    /// F-SLAB-002: Default is empty
    #[test]
    fn f_slab_002_default() {
        let slab = SlabTracker::default();
        assert_eq!(slab.allocs, 0);
    }

    /// F-SLAB-003: Alloc tracked
    #[test]
    fn f_slab_003_alloc() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        assert_eq!(slab.allocs, 1);
        assert_eq!(slab.objects_in_use, 1);
    }

    /// F-SLAB-004: Free tracked
    #[test]
    fn f_slab_004_free() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        slab.free(64);
        assert_eq!(slab.frees, 1);
        assert_eq!(slab.objects_in_use, 0);
    }

    /// F-SLAB-005: Cache miss tracked
    #[test]
    fn f_slab_005_miss() {
        let mut slab = SlabTracker::new();
        slab.cache_miss();
        assert_eq!(slab.cache_misses, 1);
    }

    /// F-SLAB-006: Memory tracking
    #[test]
    fn f_slab_006_memory() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        slab.alloc(128);
        assert_eq!(slab.memory_used, 192);
    }

    /// F-SLAB-007: Cache hit rate
    #[test]
    fn f_slab_007_hit_rate() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        slab.alloc(64);
        slab.cache_miss();
        assert!((slab.cache_hit_rate() - 50.0).abs() < 0.01);
    }

    /// F-SLAB-008: Slab count
    #[test]
    fn f_slab_008_slabs() {
        let mut slab = SlabTracker::new();
        slab.set_slabs(10);
        assert_eq!(slab.slabs, 10);
    }

    /// F-SLAB-009: Factory for_kmalloc
    #[test]
    fn f_slab_009_kmalloc() {
        let slab = SlabTracker::for_kmalloc();
        assert_eq!(slab.allocs, 0);
    }

    /// F-SLAB-010: Factory for_cache
    #[test]
    fn f_slab_010_cache() {
        let slab = SlabTracker::for_cache();
        assert_eq!(slab.allocs, 0);
    }

    /// F-SLAB-011: Reset clears counters
    #[test]
    fn f_slab_011_reset() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        slab.reset();
        assert_eq!(slab.allocs, 0);
    }

    /// F-SLAB-012: Clone preserves state
    #[test]
    fn f_slab_012_clone() {
        let mut slab = SlabTracker::new();
        slab.alloc(64);
        let cloned = slab.clone();
        assert_eq!(slab.allocs, cloned.allocs);
    }
}

// ============================================================================
// VmstatTracker - O(1) vmstat tracking (v9.39.0)
// ============================================================================

/// O(1) vmstat (virtual memory statistics) tracking.
///
/// Tracks page faults, swaps, and memory pressure.
#[derive(Debug, Clone)]
pub struct VmstatTracker {
    /// Page faults (minor)
    pub minor_faults: u64,
    /// Page faults (major)
    pub major_faults: u64,
    /// Pages swapped in
    pub swap_in: u64,
    /// Pages swapped out
    pub swap_out: u64,
    /// Pages allocated
    pub pgalloc: u64,
    /// Pages freed
    pub pgfree: u64,
}

impl Default for VmstatTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VmstatTracker {
    /// Create new vmstat tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            minor_faults: 0,
            major_faults: 0,
            swap_in: 0,
            swap_out: 0,
            pgalloc: 0,
            pgfree: 0,
        }
    }

    /// Create for process tracking.
    #[must_use]
    pub const fn for_process() -> Self {
        Self::new()
    }

    /// Create for system-wide tracking.
    #[must_use]
    pub const fn for_system() -> Self {
        Self::new()
    }

    /// Record minor fault.
    pub fn minor_fault(&mut self) {
        self.minor_faults += 1;
    }

    /// Record major fault.
    pub fn major_fault(&mut self) {
        self.major_faults += 1;
    }

    /// Record swap in.
    pub fn swap_in(&mut self, pages: u64) {
        self.swap_in += pages;
    }

    /// Record swap out.
    pub fn swap_out(&mut self, pages: u64) {
        self.swap_out += pages;
    }

    /// Record page allocation.
    pub fn pgalloc(&mut self, pages: u64) {
        self.pgalloc += pages;
    }

    /// Record page free.
    pub fn pgfree(&mut self, pages: u64) {
        self.pgfree += pages;
    }

    /// Total faults.
    #[must_use]
    pub fn total_faults(&self) -> u64 {
        self.minor_faults + self.major_faults
    }

    /// Major fault ratio.
    #[must_use]
    pub fn major_fault_ratio(&self) -> f64 {
        let total = self.total_faults();
        if total == 0 {
            return 0.0;
        }
        (self.major_faults as f64 / total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.minor_faults = 0;
        self.major_faults = 0;
        self.swap_in = 0;
        self.swap_out = 0;
        self.pgalloc = 0;
        self.pgfree = 0;
    }
}

#[cfg(test)]
mod vmstat_tests {
    use super::*;

    /// F-VMSTAT-001: New tracker is empty
    #[test]
    fn f_vmstat_001_new() {
        let vm = VmstatTracker::new();
        assert_eq!(vm.total_faults(), 0);
    }

    /// F-VMSTAT-002: Default is empty
    #[test]
    fn f_vmstat_002_default() {
        let vm = VmstatTracker::default();
        assert_eq!(vm.total_faults(), 0);
    }

    /// F-VMSTAT-003: Minor fault tracked
    #[test]
    fn f_vmstat_003_minor() {
        let mut vm = VmstatTracker::new();
        vm.minor_fault();
        assert_eq!(vm.minor_faults, 1);
    }

    /// F-VMSTAT-004: Major fault tracked
    #[test]
    fn f_vmstat_004_major() {
        let mut vm = VmstatTracker::new();
        vm.major_fault();
        assert_eq!(vm.major_faults, 1);
    }

    /// F-VMSTAT-005: Swap in tracked
    #[test]
    fn f_vmstat_005_swap_in() {
        let mut vm = VmstatTracker::new();
        vm.swap_in(10);
        assert_eq!(vm.swap_in, 10);
    }

    /// F-VMSTAT-006: Swap out tracked
    #[test]
    fn f_vmstat_006_swap_out() {
        let mut vm = VmstatTracker::new();
        vm.swap_out(10);
        assert_eq!(vm.swap_out, 10);
    }

    /// F-VMSTAT-007: Total faults
    #[test]
    fn f_vmstat_007_total() {
        let mut vm = VmstatTracker::new();
        vm.minor_fault();
        vm.major_fault();
        assert_eq!(vm.total_faults(), 2);
    }

    /// F-VMSTAT-008: Major fault ratio
    #[test]
    fn f_vmstat_008_ratio() {
        let mut vm = VmstatTracker::new();
        vm.minor_fault();
        vm.major_fault();
        assert!((vm.major_fault_ratio() - 50.0).abs() < 0.01);
    }

    /// F-VMSTAT-009: Factory for_process
    #[test]
    fn f_vmstat_009_process() {
        let vm = VmstatTracker::for_process();
        assert_eq!(vm.total_faults(), 0);
    }

    /// F-VMSTAT-010: Factory for_system
    #[test]
    fn f_vmstat_010_system() {
        let vm = VmstatTracker::for_system();
        assert_eq!(vm.total_faults(), 0);
    }

    /// F-VMSTAT-011: Reset clears counters
    #[test]
    fn f_vmstat_011_reset() {
        let mut vm = VmstatTracker::new();
        vm.minor_fault();
        vm.reset();
        assert_eq!(vm.total_faults(), 0);
    }

    /// F-VMSTAT-012: Clone preserves state
    #[test]
    fn f_vmstat_012_clone() {
        let mut vm = VmstatTracker::new();
        vm.minor_fault();
        let cloned = vm.clone();
        assert_eq!(vm.minor_faults, cloned.minor_faults);
    }
}

// ============================================================================
// ZoneTracker - O(1) memory zone tracking (v9.39.0)
// ============================================================================

/// O(1) memory zone tracking.
///
/// Tracks DMA, DMA32, Normal, HighMem zones.
#[derive(Debug, Clone)]
pub struct ZoneTracker {
    /// Free pages in zone
    pub free_pages: u64,
    /// Low watermark
    pub watermark_low: u64,
    /// High watermark
    pub watermark_high: u64,
    /// Pages scanned
    pub pages_scanned: u64,
    /// Reclaim attempts
    pub reclaim_attempts: u64,
    /// Compaction attempts
    pub compaction_attempts: u64,
}

impl Default for ZoneTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ZoneTracker {
    /// Create new zone tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            free_pages: 0,
            watermark_low: 0,
            watermark_high: 0,
            pages_scanned: 0,
            reclaim_attempts: 0,
            compaction_attempts: 0,
        }
    }

    /// Create for DMA zone.
    #[must_use]
    pub const fn for_dma() -> Self {
        Self::new()
    }

    /// Create for Normal zone.
    #[must_use]
    pub const fn for_normal() -> Self {
        Self::new()
    }

    /// Update free pages.
    pub fn set_free_pages(&mut self, pages: u64) {
        self.free_pages = pages;
    }

    /// Set watermarks.
    pub fn set_watermarks(&mut self, low: u64, high: u64) {
        self.watermark_low = low;
        self.watermark_high = high;
    }

    /// Record pages scanned.
    pub fn scan(&mut self, pages: u64) {
        self.pages_scanned += pages;
    }

    /// Record reclaim attempt.
    pub fn reclaim(&mut self) {
        self.reclaim_attempts += 1;
    }

    /// Record compaction attempt.
    pub fn compact(&mut self) {
        self.compaction_attempts += 1;
    }

    /// Check if below low watermark.
    #[must_use]
    pub fn is_low(&self) -> bool {
        self.free_pages < self.watermark_low
    }

    /// Check if above high watermark.
    #[must_use]
    pub fn is_high(&self) -> bool {
        self.free_pages > self.watermark_high
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.pages_scanned = 0;
        self.reclaim_attempts = 0;
        self.compaction_attempts = 0;
    }
}

#[cfg(test)]
mod zone_tests {
    use super::*;

    /// F-ZONE-001: New tracker is empty
    #[test]
    fn f_zone_001_new() {
        let zone = ZoneTracker::new();
        assert_eq!(zone.free_pages, 0);
    }

    /// F-ZONE-002: Default is empty
    #[test]
    fn f_zone_002_default() {
        let zone = ZoneTracker::default();
        assert_eq!(zone.free_pages, 0);
    }

    /// F-ZONE-003: Free pages tracked
    #[test]
    fn f_zone_003_free() {
        let mut zone = ZoneTracker::new();
        zone.set_free_pages(1000);
        assert_eq!(zone.free_pages, 1000);
    }

    /// F-ZONE-004: Watermarks set
    #[test]
    fn f_zone_004_watermarks() {
        let mut zone = ZoneTracker::new();
        zone.set_watermarks(100, 500);
        assert_eq!(zone.watermark_low, 100);
        assert_eq!(zone.watermark_high, 500);
    }

    /// F-ZONE-005: Scan tracked
    #[test]
    fn f_zone_005_scan() {
        let mut zone = ZoneTracker::new();
        zone.scan(100);
        assert_eq!(zone.pages_scanned, 100);
    }

    /// F-ZONE-006: Reclaim tracked
    #[test]
    fn f_zone_006_reclaim() {
        let mut zone = ZoneTracker::new();
        zone.reclaim();
        assert_eq!(zone.reclaim_attempts, 1);
    }

    /// F-ZONE-007: Compaction tracked
    #[test]
    fn f_zone_007_compact() {
        let mut zone = ZoneTracker::new();
        zone.compact();
        assert_eq!(zone.compaction_attempts, 1);
    }

    /// F-ZONE-008: Low check
    #[test]
    fn f_zone_008_is_low() {
        let mut zone = ZoneTracker::new();
        zone.set_watermarks(100, 500);
        zone.set_free_pages(50);
        assert!(zone.is_low());
    }

    /// F-ZONE-009: Factory for_dma
    #[test]
    fn f_zone_009_dma() {
        let zone = ZoneTracker::for_dma();
        assert_eq!(zone.free_pages, 0);
    }

    /// F-ZONE-010: Factory for_normal
    #[test]
    fn f_zone_010_normal() {
        let zone = ZoneTracker::for_normal();
        assert_eq!(zone.free_pages, 0);
    }

    /// F-ZONE-011: Reset clears counters
    #[test]
    fn f_zone_011_reset() {
        let mut zone = ZoneTracker::new();
        zone.scan(100);
        zone.reset();
        assert_eq!(zone.pages_scanned, 0);
    }

    /// F-ZONE-012: Clone preserves state
    #[test]
    fn f_zone_012_clone() {
        let mut zone = ZoneTracker::new();
        zone.set_free_pages(1000);
        let cloned = zone.clone();
        assert_eq!(zone.free_pages, cloned.free_pages);
    }
}

// ============================================================================
// v9.40.0: Storage Subsystem Helpers
// ============================================================================

/// O(1) block layer I/O tracker.
///
/// Tracks Linux block layer I/O operations including reads, writes, flushes,
/// and I/O scheduler metrics. Provides constant-time access to block device
/// performance data for monitoring and profiling.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::BlockLayerTracker;
///
/// let mut blk = BlockLayerTracker::new();
/// blk.read(4096);
/// blk.write(8192);
/// assert_eq!(blk.read_bytes, 4096);
/// assert_eq!(blk.write_bytes, 8192);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BlockLayerTracker {
    /// Total bytes read.
    pub read_bytes: u64,
    /// Total bytes written.
    pub write_bytes: u64,
    /// Read operations count.
    pub read_ops: u64,
    /// Write operations count.
    pub write_ops: u64,
    /// Flush operations count.
    pub flushes: u64,
    /// Discards (TRIM) count.
    pub discards: u64,
}

impl BlockLayerTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            read_bytes: 0,
            write_bytes: 0,
            read_ops: 0,
            write_ops: 0,
            flushes: 0,
            discards: 0,
        }
    }

    /// Factory for NVMe device.
    #[must_use]
    pub const fn for_nvme() -> Self {
        Self::new()
    }

    /// Factory for SCSI device.
    #[must_use]
    pub const fn for_scsi() -> Self {
        Self::new()
    }

    /// Record read operation.
    pub fn read(&mut self, bytes: u64) {
        self.read_bytes += bytes;
        self.read_ops += 1;
    }

    /// Record write operation.
    pub fn write(&mut self, bytes: u64) {
        self.write_bytes += bytes;
        self.write_ops += 1;
    }

    /// Record flush operation.
    pub fn flush(&mut self) {
        self.flushes += 1;
    }

    /// Record discard (TRIM).
    pub fn discard(&mut self) {
        self.discards += 1;
    }

    /// Get total bytes transferred.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.read_bytes + self.write_bytes
    }

    /// Get total operations.
    #[must_use]
    pub fn total_ops(&self) -> u64 {
        self.read_ops + self.write_ops + self.flushes + self.discards
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.read_bytes = 0;
        self.write_bytes = 0;
        self.read_ops = 0;
        self.write_ops = 0;
        self.flushes = 0;
        self.discards = 0;
    }
}

#[cfg(test)]
mod block_layer_tests {
    use super::*;

    /// F-BLK-001: New tracker is empty
    #[test]
    fn f_blk_001_new() {
        let blk = BlockLayerTracker::new();
        assert_eq!(blk.read_bytes, 0);
    }

    /// F-BLK-002: Default is empty
    #[test]
    fn f_blk_002_default() {
        let blk = BlockLayerTracker::default();
        assert_eq!(blk.read_bytes, 0);
    }

    /// F-BLK-003: Read tracked
    #[test]
    fn f_blk_003_read() {
        let mut blk = BlockLayerTracker::new();
        blk.read(4096);
        assert_eq!(blk.read_bytes, 4096);
        assert_eq!(blk.read_ops, 1);
    }

    /// F-BLK-004: Write tracked
    #[test]
    fn f_blk_004_write() {
        let mut blk = BlockLayerTracker::new();
        blk.write(8192);
        assert_eq!(blk.write_bytes, 8192);
        assert_eq!(blk.write_ops, 1);
    }

    /// F-BLK-005: Flush tracked
    #[test]
    fn f_blk_005_flush() {
        let mut blk = BlockLayerTracker::new();
        blk.flush();
        assert_eq!(blk.flushes, 1);
    }

    /// F-BLK-006: Discard tracked
    #[test]
    fn f_blk_006_discard() {
        let mut blk = BlockLayerTracker::new();
        blk.discard();
        assert_eq!(blk.discards, 1);
    }

    /// F-BLK-007: Total bytes
    #[test]
    fn f_blk_007_total_bytes() {
        let mut blk = BlockLayerTracker::new();
        blk.read(1000);
        blk.write(2000);
        assert_eq!(blk.total_bytes(), 3000);
    }

    /// F-BLK-008: Total ops
    #[test]
    fn f_blk_008_total_ops() {
        let mut blk = BlockLayerTracker::new();
        blk.read(100);
        blk.write(100);
        blk.flush();
        blk.discard();
        assert_eq!(blk.total_ops(), 4);
    }

    /// F-BLK-009: Factory for_nvme
    #[test]
    fn f_blk_009_nvme() {
        let blk = BlockLayerTracker::for_nvme();
        assert_eq!(blk.read_bytes, 0);
    }

    /// F-BLK-010: Factory for_scsi
    #[test]
    fn f_blk_010_scsi() {
        let blk = BlockLayerTracker::for_scsi();
        assert_eq!(blk.read_bytes, 0);
    }

    /// F-BLK-011: Reset clears counters
    #[test]
    fn f_blk_011_reset() {
        let mut blk = BlockLayerTracker::new();
        blk.read(4096);
        blk.reset();
        assert_eq!(blk.read_bytes, 0);
    }

    /// F-BLK-012: Clone preserves state
    #[test]
    fn f_blk_012_clone() {
        let mut blk = BlockLayerTracker::new();
        blk.read(4096);
        let cloned = blk;
        assert_eq!(blk.read_bytes, cloned.read_bytes);
    }
}

/// O(1) NVMe device tracker.
///
/// Tracks NVMe-specific metrics including command queues, completion queues,
/// admin commands, and NVMe-specific errors. Provides constant-time access
/// to NVMe device performance data.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::NvmeTracker;
///
/// let mut nvme = NvmeTracker::new();
/// nvme.submit(4);
/// nvme.complete(4);
/// assert_eq!(nvme.submissions, 4);
/// assert_eq!(nvme.completions, 4);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NvmeTracker {
    /// Submission queue entries.
    pub submissions: u64,
    /// Completion queue entries.
    pub completions: u64,
    /// Admin commands.
    pub admin_cmds: u64,
    /// I/O commands.
    pub io_cmds: u64,
    /// Queue depth (current).
    pub queue_depth: u32,
    /// Max queue depth seen.
    pub max_queue_depth: u32,
}

impl NvmeTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            submissions: 0,
            completions: 0,
            admin_cmds: 0,
            io_cmds: 0,
            queue_depth: 0,
            max_queue_depth: 0,
        }
    }

    /// Factory for Gen3 device.
    #[must_use]
    pub const fn for_gen3() -> Self {
        Self::new()
    }

    /// Factory for Gen4 device.
    #[must_use]
    pub const fn for_gen4() -> Self {
        Self::new()
    }

    /// Record submission.
    pub fn submit(&mut self, count: u64) {
        self.submissions += count;
        self.queue_depth = self.queue_depth.saturating_add(count as u32);
        if self.queue_depth > self.max_queue_depth {
            self.max_queue_depth = self.queue_depth;
        }
    }

    /// Record completion.
    pub fn complete(&mut self, count: u64) {
        self.completions += count;
        self.queue_depth = self.queue_depth.saturating_sub(count as u32);
    }

    /// Record admin command.
    pub fn admin(&mut self) {
        self.admin_cmds += 1;
    }

    /// Record I/O command.
    pub fn io(&mut self) {
        self.io_cmds += 1;
    }

    /// Get pending commands.
    #[must_use]
    pub fn pending(&self) -> u64 {
        self.submissions.saturating_sub(self.completions)
    }

    /// Check if queue is saturated.
    #[must_use]
    pub fn is_saturated(&self, threshold: u32) -> bool {
        self.queue_depth >= threshold
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.submissions = 0;
        self.completions = 0;
        self.admin_cmds = 0;
        self.io_cmds = 0;
        self.queue_depth = 0;
        // Keep max_queue_depth for high-water mark tracking
    }
}

#[cfg(test)]
mod nvme_tests {
    use super::*;

    /// F-NVME-001: New tracker is empty
    #[test]
    fn f_nvme_001_new() {
        let nvme = NvmeTracker::new();
        assert_eq!(nvme.submissions, 0);
    }

    /// F-NVME-002: Default is empty
    #[test]
    fn f_nvme_002_default() {
        let nvme = NvmeTracker::default();
        assert_eq!(nvme.submissions, 0);
    }

    /// F-NVME-003: Submit tracked
    #[test]
    fn f_nvme_003_submit() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(4);
        assert_eq!(nvme.submissions, 4);
        assert_eq!(nvme.queue_depth, 4);
    }

    /// F-NVME-004: Complete tracked
    #[test]
    fn f_nvme_004_complete() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(4);
        nvme.complete(2);
        assert_eq!(nvme.completions, 2);
        assert_eq!(nvme.queue_depth, 2);
    }

    /// F-NVME-005: Admin tracked
    #[test]
    fn f_nvme_005_admin() {
        let mut nvme = NvmeTracker::new();
        nvme.admin();
        assert_eq!(nvme.admin_cmds, 1);
    }

    /// F-NVME-006: IO tracked
    #[test]
    fn f_nvme_006_io() {
        let mut nvme = NvmeTracker::new();
        nvme.io();
        assert_eq!(nvme.io_cmds, 1);
    }

    /// F-NVME-007: Pending commands
    #[test]
    fn f_nvme_007_pending() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(10);
        nvme.complete(3);
        assert_eq!(nvme.pending(), 7);
    }

    /// F-NVME-008: Max queue depth
    #[test]
    fn f_nvme_008_max_depth() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(10);
        nvme.complete(5);
        nvme.submit(2);
        assert_eq!(nvme.max_queue_depth, 10);
    }

    /// F-NVME-009: Factory for_gen3
    #[test]
    fn f_nvme_009_gen3() {
        let nvme = NvmeTracker::for_gen3();
        assert_eq!(nvme.submissions, 0);
    }

    /// F-NVME-010: Factory for_gen4
    #[test]
    fn f_nvme_010_gen4() {
        let nvme = NvmeTracker::for_gen4();
        assert_eq!(nvme.submissions, 0);
    }

    /// F-NVME-011: Reset clears counters
    #[test]
    fn f_nvme_011_reset() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(10);
        nvme.reset();
        assert_eq!(nvme.submissions, 0);
    }

    /// F-NVME-012: Clone preserves state
    #[test]
    fn f_nvme_012_clone() {
        let mut nvme = NvmeTracker::new();
        nvme.submit(10);
        let cloned = nvme;
        assert_eq!(nvme.submissions, cloned.submissions);
    }
}

/// O(1) SCSI device tracker.
///
/// Tracks SCSI-specific metrics including commands, errors, timeouts,
/// and SCSI status codes. Provides constant-time access to SCSI device
/// performance data.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::ScsiTracker;
///
/// let mut scsi = ScsiTracker::new();
/// scsi.command();
/// scsi.complete_good();
/// assert_eq!(scsi.commands, 1);
/// assert_eq!(scsi.good_status, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScsiTracker {
    /// Total commands issued.
    pub commands: u64,
    /// Good (success) status.
    pub good_status: u64,
    /// Check condition status.
    pub check_condition: u64,
    /// Busy status.
    pub busy: u64,
    /// Command timeouts.
    pub timeouts: u64,
    /// Resets.
    pub resets: u64,
}

impl ScsiTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            commands: 0,
            good_status: 0,
            check_condition: 0,
            busy: 0,
            timeouts: 0,
            resets: 0,
        }
    }

    /// Factory for SAS device.
    #[must_use]
    pub const fn for_sas() -> Self {
        Self::new()
    }

    /// Factory for SATA device (via libata).
    #[must_use]
    pub const fn for_sata() -> Self {
        Self::new()
    }

    /// Record command issued.
    pub fn command(&mut self) {
        self.commands += 1;
    }

    /// Record good completion.
    pub fn complete_good(&mut self) {
        self.good_status += 1;
    }

    /// Record check condition.
    pub fn check(&mut self) {
        self.check_condition += 1;
    }

    /// Record busy.
    pub fn busy(&mut self) {
        self.busy += 1;
    }

    /// Record timeout.
    pub fn timeout(&mut self) {
        self.timeouts += 1;
    }

    /// Record reset.
    pub fn reset_device(&mut self) {
        self.resets += 1;
    }

    /// Get error rate (errors per total commands).
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        if self.commands == 0 {
            return 0.0;
        }
        let errors = self.check_condition + self.busy + self.timeouts;
        (errors as f64) / (self.commands as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.commands = 0;
        self.good_status = 0;
        self.check_condition = 0;
        self.busy = 0;
        self.timeouts = 0;
        self.resets = 0;
    }
}

#[cfg(test)]
mod scsi_tests {
    use super::*;

    /// F-SCSI-001: New tracker is empty
    #[test]
    fn f_scsi_001_new() {
        let scsi = ScsiTracker::new();
        assert_eq!(scsi.commands, 0);
    }

    /// F-SCSI-002: Default is empty
    #[test]
    fn f_scsi_002_default() {
        let scsi = ScsiTracker::default();
        assert_eq!(scsi.commands, 0);
    }

    /// F-SCSI-003: Command tracked
    #[test]
    fn f_scsi_003_command() {
        let mut scsi = ScsiTracker::new();
        scsi.command();
        assert_eq!(scsi.commands, 1);
    }

    /// F-SCSI-004: Good status tracked
    #[test]
    fn f_scsi_004_good() {
