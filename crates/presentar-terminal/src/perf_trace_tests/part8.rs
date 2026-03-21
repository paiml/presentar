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

// ============================================================================
// PageCacheTracker - O(1) page cache tracking
// ============================================================================

/// O(1) page cache hit/miss tracking.
///
/// Tracks page cache efficiency for memory-mapped files.
#[derive(Debug, Clone)]
pub struct PageCacheTracker {
    /// Cache hits
    pub hits: u64,
    /// Cache misses
    pub misses: u64,
    /// Pages evicted
    pub evictions: u64,
    /// Dirty pages written back
    pub writebacks: u64,
    /// Total pages
    pub total_pages: u64,
}

impl Default for PageCacheTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl PageCacheTracker {
    /// Create new page cache tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            evictions: 0,
            writebacks: 0,
            total_pages: 0,
        }
    }

    /// Factory for file cache tracking.
    #[must_use]
    pub fn for_file_cache() -> Self {
        Self::new()
    }

    /// Factory for mmap tracking.
    #[must_use]
    pub fn for_mmap() -> Self {
        Self::new()
    }

    /// Record cache hit.
    pub fn hit(&mut self) {
        self.hits += 1;
    }

    /// Record cache miss.
    pub fn miss(&mut self) {
        self.misses += 1;
        self.total_pages += 1;
    }

    /// Record page eviction.
    pub fn evict(&mut self) {
        self.evictions += 1;
        if self.total_pages > 0 {
            self.total_pages -= 1;
        }
    }

    /// Record dirty page writeback.
    pub fn writeback(&mut self) {
        self.writebacks += 1;
    }

    /// Get hit rate percentage.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        (self.hits as f64 / total as f64) * 100.0
    }

    /// Get eviction rate percentage.
    #[must_use]
    pub fn eviction_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        (self.evictions as f64 / total as f64) * 100.0
    }

    /// Check if cache is effective (hit rate > 80%).
    #[must_use]
    pub fn is_effective(&self) -> bool {
        self.hit_rate() > 80.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.evictions = 0;
        self.writebacks = 0;
        self.total_pages = 0;
    }
}

#[cfg(test)]
mod page_cache_tests {
    use super::*;

    /// F-PCACHE-001: New tracker is empty
    #[test]
    fn f_pcache_001_new() {
        let pc = PageCacheTracker::new();
        assert_eq!(pc.hits, 0);
    }

    /// F-PCACHE-002: Default is empty
    #[test]
    fn f_pcache_002_default() {
        let pc = PageCacheTracker::default();
        assert_eq!(pc.hits, 0);
    }

    /// F-PCACHE-003: Hit recorded
    #[test]
    fn f_pcache_003_hit() {
        let mut pc = PageCacheTracker::new();
        pc.hit();
        assert_eq!(pc.hits, 1);
    }

    /// F-PCACHE-004: Miss recorded
    #[test]
    fn f_pcache_004_miss() {
        let mut pc = PageCacheTracker::new();
        pc.miss();
        assert_eq!(pc.misses, 1);
        assert_eq!(pc.total_pages, 1);
    }

    /// F-PCACHE-005: Hit rate calculated
    #[test]
    fn f_pcache_005_hit_rate() {
        let mut pc = PageCacheTracker::new();
        pc.hit();
        pc.miss();
        assert!((pc.hit_rate() - 50.0).abs() < 0.01);
    }

    /// F-PCACHE-006: Eviction tracked
    #[test]
    fn f_pcache_006_evict() {
        let mut pc = PageCacheTracker::new();
        pc.miss();
        pc.evict();
        assert_eq!(pc.evictions, 1);
    }

    /// F-PCACHE-007: Factory for_file_cache
    #[test]
    fn f_pcache_007_for_file_cache() {
        let pc = PageCacheTracker::for_file_cache();
        assert_eq!(pc.hits, 0);
    }

    /// F-PCACHE-008: Factory for_mmap
    #[test]
    fn f_pcache_008_for_mmap() {
        let pc = PageCacheTracker::for_mmap();
        assert_eq!(pc.hits, 0);
    }

    /// F-PCACHE-009: Writeback tracked
    #[test]
    fn f_pcache_009_writeback() {
        let mut pc = PageCacheTracker::new();
        pc.writeback();
        assert_eq!(pc.writebacks, 1);
    }

    /// F-PCACHE-010: Is effective check
    #[test]
    fn f_pcache_010_effective() {
        let mut pc = PageCacheTracker::new();
        for _ in 0..9 {
            pc.hit();
        }
        pc.miss();
        assert!(pc.is_effective());
    }

    /// F-PCACHE-011: Reset clears state
    #[test]
    fn f_pcache_011_reset() {
        let mut pc = PageCacheTracker::new();
        pc.hit();
        pc.reset();
        assert_eq!(pc.hits, 0);
    }

    /// F-PCACHE-012: Clone preserves state
    #[test]
    fn f_pcache_012_clone() {
        let mut pc = PageCacheTracker::new();
        pc.hit();
        let cloned = pc.clone();
        assert_eq!(pc.hits, cloned.hits);
    }
}

// ============================================================================
// BufferPoolTracker - O(1) buffer pool tracking
// ============================================================================

/// O(1) buffer pool utilization tracking.
///
/// Tracks buffer allocation, reuse, and memory efficiency.
#[derive(Debug, Clone)]
pub struct BufferPoolTracker {
    /// Total buffers in pool
    pub capacity: u32,
    /// Currently allocated buffers
    pub allocated: u32,
    /// Buffers reused from pool
    pub reuses: u64,
    /// New allocations (pool empty)
    pub allocations: u64,
    /// Peak allocated count
    pub peak_allocated: u32,
}

impl Default for BufferPoolTracker {
    fn default() -> Self {
        Self::for_small()
    }
}

impl BufferPoolTracker {
    /// Create new buffer pool tracker.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        Self {
            capacity,
            allocated: 0,
            reuses: 0,
            allocations: 0,
            peak_allocated: 0,
        }
    }

    /// Factory for small buffer pools (64 buffers).
    #[must_use]
    pub fn for_small() -> Self {
        Self::new(64)
    }

    /// Factory for large buffer pools (1024 buffers).
    #[must_use]
    pub fn for_large() -> Self {
        Self::new(1024)
    }

    /// Get buffer from pool.
    pub fn get(&mut self) {
        self.allocated += 1;
        if self.allocated > self.peak_allocated {
            self.peak_allocated = self.allocated;
        }
        if self.allocated <= self.capacity {
            self.reuses += 1;
        } else {
            self.allocations += 1;
        }
    }

    /// Return buffer to pool.
    pub fn put(&mut self) {
        self.allocated = self.allocated.saturating_sub(1);
    }

    /// Get utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        (self.allocated as f64 / self.capacity as f64) * 100.0
    }

    /// Get reuse rate percentage.
    #[must_use]
    pub fn reuse_rate(&self) -> f64 {
        let total = self.reuses + self.allocations;
        if total == 0 {
            return 0.0;
        }
        (self.reuses as f64 / total as f64) * 100.0
    }

    /// Check if pool is efficient (reuse > 90%).
    #[must_use]
    pub fn is_efficient(&self) -> bool {
        self.reuse_rate() > 90.0
    }

    /// Check if pool needs expansion.
    #[must_use]
    pub fn needs_expansion(&self) -> bool {
        self.peak_allocated > self.capacity
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocated = 0;
        self.reuses = 0;
        self.allocations = 0;
        self.peak_allocated = 0;
    }
}

#[cfg(test)]
mod buffer_pool_tests {
    use super::*;

    /// F-BPOOL-001: New tracker has capacity
    #[test]
    fn f_bpool_001_new() {
        let bp = BufferPoolTracker::new(100);
        assert_eq!(bp.capacity, 100);
    }

    /// F-BPOOL-002: Default uses small
    #[test]
    fn f_bpool_002_default() {
        let bp = BufferPoolTracker::default();
        assert_eq!(bp.capacity, 64);
    }

    /// F-BPOOL-003: Get increases allocated
    #[test]
    fn f_bpool_003_get() {
        let mut bp = BufferPoolTracker::new(100);
        bp.get();
        assert_eq!(bp.allocated, 1);
    }

    /// F-BPOOL-004: Put decreases allocated
    #[test]
    fn f_bpool_004_put() {
        let mut bp = BufferPoolTracker::new(100);
        bp.get();
        bp.put();
        assert_eq!(bp.allocated, 0);
    }

    /// F-BPOOL-005: Utilization calculated
    #[test]
    fn f_bpool_005_utilization() {
        let mut bp = BufferPoolTracker::new(100);
        for _ in 0..50 {
            bp.get();
        }
        assert!((bp.utilization() - 50.0).abs() < 0.01);
    }

    /// F-BPOOL-006: Reuse tracked
    #[test]
    fn f_bpool_006_reuse() {
        let mut bp = BufferPoolTracker::new(100);
        bp.get();
        assert_eq!(bp.reuses, 1);
    }

    /// F-BPOOL-007: Factory for_small
    #[test]
    fn f_bpool_007_for_small() {
        let bp = BufferPoolTracker::for_small();
        assert_eq!(bp.capacity, 64);
    }

    /// F-BPOOL-008: Factory for_large
    #[test]
    fn f_bpool_008_for_large() {
        let bp = BufferPoolTracker::for_large();
        assert_eq!(bp.capacity, 1024);
    }

    /// F-BPOOL-009: Reuse rate calculated
    #[test]
    fn f_bpool_009_reuse_rate() {
        let mut bp = BufferPoolTracker::new(10);
        for _ in 0..10 {
            bp.get();
        }
        assert!((bp.reuse_rate() - 100.0).abs() < 0.01);
    }

    /// F-BPOOL-010: Needs expansion detection
    #[test]
    fn f_bpool_010_needs_expansion() {
        let mut bp = BufferPoolTracker::new(10);
        for _ in 0..15 {
            bp.get();
        }
        assert!(bp.needs_expansion());
    }

    /// F-BPOOL-011: Reset clears state
    #[test]
    fn f_bpool_011_reset() {
        let mut bp = BufferPoolTracker::new(100);
        bp.get();
        bp.reset();
        assert_eq!(bp.allocated, 0);
    }

    /// F-BPOOL-012: Clone preserves state
    #[test]
    fn f_bpool_012_clone() {
        let mut bp = BufferPoolTracker::new(100);
        bp.get();
        let cloned = bp.clone();
        assert_eq!(bp.allocated, cloned.allocated);
    }
}

// ============================================================================
// AsyncTaskTracker - O(1) async task tracking
// ============================================================================

/// O(1) async task state tracking.
///
/// Tracks async task lifecycle, pending/running/completed states.
#[derive(Debug, Clone)]
pub struct AsyncTaskTracker {
    /// Pending tasks
    pub pending: u64,
    /// Running tasks
    pub running: u64,
    /// Completed tasks
    pub completed: u64,
    /// Failed tasks
    pub failed: u64,
    /// Peak concurrent tasks
    pub peak_concurrent: u64,
}

impl Default for AsyncTaskTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl AsyncTaskTracker {
    /// Create new async task tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: 0,
            running: 0,
            completed: 0,
            failed: 0,
            peak_concurrent: 0,
        }
    }

    /// Factory for IO-bound tasks.
    #[must_use]
    pub fn for_io() -> Self {
        Self::new()
    }

    /// Factory for CPU-bound tasks.
    #[must_use]
    pub fn for_cpu() -> Self {
        Self::new()
    }

    /// Spawn new task (pending).
    pub fn spawn(&mut self) {
        self.pending += 1;
    }

    /// Task starts running.
    pub fn start(&mut self) {
        if self.pending > 0 {
            self.pending -= 1;
        }
        self.running += 1;
        let concurrent = self.pending + self.running;
        if concurrent > self.peak_concurrent {
            self.peak_concurrent = concurrent;
        }
    }

    /// Task completes successfully.
    pub fn complete(&mut self) {
        if self.running > 0 {
            self.running -= 1;
        }
        self.completed += 1;
    }

    /// Task fails.
    pub fn fail(&mut self) {
        if self.running > 0 {
            self.running -= 1;
        }
        self.failed += 1;
    }

    /// Get success rate percentage.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        let total = self.completed + self.failed;
        if total == 0 {
            return 0.0;
        }
        (self.completed as f64 / total as f64) * 100.0
    }

    /// Get total active tasks.
    #[must_use]
    pub fn active(&self) -> u64 {
        self.pending + self.running
    }

    /// Check if healthy (success rate > 95%).
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.success_rate() > 95.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.pending = 0;
        self.running = 0;
        self.completed = 0;
        self.failed = 0;
        self.peak_concurrent = 0;
    }
}

#[cfg(test)]
mod async_task_tests {
    use super::*;

    /// F-ASYNC-001: New tracker is empty
    #[test]
    fn f_async_001_new() {
        let at = AsyncTaskTracker::new();
        assert_eq!(at.active(), 0);
    }

    /// F-ASYNC-002: Default is empty
    #[test]
    fn f_async_002_default() {
        let at = AsyncTaskTracker::default();
        assert_eq!(at.active(), 0);
    }

    /// F-ASYNC-003: Spawn increases pending
    #[test]
    fn f_async_003_spawn() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        assert_eq!(at.pending, 1);
    }

    /// F-ASYNC-004: Start moves to running
    #[test]
    fn f_async_004_start() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        at.start();
        assert_eq!(at.pending, 0);
        assert_eq!(at.running, 1);
    }

    /// F-ASYNC-005: Complete decreases running
    #[test]
    fn f_async_005_complete() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        at.start();
        at.complete();
        assert_eq!(at.running, 0);
        assert_eq!(at.completed, 1);
    }

    /// F-ASYNC-006: Fail decreases running
    #[test]
    fn f_async_006_fail() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        at.start();
        at.fail();
        assert_eq!(at.running, 0);
        assert_eq!(at.failed, 1);
    }

    /// F-ASYNC-007: Factory for_io
    #[test]
    fn f_async_007_for_io() {
        let at = AsyncTaskTracker::for_io();
        assert_eq!(at.active(), 0);
    }

    /// F-ASYNC-008: Factory for_cpu
    #[test]
    fn f_async_008_for_cpu() {
        let at = AsyncTaskTracker::for_cpu();
        assert_eq!(at.active(), 0);
    }

    /// F-ASYNC-009: Success rate calculated
    #[test]
    fn f_async_009_success_rate() {
        let mut at = AsyncTaskTracker::new();
        at.completed = 9;
        at.failed = 1;
        assert!((at.success_rate() - 90.0).abs() < 0.01);
    }

    /// F-ASYNC-010: Is healthy check
    #[test]
    fn f_async_010_healthy() {
        let mut at = AsyncTaskTracker::new();
        at.completed = 100;
        assert!(at.is_healthy());
    }

    /// F-ASYNC-011: Reset clears state
    #[test]
    fn f_async_011_reset() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        at.reset();
        assert_eq!(at.pending, 0);
    }

    /// F-ASYNC-012: Clone preserves state
    #[test]
    fn f_async_012_clone() {
        let mut at = AsyncTaskTracker::new();
        at.spawn();
        let cloned = at.clone();
        assert_eq!(at.pending, cloned.pending);
    }
}

// ============================================================================
// ContextSwitchTracker - O(1) context switch tracking
// ============================================================================

/// O(1) context switch tracking.
///
/// Tracks voluntary/involuntary context switches for CPU affinity analysis.
#[derive(Debug, Clone)]
pub struct ContextSwitchTracker {
    /// Voluntary context switches (yield/sleep)
    pub voluntary: u64,
    /// Involuntary context switches (preemption)
    pub involuntary: u64,
    /// Total switches
    pub total: u64,
    /// Peak switches per interval
    pub peak_rate: u64,
    /// Last interval count
    pub last_interval: u64,
}

impl Default for ContextSwitchTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextSwitchTracker {
    /// Create new context switch tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            voluntary: 0,
            involuntary: 0,
            total: 0,
            peak_rate: 0,
            last_interval: 0,
        }
    }

    /// Factory for process tracking.
    #[must_use]
    pub fn for_process() -> Self {
        Self::new()
    }

    /// Factory for thread tracking.
    #[must_use]
    pub fn for_thread() -> Self {
        Self::new()
    }

    /// Record voluntary context switch.
    pub fn voluntary_switch(&mut self) {
        self.voluntary += 1;
        self.total += 1;
        self.last_interval += 1;
    }

    /// Record involuntary context switch.
    pub fn involuntary_switch(&mut self) {
        self.involuntary += 1;
        self.total += 1;
        self.last_interval += 1;
    }

    /// End interval and update peak.
    pub fn end_interval(&mut self) {
        if self.last_interval > self.peak_rate {
            self.peak_rate = self.last_interval;
        }
        self.last_interval = 0;
    }

    /// Get voluntary percentage.
    #[must_use]
    pub fn voluntary_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.voluntary as f64 / self.total as f64) * 100.0
    }

    /// Check if excessive involuntary switches (>30%).
    #[must_use]
    pub fn has_preemption_issue(&self) -> bool {
        self.voluntary_rate() < 70.0 && self.total > 0
    }

    /// Get switches per interval.
    #[must_use]
    pub fn rate(&self) -> u64 {
        self.last_interval
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.voluntary = 0;
        self.involuntary = 0;
        self.total = 0;
        self.peak_rate = 0;
        self.last_interval = 0;
    }
}

#[cfg(test)]
mod context_switch_tests {
    use super::*;

    /// F-CTXSW-001: New tracker is empty
    #[test]
    fn f_ctxsw_001_new() {
        let cs = ContextSwitchTracker::new();
        assert_eq!(cs.total, 0);
    }

    /// F-CTXSW-002: Default is empty
    #[test]
    fn f_ctxsw_002_default() {
        let cs = ContextSwitchTracker::default();
        assert_eq!(cs.total, 0);
    }

    /// F-CTXSW-003: Voluntary switch tracked
    #[test]
    fn f_ctxsw_003_voluntary() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        assert_eq!(cs.voluntary, 1);
        assert_eq!(cs.total, 1);
    }

    /// F-CTXSW-004: Involuntary switch tracked
    #[test]
    fn f_ctxsw_004_involuntary() {
        let mut cs = ContextSwitchTracker::new();
        cs.involuntary_switch();
        assert_eq!(cs.involuntary, 1);
        assert_eq!(cs.total, 1);
    }

    /// F-CTXSW-005: Voluntary rate calculated
    #[test]
    fn f_ctxsw_005_voluntary_rate() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        cs.involuntary_switch();
        assert!((cs.voluntary_rate() - 50.0).abs() < 0.01);
    }

    /// F-CTXSW-006: End interval updates peak
    #[test]
    fn f_ctxsw_006_end_interval() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        cs.voluntary_switch();
        cs.end_interval();
        assert_eq!(cs.peak_rate, 2);
        assert_eq!(cs.last_interval, 0);
    }

    /// F-CTXSW-007: Factory for_process
    #[test]
    fn f_ctxsw_007_for_process() {
        let cs = ContextSwitchTracker::for_process();
        assert_eq!(cs.total, 0);
    }

    /// F-CTXSW-008: Factory for_thread
    #[test]
    fn f_ctxsw_008_for_thread() {
        let cs = ContextSwitchTracker::for_thread();
        assert_eq!(cs.total, 0);
    }

    /// F-CTXSW-009: Preemption issue detected
    #[test]
    fn f_ctxsw_009_preemption() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary = 3;
        cs.involuntary = 7;
        cs.total = 10;
        assert!(cs.has_preemption_issue());
    }

    /// F-CTXSW-010: Rate returns interval count
    #[test]
    fn f_ctxsw_010_rate() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        assert_eq!(cs.rate(), 1);
    }

    /// F-CTXSW-011: Reset clears state
    #[test]
    fn f_ctxsw_011_reset() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        cs.reset();
        assert_eq!(cs.total, 0);
    }

    /// F-CTXSW-012: Clone preserves state
    #[test]
    fn f_ctxsw_012_clone() {
        let mut cs = ContextSwitchTracker::new();
        cs.voluntary_switch();
        let cloned = cs.clone();
        assert_eq!(cs.total, cloned.total);
    }
}

// ============================================================================
