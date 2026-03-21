        let mut scsi = ScsiTracker::new();
        scsi.complete_good();
        assert_eq!(scsi.good_status, 1);
    }

    /// F-SCSI-005: Check condition tracked
    #[test]
    fn f_scsi_005_check() {
        let mut scsi = ScsiTracker::new();
        scsi.check();
        assert_eq!(scsi.check_condition, 1);
    }

    /// F-SCSI-006: Busy tracked
    #[test]
    fn f_scsi_006_busy() {
        let mut scsi = ScsiTracker::new();
        scsi.busy();
        assert_eq!(scsi.busy, 1);
    }

    /// F-SCSI-007: Timeout tracked
    #[test]
    fn f_scsi_007_timeout() {
        let mut scsi = ScsiTracker::new();
        scsi.timeout();
        assert_eq!(scsi.timeouts, 1);
    }

    /// F-SCSI-008: Error rate
    #[test]
    fn f_scsi_008_error_rate() {
        let mut scsi = ScsiTracker::new();
        for _ in 0..100 {
            scsi.command();
        }
        scsi.check();
        scsi.timeout();
        assert!((scsi.error_rate() - 0.02).abs() < 0.001);
    }

    /// F-SCSI-009: Factory for_sas
    #[test]
    fn f_scsi_009_sas() {
        let scsi = ScsiTracker::for_sas();
        assert_eq!(scsi.commands, 0);
    }

    /// F-SCSI-010: Factory for_sata
    #[test]
    fn f_scsi_010_sata() {
        let scsi = ScsiTracker::for_sata();
        assert_eq!(scsi.commands, 0);
    }

    /// F-SCSI-011: Reset clears counters
    #[test]
    fn f_scsi_011_reset() {
        let mut scsi = ScsiTracker::new();
        scsi.command();
        scsi.reset();
        assert_eq!(scsi.commands, 0);
    }

    /// F-SCSI-012: Clone preserves state
    #[test]
    fn f_scsi_012_clone() {
        let mut scsi = ScsiTracker::new();
        scsi.command();
        let cloned = scsi;
        assert_eq!(scsi.commands, cloned.commands);
    }
}

/// O(1) MD (software RAID) tracker.
///
/// Tracks Linux software RAID (md) metrics including rebuild progress,
/// sync operations, and member disk status. Provides constant-time access
/// to RAID array health data.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::MdTracker;
///
/// let mut md = MdTracker::new();
/// md.set_members(4, 4);
/// md.sync_progress(50);
/// assert_eq!(md.active_members, 4);
/// assert_eq!(md.sync_percent, 50);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MdTracker {
    /// Total members in array.
    pub total_members: u32,
    /// Active (healthy) members.
    pub active_members: u32,
    /// Sync/rebuild percentage.
    pub sync_percent: u8,
    /// Is sync in progress.
    pub syncing: bool,
    /// Read errors.
    pub read_errors: u64,
    /// Write errors.
    pub write_errors: u64,
}

impl MdTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            total_members: 0,
            active_members: 0,
            sync_percent: 100,
            syncing: false,
            read_errors: 0,
            write_errors: 0,
        }
    }

    /// Factory for RAID0 array.
    #[must_use]
    pub const fn for_raid0() -> Self {
        Self::new()
    }

    /// Factory for RAID1 array.
    #[must_use]
    pub const fn for_raid1() -> Self {
        Self::new()
    }

    /// Set member counts.
    pub fn set_members(&mut self, total: u32, active: u32) {
        self.total_members = total;
        self.active_members = active;
    }

    /// Set sync progress.
    pub fn sync_progress(&mut self, percent: u8) {
        self.sync_percent = percent.min(100);
        self.syncing = percent < 100;
    }

    /// Record read error.
    pub fn read_error(&mut self) {
        self.read_errors += 1;
    }

    /// Record write error.
    pub fn write_error(&mut self) {
        self.write_errors += 1;
    }

    /// Check if array is degraded.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        self.active_members < self.total_members
    }

    /// Check if array is healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.active_members == self.total_members && !self.syncing
    }

    /// Get total errors.
    #[must_use]
    pub fn total_errors(&self) -> u64 {
        self.read_errors + self.write_errors
    }

    /// Reset counters (not member state).
    pub fn reset(&mut self) {
        self.read_errors = 0;
        self.write_errors = 0;
    }
}

#[cfg(test)]
mod md_tests {
    use super::*;

    /// F-MD-001: New tracker is empty
    #[test]
    fn f_md_001_new() {
        let md = MdTracker::new();
        assert_eq!(md.total_members, 0);
    }

    /// F-MD-002: Default is empty
    #[test]
    fn f_md_002_default() {
        let md = MdTracker::default();
        assert_eq!(md.total_members, 0);
    }

    /// F-MD-003: Members set
    #[test]
    fn f_md_003_members() {
        let mut md = MdTracker::new();
        md.set_members(4, 4);
        assert_eq!(md.total_members, 4);
        assert_eq!(md.active_members, 4);
    }

    /// F-MD-004: Sync progress tracked
    #[test]
    fn f_md_004_sync() {
        let mut md = MdTracker::new();
        md.sync_progress(50);
        assert_eq!(md.sync_percent, 50);
        assert!(md.syncing);
    }

    /// F-MD-005: Read error tracked
    #[test]
    fn f_md_005_read_error() {
        let mut md = MdTracker::new();
        md.read_error();
        assert_eq!(md.read_errors, 1);
    }

    /// F-MD-006: Write error tracked
    #[test]
    fn f_md_006_write_error() {
        let mut md = MdTracker::new();
        md.write_error();
        assert_eq!(md.write_errors, 1);
    }

    /// F-MD-007: Degraded check
    #[test]
    fn f_md_007_degraded() {
        let mut md = MdTracker::new();
        md.set_members(4, 3);
        assert!(md.is_degraded());
    }

    /// F-MD-008: Healthy check
    #[test]
    fn f_md_008_healthy() {
        let mut md = MdTracker::new();
        md.set_members(4, 4);
        md.sync_progress(100);
        assert!(md.is_healthy());
    }

    /// F-MD-009: Factory for_raid0
    #[test]
    fn f_md_009_raid0() {
        let md = MdTracker::for_raid0();
        assert_eq!(md.total_members, 0);
    }

    /// F-MD-010: Factory for_raid1
    #[test]
    fn f_md_010_raid1() {
        let md = MdTracker::for_raid1();
        assert_eq!(md.total_members, 0);
    }

    /// F-MD-011: Reset clears errors
    #[test]
    fn f_md_011_reset() {
        let mut md = MdTracker::new();
        md.read_error();
        md.reset();
        assert_eq!(md.read_errors, 0);
    }

    /// F-MD-012: Clone preserves state
    #[test]
    fn f_md_012_clone() {
        let mut md = MdTracker::new();
        md.set_members(4, 4);
        let cloned = md;
        assert_eq!(md.total_members, cloned.total_members);
    }
}

// ============================================================================
// v9.41.0: File System Helpers
// ============================================================================

/// O(1) VFS (Virtual File System) tracker.
///
/// Tracks VFS-level operations including lookups, creates, unlinks,
/// and path traversals. Provides constant-time access to filesystem
/// operation counts.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::VfsTracker;
///
/// let mut vfs = VfsTracker::new();
/// vfs.lookup();
/// vfs.create();
/// assert_eq!(vfs.lookups, 1);
/// assert_eq!(vfs.creates, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VfsTracker {
    /// Path lookups.
    pub lookups: u64,
    /// File/dir creates.
    pub creates: u64,
    /// File/dir unlinks.
    pub unlinks: u64,
    /// Renames.
    pub renames: u64,
    /// Opens.
    pub opens: u64,
    /// Closes.
    pub closes: u64,
}

impl VfsTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            lookups: 0,
            creates: 0,
            unlinks: 0,
            renames: 0,
            opens: 0,
            closes: 0,
        }
    }

    /// Factory for ext4 filesystem.
    #[must_use]
    pub const fn for_ext4() -> Self {
        Self::new()
    }

    /// Factory for xfs filesystem.
    #[must_use]
    pub const fn for_xfs() -> Self {
        Self::new()
    }

    /// Record lookup.
    pub fn lookup(&mut self) {
        self.lookups += 1;
    }

    /// Record create.
    pub fn create(&mut self) {
        self.creates += 1;
    }

    /// Record unlink.
    pub fn unlink(&mut self) {
        self.unlinks += 1;
    }

    /// Record rename.
    pub fn rename(&mut self) {
        self.renames += 1;
    }

    /// Record open.
    pub fn open(&mut self) {
        self.opens += 1;
    }

    /// Record close.
    pub fn close(&mut self) {
        self.closes += 1;
    }

    /// Get total operations.
    #[must_use]
    pub fn total_ops(&self) -> u64 {
        self.lookups + self.creates + self.unlinks + self.renames + self.opens + self.closes
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.lookups = 0;
        self.creates = 0;
        self.unlinks = 0;
        self.renames = 0;
        self.opens = 0;
        self.closes = 0;
    }
}

#[cfg(test)]
mod vfs_tests {
    use super::*;

    /// F-VFS-001: New tracker is empty
    #[test]
    fn f_vfs_001_new() {
        let vfs = VfsTracker::new();
        assert_eq!(vfs.lookups, 0);
    }

    /// F-VFS-002: Default is empty
    #[test]
    fn f_vfs_002_default() {
        let vfs = VfsTracker::default();
        assert_eq!(vfs.lookups, 0);
    }

    /// F-VFS-003: Lookup tracked
    #[test]
    fn f_vfs_003_lookup() {
        let mut vfs = VfsTracker::new();
        vfs.lookup();
        assert_eq!(vfs.lookups, 1);
    }

    /// F-VFS-004: Create tracked
    #[test]
    fn f_vfs_004_create() {
        let mut vfs = VfsTracker::new();
        vfs.create();
        assert_eq!(vfs.creates, 1);
    }

    /// F-VFS-005: Unlink tracked
    #[test]
    fn f_vfs_005_unlink() {
        let mut vfs = VfsTracker::new();
        vfs.unlink();
        assert_eq!(vfs.unlinks, 1);
    }

    /// F-VFS-006: Rename tracked
    #[test]
    fn f_vfs_006_rename() {
        let mut vfs = VfsTracker::new();
        vfs.rename();
        assert_eq!(vfs.renames, 1);
    }

    /// F-VFS-007: Open tracked
    #[test]
    fn f_vfs_007_open() {
        let mut vfs = VfsTracker::new();
        vfs.open();
        assert_eq!(vfs.opens, 1);
    }

    /// F-VFS-008: Close tracked
    #[test]
    fn f_vfs_008_close() {
        let mut vfs = VfsTracker::new();
        vfs.close();
        assert_eq!(vfs.closes, 1);
    }

    /// F-VFS-009: Factory for_ext4
    #[test]
    fn f_vfs_009_ext4() {
        let vfs = VfsTracker::for_ext4();
        assert_eq!(vfs.lookups, 0);
    }

    /// F-VFS-010: Factory for_xfs
    #[test]
    fn f_vfs_010_xfs() {
        let vfs = VfsTracker::for_xfs();
        assert_eq!(vfs.lookups, 0);
    }

    /// F-VFS-011: Reset clears counters
    #[test]
    fn f_vfs_011_reset() {
        let mut vfs = VfsTracker::new();
        vfs.lookup();
        vfs.reset();
        assert_eq!(vfs.lookups, 0);
    }

    /// F-VFS-012: Clone preserves state
    #[test]
    fn f_vfs_012_clone() {
        let mut vfs = VfsTracker::new();
        vfs.lookup();
        let cloned = vfs;
        assert_eq!(vfs.lookups, cloned.lookups);
    }
}

/// O(1) inode tracker.
///
/// Tracks inode allocation and deallocation metrics including
/// allocations, frees, and inode table utilization.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::InodeTracker;
///
/// let mut inode = InodeTracker::new();
/// inode.alloc();
/// inode.free();
/// assert_eq!(inode.allocs, 1);
/// assert_eq!(inode.frees, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InodeTracker {
    /// Allocations.
    pub allocs: u64,
    /// Frees.
    pub frees: u64,
    /// Current in use.
    pub in_use: u64,
    /// Peak in use.
    pub peak_in_use: u64,
    /// Total capacity.
    pub capacity: u64,
    /// Evictions from cache.
    pub evictions: u64,
}

impl InodeTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allocs: 0,
            frees: 0,
            in_use: 0,
            peak_in_use: 0,
            capacity: 0,
            evictions: 0,
        }
    }

    /// Factory for ext4 filesystem.
    #[must_use]
    pub const fn for_ext4() -> Self {
        Self::new()
    }

    /// Factory for btrfs filesystem.
    #[must_use]
    pub const fn for_btrfs() -> Self {
        Self::new()
    }

    /// Record allocation.
    pub fn alloc(&mut self) {
        self.allocs += 1;
        self.in_use += 1;
        if self.in_use > self.peak_in_use {
            self.peak_in_use = self.in_use;
        }
    }

    /// Record free.
    pub fn free(&mut self) {
        self.frees += 1;
        self.in_use = self.in_use.saturating_sub(1);
    }

    /// Record eviction.
    pub fn evict(&mut self) {
        self.evictions += 1;
    }

    /// Set capacity.
    pub fn set_capacity(&mut self, cap: u64) {
        self.capacity = cap;
    }

    /// Get utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        (self.in_use as f64) / (self.capacity as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocs = 0;
        self.frees = 0;
        self.evictions = 0;
        // Keep in_use, peak, capacity for state tracking
    }
}

#[cfg(test)]
mod inode_tests {
    use super::*;

    /// F-INODE-001: New tracker is empty
    #[test]
    fn f_inode_001_new() {
        let inode = InodeTracker::new();
        assert_eq!(inode.allocs, 0);
    }

    /// F-INODE-002: Default is empty
    #[test]
    fn f_inode_002_default() {
        let inode = InodeTracker::default();
        assert_eq!(inode.allocs, 0);
    }

    /// F-INODE-003: Alloc tracked
    #[test]
    fn f_inode_003_alloc() {
        let mut inode = InodeTracker::new();
        inode.alloc();
        assert_eq!(inode.allocs, 1);
        assert_eq!(inode.in_use, 1);
    }

    /// F-INODE-004: Free tracked
    #[test]
    fn f_inode_004_free() {
        let mut inode = InodeTracker::new();
        inode.alloc();
        inode.free();
        assert_eq!(inode.frees, 1);
        assert_eq!(inode.in_use, 0);
    }

    /// F-INODE-005: Eviction tracked
    #[test]
    fn f_inode_005_evict() {
        let mut inode = InodeTracker::new();
        inode.evict();
        assert_eq!(inode.evictions, 1);
    }

    /// F-INODE-006: Peak tracked
    #[test]
    fn f_inode_006_peak() {
        let mut inode = InodeTracker::new();
        inode.alloc();
        inode.alloc();
        inode.free();
        assert_eq!(inode.peak_in_use, 2);
    }

    /// F-INODE-007: Capacity set
    #[test]
    fn f_inode_007_capacity() {
        let mut inode = InodeTracker::new();
        inode.set_capacity(1000);
        assert_eq!(inode.capacity, 1000);
    }

    /// F-INODE-008: Utilization calculated
    #[test]
    fn f_inode_008_utilization() {
        let mut inode = InodeTracker::new();
        inode.set_capacity(100);
        inode.alloc();
        assert!((inode.utilization() - 1.0).abs() < 0.01);
    }

    /// F-INODE-009: Factory for_ext4
    #[test]
    fn f_inode_009_ext4() {
        let inode = InodeTracker::for_ext4();
        assert_eq!(inode.allocs, 0);
    }

    /// F-INODE-010: Factory for_btrfs
    #[test]
    fn f_inode_010_btrfs() {
        let inode = InodeTracker::for_btrfs();
        assert_eq!(inode.allocs, 0);
    }

    /// F-INODE-011: Reset clears counters
    #[test]
    fn f_inode_011_reset() {
        let mut inode = InodeTracker::new();
        inode.alloc();
        inode.reset();
        assert_eq!(inode.allocs, 0);
    }

    /// F-INODE-012: Clone preserves state
    #[test]
    fn f_inode_012_clone() {
        let mut inode = InodeTracker::new();
        inode.alloc();
        let cloned = inode;
        assert_eq!(inode.allocs, cloned.allocs);
    }
}

/// O(1) dentry (directory entry) cache tracker.
///
/// Tracks dentry cache metrics including lookups, hits, misses,
/// and negative dentries for filesystem path resolution.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::DentryTracker;
///
/// let mut dentry = DentryTracker::new();
/// dentry.lookup_hit();
/// dentry.lookup_miss();
/// assert_eq!(dentry.hit_rate(), 50.0);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DentryTracker {
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
    /// Negative dentries (cached non-existence).
    pub negative: u64,
    /// Entries in cache.
    pub cached: u64,
    /// Reclaims.
    pub reclaims: u64,
    /// Peak cache size.
    pub peak_cached: u64,
}

impl DentryTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            hits: 0,
            misses: 0,
            negative: 0,
            cached: 0,
            reclaims: 0,
            peak_cached: 0,
        }
    }

    /// Factory for dcache.
    #[must_use]
    pub const fn for_dcache() -> Self {
        Self::new()
    }

    /// Factory for path resolution.
    #[must_use]
    pub const fn for_pathwalk() -> Self {
        Self::new()
    }

    /// Record cache hit.
    pub fn lookup_hit(&mut self) {
        self.hits += 1;
    }

    /// Record cache miss.
    pub fn lookup_miss(&mut self) {
        self.misses += 1;
    }

    /// Record negative dentry.
    pub fn negative_entry(&mut self) {
        self.negative += 1;
    }

    /// Set cached count.
    pub fn set_cached(&mut self, count: u64) {
        self.cached = count;
        if count > self.peak_cached {
            self.peak_cached = count;
        }
    }

    /// Record reclaim.
    pub fn reclaim(&mut self) {
        self.reclaims += 1;
    }

    /// Get hit rate percentage.
    #[must_use]
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            return 0.0;
        }
        (self.hits as f64) / (total as f64) * 100.0
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.hits = 0;
        self.misses = 0;
        self.negative = 0;
        self.reclaims = 0;
    }
}

#[cfg(test)]
mod dentry_tests {
    use super::*;

    /// F-DENTRY-001: New tracker is empty
    #[test]
    fn f_dentry_001_new() {
        let dentry = DentryTracker::new();
        assert_eq!(dentry.hits, 0);
    }

    /// F-DENTRY-002: Default is empty
    #[test]
    fn f_dentry_002_default() {
        let dentry = DentryTracker::default();
        assert_eq!(dentry.hits, 0);
    }

    /// F-DENTRY-003: Hit tracked
    #[test]
    fn f_dentry_003_hit() {
        let mut dentry = DentryTracker::new();
        dentry.lookup_hit();
        assert_eq!(dentry.hits, 1);
    }

    /// F-DENTRY-004: Miss tracked
    #[test]
    fn f_dentry_004_miss() {
        let mut dentry = DentryTracker::new();
        dentry.lookup_miss();
        assert_eq!(dentry.misses, 1);
    }

    /// F-DENTRY-005: Negative tracked
    #[test]
    fn f_dentry_005_negative() {
        let mut dentry = DentryTracker::new();
        dentry.negative_entry();
        assert_eq!(dentry.negative, 1);
    }

    /// F-DENTRY-006: Cached tracked
    #[test]
    fn f_dentry_006_cached() {
        let mut dentry = DentryTracker::new();
        dentry.set_cached(1000);
        assert_eq!(dentry.cached, 1000);
    }

    /// F-DENTRY-007: Hit rate
    #[test]
    fn f_dentry_007_hit_rate() {
        let mut dentry = DentryTracker::new();
        dentry.lookup_hit();
        dentry.lookup_miss();
        assert!((dentry.hit_rate() - 50.0).abs() < 0.01);
    }

    /// F-DENTRY-008: Reclaim tracked
    #[test]
    fn f_dentry_008_reclaim() {
        let mut dentry = DentryTracker::new();
        dentry.reclaim();
        assert_eq!(dentry.reclaims, 1);
    }

    /// F-DENTRY-009: Factory for_dcache
    #[test]
    fn f_dentry_009_dcache() {
        let dentry = DentryTracker::for_dcache();
        assert_eq!(dentry.hits, 0);
    }

    /// F-DENTRY-010: Factory for_pathwalk
    #[test]
    fn f_dentry_010_pathwalk() {
        let dentry = DentryTracker::for_pathwalk();
        assert_eq!(dentry.hits, 0);
    }

    /// F-DENTRY-011: Reset clears counters
    #[test]
    fn f_dentry_011_reset() {
        let mut dentry = DentryTracker::new();
        dentry.lookup_hit();
        dentry.reset();
        assert_eq!(dentry.hits, 0);
    }

    /// F-DENTRY-012: Clone preserves state
    #[test]
    fn f_dentry_012_clone() {
        let mut dentry = DentryTracker::new();
        dentry.lookup_hit();
        let cloned = dentry;
        assert_eq!(dentry.hits, cloned.hits);
    }
}

/// O(1) extent tracker.
///
/// Tracks filesystem extent allocations for extent-based filesystems
/// (ext4, xfs, btrfs) including allocations, merges, splits, and
/// fragmentation metrics.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::ExtentTracker;
///
/// let mut extent = ExtentTracker::new();
/// extent.alloc(16);  // 16 blocks
/// extent.merge();
/// assert_eq!(extent.allocs, 1);
/// assert_eq!(extent.merges, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ExtentTracker {
    /// Extent allocations.
    pub allocs: u64,
    /// Extent merges.
    pub merges: u64,
    /// Extent splits.
    pub splits: u64,
    /// Total blocks allocated.
    pub blocks: u64,
    /// Average extent size (blocks).
    pub avg_size: u32,
    /// Max extent size seen.
    pub max_size: u32,
}

impl ExtentTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allocs: 0,
            merges: 0,
            splits: 0,
            blocks: 0,
            avg_size: 0,
            max_size: 0,
        }
    }

    /// Factory for ext4 filesystem.
    #[must_use]
    pub const fn for_ext4() -> Self {
        Self::new()
    }

    /// Factory for xfs filesystem.
    #[must_use]
    pub const fn for_xfs() -> Self {
        Self::new()
    }

    /// Record extent allocation.
    pub fn alloc(&mut self, blocks: u32) {
        self.allocs += 1;
        self.blocks += blocks as u64;
        if blocks > self.max_size {
            self.max_size = blocks;
        }
        // Update rolling average
        self.avg_size = (self.blocks / self.allocs) as u32;
    }

    /// Record extent merge.
    pub fn merge(&mut self) {
        self.merges += 1;
    }

    /// Record extent split.
    pub fn split(&mut self) {
        self.splits += 1;
    }

    /// Get fragmentation ratio (splits / allocs).
    #[must_use]
    pub fn fragmentation(&self) -> f64 {
        if self.allocs == 0 {
            return 0.0;
        }
        (self.splits as f64) / (self.allocs as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocs = 0;
        self.merges = 0;
        self.splits = 0;
        self.blocks = 0;
        self.avg_size = 0;
        // Keep max_size for high-water mark
    }
}

#[cfg(test)]
mod extent_tests {
    use super::*;

    /// F-EXTENT-001: New tracker is empty
    #[test]
    fn f_extent_001_new() {
        let extent = ExtentTracker::new();
        assert_eq!(extent.allocs, 0);
    }

    /// F-EXTENT-002: Default is empty
    #[test]
    fn f_extent_002_default() {
        let extent = ExtentTracker::default();
        assert_eq!(extent.allocs, 0);
    }

    /// F-EXTENT-003: Alloc tracked
    #[test]
    fn f_extent_003_alloc() {
        let mut extent = ExtentTracker::new();
        extent.alloc(16);
        assert_eq!(extent.allocs, 1);
        assert_eq!(extent.blocks, 16);
    }

    /// F-EXTENT-004: Merge tracked
    #[test]
    fn f_extent_004_merge() {
        let mut extent = ExtentTracker::new();
        extent.merge();
        assert_eq!(extent.merges, 1);
    }

    /// F-EXTENT-005: Split tracked
    #[test]
    fn f_extent_005_split() {
        let mut extent = ExtentTracker::new();
        extent.split();
        assert_eq!(extent.splits, 1);
    }

    /// F-EXTENT-006: Max size tracked
    #[test]
    fn f_extent_006_max_size() {
        let mut extent = ExtentTracker::new();
        extent.alloc(8);
        extent.alloc(32);
        extent.alloc(16);
        assert_eq!(extent.max_size, 32);
    }

    /// F-EXTENT-007: Average size
    #[test]
    fn f_extent_007_avg_size() {
        let mut extent = ExtentTracker::new();
        extent.alloc(10);
        extent.alloc(20);
        assert_eq!(extent.avg_size, 15);
    }

    /// F-EXTENT-008: Fragmentation
    #[test]
    fn f_extent_008_fragmentation() {
        let mut extent = ExtentTracker::new();
        extent.alloc(16);
        extent.alloc(16);
        extent.split();
        assert!((extent.fragmentation() - 0.5).abs() < 0.01);
    }

    /// F-EXTENT-009: Factory for_ext4
    #[test]
    fn f_extent_009_ext4() {
        let extent = ExtentTracker::for_ext4();
        assert_eq!(extent.allocs, 0);
    }

    /// F-EXTENT-010: Factory for_xfs
    #[test]
    fn f_extent_010_xfs() {
        let extent = ExtentTracker::for_xfs();
        assert_eq!(extent.allocs, 0);
    }

    /// F-EXTENT-011: Reset clears counters
    #[test]
    fn f_extent_011_reset() {
        let mut extent = ExtentTracker::new();
        extent.alloc(16);
        extent.reset();
        assert_eq!(extent.allocs, 0);
    }

    /// F-EXTENT-012: Clone preserves state
    #[test]
    fn f_extent_012_clone() {
        let mut extent = ExtentTracker::new();
        extent.alloc(16);
        let cloned = extent;
        assert_eq!(extent.allocs, cloned.allocs);
    }
}

// ============================================================================
// v9.42.0: Network Subsystem Helpers
// ============================================================================

/// O(1) TCP connection tracker.
///
/// Tracks TCP connection metrics including established connections,
/// retransmissions, resets, and connection state transitions.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::TcpTracker;
///
/// let mut tcp = TcpTracker::new();
/// tcp.connect();
/// tcp.established();
/// assert_eq!(tcp.connections, 1);
/// assert_eq!(tcp.established, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TcpTracker {
    /// Connection attempts.
    pub connections: u64,
    /// Established connections.
    pub established: u64,
    /// Retransmissions.
    pub retransmits: u64,
    /// Resets sent.
    pub resets: u64,
    /// Timeouts.
    pub timeouts: u64,
    /// Bytes transmitted.
    pub bytes_tx: u64,
}

impl TcpTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            connections: 0,
            established: 0,
            retransmits: 0,
            resets: 0,
            timeouts: 0,
            bytes_tx: 0,
        }
    }

    /// Factory for IPv4.
    #[must_use]
    pub const fn for_ipv4() -> Self {
        Self::new()
    }

    /// Factory for IPv6.
    #[must_use]
    pub const fn for_ipv6() -> Self {
        Self::new()
    }

    /// Record connection attempt.
    pub fn connect(&mut self) {
        self.connections += 1;
    }

    /// Record established connection.
    pub fn established(&mut self) {
        self.established += 1;
    }

    /// Record retransmission.
    pub fn retransmit(&mut self) {
        self.retransmits += 1;
    }

    /// Record reset.
    pub fn reset_conn(&mut self) {
        self.resets += 1;
    }

    /// Record timeout.
    pub fn timeout(&mut self) {
        self.timeouts += 1;
    }

    /// Record bytes transmitted.
    pub fn transmit(&mut self, bytes: u64) {
        self.bytes_tx += bytes;
    }

    /// Get retransmission rate.
    #[must_use]
    pub fn retransmit_rate(&self) -> f64 {
        if self.connections == 0 {
            return 0.0;
        }
        (self.retransmits as f64) / (self.connections as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.connections = 0;
        self.established = 0;
        self.retransmits = 0;
        self.resets = 0;
        self.timeouts = 0;
        self.bytes_tx = 0;
    }
}

#[cfg(test)]
mod tcp_tests {
    use super::*;

    /// F-TCP-001: New tracker is empty
    #[test]
    fn f_tcp_001_new() {
        let tcp = TcpTracker::new();
        assert_eq!(tcp.connections, 0);
    }

    /// F-TCP-002: Default is empty
    #[test]
    fn f_tcp_002_default() {
        let tcp = TcpTracker::default();
        assert_eq!(tcp.connections, 0);
    }

    /// F-TCP-003: Connect tracked
    #[test]
    fn f_tcp_003_connect() {
        let mut tcp = TcpTracker::new();
        tcp.connect();
        assert_eq!(tcp.connections, 1);
    }

    /// F-TCP-004: Established tracked
    #[test]
    fn f_tcp_004_established() {
        let mut tcp = TcpTracker::new();
        tcp.established();
        assert_eq!(tcp.established, 1);
    }

    /// F-TCP-005: Retransmit tracked
    #[test]
    fn f_tcp_005_retransmit() {
        let mut tcp = TcpTracker::new();
        tcp.retransmit();
        assert_eq!(tcp.retransmits, 1);
    }

    /// F-TCP-006: Reset tracked
    #[test]
    fn f_tcp_006_reset() {
        let mut tcp = TcpTracker::new();
        tcp.reset_conn();
        assert_eq!(tcp.resets, 1);
    }

    /// F-TCP-007: Timeout tracked
    #[test]
    fn f_tcp_007_timeout() {
        let mut tcp = TcpTracker::new();
        tcp.timeout();
        assert_eq!(tcp.timeouts, 1);
    }

    /// F-TCP-008: Bytes tracked
    #[test]
    fn f_tcp_008_bytes() {
        let mut tcp = TcpTracker::new();
        tcp.transmit(1000);
        assert_eq!(tcp.bytes_tx, 1000);
    }

    /// F-TCP-009: Factory for_ipv4
    #[test]
    fn f_tcp_009_ipv4() {
        let tcp = TcpTracker::for_ipv4();
        assert_eq!(tcp.connections, 0);
    }

    /// F-TCP-010: Factory for_ipv6
    #[test]
    fn f_tcp_010_ipv6() {
        let tcp = TcpTracker::for_ipv6();
        assert_eq!(tcp.connections, 0);
    }

    /// F-TCP-011: Reset clears counters
    #[test]
    fn f_tcp_011_reset() {
        let mut tcp = TcpTracker::new();
        tcp.connect();
        tcp.reset();
        assert_eq!(tcp.connections, 0);
    }

    /// F-TCP-012: Clone preserves state
    #[test]
    fn f_tcp_012_clone() {
        let mut tcp = TcpTracker::new();
        tcp.connect();
        let cloned = tcp;
        assert_eq!(tcp.connections, cloned.connections);
    }
}

/// O(1) UDP packet tracker.
///
/// Tracks UDP packet metrics including sends, receives, drops,
/// and buffer errors for UDP socket operations.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::UdpTracker;
///
/// let mut udp = UdpTracker::new();
/// udp.send(1000);
/// udp.recv(500);
/// assert_eq!(udp.packets_tx, 1);
/// assert_eq!(udp.packets_rx, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UdpTracker {
    /// Packets sent.
    pub packets_tx: u64,
    /// Packets received.
    pub packets_rx: u64,
    /// Bytes sent.
    pub bytes_tx: u64,
    /// Bytes received.
    pub bytes_rx: u64,
    /// Dropped packets.
    pub drops: u64,
    /// Buffer errors.
    pub buf_errors: u64,
}

impl UdpTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packets_tx: 0,
            packets_rx: 0,
            bytes_tx: 0,
            bytes_rx: 0,
            drops: 0,
            buf_errors: 0,
        }
    }

    /// Factory for IPv4.
    #[must_use]
    pub const fn for_ipv4() -> Self {
        Self::new()
    }

    /// Factory for IPv6.
    #[must_use]
    pub const fn for_ipv6() -> Self {
        Self::new()
    }

    /// Record send.
    pub fn send(&mut self, bytes: u64) {
        self.packets_tx += 1;
        self.bytes_tx += bytes;
    }

    /// Record receive.
    pub fn recv(&mut self, bytes: u64) {
        self.packets_rx += 1;
        self.bytes_rx += bytes;
    }

    /// Record drop.
    pub fn drop_pkt(&mut self) {
        self.drops += 1;
    }

    /// Record buffer error.
    pub fn buf_error(&mut self) {
        self.buf_errors += 1;
    }

    /// Get drop rate.
    #[must_use]
    pub fn drop_rate(&self) -> f64 {
        let total = self.packets_tx + self.packets_rx;
        if total == 0 {
            return 0.0;
        }
        (self.drops as f64) / (total as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.packets_tx = 0;
        self.packets_rx = 0;
        self.bytes_tx = 0;
        self.bytes_rx = 0;
        self.drops = 0;
        self.buf_errors = 0;
    }
}

#[cfg(test)]
mod udp_tests {
    use super::*;

    /// F-UDP-001: New tracker is empty
    #[test]
    fn f_udp_001_new() {
        let udp = UdpTracker::new();
        assert_eq!(udp.packets_tx, 0);
    }

    /// F-UDP-002: Default is empty
    #[test]
    fn f_udp_002_default() {
        let udp = UdpTracker::default();
        assert_eq!(udp.packets_tx, 0);
    }

    /// F-UDP-003: Send tracked
    #[test]
    fn f_udp_003_send() {
        let mut udp = UdpTracker::new();
        udp.send(1000);
        assert_eq!(udp.packets_tx, 1);
        assert_eq!(udp.bytes_tx, 1000);
    }

    /// F-UDP-004: Recv tracked
    #[test]
    fn f_udp_004_recv() {
        let mut udp = UdpTracker::new();
        udp.recv(500);
        assert_eq!(udp.packets_rx, 1);
        assert_eq!(udp.bytes_rx, 500);
    }

    /// F-UDP-005: Drop tracked
    #[test]
    fn f_udp_005_drop() {
        let mut udp = UdpTracker::new();
        udp.drop_pkt();
        assert_eq!(udp.drops, 1);
    }

    /// F-UDP-006: Buffer error tracked
    #[test]
    fn f_udp_006_buf_error() {
        let mut udp = UdpTracker::new();
        udp.buf_error();
        assert_eq!(udp.buf_errors, 1);
    }

    /// F-UDP-007: Drop rate
    #[test]
    fn f_udp_007_drop_rate() {
        let mut udp = UdpTracker::new();
        udp.send(100);
        udp.recv(100);
        udp.drop_pkt();
        assert!((udp.drop_rate() - 0.5).abs() < 0.01);
    }

    /// F-UDP-008: Total bytes
    #[test]
    fn f_udp_008_total_bytes() {
        let mut udp = UdpTracker::new();
        udp.send(1000);
        udp.recv(500);
        assert_eq!(udp.bytes_tx + udp.bytes_rx, 1500);
    }

    /// F-UDP-009: Factory for_ipv4
    #[test]
    fn f_udp_009_ipv4() {
        let udp = UdpTracker::for_ipv4();
        assert_eq!(udp.packets_tx, 0);
    }

    /// F-UDP-010: Factory for_ipv6
    #[test]
    fn f_udp_010_ipv6() {
        let udp = UdpTracker::for_ipv6();
        assert_eq!(udp.packets_tx, 0);
    }

    /// F-UDP-011: Reset clears counters
    #[test]
    fn f_udp_011_reset() {
        let mut udp = UdpTracker::new();
        udp.send(1000);
        udp.reset();
        assert_eq!(udp.packets_tx, 0);
    }

    /// F-UDP-012: Clone preserves state
    #[test]
    fn f_udp_012_clone() {
        let mut udp = UdpTracker::new();
        udp.send(1000);
        let cloned = udp;
        assert_eq!(udp.packets_tx, cloned.packets_tx);
    }
}

/// O(1) socket buffer (skb) tracker.
///
/// Tracks Linux socket buffer allocations, frees, and clone operations
/// for network packet processing.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::SkbTracker;
///
/// let mut skb = SkbTracker::new();
/// skb.alloc(1500);
/// skb.free();
/// assert_eq!(skb.allocs, 1);
/// assert_eq!(skb.frees, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkbTracker {
    /// Allocations.
    pub allocs: u64,
    /// Frees.
    pub frees: u64,
    /// Clones.
    pub clones: u64,
    /// Bytes allocated.
    pub bytes_alloc: u64,
    /// Current in flight.
    pub in_flight: u64,
    /// Peak in flight.
    pub peak_in_flight: u64,
}

impl SkbTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allocs: 0,
            frees: 0,
            clones: 0,
            bytes_alloc: 0,
            in_flight: 0,
            peak_in_flight: 0,
        }
    }

    /// Factory for RX path.
    #[must_use]
    pub const fn for_rx() -> Self {
        Self::new()
    }

    /// Factory for TX path.
    #[must_use]
    pub const fn for_tx() -> Self {
        Self::new()
    }

    /// Record allocation.
    pub fn alloc(&mut self, bytes: u64) {
        self.allocs += 1;
        self.bytes_alloc += bytes;
        self.in_flight += 1;
        if self.in_flight > self.peak_in_flight {
            self.peak_in_flight = self.in_flight;
        }
    }

    /// Record free.
    pub fn free(&mut self) {
        self.frees += 1;
        self.in_flight = self.in_flight.saturating_sub(1);
    }

    /// Record clone.
    pub fn clone_skb(&mut self) {
        self.clones += 1;
        self.in_flight += 1;
        if self.in_flight > self.peak_in_flight {
            self.peak_in_flight = self.in_flight;
        }
    }

    /// Get average size.
    #[must_use]
    pub fn avg_size(&self) -> u64 {
        if self.allocs == 0 {
            return 0;
        }
        self.bytes_alloc / self.allocs
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.allocs = 0;
        self.frees = 0;
        self.clones = 0;
        self.bytes_alloc = 0;
        // Keep in_flight and peak for state tracking
    }
}

#[cfg(test)]
mod skb_tests {
    use super::*;

    /// F-SKB-001: New tracker is empty
    #[test]
    fn f_skb_001_new() {
        let skb = SkbTracker::new();
        assert_eq!(skb.allocs, 0);
    }

    /// F-SKB-002: Default is empty
    #[test]
    fn f_skb_002_default() {
        let skb = SkbTracker::default();
        assert_eq!(skb.allocs, 0);
    }

    /// F-SKB-003: Alloc tracked
    #[test]
    fn f_skb_003_alloc() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        assert_eq!(skb.allocs, 1);
        assert_eq!(skb.bytes_alloc, 1500);
    }

    /// F-SKB-004: Free tracked
    #[test]
    fn f_skb_004_free() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        skb.free();
        assert_eq!(skb.frees, 1);
        assert_eq!(skb.in_flight, 0);
    }

    /// F-SKB-005: Clone tracked
    #[test]
    fn f_skb_005_clone() {
        let mut skb = SkbTracker::new();
        skb.clone_skb();
        assert_eq!(skb.clones, 1);
    }

    /// F-SKB-006: In flight tracked
    #[test]
    fn f_skb_006_in_flight() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        skb.alloc(1500);
        assert_eq!(skb.in_flight, 2);
    }

    /// F-SKB-007: Peak tracked
    #[test]
    fn f_skb_007_peak() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        skb.alloc(1500);
        skb.free();
        assert_eq!(skb.peak_in_flight, 2);
    }

    /// F-SKB-008: Avg size
    #[test]
    fn f_skb_008_avg_size() {
        let mut skb = SkbTracker::new();
        skb.alloc(1000);
        skb.alloc(2000);
        assert_eq!(skb.avg_size(), 1500);
    }

    /// F-SKB-009: Factory for_rx
    #[test]
    fn f_skb_009_rx() {
        let skb = SkbTracker::for_rx();
        assert_eq!(skb.allocs, 0);
    }

    /// F-SKB-010: Factory for_tx
    #[test]
    fn f_skb_010_tx() {
        let skb = SkbTracker::for_tx();
        assert_eq!(skb.allocs, 0);
    }

    /// F-SKB-011: Reset clears counters
    #[test]
    fn f_skb_011_reset() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        skb.reset();
        assert_eq!(skb.allocs, 0);
    }

    /// F-SKB-012: Clone preserves state
    #[test]
    fn f_skb_012_clone() {
        let mut skb = SkbTracker::new();
        skb.alloc(1500);
        let cloned = skb;
        assert_eq!(skb.allocs, cloned.allocs);
    }
}

/// O(1) network device tracker.
///
/// Tracks network device statistics including packets, bytes, errors,
/// and drops for both RX and TX paths.
///
/// # Performance
/// - All operations are O(1) with no allocations
/// - Clone is O(1) - just copies stack data
/// - Reset is O(1) - just zeroes fields
///
/// # Example
/// ```
/// use presentar_terminal::perf_trace::NetDevTracker;
///
/// let mut netdev = NetDevTracker::new();
/// netdev.rx(1500);
/// netdev.tx(1000);
/// assert_eq!(netdev.rx_packets, 1);
/// assert_eq!(netdev.tx_packets, 1);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetDevTracker {
    /// RX packets.
    pub rx_packets: u64,
    /// TX packets.
    pub tx_packets: u64,
    /// RX bytes.
    pub rx_bytes: u64,
    /// TX bytes.
    pub tx_bytes: u64,
    /// RX errors.
    pub rx_errors: u64,
    /// TX errors.
    pub tx_errors: u64,
}

impl NetDevTracker {
    /// Create new empty tracker.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rx_packets: 0,
            tx_packets: 0,
            rx_bytes: 0,
            tx_bytes: 0,
            rx_errors: 0,
            tx_errors: 0,
        }
    }

    /// Factory for ethernet device.
    #[must_use]
    pub const fn for_eth() -> Self {
        Self::new()
    }

    /// Factory for loopback device.
    #[must_use]
    pub const fn for_lo() -> Self {
        Self::new()
    }

    /// Record RX packet.
    pub fn rx(&mut self, bytes: u64) {
        self.rx_packets += 1;
        self.rx_bytes += bytes;
    }

    /// Record TX packet.
    pub fn tx(&mut self, bytes: u64) {
        self.tx_packets += 1;
        self.tx_bytes += bytes;
    }

    /// Record RX error.
    pub fn rx_error(&mut self) {
        self.rx_errors += 1;
    }

    /// Record TX error.
    pub fn tx_error(&mut self) {
        self.tx_errors += 1;
    }

    /// Get total packets.
    #[must_use]
    pub fn total_packets(&self) -> u64 {
        self.rx_packets + self.tx_packets
    }

    /// Get total bytes.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.rx_bytes + self.tx_bytes
    }

    /// Get error rate.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        let total = self.total_packets();
        if total == 0 {
            return 0.0;
        }
        let errors = self.rx_errors + self.tx_errors;
        (errors as f64) / (total as f64)
    }

    /// Reset counters.
    pub fn reset(&mut self) {
        self.rx_packets = 0;
        self.tx_packets = 0;
        self.rx_bytes = 0;
        self.tx_bytes = 0;
        self.rx_errors = 0;
        self.tx_errors = 0;
    }
}

#[cfg(test)]
mod netdev_tests {
    use super::*;

    /// F-NETDEV-001: New tracker is empty
    #[test]
    fn f_netdev_001_new() {
        let netdev = NetDevTracker::new();
        assert_eq!(netdev.rx_packets, 0);
    }

    /// F-NETDEV-002: Default is empty
    #[test]
    fn f_netdev_002_default() {
        let netdev = NetDevTracker::default();
        assert_eq!(netdev.rx_packets, 0);
    }

    /// F-NETDEV-003: RX tracked
    #[test]
