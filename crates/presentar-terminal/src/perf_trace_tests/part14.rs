    pub const fn total_syscalls(&self) -> u64 {
        self.read_syscalls + self.write_syscalls
    }
}

define_tracker! {
    /// Scheduler accounting tracker - per-task scheduling statistics.
    ///
    /// O(1) tracking of scheduling latency and policy from /proc/[pid]/sched.
    pub struct SchedAccountingTracker {
        /// Number of times scheduled
        pub nr_switches: u64,
        /// Total run time (ns)
        pub sum_exec_runtime: u64,
        /// Total wait time (ns)
        pub wait_sum: u64,
        /// Max wait time (ns)
        pub wait_max: u64,
        /// Time slices used
        pub timeslices: u64,
        /// Priority inversions
        pub prio_inversions: u64,
    }
}

impl SchedAccountingTracker {
    /// Factory: Create from sched stats
    #[inline]
    #[must_use]
    pub fn for_sched(switches: u64, runtime_ns: u64) -> Self {
        Self {
            nr_switches: switches,
            sum_exec_runtime: runtime_ns,
            ..Self::new()
        }
    }

    /// Record context switch
    #[inline]
    pub fn switch(&mut self, runtime_ns: u64) {
        self.nr_switches = self.nr_switches.saturating_add(1);
        self.sum_exec_runtime = self.sum_exec_runtime.saturating_add(runtime_ns);
    }

    /// Record wait time
    #[inline]
    pub fn wait(&mut self, wait_ns: u64) {
        self.wait_sum = self.wait_sum.saturating_add(wait_ns);
        if wait_ns > self.wait_max {
            self.wait_max = wait_ns;
        }
    }

    /// Record timeslice
    #[inline]
    pub fn timeslice(&mut self) {
        self.timeslices = self.timeslices.saturating_add(1);
    }

    /// Record priority inversion
    #[inline]
    pub fn prio_inversion(&mut self) {
        self.prio_inversions = self.prio_inversions.saturating_add(1);
    }

    /// Average runtime per switch (ns)
    #[inline]
    #[must_use]
    pub fn avg_runtime(&self) -> u64 {
        if self.nr_switches > 0 {
            self.sum_exec_runtime / self.nr_switches
        } else {
            0
        }
    }
}

define_tracker! {
    /// Memory accounting tracker - per-task memory statistics.
    ///
    /// O(1) tracking of memory usage from /proc/[pid]/statm.
    pub struct MemAccountingTracker {
        /// Virtual memory size (pages)
        pub vsize: u64,
        /// Resident set size (pages)
        pub rss: u64,
        /// Shared pages
        pub shared: u64,
        /// Text (code) pages
        pub text: u64,
        /// Data + stack pages
        pub data: u64,
        /// Peak RSS (pages)
        pub peak_rss: u64,
    }
}

impl MemAccountingTracker {
    /// Factory: Create from statm values
    #[inline]
    #[must_use]
    pub fn for_statm(vsize: u64, rss: u64) -> Self {
        Self {
            vsize,
            rss,
            peak_rss: rss,
            ..Self::new()
        }
    }

    /// Update memory stats
    #[inline]
    pub fn update(&mut self, vsize: u64, rss: u64) {
        self.vsize = vsize;
        self.rss = rss;
        if rss > self.peak_rss {
            self.peak_rss = rss;
        }
    }

    /// Set shared pages
    #[inline]
    pub fn set_shared(&mut self, shared: u64) {
        self.shared = shared;
    }

    /// Set text pages
    #[inline]
    pub fn set_text(&mut self, text: u64) {
        self.text = text;
    }

    /// Set data pages
    #[inline]
    pub fn set_data(&mut self, data: u64) {
        self.data = data;
    }

    /// Private memory (rss - shared)
    #[inline]
    #[must_use]
    pub fn private_mem(&self) -> u64 {
        self.rss.saturating_sub(self.shared)
    }
}

#[cfg(test)]
mod task_acct_tests {
    use super::*;

    /// F-TACCT-001: New tracker is zeroed
    #[test]
    fn f_tacct_001_new() {
        let tracker = TaskAccountingTracker::new();
        assert_eq!(tracker.utime, 0);
        assert_eq!(tracker.stime, 0);
    }

    /// F-TACCT-002: Default is zeroed
    #[test]
    fn f_tacct_002_default() {
        let tracker = TaskAccountingTracker::default();
        assert_eq!(tracker.total_cpu(), 0);
    }

    /// F-TACCT-003: Factory sets utime/stime
    #[test]
    fn f_tacct_003_factory() {
        let tracker = TaskAccountingTracker::for_proc(100, 50);
        assert_eq!(tracker.utime, 100);
        assert_eq!(tracker.stime, 50);
    }

    /// F-TACCT-004: Add utime increments
    #[test]
    fn f_tacct_004_add_utime() {
        let mut tracker = TaskAccountingTracker::new();
        tracker.add_utime(100);
        assert_eq!(tracker.utime, 100);
    }

    /// F-TACCT-005: Add stime increments
    #[test]
    fn f_tacct_005_add_stime() {
        let mut tracker = TaskAccountingTracker::new();
        tracker.add_stime(50);
        assert_eq!(tracker.stime, 50);
    }

    /// F-TACCT-006: Voluntary switch increments
    #[test]
    fn f_tacct_006_voluntary() {
        let mut tracker = TaskAccountingTracker::new();
        tracker.voluntary_switch();
        assert_eq!(tracker.voluntary_ctxt_switches, 1);
    }

    /// F-TACCT-007: Involuntary switch increments
    #[test]
    fn f_tacct_007_involuntary() {
        let mut tracker = TaskAccountingTracker::new();
        tracker.involuntary_switch();
        assert_eq!(tracker.nonvoluntary_ctxt_switches, 1);
    }

    /// F-TACCT-008: Total CPU sums user+sys
    #[test]
    fn f_tacct_008_total_cpu() {
        let tracker = TaskAccountingTracker::for_proc(100, 50);
        assert_eq!(tracker.total_cpu(), 150);
    }

    /// F-TACCT-009: Total switches sums vol+invol
    #[test]
    fn f_tacct_009_total_switches() {
        let mut tracker = TaskAccountingTracker::new();
        tracker.voluntary_switch();
        tracker.involuntary_switch();
        assert_eq!(tracker.total_switches(), 2);
    }

    /// F-TACCT-010: Saturating add prevents overflow
    #[test]
    fn f_tacct_010_saturating() {
        let mut tracker = TaskAccountingTracker::for_proc(u64::MAX - 1, 0);
        tracker.add_utime(10);
        assert_eq!(tracker.utime, u64::MAX);
    }

    /// F-TACCT-011: Reset clears counters
    #[test]
    fn f_tacct_011_reset() {
        let mut tracker = TaskAccountingTracker::for_proc(100, 50);
        tracker.reset();
        assert_eq!(tracker.total_cpu(), 0);
    }

    /// F-TACCT-012: Clone preserves state
    #[test]
    fn f_tacct_012_clone() {
        let tracker = TaskAccountingTracker::for_proc(100, 50);
        let cloned = tracker;
        assert_eq!(tracker.utime, cloned.utime);
    }
}

#[cfg(test)]
mod io_acct_tests {
    use super::*;

    /// F-IOACCT-001: New tracker is zeroed
    #[test]
    fn f_ioacct_001_new() {
        let tracker = IoAccountingTracker::new();
        assert_eq!(tracker.read_bytes, 0);
    }

    /// F-IOACCT-002: Default is zeroed
    #[test]
    fn f_ioacct_002_default() {
        let tracker = IoAccountingTracker::default();
        assert_eq!(tracker.total_bytes(), 0);
    }

    /// F-IOACCT-003: Factory sets read/write bytes
    #[test]
    fn f_ioacct_003_factory() {
        let tracker = IoAccountingTracker::for_proc_io(1000, 500);
        assert_eq!(tracker.read_bytes, 1000);
        assert_eq!(tracker.write_bytes, 500);
    }

    /// F-IOACCT-004: Read increments bytes and syscalls
    #[test]
    fn f_ioacct_004_read() {
        let mut tracker = IoAccountingTracker::new();
        tracker.read(1024);
        assert_eq!(tracker.read_bytes, 1024);
        assert_eq!(tracker.read_syscalls, 1);
    }

    /// F-IOACCT-005: Write increments bytes and syscalls
    #[test]
    fn f_ioacct_005_write() {
        let mut tracker = IoAccountingTracker::new();
        tracker.write(2048);
        assert_eq!(tracker.write_bytes, 2048);
        assert_eq!(tracker.write_syscalls, 1);
    }

    /// F-IOACCT-006: Disk read tracks separately
    #[test]
    fn f_ioacct_006_disk_read() {
        let mut tracker = IoAccountingTracker::new();
        tracker.disk_read(4096);
        assert_eq!(tracker.disk_read_bytes, 4096);
    }

    /// F-IOACCT-007: Disk write tracks separately
    #[test]
    fn f_ioacct_007_disk_write() {
        let mut tracker = IoAccountingTracker::new();
        tracker.disk_write(8192);
        assert_eq!(tracker.disk_write_bytes, 8192);
    }

    /// F-IOACCT-008: Total bytes sums read+write
    #[test]
    fn f_ioacct_008_total_bytes() {
        let tracker = IoAccountingTracker::for_proc_io(1000, 500);
        assert_eq!(tracker.total_bytes(), 1500);
    }

    /// F-IOACCT-009: Total syscalls sums read+write
    #[test]
    fn f_ioacct_009_total_syscalls() {
        let mut tracker = IoAccountingTracker::new();
        tracker.read(1024);
        tracker.write(1024);
        assert_eq!(tracker.total_syscalls(), 2);
    }

    /// F-IOACCT-010: Saturating add prevents overflow
    #[test]
    fn f_ioacct_010_saturating() {
        let mut tracker = IoAccountingTracker::for_proc_io(u64::MAX - 1, 0);
        tracker.read(10);
        assert_eq!(tracker.read_bytes, u64::MAX);
    }

    /// F-IOACCT-011: Reset clears counters
    #[test]
    fn f_ioacct_011_reset() {
        let mut tracker = IoAccountingTracker::for_proc_io(1000, 500);
        tracker.reset();
        assert_eq!(tracker.total_bytes(), 0);
    }

    /// F-IOACCT-012: Clone preserves state
    #[test]
    fn f_ioacct_012_clone() {
        let tracker = IoAccountingTracker::for_proc_io(1000, 500);
        let cloned = tracker;
        assert_eq!(tracker.read_bytes, cloned.read_bytes);
    }
}

#[cfg(test)]
mod sched_acct_tests {
    use super::*;

    /// F-SCHEDACCT-001: New tracker is zeroed
    #[test]
    fn f_schedacct_001_new() {
        let tracker = SchedAccountingTracker::new();
        assert_eq!(tracker.nr_switches, 0);
    }

    /// F-SCHEDACCT-002: Default is zeroed
    #[test]
    fn f_schedacct_002_default() {
        let tracker = SchedAccountingTracker::default();
        assert_eq!(tracker.sum_exec_runtime, 0);
    }

    /// F-SCHEDACCT-003: Factory sets switches and runtime
    #[test]
    fn f_schedacct_003_factory() {
        let tracker = SchedAccountingTracker::for_sched(100, 1_000_000);
        assert_eq!(tracker.nr_switches, 100);
        assert_eq!(tracker.sum_exec_runtime, 1_000_000);
    }

    /// F-SCHEDACCT-004: Switch increments count and runtime
    #[test]
    fn f_schedacct_004_switch() {
        let mut tracker = SchedAccountingTracker::new();
        tracker.switch(10_000);
        assert_eq!(tracker.nr_switches, 1);
        assert_eq!(tracker.sum_exec_runtime, 10_000);
    }

    /// F-SCHEDACCT-005: Wait tracks sum and max
    #[test]
    fn f_schedacct_005_wait() {
        let mut tracker = SchedAccountingTracker::new();
        tracker.wait(5000);
        tracker.wait(10000);
        assert_eq!(tracker.wait_sum, 15000);
        assert_eq!(tracker.wait_max, 10000);
    }

    /// F-SCHEDACCT-006: Timeslice increments
    #[test]
    fn f_schedacct_006_timeslice() {
        let mut tracker = SchedAccountingTracker::new();
        tracker.timeslice();
        assert_eq!(tracker.timeslices, 1);
    }

    /// F-SCHEDACCT-007: Priority inversion increments
    #[test]
    fn f_schedacct_007_prio_inversion() {
        let mut tracker = SchedAccountingTracker::new();
        tracker.prio_inversion();
        assert_eq!(tracker.prio_inversions, 1);
    }

    /// F-SCHEDACCT-008: Avg runtime calculates correctly
    #[test]
    fn f_schedacct_008_avg_runtime() {
        let tracker = SchedAccountingTracker::for_sched(10, 100_000);
        assert_eq!(tracker.avg_runtime(), 10_000);
    }

    /// F-SCHEDACCT-009: Avg runtime returns 0 for no switches
    #[test]
    fn f_schedacct_009_avg_zero() {
        let tracker = SchedAccountingTracker::new();
        assert_eq!(tracker.avg_runtime(), 0);
    }

    /// F-SCHEDACCT-010: Saturating add prevents overflow
    #[test]
    fn f_schedacct_010_saturating() {
        let mut tracker = SchedAccountingTracker::for_sched(u64::MAX - 1, 0);
        tracker.switch(0);
        assert_eq!(tracker.nr_switches, u64::MAX);
    }

    /// F-SCHEDACCT-011: Reset clears counters
    #[test]
    fn f_schedacct_011_reset() {
        let mut tracker = SchedAccountingTracker::for_sched(100, 1_000_000);
        tracker.reset();
        assert_eq!(tracker.nr_switches, 0);
    }

    /// F-SCHEDACCT-012: Clone preserves state
    #[test]
    fn f_schedacct_012_clone() {
        let tracker = SchedAccountingTracker::for_sched(100, 1_000_000);
        let cloned = tracker;
        assert_eq!(tracker.nr_switches, cloned.nr_switches);
    }
}

#[cfg(test)]
mod mem_acct_tests {
    use super::*;

    /// F-MEMACCT-001: New tracker is zeroed
    #[test]
    fn f_memacct_001_new() {
        let tracker = MemAccountingTracker::new();
        assert_eq!(tracker.vsize, 0);
        assert_eq!(tracker.rss, 0);
    }

    /// F-MEMACCT-002: Default is zeroed
    #[test]
    fn f_memacct_002_default() {
        let tracker = MemAccountingTracker::default();
        assert_eq!(tracker.peak_rss, 0);
    }

    /// F-MEMACCT-003: Factory sets vsize and rss
    #[test]
    fn f_memacct_003_factory() {
        let tracker = MemAccountingTracker::for_statm(1000, 500);
        assert_eq!(tracker.vsize, 1000);
        assert_eq!(tracker.rss, 500);
        assert_eq!(tracker.peak_rss, 500);
    }

    /// F-MEMACCT-004: Update changes vsize/rss
    #[test]
    fn f_memacct_004_update() {
        let mut tracker = MemAccountingTracker::new();
        tracker.update(2000, 1000);
        assert_eq!(tracker.vsize, 2000);
        assert_eq!(tracker.rss, 1000);
    }

    /// F-MEMACCT-005: Update tracks peak RSS
    #[test]
    fn f_memacct_005_peak() {
        let mut tracker = MemAccountingTracker::for_statm(1000, 500);
        tracker.update(1000, 600);
        tracker.update(1000, 400);
        assert_eq!(tracker.peak_rss, 600);
    }

    /// F-MEMACCT-006: Set shared pages
    #[test]
    fn f_memacct_006_shared() {
        let mut tracker = MemAccountingTracker::new();
        tracker.set_shared(100);
        assert_eq!(tracker.shared, 100);
    }

    /// F-MEMACCT-007: Set text pages
    #[test]
    fn f_memacct_007_text() {
        let mut tracker = MemAccountingTracker::new();
        tracker.set_text(50);
        assert_eq!(tracker.text, 50);
    }

    /// F-MEMACCT-008: Set data pages
    #[test]
    fn f_memacct_008_data() {
        let mut tracker = MemAccountingTracker::new();
        tracker.set_data(200);
        assert_eq!(tracker.data, 200);
    }

    /// F-MEMACCT-009: Private mem = rss - shared
    #[test]
    fn f_memacct_009_private() {
        let mut tracker = MemAccountingTracker::for_statm(1000, 500);
        tracker.set_shared(100);
        assert_eq!(tracker.private_mem(), 400);
    }

    /// F-MEMACCT-010: Private mem saturates at 0
    #[test]
    fn f_memacct_010_private_saturate() {
        let mut tracker = MemAccountingTracker::for_statm(1000, 100);
        tracker.set_shared(200);
        assert_eq!(tracker.private_mem(), 0);
    }

    /// F-MEMACCT-011: Reset clears counters
    #[test]
    fn f_memacct_011_reset() {
        let mut tracker = MemAccountingTracker::for_statm(1000, 500);
        tracker.reset();
        assert_eq!(tracker.vsize, 0);
    }

    /// F-MEMACCT-012: Clone preserves state
    #[test]
    fn f_memacct_012_clone() {
        let tracker = MemAccountingTracker::for_statm(1000, 500);
        let cloned = tracker;
        assert_eq!(tracker.rss, cloned.rss);
    }
}

// ============================================================================
// v9.46.0: Namespace & Security O(1) Helpers
// ============================================================================

define_tracker! {
    /// PID namespace tracker - process ID tracking.
    ///
    /// O(1) tracking of PID space usage and recycling.
    pub struct PidTracker {
        /// Current active PIDs
        pub active_pids: u32,
        /// Peak active PIDs
        pub peak_pids: u32,
        /// Total PIDs allocated
        pub allocated: u64,
        /// Total PIDs recycled
        pub recycled: u64,
        /// PID wraps (reached max)
        pub wraps: u64,
        /// Allocation failures
        pub failures: u64,
    }
}

impl PidTracker {
    /// Factory: Create for namespace with active count
    #[inline]
    #[must_use]
    pub fn for_namespace(active: u32) -> Self {
        Self {
            active_pids: active,
            peak_pids: active,
            ..Self::new()
        }
    }

    /// Allocate a PID
    #[inline]
    pub fn allocate(&mut self) -> bool {
        if self.active_pids < u32::MAX {
            self.active_pids = self.active_pids.saturating_add(1);
            self.allocated = self.allocated.saturating_add(1);
            if self.active_pids > self.peak_pids {
                self.peak_pids = self.active_pids;
            }
            true
        } else {
            self.failures = self.failures.saturating_add(1);
            false
        }
    }

    /// Free a PID
    #[inline]
    pub fn free(&mut self) {
        if self.active_pids > 0 {
            self.active_pids = self.active_pids.saturating_sub(1);
            self.recycled = self.recycled.saturating_add(1);
        }
    }

    /// Record PID wrap event
    #[inline]
    pub fn wrap(&mut self) {
        self.wraps = self.wraps.saturating_add(1);
    }

    /// Utilization percentage
    #[inline]
    #[must_use]
    pub fn utilization(&self, max_pids: u32) -> f32 {
        if max_pids > 0 {
            (self.active_pids as f32 / max_pids as f32) * 100.0
        } else {
            0.0
        }
    }
}

define_tracker! {
    /// UID namespace tracker - user ID tracking.
    ///
    /// O(1) tracking of UID mappings and lookups.
    pub struct UidTracker {
        /// Number of UID mappings
        pub mappings: u32,
        /// UID lookups
        pub lookups: u64,
        /// Successful translations
        pub translations: u64,
        /// Translation failures
        pub failures: u64,
        /// Root mappings (uid 0)
        pub root_mappings: u32,
        /// Unprivileged mappings
        pub unpriv_mappings: u32,
    }
}

impl UidTracker {
    /// Factory: Create for user namespace
    #[inline]
    #[must_use]
    pub fn for_userns(mappings: u32) -> Self {
        Self {
            mappings,
            ..Self::new()
        }
    }

    /// Add a UID mapping
    #[inline]
    pub fn add_mapping(&mut self, is_root: bool) {
        self.mappings = self.mappings.saturating_add(1);
        if is_root {
            self.root_mappings = self.root_mappings.saturating_add(1);
        } else {
            self.unpriv_mappings = self.unpriv_mappings.saturating_add(1);
        }
    }

    /// Record UID lookup
    #[inline]
    pub fn lookup(&mut self, success: bool) {
        self.lookups = self.lookups.saturating_add(1);
        if success {
            self.translations = self.translations.saturating_add(1);
        } else {
            self.failures = self.failures.saturating_add(1);
        }
    }

    /// Translation success rate
    #[inline]
    #[must_use]
    pub fn success_rate(&self) -> f32 {
        if self.lookups > 0 {
            (self.translations as f32 / self.lookups as f32) * 100.0
        } else {
            100.0
        }
    }
}

define_tracker! {
    /// Namespace tracker - Linux namespace tracking.
    ///
    /// O(1) tracking of namespace operations.
    pub struct NamespaceTracker {
        /// Active namespaces
        pub active: u32,
        /// Created namespaces
        pub created: u64,
        /// Destroyed namespaces
        pub destroyed: u64,
        /// Setns operations
        pub setns_ops: u64,
        /// Unshare operations
        pub unshare_ops: u64,
        /// Clone with new ns
        pub clone_newns: u64,
    }
}

impl NamespaceTracker {
    /// Factory: Create for initial namespace count
    #[inline]
    #[must_use]
    pub fn for_system(active: u32) -> Self {
        Self {
            active,
            ..Self::new()
        }
    }

    /// Create namespace
    #[inline]
    pub fn create(&mut self) {
        self.active = self.active.saturating_add(1);
        self.created = self.created.saturating_add(1);
    }

    /// Destroy namespace
    #[inline]
    pub fn destroy(&mut self) {
        self.active = self.active.saturating_sub(1);
        self.destroyed = self.destroyed.saturating_add(1);
    }

    /// Record setns operation
    #[inline]
    pub fn setns(&mut self) {
        self.setns_ops = self.setns_ops.saturating_add(1);
    }

    /// Record unshare operation
    #[inline]
    pub fn unshare(&mut self) {
        self.unshare_ops = self.unshare_ops.saturating_add(1);
        self.create();
    }

    /// Record clone with CLONE_NEWNS
    #[inline]
    pub fn clone_ns(&mut self) {
        self.clone_newns = self.clone_newns.saturating_add(1);
        self.create();
    }
}

define_tracker! {
    /// Seccomp tracker - seccomp filter tracking.
    ///
    /// O(1) tracking of seccomp operations and violations.
    pub struct SeccompTracker {
        /// Active filters
        pub filters: u32,
        /// Syscalls checked
        pub checks: u64,
        /// Syscalls allowed
        pub allowed: u64,
        /// Syscalls denied
        pub denied: u64,
        /// Filter additions
        pub filter_adds: u64,
        /// Audit log events
        pub audit_events: u64,
    }
}

impl SeccompTracker {
    /// Factory: Create with initial filter count
    #[inline]
    #[must_use]
    pub fn for_process(filters: u32) -> Self {
        Self {
            filters,
            ..Self::new()
        }
    }

    /// Add a filter
    #[inline]
    pub fn add_filter(&mut self) {
        self.filters = self.filters.saturating_add(1);
        self.filter_adds = self.filter_adds.saturating_add(1);
    }

    /// Check syscall
    #[inline]
    pub fn check(&mut self, allowed: bool) {
        self.checks = self.checks.saturating_add(1);
        if allowed {
            self.allowed = self.allowed.saturating_add(1);
        } else {
            self.denied = self.denied.saturating_add(1);
        }
    }

    /// Record audit event
    #[inline]
    pub fn audit(&mut self) {
        self.audit_events = self.audit_events.saturating_add(1);
    }

    /// Allow rate percentage
    #[inline]
    #[must_use]
    pub fn allow_rate(&self) -> f32 {
        if self.checks > 0 {
            (self.allowed as f32 / self.checks as f32) * 100.0
        } else {
            100.0
        }
    }

    /// Deny rate percentage
    #[inline]
    #[must_use]
    pub fn deny_rate(&self) -> f32 {
        if self.checks > 0 {
            (self.denied as f32 / self.checks as f32) * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod pid_tests {
    use super::*;

    /// F-PID-001: New tracker is zeroed
    #[test]
    fn f_pid_001_new() {
        let tracker = PidTracker::new();
        assert_eq!(tracker.active_pids, 0);
    }

    /// F-PID-002: Default is zeroed
    #[test]
    fn f_pid_002_default() {
        let tracker = PidTracker::default();
        assert_eq!(tracker.allocated, 0);
    }

    /// F-PID-003: Factory sets active count
    #[test]
    fn f_pid_003_factory() {
        let tracker = PidTracker::for_namespace(100);
        assert_eq!(tracker.active_pids, 100);
        assert_eq!(tracker.peak_pids, 100);
    }

    /// F-PID-004: Allocate increments active
    #[test]
    fn f_pid_004_allocate() {
        let mut tracker = PidTracker::new();
        assert!(tracker.allocate());
        assert_eq!(tracker.active_pids, 1);
        assert_eq!(tracker.allocated, 1);
    }

    /// F-PID-005: Free decrements active
    #[test]
    fn f_pid_005_free() {
        let mut tracker = PidTracker::for_namespace(5);
        tracker.free();
        assert_eq!(tracker.active_pids, 4);
        assert_eq!(tracker.recycled, 1);
    }

    /// F-PID-006: Peak tracks maximum
    #[test]
    fn f_pid_006_peak() {
        let mut tracker = PidTracker::new();
        tracker.allocate();
        tracker.allocate();
        tracker.free();
        assert_eq!(tracker.peak_pids, 2);
    }

    /// F-PID-007: Wrap increments
    #[test]
    fn f_pid_007_wrap() {
        let mut tracker = PidTracker::new();
        tracker.wrap();
        assert_eq!(tracker.wraps, 1);
    }

    /// F-PID-008: Utilization calculates correctly
    #[test]
    fn f_pid_008_utilization() {
        let tracker = PidTracker::for_namespace(50);
        let util = tracker.utilization(100);
        assert!((util - 50.0).abs() < 0.1);
    }

    /// F-PID-009: Utilization handles zero max
    #[test]
    fn f_pid_009_util_zero() {
        let tracker = PidTracker::for_namespace(10);
        assert_eq!(tracker.utilization(0), 0.0);
    }

    /// F-PID-010: Free doesn't underflow
    #[test]
    fn f_pid_010_free_underflow() {
        let mut tracker = PidTracker::new();
        tracker.free();
        assert_eq!(tracker.active_pids, 0);
    }

    /// F-PID-011: Reset clears counters
    #[test]
    fn f_pid_011_reset() {
        let mut tracker = PidTracker::for_namespace(100);
        tracker.reset();
        assert_eq!(tracker.active_pids, 0);
    }

    /// F-PID-012: Clone preserves state
    #[test]
    fn f_pid_012_clone() {
        let tracker = PidTracker::for_namespace(100);
        let cloned = tracker;
        assert_eq!(tracker.active_pids, cloned.active_pids);
    }
}

#[cfg(test)]
mod uid_tests {
    use super::*;

    /// F-UID-001: New tracker is zeroed
    #[test]
    fn f_uid_001_new() {
        let tracker = UidTracker::new();
        assert_eq!(tracker.mappings, 0);
    }

    /// F-UID-002: Default is zeroed
    #[test]
    fn f_uid_002_default() {
        let tracker = UidTracker::default();
        assert_eq!(tracker.lookups, 0);
    }

    /// F-UID-003: Factory sets mappings
    #[test]
    fn f_uid_003_factory() {
        let tracker = UidTracker::for_userns(5);
        assert_eq!(tracker.mappings, 5);
    }

    /// F-UID-004: Add root mapping
    #[test]
    fn f_uid_004_root_mapping() {
        let mut tracker = UidTracker::new();
        tracker.add_mapping(true);
        assert_eq!(tracker.mappings, 1);
        assert_eq!(tracker.root_mappings, 1);
    }

    /// F-UID-005: Add unpriv mapping
    #[test]
    fn f_uid_005_unpriv_mapping() {
        let mut tracker = UidTracker::new();
        tracker.add_mapping(false);
        assert_eq!(tracker.mappings, 1);
        assert_eq!(tracker.unpriv_mappings, 1);
    }

    /// F-UID-006: Lookup success
    #[test]
    fn f_uid_006_lookup_success() {
        let mut tracker = UidTracker::new();
        tracker.lookup(true);
        assert_eq!(tracker.lookups, 1);
        assert_eq!(tracker.translations, 1);
    }

    /// F-UID-007: Lookup failure
    #[test]
    fn f_uid_007_lookup_failure() {
        let mut tracker = UidTracker::new();
        tracker.lookup(false);
        assert_eq!(tracker.lookups, 1);
        assert_eq!(tracker.failures, 1);
    }

    /// F-UID-008: Success rate calculates
    #[test]
    fn f_uid_008_success_rate() {
        let mut tracker = UidTracker::new();
        tracker.lookup(true);
        tracker.lookup(false);
        let rate = tracker.success_rate();
        assert!((rate - 50.0).abs() < 0.1);
    }

    /// F-UID-009: Success rate default 100%
    #[test]
    fn f_uid_009_default_rate() {
        let tracker = UidTracker::new();
        assert_eq!(tracker.success_rate(), 100.0);
    }

    /// F-UID-010: Mixed mappings
    #[test]
    fn f_uid_010_mixed() {
        let mut tracker = UidTracker::new();
        tracker.add_mapping(true);
        tracker.add_mapping(false);
        tracker.add_mapping(false);
        assert_eq!(tracker.mappings, 3);
        assert_eq!(tracker.root_mappings, 1);
        assert_eq!(tracker.unpriv_mappings, 2);
    }

    /// F-UID-011: Reset clears counters
    #[test]
    fn f_uid_011_reset() {
        let mut tracker = UidTracker::for_userns(5);
        tracker.reset();
        assert_eq!(tracker.mappings, 0);
    }

    /// F-UID-012: Clone preserves state
    #[test]
    fn f_uid_012_clone() {
        let tracker = UidTracker::for_userns(5);
        let cloned = tracker;
        assert_eq!(tracker.mappings, cloned.mappings);
    }
}

#[cfg(test)]
mod namespace_tests {
    use super::*;

    /// F-NS-001: New tracker is zeroed
    #[test]
    fn f_ns_001_new() {
        let tracker = NamespaceTracker::new();
        assert_eq!(tracker.active, 0);
    }

    /// F-NS-002: Default is zeroed
    #[test]
    fn f_ns_002_default() {
        let tracker = NamespaceTracker::default();
        assert_eq!(tracker.created, 0);
    }

    /// F-NS-003: Factory sets active count
    #[test]
    fn f_ns_003_factory() {
        let tracker = NamespaceTracker::for_system(10);
        assert_eq!(tracker.active, 10);
    }

    /// F-NS-004: Create increments active
    #[test]
    fn f_ns_004_create() {
        let mut tracker = NamespaceTracker::new();
        tracker.create();
        assert_eq!(tracker.active, 1);
        assert_eq!(tracker.created, 1);
    }

    /// F-NS-005: Destroy decrements active
    #[test]
    fn f_ns_005_destroy() {
        let mut tracker = NamespaceTracker::for_system(5);
        tracker.destroy();
        assert_eq!(tracker.active, 4);
        assert_eq!(tracker.destroyed, 1);
    }

    /// F-NS-006: Setns records op
    #[test]
    fn f_ns_006_setns() {
        let mut tracker = NamespaceTracker::new();
        tracker.setns();
        assert_eq!(tracker.setns_ops, 1);
    }

    /// F-NS-007: Unshare creates ns
    #[test]
    fn f_ns_007_unshare() {
        let mut tracker = NamespaceTracker::new();
        tracker.unshare();
        assert_eq!(tracker.unshare_ops, 1);
        assert_eq!(tracker.active, 1);
        assert_eq!(tracker.created, 1);
    }

    /// F-NS-008: Clone ns creates ns
    #[test]
    fn f_ns_008_clone_ns() {
        let mut tracker = NamespaceTracker::new();
        tracker.clone_ns();
        assert_eq!(tracker.clone_newns, 1);
        assert_eq!(tracker.active, 1);
    }

    /// F-NS-009: Destroy doesn't underflow
    #[test]
    fn f_ns_009_destroy_underflow() {
        let mut tracker = NamespaceTracker::new();
        tracker.destroy();
        assert_eq!(tracker.active, 0);
    }

    /// F-NS-010: Multiple creates
    #[test]
    fn f_ns_010_multiple() {
        let mut tracker = NamespaceTracker::new();
        tracker.create();
        tracker.unshare();
        tracker.clone_ns();
        assert_eq!(tracker.active, 3);
        assert_eq!(tracker.created, 3);
    }

    /// F-NS-011: Reset clears counters
    #[test]
    fn f_ns_011_reset() {
        let mut tracker = NamespaceTracker::for_system(10);
        tracker.reset();
        assert_eq!(tracker.active, 0);
    }

    /// F-NS-012: Clone preserves state
    #[test]
    fn f_ns_012_clone() {
        let tracker = NamespaceTracker::for_system(10);
        let cloned = tracker;
        assert_eq!(tracker.active, cloned.active);
    }
}

#[cfg(test)]
mod seccomp_tests {
    use super::*;

    /// F-SECCOMP-001: New tracker is zeroed
    #[test]
    fn f_seccomp_001_new() {
        let tracker = SeccompTracker::new();
        assert_eq!(tracker.filters, 0);
    }

    /// F-SECCOMP-002: Default is zeroed
    #[test]
    fn f_seccomp_002_default() {
        let tracker = SeccompTracker::default();
        assert_eq!(tracker.checks, 0);
    }

    /// F-SECCOMP-003: Factory sets filter count
    #[test]
    fn f_seccomp_003_factory() {
        let tracker = SeccompTracker::for_process(3);
        assert_eq!(tracker.filters, 3);
    }

    /// F-SECCOMP-004: Add filter increments
    #[test]
    fn f_seccomp_004_add_filter() {
        let mut tracker = SeccompTracker::new();
        tracker.add_filter();
        assert_eq!(tracker.filters, 1);
        assert_eq!(tracker.filter_adds, 1);
    }

    /// F-SECCOMP-005: Check allowed
    #[test]
    fn f_seccomp_005_check_allow() {
        let mut tracker = SeccompTracker::new();
        tracker.check(true);
        assert_eq!(tracker.checks, 1);
        assert_eq!(tracker.allowed, 1);
    }

    /// F-SECCOMP-006: Check denied
    #[test]
    fn f_seccomp_006_check_deny() {
        let mut tracker = SeccompTracker::new();
        tracker.check(false);
        assert_eq!(tracker.checks, 1);
        assert_eq!(tracker.denied, 1);
    }

    /// F-SECCOMP-007: Audit records event
    #[test]
    fn f_seccomp_007_audit() {
        let mut tracker = SeccompTracker::new();
        tracker.audit();
        assert_eq!(tracker.audit_events, 1);
    }

    /// F-SECCOMP-008: Allow rate calculates
    #[test]
    fn f_seccomp_008_allow_rate() {
        let mut tracker = SeccompTracker::new();
        tracker.check(true);
        tracker.check(false);
        let rate = tracker.allow_rate();
        assert!((rate - 50.0).abs() < 0.1);
    }

    /// F-SECCOMP-009: Default allow rate 100%
    #[test]
    fn f_seccomp_009_default_rate() {
        let tracker = SeccompTracker::new();
        assert_eq!(tracker.allow_rate(), 100.0);
    }

    /// F-SECCOMP-010: Deny rate calculates
    #[test]
    fn f_seccomp_010_deny_rate() {
        let mut tracker = SeccompTracker::new();
        tracker.check(false);
        let rate = tracker.deny_rate();
        assert!((rate - 100.0).abs() < 0.1);
    }

    /// F-SECCOMP-011: Reset clears counters
    #[test]
    fn f_seccomp_011_reset() {
        let mut tracker = SeccompTracker::for_process(3);
        tracker.reset();
        assert_eq!(tracker.filters, 0);
    }

    /// F-SECCOMP-012: Clone preserves state
    #[test]
    fn f_seccomp_012_clone() {
        let tracker = SeccompTracker::for_process(3);
        let cloned = tracker;
        assert_eq!(tracker.filters, cloned.filters);
    }
}

// ============================================================================
// v9.47.0: Security Subsystem O(1) Helpers
// ============================================================================

define_tracker! {
    /// Capabilities tracker - Linux capabilities tracking.
    ///
    /// O(1) tracking of capability checks and changes.
    pub struct CapabilitiesTracker {
        /// Capability checks performed
        pub checks: u64,
        /// Capabilities granted
        pub granted: u64,
        /// Capabilities denied
        pub denied: u64,
        /// Capability set operations
        pub set_ops: u64,
        /// Capability drops
        pub drops: u64,
        /// Ambient caps raised
        pub ambient_raises: u64,
    }
}

impl CapabilitiesTracker {
    /// Factory: Create for process
    #[inline]
    #[must_use]
    pub fn for_process() -> Self {
        Self::new()
    }

    /// Check capability
    #[inline]
    pub fn check(&mut self, has_cap: bool) {
        self.checks = self.checks.saturating_add(1);
        if has_cap {
            self.granted = self.granted.saturating_add(1);
        } else {
            self.denied = self.denied.saturating_add(1);
        }
    }

    /// Set capability
    #[inline]
    pub fn set_cap(&mut self) {
        self.set_ops = self.set_ops.saturating_add(1);
    }

    /// Drop capability
    #[inline]
    pub fn drop_cap(&mut self) {
        self.drops = self.drops.saturating_add(1);
    }

    /// Raise ambient cap
    #[inline]
    pub fn raise_ambient(&mut self) {
        self.ambient_raises = self.ambient_raises.saturating_add(1);
    }

    /// Grant rate percentage
    #[inline]
    #[must_use]
    pub fn grant_rate(&self) -> f32 {
        if self.checks > 0 {
            (self.granted as f32 / self.checks as f32) * 100.0
        } else {
            100.0
        }
    }
}

define_tracker! {
    /// LSM (Linux Security Module) tracker.
    ///
    /// O(1) tracking of LSM hooks and decisions.
    pub struct LsmTracker {
        /// Hook invocations
        pub hooks: u64,
        /// Allowed decisions
        pub allowed: u64,
        /// Denied decisions
        pub denied: u64,
        /// Audit events
        pub audits: u64,
        /// Policy loads
        pub policy_loads: u64,
        /// Label transitions
        pub transitions: u64,
    }
}

impl LsmTracker {
    /// Factory: Create for security module
    #[inline]
    #[must_use]
    pub fn for_selinux() -> Self {
        Self::new()
    }

    /// Record hook invocation
    #[inline]
    pub fn hook(&mut self, allowed: bool) {
        self.hooks = self.hooks.saturating_add(1);
        if allowed {
            self.allowed = self.allowed.saturating_add(1);
        } else {
            self.denied = self.denied.saturating_add(1);
        }
    }

    /// Record audit event
    #[inline]
    pub fn audit(&mut self) {
        self.audits = self.audits.saturating_add(1);
    }

    /// Record policy load
    #[inline]
    pub fn load_policy(&mut self) {
        self.policy_loads = self.policy_loads.saturating_add(1);
    }

    /// Record label transition
    #[inline]
    pub fn transition(&mut self) {
        self.transitions = self.transitions.saturating_add(1);
    }

    /// Allow rate percentage
    #[inline]
    #[must_use]
    pub fn allow_rate(&self) -> f32 {
        if self.hooks > 0 {
            (self.allowed as f32 / self.hooks as f32) * 100.0
        } else {
            100.0
        }
    }
}

define_tracker! {
    /// Audit tracker - Linux audit subsystem tracking.
    ///
    /// O(1) tracking of audit events and records.
    pub struct AuditTracker {
        /// Audit records generated
        pub records: u64,
        /// Records written
        pub written: u64,
        /// Records dropped
        pub dropped: u64,
        /// Backlog size
        pub backlog: u32,
        /// Peak backlog
        pub peak_backlog: u32,
        /// Rules loaded
        pub rules: u32,
    }
}

impl AuditTracker {
    /// Factory: Create for audit daemon
    #[inline]
    #[must_use]
    pub fn for_auditd(rules: u32) -> Self {
        Self {
            rules,
            ..Self::new()
        }
    }

    /// Generate audit record
    #[inline]
    pub fn generate(&mut self) {
        self.records = self.records.saturating_add(1);
        self.backlog = self.backlog.saturating_add(1);
        if self.backlog > self.peak_backlog {
            self.peak_backlog = self.backlog;
        }
    }

    /// Write record
    #[inline]
    pub fn write(&mut self) {
        self.written = self.written.saturating_add(1);
        self.backlog = self.backlog.saturating_sub(1);
    }

    /// Drop record
    #[inline]
    pub fn drop_record(&mut self) {
        self.dropped = self.dropped.saturating_add(1);
        self.backlog = self.backlog.saturating_sub(1);
    }

    /// Add audit rule
    #[inline]
    pub fn add_rule(&mut self) {
        self.rules = self.rules.saturating_add(1);
    }

    /// Drop rate percentage
    #[inline]
    #[must_use]
    pub fn drop_rate(&self) -> f32 {
        if self.records > 0 {
            (self.dropped as f32 / self.records as f32) * 100.0
        } else {
            0.0
        }
    }
}

define_tracker! {
    /// Integrity tracker - IMA/EVM tracking.
    ///
    /// O(1) tracking of integrity measurements and verifications.
    pub struct IntegrityTracker {
        /// Measurements taken
        pub measurements: u64,
        /// Verifications passed
        pub verified: u64,
        /// Verifications failed
        pub failed: u64,
        /// Appraisals performed
        pub appraisals: u64,
        /// Signatures validated
        pub signatures: u64,
        /// Policy violations
        pub violations: u64,
    }
}

impl IntegrityTracker {
    /// Factory: Create for IMA
    #[inline]
    #[must_use]
    pub fn for_ima() -> Self {
        Self::new()
    }

    /// Record measurement
    #[inline]
    pub fn measure(&mut self) {
        self.measurements = self.measurements.saturating_add(1);
    }

    /// Record verification
    #[inline]
    pub fn verify(&mut self, success: bool) {
        if success {
            self.verified = self.verified.saturating_add(1);
        } else {
            self.failed = self.failed.saturating_add(1);
        }
    }

    /// Record appraisal
    #[inline]
    pub fn appraise(&mut self) {
        self.appraisals = self.appraisals.saturating_add(1);
    }

    /// Record signature validation
    #[inline]
    pub fn validate_sig(&mut self) {
        self.signatures = self.signatures.saturating_add(1);
    }

    /// Record policy violation
    #[inline]
    pub fn violation(&mut self) {
        self.violations = self.violations.saturating_add(1);
    }

    /// Verification success rate
    #[inline]
    #[must_use]
    pub fn success_rate(&self) -> f32 {
        let total = self.verified + self.failed;
        if total > 0 {
            (self.verified as f32 / total as f32) * 100.0
        } else {
            100.0
        }
    }
}

#[cfg(test)]
mod cap_tests {
    use super::*;

    /// F-CAP-001: New tracker is zeroed
    #[test]
    fn f_cap_001_new() {
        let tracker = CapabilitiesTracker::new();
        assert_eq!(tracker.checks, 0);
    }

    /// F-CAP-002: Default is zeroed
    #[test]
    fn f_cap_002_default() {
        let tracker = CapabilitiesTracker::default();
        assert_eq!(tracker.granted, 0);
    }

    /// F-CAP-003: Factory creates tracker
    #[test]
    fn f_cap_003_factory() {
        let tracker = CapabilitiesTracker::for_process();
        assert_eq!(tracker.checks, 0);
    }

    /// F-CAP-004: Check granted
    #[test]
    fn f_cap_004_check_granted() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.check(true);
        assert_eq!(tracker.checks, 1);
        assert_eq!(tracker.granted, 1);
    }

    /// F-CAP-005: Check denied
    #[test]
    fn f_cap_005_check_denied() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.check(false);
        assert_eq!(tracker.checks, 1);
        assert_eq!(tracker.denied, 1);
    }

    /// F-CAP-006: Set cap increments
    #[test]
    fn f_cap_006_set_cap() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.set_cap();
        assert_eq!(tracker.set_ops, 1);
    }

    /// F-CAP-007: Drop cap increments
    #[test]
    fn f_cap_007_drop_cap() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.drop_cap();
        assert_eq!(tracker.drops, 1);
    }

    /// F-CAP-008: Raise ambient increments
    #[test]
    fn f_cap_008_ambient() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.raise_ambient();
        assert_eq!(tracker.ambient_raises, 1);
    }

    /// F-CAP-009: Grant rate calculates
    #[test]
    fn f_cap_009_grant_rate() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.check(true);
        tracker.check(false);
        let rate = tracker.grant_rate();
        assert!((rate - 50.0).abs() < 0.1);
    }

    /// F-CAP-010: Default grant rate 100%
    #[test]
    fn f_cap_010_default_rate() {
        let tracker = CapabilitiesTracker::new();
        assert_eq!(tracker.grant_rate(), 100.0);
    }

    /// F-CAP-011: Reset clears counters
    #[test]
    fn f_cap_011_reset() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.check(true);
        tracker.reset();
        assert_eq!(tracker.checks, 0);
    }

    /// F-CAP-012: Clone preserves state
    #[test]
    fn f_cap_012_clone() {
        let mut tracker = CapabilitiesTracker::new();
        tracker.check(true);
        let cloned = tracker;
        assert_eq!(tracker.checks, cloned.checks);
    }
}

#[cfg(test)]
mod lsm_tests {
    use super::*;

    /// F-LSM-001: New tracker is zeroed
    #[test]
    fn f_lsm_001_new() {
        let tracker = LsmTracker::new();
        assert_eq!(tracker.hooks, 0);
    }

    /// F-LSM-002: Default is zeroed
    #[test]
    fn f_lsm_002_default() {
        let tracker = LsmTracker::default();
        assert_eq!(tracker.allowed, 0);
    }

    /// F-LSM-003: Factory creates tracker
    #[test]
    fn f_lsm_003_factory() {
        let tracker = LsmTracker::for_selinux();
        assert_eq!(tracker.hooks, 0);
    }

    /// F-LSM-004: Hook allowed
    #[test]
    fn f_lsm_004_hook_allowed() {
        let mut tracker = LsmTracker::new();
        tracker.hook(true);
        assert_eq!(tracker.hooks, 1);
        assert_eq!(tracker.allowed, 1);
    }

    /// F-LSM-005: Hook denied
    #[test]
    fn f_lsm_005_hook_denied() {
        let mut tracker = LsmTracker::new();
        tracker.hook(false);
        assert_eq!(tracker.hooks, 1);
        assert_eq!(tracker.denied, 1);
    }

    /// F-LSM-006: Audit increments
    #[test]
    fn f_lsm_006_audit() {
        let mut tracker = LsmTracker::new();
        tracker.audit();
        assert_eq!(tracker.audits, 1);
    }

    /// F-LSM-007: Policy load increments
    #[test]
    fn f_lsm_007_policy() {
        let mut tracker = LsmTracker::new();
        tracker.load_policy();
        assert_eq!(tracker.policy_loads, 1);
    }

    /// F-LSM-008: Transition increments
    #[test]
    fn f_lsm_008_transition() {
        let mut tracker = LsmTracker::new();
        tracker.transition();
        assert_eq!(tracker.transitions, 1);
    }

    /// F-LSM-009: Allow rate calculates
    #[test]
    fn f_lsm_009_allow_rate() {
        let mut tracker = LsmTracker::new();
        tracker.hook(true);
        tracker.hook(false);
        let rate = tracker.allow_rate();
        assert!((rate - 50.0).abs() < 0.1);
    }

    /// F-LSM-010: Default allow rate 100%
    #[test]
    fn f_lsm_010_default_rate() {
        let tracker = LsmTracker::new();
        assert_eq!(tracker.allow_rate(), 100.0);
    }

    /// F-LSM-011: Reset clears counters
    #[test]
    fn f_lsm_011_reset() {
        let mut tracker = LsmTracker::new();
        tracker.hook(true);
        tracker.reset();
        assert_eq!(tracker.hooks, 0);
    }

    /// F-LSM-012: Clone preserves state
    #[test]
    fn f_lsm_012_clone() {
        let mut tracker = LsmTracker::new();
        tracker.hook(true);
        let cloned = tracker;
        assert_eq!(tracker.hooks, cloned.hooks);
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    /// F-AUDIT-001: New tracker is zeroed
    #[test]
    fn f_audit_001_new() {
        let tracker = AuditTracker::new();
        assert_eq!(tracker.records, 0);
    }

    /// F-AUDIT-002: Default is zeroed
    #[test]
    fn f_audit_002_default() {
        let tracker = AuditTracker::default();
        assert_eq!(tracker.written, 0);
    }

    /// F-AUDIT-003: Factory sets rules
    #[test]
    fn f_audit_003_factory() {
        let tracker = AuditTracker::for_auditd(10);
        assert_eq!(tracker.rules, 10);
    }

    /// F-AUDIT-004: Generate increments records and backlog
    #[test]
    fn f_audit_004_generate() {
        let mut tracker = AuditTracker::new();
        tracker.generate();
        assert_eq!(tracker.records, 1);
        assert_eq!(tracker.backlog, 1);
    }

    /// F-AUDIT-005: Write decrements backlog
    #[test]
    fn f_audit_005_write() {
        let mut tracker = AuditTracker::new();
        tracker.generate();
        tracker.write();
        assert_eq!(tracker.written, 1);
        assert_eq!(tracker.backlog, 0);
    }

    /// F-AUDIT-006: Drop records tracked
    #[test]
    fn f_audit_006_drop() {
        let mut tracker = AuditTracker::new();
        tracker.generate();
        tracker.drop_record();
        assert_eq!(tracker.dropped, 1);
        assert_eq!(tracker.backlog, 0);
    }

    /// F-AUDIT-007: Add rule increments
    #[test]
    fn f_audit_007_add_rule() {
        let mut tracker = AuditTracker::new();
        tracker.add_rule();
        assert_eq!(tracker.rules, 1);
    }

    /// F-AUDIT-008: Peak backlog tracks max
    #[test]
    fn f_audit_008_peak() {
        let mut tracker = AuditTracker::new();
        tracker.generate();
        tracker.generate();
        tracker.write();
        assert_eq!(tracker.peak_backlog, 2);
    }

    /// F-AUDIT-009: Drop rate calculates
    #[test]
    fn f_audit_009_drop_rate() {
        let mut tracker = AuditTracker::new();
        tracker.generate();
        tracker.drop_record();
        let rate = tracker.drop_rate();
        assert!((rate - 100.0).abs() < 0.1);
    }

    /// F-AUDIT-010: Default drop rate 0%
    #[test]
    fn f_audit_010_default_rate() {
        let tracker = AuditTracker::new();
        assert_eq!(tracker.drop_rate(), 0.0);
    }

    /// F-AUDIT-011: Reset clears counters
    #[test]
    fn f_audit_011_reset() {
        let mut tracker = AuditTracker::for_auditd(10);
        tracker.reset();
        assert_eq!(tracker.rules, 0);
    }

    /// F-AUDIT-012: Clone preserves state
    #[test]
    fn f_audit_012_clone() {
        let tracker = AuditTracker::for_auditd(10);
        let cloned = tracker;
        assert_eq!(tracker.rules, cloned.rules);
    }
}

#[cfg(test)]
mod integrity_tests {
    use super::*;

    /// F-INTEGRITY-001: New tracker is zeroed
    #[test]
    fn f_integrity_001_new() {
        let tracker = IntegrityTracker::new();
        assert_eq!(tracker.measurements, 0);
    }

    /// F-INTEGRITY-002: Default is zeroed
    #[test]
    fn f_integrity_002_default() {
        let tracker = IntegrityTracker::default();
        assert_eq!(tracker.verified, 0);
    }

    /// F-INTEGRITY-003: Factory creates tracker
    #[test]
    fn f_integrity_003_factory() {
        let tracker = IntegrityTracker::for_ima();
        assert_eq!(tracker.measurements, 0);
    }

    /// F-INTEGRITY-004: Measure increments
    #[test]
    fn f_integrity_004_measure() {
        let mut tracker = IntegrityTracker::new();
        tracker.measure();
        assert_eq!(tracker.measurements, 1);
    }

    /// F-INTEGRITY-005: Verify success
    #[test]
    fn f_integrity_005_verify_success() {
        let mut tracker = IntegrityTracker::new();
        tracker.verify(true);
        assert_eq!(tracker.verified, 1);
    }

    /// F-INTEGRITY-006: Verify failure
