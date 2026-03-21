    }

    /// F-SNAP-002: Default equals new
    #[test]
    fn f_snap_002_default() {
        let st = SnapshotTracker::default();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-003: Snapshot increments count
    #[test]
    fn f_snap_003_snapshot() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        assert_eq!(st.snapshot_count(), 1);
    }

    /// F-SNAP-004: Total bytes tracked
    #[test]
    fn f_snap_004_total_bytes() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.snapshot(2048, 2000);
        assert_eq!(st.total_bytes(), 3072);
    }

    /// F-SNAP-005: Max size tracked
    #[test]
    fn f_snap_005_max_size() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.snapshot(4096, 2000);
        st.snapshot(2048, 3000);
        assert_eq!(st.max_size_bytes(), 4096);
    }

    /// F-SNAP-006: Average size calculated
    #[test]
    fn f_snap_006_avg_size() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1000, 1000);
        st.snapshot(2000, 2000);
        assert!((st.avg_size_bytes() - 1500.0).abs() < 0.01);
    }

    /// F-SNAP-007: Factory for_database
    #[test]
    fn f_snap_007_for_database() {
        let st = SnapshotTracker::for_database();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-008: Factory for_state
    #[test]
    fn f_snap_008_for_state() {
        let st = SnapshotTracker::for_state();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-009: Avg interval tracked
    #[test]
    fn f_snap_009_avg_interval() {
        let mut st = SnapshotTracker::new();
        st.snapshot(100, 1000);
        st.snapshot(100, 2000); // 1000us interval
        assert!(st.avg_interval_us() > 0.0);
    }

    /// F-SNAP-010: Last snapshot timestamp
    #[test]
    fn f_snap_010_last_snapshot() {
        let mut st = SnapshotTracker::new();
        st.snapshot(100, 5000);
        assert_eq!(st.last_snapshot_us(), 5000);
    }

    /// F-SNAP-011: Reset clears state
    #[test]
    fn f_snap_011_reset() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        st.reset();
        assert_eq!(st.snapshot_count(), 0);
    }

    /// F-SNAP-012: Clone preserves state
    #[test]
    fn f_snap_012_clone() {
        let mut st = SnapshotTracker::new();
        st.snapshot(1024, 1000);
        let cloned = st.clone();
        assert_eq!(st.snapshot_count(), cloned.snapshot_count());
    }
}

// ============================================================================
// VersionTracker - O(1) version/generation tracking
// ============================================================================

/// O(1) version/generation tracking for optimistic concurrency.
///
/// Tracks version numbers, conflicts, and updates for optimistic locking.
#[derive(Debug, Clone)]
pub struct VersionTracker {
    current_version: u64,
    updates: u64,
    conflicts: u64,
    last_update_us: u64,
}

impl Default for VersionTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionTracker {
    /// Create a new version tracker starting at version 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            current_version: 0,
            updates: 0,
            conflicts: 0,
            last_update_us: 0,
        }
    }

    /// Factory for database records.
    #[must_use]
    pub fn for_record() -> Self {
        Self::new()
    }

    /// Factory for cache entries.
    #[must_use]
    pub fn for_cache() -> Self {
        Self::new()
    }

    /// Attempt to update with expected version (CAS-like).
    /// Returns true if update succeeds (version matches), false on conflict.
    pub fn try_update(&mut self, expected_version: u64, now_us: u64) -> bool {
        if self.current_version == expected_version {
            self.current_version += 1;
            self.updates += 1;
            self.last_update_us = now_us;
            true
        } else {
            self.conflicts += 1;
            false
        }
    }

    /// Force update regardless of version (for recovery).
    pub fn force_update(&mut self, now_us: u64) {
        self.current_version += 1;
        self.updates += 1;
        self.last_update_us = now_us;
    }

    /// Get current version.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.current_version
    }

    /// Get total successful updates.
    #[must_use]
    pub fn updates(&self) -> u64 {
        self.updates
    }

    /// Get conflict count.
    #[must_use]
    pub fn conflicts(&self) -> u64 {
        self.conflicts
    }

    /// Get conflict rate (conflicts / total attempts).
    #[must_use]
    pub fn conflict_rate(&self) -> f64 {
        let total = self.updates + self.conflicts;
        if total == 0 {
            0.0
        } else {
            self.conflicts as f64 / total as f64
        }
    }

    /// Check if conflict rate is acceptable.
    #[must_use]
    pub fn is_healthy(&self, max_conflict_rate: f64) -> bool {
        self.conflict_rate() <= max_conflict_rate
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.current_version = 0;
        self.updates = 0;
        self.conflicts = 0;
        self.last_update_us = 0;
    }
}

#[cfg(test)]
mod version_tracker_tests {
    use super::*;

    /// F-VER-001: New tracker starts at version 0
    #[test]
    fn f_ver_001_new() {
        let vt = VersionTracker::new();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-002: Default equals new
    #[test]
    fn f_ver_002_default() {
        let vt = VersionTracker::default();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-003: Try update success
    #[test]
    fn f_ver_003_try_update_success() {
        let mut vt = VersionTracker::new();
        assert!(vt.try_update(0, 1000));
        assert_eq!(vt.version(), 1);
    }

    /// F-VER-004: Try update conflict
    #[test]
    fn f_ver_004_try_update_conflict() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000); // v0 -> v1
        assert!(!vt.try_update(0, 2000)); // conflict: expected 0, got 1
    }

    /// F-VER-005: Force update increments
    #[test]
    fn f_ver_005_force_update() {
        let mut vt = VersionTracker::new();
        vt.force_update(1000);
        assert_eq!(vt.version(), 1);
    }

    /// F-VER-006: Conflict count tracked
    #[test]
    fn f_ver_006_conflicts() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        vt.try_update(0, 2000); // conflict
        assert_eq!(vt.conflicts(), 1);
    }

    /// F-VER-007: Factory for_record
    #[test]
    fn f_ver_007_for_record() {
        let vt = VersionTracker::for_record();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-008: Factory for_cache
    #[test]
    fn f_ver_008_for_cache() {
        let vt = VersionTracker::for_cache();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-009: Conflict rate calculated
    #[test]
    fn f_ver_009_conflict_rate() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000); // success
        vt.try_update(0, 2000); // conflict
        assert!((vt.conflict_rate() - 0.5).abs() < 0.01);
    }

    /// F-VER-010: Healthy when low conflicts
    #[test]
    fn f_ver_010_healthy() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        assert!(vt.is_healthy(0.1));
    }

    /// F-VER-011: Reset clears state
    #[test]
    fn f_ver_011_reset() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        vt.reset();
        assert_eq!(vt.version(), 0);
    }

    /// F-VER-012: Clone preserves state
    #[test]
    fn f_ver_012_clone() {
        let mut vt = VersionTracker::new();
        vt.try_update(0, 1000);
        let cloned = vt.clone();
        assert_eq!(vt.version(), cloned.version());
    }
}

// ============================================================================
// TokenBucketShaper - O(1) traffic shaping
// ============================================================================

/// O(1) traffic shaping with guaranteed bandwidth.
///
/// Implements token bucket with configurable burst and sustained rates.
#[derive(Debug, Clone)]
pub struct TokenBucketShaper {
    bucket_size: u64,
    tokens: u64,
    fill_rate_per_us: f64,
    last_fill_us: u64,
    bytes_shaped: u64,
    drops: u64,
}

impl Default for TokenBucketShaper {
    fn default() -> Self {
        Self::for_network()
    }
}

impl TokenBucketShaper {
    /// Create a new shaper with bucket size and fill rate (bytes/second).
    #[must_use]
    pub fn new(bucket_size: u64, fill_rate_per_sec: u64) -> Self {
        Self {
            bucket_size,
            tokens: bucket_size, // Start full
            fill_rate_per_us: fill_rate_per_sec as f64 / 1_000_000.0,
            last_fill_us: 0,
            bytes_shaped: 0,
            drops: 0,
        }
    }

    /// Factory for network traffic (1MB bucket, 100KB/s).
    #[must_use]
    pub fn for_network() -> Self {
        Self::new(1_000_000, 100_000)
    }

    /// Factory for API rate limiting (10KB bucket, 1KB/s).
    #[must_use]
    pub fn for_api() -> Self {
        Self::new(10_000, 1_000)
    }

    /// Refill tokens based on elapsed time.
    fn refill(&mut self, now_us: u64) {
        if self.last_fill_us > 0 && now_us > self.last_fill_us {
            let elapsed = now_us - self.last_fill_us;
            let new_tokens = (elapsed as f64 * self.fill_rate_per_us) as u64;
            self.tokens = (self.tokens + new_tokens).min(self.bucket_size);
        }
        self.last_fill_us = now_us;
    }

    /// Try to consume tokens (returns true if allowed).
    pub fn try_consume(&mut self, bytes: u64, now_us: u64) -> bool {
        self.refill(now_us);
        if self.tokens >= bytes {
            self.tokens -= bytes;
            self.bytes_shaped += bytes;
            true
        } else {
            self.drops += 1;
            false
        }
    }

    /// Get current token count.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    /// Get total bytes shaped.
    #[must_use]
    pub fn bytes_shaped(&self) -> u64 {
        self.bytes_shaped
    }

    /// Get drop count.
    #[must_use]
    pub fn drops(&self) -> u64 {
        self.drops
    }

    /// Get fill percentage.
    #[must_use]
    pub fn fill_percentage(&self) -> f64 {
        if self.bucket_size == 0 {
            0.0
        } else {
            (self.tokens as f64 / self.bucket_size as f64) * 100.0
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.tokens = self.bucket_size;
        self.last_fill_us = 0;
        self.bytes_shaped = 0;
        self.drops = 0;
    }
}

#[cfg(test)]
mod token_bucket_shaper_tests {
    use super::*;

    /// F-SHAPE-001: New shaper starts full
    #[test]
    fn f_shape_001_new() {
        let ts = TokenBucketShaper::new(1000, 100);
        assert_eq!(ts.tokens(), 1000);
    }

    /// F-SHAPE-002: Default uses network settings
    #[test]
    fn f_shape_002_default() {
        let ts = TokenBucketShaper::default();
        assert_eq!(ts.tokens(), 1_000_000);
    }

    /// F-SHAPE-003: Consume reduces tokens
    #[test]
    fn f_shape_003_consume() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        assert_eq!(ts.tokens(), 900);
    }

    /// F-SHAPE-004: Consume fails when insufficient
    #[test]
    fn f_shape_004_consume_fail() {
        let mut ts = TokenBucketShaper::new(100, 10);
        assert!(!ts.try_consume(200, 1000));
    }

    /// F-SHAPE-005: Drops counted
    #[test]
    fn f_shape_005_drops() {
        let mut ts = TokenBucketShaper::new(100, 10);
        ts.try_consume(200, 1000);
        assert_eq!(ts.drops(), 1);
    }

    /// F-SHAPE-006: Bytes shaped tracked
    #[test]
    fn f_shape_006_bytes_shaped() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        ts.try_consume(200, 2000);
        assert_eq!(ts.bytes_shaped(), 300);
    }

    /// F-SHAPE-007: Factory for_network
    #[test]
    fn f_shape_007_for_network() {
        let ts = TokenBucketShaper::for_network();
        assert_eq!(ts.tokens(), 1_000_000);
    }

    /// F-SHAPE-008: Factory for_api
    #[test]
    fn f_shape_008_for_api() {
        let ts = TokenBucketShaper::for_api();
        assert_eq!(ts.tokens(), 10_000);
    }

    /// F-SHAPE-009: Fill percentage calculated
    #[test]
    fn f_shape_009_fill_percentage() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(500, 1000);
        assert!((ts.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SHAPE-010: Refill adds tokens over time
    #[test]
    fn f_shape_010_refill() {
        let mut ts = TokenBucketShaper::new(1000, 1_000_000); // 1 byte/us
        ts.try_consume(500, 0);
        ts.try_consume(0, 250); // 250us later, refill 250 tokens
        assert!(ts.tokens() >= 500); // Should have refilled some
    }

    /// F-SHAPE-011: Reset restores full bucket
    #[test]
    fn f_shape_011_reset() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(500, 1000);
        ts.reset();
        assert_eq!(ts.tokens(), 1000);
    }

    /// F-SHAPE-012: Clone preserves state
    #[test]
    fn f_shape_012_clone() {
        let mut ts = TokenBucketShaper::new(1000, 100);
        ts.try_consume(100, 1000);
        let cloned = ts.clone();
        assert_eq!(ts.tokens(), cloned.tokens());
    }
}

// ============================================================================
// LeaderElection - O(1) leader election state tracking
// ============================================================================

/// O(1) leader election state tracking.
///
/// Simple leader/follower state machine with term tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElectionState {
    Follower,
    Candidate,
    Leader,
}

/// O(1) leader election tracker.
#[derive(Debug, Clone)]
pub struct LeaderElection {
    state: ElectionState,
    term: u64,
    elections: u64,
    terms_as_leader: u64,
    last_heartbeat_us: u64,
}

impl Default for LeaderElection {
    fn default() -> Self {
        Self::new()
    }
}

impl LeaderElection {
    /// Create a new election tracker starting as follower.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: ElectionState::Follower,
            term: 0,
            elections: 0,
            terms_as_leader: 0,
            last_heartbeat_us: 0,
        }
    }

    /// Factory for cluster leadership.
    #[must_use]
    pub fn for_cluster() -> Self {
        Self::new()
    }

    /// Start election (become candidate).
    pub fn start_election(&mut self, now_us: u64) {
        self.state = ElectionState::Candidate;
        self.term += 1;
        self.elections += 1;
        self.last_heartbeat_us = now_us;
    }

    /// Win election (become leader).
    pub fn win_election(&mut self, now_us: u64) {
        if self.state == ElectionState::Candidate {
            self.state = ElectionState::Leader;
            self.terms_as_leader += 1;
            self.last_heartbeat_us = now_us;
        }
    }

    /// Lose election or step down (become follower).
    pub fn step_down(&mut self, new_term: u64) {
        if new_term > self.term {
            self.term = new_term;
        }
        self.state = ElectionState::Follower;
    }

    /// Record heartbeat (leader activity).
    pub fn heartbeat(&mut self, now_us: u64) {
        self.last_heartbeat_us = now_us;
    }

    /// Get current state.
    #[must_use]
    pub fn state(&self) -> ElectionState {
        self.state
    }

    /// Get current term.
    #[must_use]
    pub fn term(&self) -> u64 {
        self.term
    }

    /// Check if currently leader.
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.state == ElectionState::Leader
    }

    /// Get total elections.
    #[must_use]
    pub fn elections(&self) -> u64 {
        self.elections
    }

    /// Get terms as leader.
    #[must_use]
    pub fn terms_as_leader(&self) -> u64 {
        self.terms_as_leader
    }

    /// Reset to initial state.
    pub fn reset(&mut self) {
        self.state = ElectionState::Follower;
        self.term = 0;
        self.elections = 0;
        self.terms_as_leader = 0;
        self.last_heartbeat_us = 0;
    }
}

#[cfg(test)]
mod leader_election_tests {
    use super::*;

    /// F-ELECT-001: New tracker starts as follower
    #[test]
    fn f_elect_001_new() {
        let le = LeaderElection::new();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-002: Default equals new
    #[test]
    fn f_elect_002_default() {
        let le = LeaderElection::default();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-003: Start election becomes candidate
    #[test]
    fn f_elect_003_start_election() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        assert_eq!(le.state(), ElectionState::Candidate);
    }

    /// F-ELECT-004: Term increments on election
    #[test]
    fn f_elect_004_term_increment() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        assert_eq!(le.term(), 1);
    }

    /// F-ELECT-005: Win election becomes leader
    #[test]
    fn f_elect_005_win_election() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        assert!(le.is_leader());
    }

    /// F-ELECT-006: Step down becomes follower
    #[test]
    fn f_elect_006_step_down() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        le.step_down(2);
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-007: Factory for_cluster
    #[test]
    fn f_elect_007_for_cluster() {
        let le = LeaderElection::for_cluster();
        assert_eq!(le.term(), 0);
    }

    /// F-ELECT-008: Elections counted
    #[test]
    fn f_elect_008_elections() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.start_election(2000);
        assert_eq!(le.elections(), 2);
    }

    /// F-ELECT-009: Terms as leader tracked
    #[test]
    fn f_elect_009_terms_as_leader() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        assert_eq!(le.terms_as_leader(), 1);
    }

    /// F-ELECT-010: Win only works from candidate
    #[test]
    fn f_elect_010_win_requires_candidate() {
        let mut le = LeaderElection::new();
        le.win_election(1000); // Should not become leader
        assert!(!le.is_leader());
    }

    /// F-ELECT-011: Reset clears state
    #[test]
    fn f_elect_011_reset() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        le.win_election(2000);
        le.reset();
        assert_eq!(le.state(), ElectionState::Follower);
    }

    /// F-ELECT-012: Clone preserves state
    #[test]
    fn f_elect_012_clone() {
        let mut le = LeaderElection::new();
        le.start_election(1000);
        let cloned = le.clone();
        assert_eq!(le.state(), cloned.state());
    }
}

// ============================================================================
// CheckpointTracker - O(1) checkpoint/recovery point tracking
// ============================================================================

/// O(1) checkpoint/recovery point tracking.
///
/// Tracks checkpoint frequency, duration, and recovery points.
#[derive(Debug, Clone)]
pub struct CheckpointTracker {
    checkpoints: u64,
    total_duration_us: u64,
    last_checkpoint_us: u64,
    bytes_written: u64,
    failures: u64,
}

impl Default for CheckpointTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointTracker {
    /// Create a new checkpoint tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            checkpoints: 0,
            total_duration_us: 0,
            last_checkpoint_us: 0,
            bytes_written: 0,
            failures: 0,
        }
    }

    /// Factory for database checkpoints.
    #[must_use]
    pub fn for_database() -> Self {
        Self::new()
    }

    /// Factory for WAL checkpoints.
    #[must_use]
    pub fn for_wal() -> Self {
        Self::new()
    }

    /// Record a successful checkpoint.
    pub fn checkpoint(&mut self, duration_us: u64, bytes: u64, now_us: u64) {
        self.checkpoints += 1;
        self.total_duration_us += duration_us;
        self.bytes_written += bytes;
        self.last_checkpoint_us = now_us;
    }

    /// Record a failed checkpoint.
    pub fn fail(&mut self) {
        self.failures += 1;
    }

    /// Get checkpoint count.
    #[must_use]
    pub fn checkpoint_count(&self) -> u64 {
        self.checkpoints
    }

    /// Get average duration in microseconds.
    #[must_use]
    pub fn avg_duration_us(&self) -> f64 {
        if self.checkpoints == 0 {
            0.0
        } else {
            self.total_duration_us as f64 / self.checkpoints as f64
        }
    }

    /// Get total bytes written.
    #[must_use]
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Get failure rate.
    #[must_use]
    pub fn failure_rate(&self) -> f64 {
        let total = self.checkpoints + self.failures;
        if total == 0 {
            0.0
        } else {
            self.failures as f64 / total as f64
        }
    }

    /// Check if checkpoint system is healthy.
    #[must_use]
    pub fn is_healthy(&self, max_failure_rate: f64) -> bool {
        self.failure_rate() <= max_failure_rate
    }

    /// Get time since last checkpoint in microseconds.
    #[must_use]
    pub fn time_since_checkpoint(&self, now_us: u64) -> u64 {
        if self.last_checkpoint_us == 0 {
            0
        } else {
            now_us.saturating_sub(self.last_checkpoint_us)
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.checkpoints = 0;
        self.total_duration_us = 0;
        self.last_checkpoint_us = 0;
        self.bytes_written = 0;
        self.failures = 0;
    }
}

#[cfg(test)]
mod checkpoint_tracker_tests {
    use super::*;

    /// F-CKPT-001: New tracker starts empty
    #[test]
    fn f_ckpt_001_new() {
        let ct = CheckpointTracker::new();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-002: Default equals new
    #[test]
    fn f_ckpt_002_default() {
        let ct = CheckpointTracker::default();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-003: Checkpoint increments count
    #[test]
    fn f_ckpt_003_checkpoint() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 1024, 10000);
        assert_eq!(ct.checkpoint_count(), 1);
    }

    /// F-CKPT-004: Bytes written tracked
    #[test]
    fn f_ckpt_004_bytes_written() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 1024, 10000);
        ct.checkpoint(1000, 2048, 20000);
        assert_eq!(ct.bytes_written(), 3072);
    }

    /// F-CKPT-005: Average duration calculated
    #[test]
    fn f_ckpt_005_avg_duration() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.checkpoint(2000, 100, 20000);
        assert!((ct.avg_duration_us() - 1500.0).abs() < 0.01);
    }

    /// F-CKPT-006: Failures tracked
    #[test]
    fn f_ckpt_006_failures() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.fail();
        assert!((ct.failure_rate() - 0.5).abs() < 0.01);
    }

    /// F-CKPT-007: Factory for_database
    #[test]
    fn f_ckpt_007_for_database() {
        let ct = CheckpointTracker::for_database();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-008: Factory for_wal
    #[test]
    fn f_ckpt_008_for_wal() {
        let ct = CheckpointTracker::for_wal();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-009: Healthy when low failures
    #[test]
    fn f_ckpt_009_healthy() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        assert!(ct.is_healthy(0.1));
    }

    /// F-CKPT-010: Time since checkpoint
    #[test]
    fn f_ckpt_010_time_since() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        assert_eq!(ct.time_since_checkpoint(15000), 5000);
    }

    /// F-CKPT-011: Reset clears state
    #[test]
    fn f_ckpt_011_reset() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        ct.reset();
        assert_eq!(ct.checkpoint_count(), 0);
    }

    /// F-CKPT-012: Clone preserves state
    #[test]
    fn f_ckpt_012_clone() {
        let mut ct = CheckpointTracker::new();
        ct.checkpoint(1000, 100, 10000);
        let cloned = ct.clone();
        assert_eq!(ct.checkpoint_count(), cloned.checkpoint_count());
    }
}

// ============================================================================
// ReplicationLag - O(1) replication lag monitoring
// ============================================================================

/// O(1) replication lag monitoring.
///
/// Tracks replication lag between primary and replica.
#[derive(Debug, Clone)]
pub struct ReplicationLag {
    samples: u64,
    total_lag_us: u64,
    max_lag_us: u64,
    current_lag_us: u64,
    threshold_us: u64,
    breaches: u64,
}

impl Default for ReplicationLag {
    fn default() -> Self {
        Self::for_database()
    }
}

impl ReplicationLag {
    /// Create a new replication lag tracker with threshold.
    #[must_use]
    pub fn new(threshold_us: u64) -> Self {
        Self {
            samples: 0,
            total_lag_us: 0,
            max_lag_us: 0,
            current_lag_us: 0,
            threshold_us,
            breaches: 0,
        }
    }

    /// Factory for database replication (1 second threshold).
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(1_000_000) // 1 second
    }

    /// Factory for cache replication (100ms threshold).
    #[must_use]
    pub fn for_cache() -> Self {
        Self::new(100_000) // 100ms
    }

    /// Record a lag measurement.
    pub fn record(&mut self, lag_us: u64) {
        self.samples += 1;
        self.total_lag_us += lag_us;
        self.current_lag_us = lag_us;
        if lag_us > self.max_lag_us {
            self.max_lag_us = lag_us;
        }
        if lag_us > self.threshold_us {
            self.breaches += 1;
        }
    }

    /// Get current lag in microseconds.
    #[must_use]
    pub fn current_lag_us(&self) -> u64 {
        self.current_lag_us
    }

    /// Get average lag in microseconds.
    #[must_use]
    pub fn avg_lag_us(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.total_lag_us as f64 / self.samples as f64
        }
    }

    /// Get max lag in microseconds.
    #[must_use]
    pub fn max_lag_us(&self) -> u64 {
        self.max_lag_us
    }

    /// Get breach count (exceeded threshold).
    #[must_use]
    pub fn breaches(&self) -> u64 {
        self.breaches
    }

    /// Check if currently within threshold.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.current_lag_us <= self.threshold_us
    }

    /// Get breach rate.
    #[must_use]
    pub fn breach_rate(&self) -> f64 {
        if self.samples == 0 {
            0.0
        } else {
            self.breaches as f64 / self.samples as f64
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.samples = 0;
        self.total_lag_us = 0;
        self.max_lag_us = 0;
        self.current_lag_us = 0;
        self.breaches = 0;
    }
}

#[cfg(test)]
mod replication_lag_tests {
    use super::*;

    /// F-REPL-001: New tracker starts empty
    #[test]
    fn f_repl_001_new() {
        let rl = ReplicationLag::new(1000);
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-002: Default uses database threshold
    #[test]
    fn f_repl_002_default() {
        let rl = ReplicationLag::default();
        assert!(rl.is_healthy()); // 0 lag is healthy
    }

    /// F-REPL-003: Record updates current
    #[test]
    fn f_repl_003_record() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        assert_eq!(rl.current_lag_us(), 500);
    }

    /// F-REPL-004: Max lag tracked
    #[test]
    fn f_repl_004_max_lag() {
        let mut rl = ReplicationLag::new(10000);
        rl.record(500);
        rl.record(2000);
        rl.record(800);
        assert_eq!(rl.max_lag_us(), 2000);
    }

    /// F-REPL-005: Average lag calculated
    #[test]
    fn f_repl_005_avg_lag() {
        let mut rl = ReplicationLag::new(10000);
        rl.record(1000);
        rl.record(2000);
        assert!((rl.avg_lag_us() - 1500.0).abs() < 0.01);
    }

    /// F-REPL-006: Breaches counted
    #[test]
    fn f_repl_006_breaches() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        rl.record(1500); // breach
        assert_eq!(rl.breaches(), 1);
    }

    /// F-REPL-007: Factory for_database
    #[test]
    fn f_repl_007_for_database() {
        let rl = ReplicationLag::for_database();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-008: Factory for_cache
    #[test]
    fn f_repl_008_for_cache() {
        let rl = ReplicationLag::for_cache();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-009: Healthy when under threshold
    #[test]
    fn f_repl_009_healthy() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        assert!(rl.is_healthy());
    }

    /// F-REPL-010: Not healthy when over threshold
    #[test]
    fn f_repl_010_unhealthy() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(1500);
        assert!(!rl.is_healthy());
    }

    /// F-REPL-011: Reset clears state
    #[test]
    fn f_repl_011_reset() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        rl.reset();
        assert_eq!(rl.current_lag_us(), 0);
    }

    /// F-REPL-012: Clone preserves state
    #[test]
    fn f_repl_012_clone() {
        let mut rl = ReplicationLag::new(1000);
        rl.record(500);
        let cloned = rl.clone();
        assert_eq!(rl.current_lag_us(), cloned.current_lag_us());
    }
}

// ============================================================================
// QuorumTracker - O(1) consensus quorum tracking
// ============================================================================

/// O(1) consensus quorum tracking.
///
/// Tracks votes and quorum achievement for distributed consensus.
#[derive(Debug, Clone)]
pub struct QuorumTracker {
    total_nodes: u32,
    votes_received: u32,
    quorum_threshold: u32,
    rounds: u64,
    quorum_achieved: u64,
}

impl Default for QuorumTracker {
    fn default() -> Self {
        Self::for_cluster(3)
    }
}

impl QuorumTracker {
    /// Create a new quorum tracker with total nodes.
    #[must_use]
    pub fn new(total_nodes: u32) -> Self {
        Self {
            total_nodes,
            votes_received: 0,
            quorum_threshold: total_nodes / 2 + 1, // Majority
            rounds: 0,
            quorum_achieved: 0,
        }
    }

    /// Factory for cluster consensus.
    #[must_use]
    pub fn for_cluster(nodes: u32) -> Self {
        Self::new(nodes)
    }

    /// Start a new voting round.
    pub fn start_round(&mut self) {
        self.votes_received = 0;
        self.rounds += 1;
    }

    /// Record a vote.
    pub fn vote(&mut self) {
        if self.votes_received < self.total_nodes {
            self.votes_received += 1;
            if self.votes_received == self.quorum_threshold {
                self.quorum_achieved += 1;
            }
        }
    }

    /// Check if quorum is achieved.
    #[must_use]
    pub fn has_quorum(&self) -> bool {
        self.votes_received >= self.quorum_threshold
    }

    /// Get votes received.
    #[must_use]
    pub fn votes(&self) -> u32 {
        self.votes_received
    }

    /// Get votes needed for quorum.
    #[must_use]
    pub fn votes_needed(&self) -> u32 {
        self.quorum_threshold.saturating_sub(self.votes_received)
    }

    /// Get total rounds.
    #[must_use]
    pub fn rounds(&self) -> u64 {
        self.rounds
    }

    /// Get quorum success rate.
    #[must_use]
    pub fn success_rate(&self) -> f64 {
        if self.rounds == 0 {
            0.0
        } else {
            self.quorum_achieved as f64 / self.rounds as f64
        }
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.votes_received = 0;
        self.rounds = 0;
        self.quorum_achieved = 0;
    }
}

#[cfg(test)]
mod quorum_tracker_tests {
    use super::*;

    /// F-QUORUM-001: New tracker starts with no votes
    #[test]
    fn f_quorum_001_new() {
        let qt = QuorumTracker::new(5);
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-002: Default uses 3 nodes
    #[test]
    fn f_quorum_002_default() {
        let qt = QuorumTracker::default();
        assert!(!qt.has_quorum());
    }

    /// F-QUORUM-003: Vote increments count
    #[test]
    fn f_quorum_003_vote() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        assert_eq!(qt.votes(), 1);
    }

    /// F-QUORUM-004: Quorum achieved with majority
    #[test]
    fn f_quorum_004_quorum() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote();
        qt.vote(); // 3/5 = quorum
        assert!(qt.has_quorum());
    }

    /// F-QUORUM-005: No quorum without majority
    #[test]
    fn f_quorum_005_no_quorum() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote(); // 2/5 < quorum
        assert!(!qt.has_quorum());
    }

    /// F-QUORUM-006: Votes needed calculated
    #[test]
    fn f_quorum_006_votes_needed() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        assert_eq!(qt.votes_needed(), 2); // Need 3, have 1
    }

    /// F-QUORUM-007: Factory for_cluster
    #[test]
    fn f_quorum_007_for_cluster() {
        let qt = QuorumTracker::for_cluster(7);
        assert_eq!(qt.votes_needed(), 4); // 7/2+1 = 4
    }

    /// F-QUORUM-008: Start round resets votes
    #[test]
    fn f_quorum_008_start_round() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.vote();
        qt.start_round();
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-009: Rounds counted
    #[test]
    fn f_quorum_009_rounds() {
        let mut qt = QuorumTracker::new(5);
        qt.start_round();
        qt.start_round();
        assert_eq!(qt.rounds(), 2);
    }

    /// F-QUORUM-010: Success rate calculated
    #[test]
    fn f_quorum_010_success_rate() {
        let mut qt = QuorumTracker::new(3);
        qt.start_round();
        qt.vote();
        qt.vote(); // quorum achieved
        qt.start_round();
        // no votes = no quorum
        assert!((qt.success_rate() - 0.5).abs() < 0.01);
    }

    /// F-QUORUM-011: Reset clears state
    #[test]
    fn f_quorum_011_reset() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        qt.reset();
        assert_eq!(qt.votes(), 0);
    }

    /// F-QUORUM-012: Clone preserves state
    #[test]
    fn f_quorum_012_clone() {
        let mut qt = QuorumTracker::new(5);
        qt.vote();
        let cloned = qt.clone();
        assert_eq!(qt.votes(), cloned.votes());
    }
}

// ============================================================================
// PartitionTracker - O(1) partition/shard tracking
// ============================================================================

/// O(1) partition/shard tracking.
///
/// Tracks partition health, assignment, and rebalancing.
#[derive(Debug, Clone)]
pub struct PartitionTracker {
    total_partitions: u32,
    assigned: u32,
    healthy: u32,
    rebalances: u64,
    last_rebalance_us: u64,
}

impl Default for PartitionTracker {
    fn default() -> Self {
        Self::for_kafka()
    }
}

impl PartitionTracker {
    /// Create a new partition tracker.
    #[must_use]
    pub fn new(total_partitions: u32) -> Self {
        Self {
            total_partitions,
            assigned: 0,
            healthy: 0,
            rebalances: 0,
            last_rebalance_us: 0,
        }
    }

    /// Factory for Kafka-style partitions (12 default).
    #[must_use]
    pub fn for_kafka() -> Self {
        Self::new(12)
    }

    /// Factory for database shards (8 default).
    #[must_use]
    pub fn for_shards() -> Self {
        Self::new(8)
    }

    /// Assign partitions.
    pub fn assign(&mut self, count: u32) {
        self.assigned = count.min(self.total_partitions);
    }

    /// Mark partitions as healthy.
    pub fn mark_healthy(&mut self, count: u32) {
        self.healthy = count.min(self.assigned);
    }

    /// Record a rebalance event.
    pub fn rebalance(&mut self, now_us: u64) {
        self.rebalances += 1;
        self.last_rebalance_us = now_us;
    }

    /// Get assigned partition count.
    #[must_use]
    pub fn assigned(&self) -> u32 {
        self.assigned
    }

    /// Get healthy partition count.
    #[must_use]
    pub fn healthy(&self) -> u32 {
        self.healthy
    }

    /// Get assignment percentage.
    #[must_use]
    pub fn assignment_rate(&self) -> f64 {
        if self.total_partitions == 0 {
            0.0
        } else {
            (self.assigned as f64 / self.total_partitions as f64) * 100.0
        }
    }

    /// Get health percentage.
    #[must_use]
    pub fn health_rate(&self) -> f64 {
        if self.assigned == 0 {
            0.0
        } else {
            (self.healthy as f64 / self.assigned as f64) * 100.0
        }
    }

    /// Check if all assigned partitions are healthy.
    #[must_use]
    pub fn is_fully_healthy(&self) -> bool {
        self.healthy == self.assigned && self.assigned > 0
    }

    /// Get rebalance count.
    #[must_use]
    pub fn rebalances(&self) -> u64 {
        self.rebalances
    }

    /// Reset tracker state.
    pub fn reset(&mut self) {
        self.assigned = 0;
        self.healthy = 0;
        self.rebalances = 0;
        self.last_rebalance_us = 0;
    }
}

#[cfg(test)]
mod partition_tracker_tests {
    use super::*;

    /// F-PART-001: New tracker starts empty
    #[test]
    fn f_part_001_new() {
        let pt = PartitionTracker::new(10);
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-002: Default uses Kafka defaults
    #[test]
    fn f_part_002_default() {
        let pt = PartitionTracker::default();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-003: Assign sets count
    #[test]
    fn f_part_003_assign() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        assert_eq!(pt.assigned(), 5);
    }

    /// F-PART-004: Assign capped at total
    #[test]
    fn f_part_004_assign_cap() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(15);
        assert_eq!(pt.assigned(), 10);
    }

    /// F-PART-005: Mark healthy sets count
    #[test]
    fn f_part_005_mark_healthy() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.mark_healthy(3);
        assert_eq!(pt.healthy(), 3);
    }

    /// F-PART-006: Health rate calculated
    #[test]
    fn f_part_006_health_rate() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(10);
        pt.mark_healthy(5);
        assert!((pt.health_rate() - 50.0).abs() < 0.01);
    }

    /// F-PART-007: Factory for_kafka
    #[test]
    fn f_part_007_for_kafka() {
        let pt = PartitionTracker::for_kafka();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-008: Factory for_shards
    #[test]
    fn f_part_008_for_shards() {
        let pt = PartitionTracker::for_shards();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-009: Fully healthy when all healthy
    #[test]
    fn f_part_009_fully_healthy() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.mark_healthy(5);
        assert!(pt.is_fully_healthy());
    }

    /// F-PART-010: Rebalances tracked
    #[test]
    fn f_part_010_rebalances() {
        let mut pt = PartitionTracker::new(10);
        pt.rebalance(1000);
        pt.rebalance(2000);
        assert_eq!(pt.rebalances(), 2);
    }

    /// F-PART-011: Reset clears state
    #[test]
    fn f_part_011_reset() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        pt.reset();
        assert_eq!(pt.assigned(), 0);
    }

    /// F-PART-012: Clone preserves state
    #[test]
    fn f_part_012_clone() {
        let mut pt = PartitionTracker::new(10);
        pt.assign(5);
        let cloned = pt.clone();
        assert_eq!(pt.assigned(), cloned.assigned());
    }
}

// ============================================================================
// ConnectionPool - O(1) connection pool state tracking
// ============================================================================

/// O(1) connection pool state tracking.
///
/// Tracks active connections, idle pool, and connection lifecycle.
#[derive(Debug, Clone)]
pub struct ConnectionPool {
    max_size: u32,
    active: u32,
    idle: u32,
    created: u64,
    destroyed: u64,
    wait_count: u64,
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::for_database()
    }
}

impl ConnectionPool {
    /// Create a new connection pool tracker.
    #[must_use]
    pub fn new(max_size: u32) -> Self {
        Self {
            max_size,
            active: 0,
            idle: 0,
            created: 0,
            destroyed: 0,
            wait_count: 0,
        }
    }

    /// Factory for database pool (20 connections).
    #[must_use]
    pub fn for_database() -> Self {
        Self::new(20)
    }

    /// Factory for HTTP pool (100 connections).
    #[must_use]
    pub fn for_http() -> Self {
        Self::new(100)
    }

    /// Acquire a connection from pool.
    pub fn acquire(&mut self) -> bool {
        if self.idle > 0 {
            self.idle -= 1;
            self.active += 1;
            true
        } else if self.active + self.idle < self.max_size {
            self.active += 1;
            self.created += 1;
            true
        } else {
            self.wait_count += 1;
            false
        }
    }

    /// Release a connection back to pool.
    pub fn release(&mut self) {
        if self.active > 0 {
            self.active -= 1;
            self.idle += 1;
        }
    }

    /// Destroy a connection (evict from pool).
    pub fn destroy(&mut self) {
        if self.idle > 0 {
            self.idle -= 1;
            self.destroyed += 1;
        }
    }

    /// Get active connection count.
    #[must_use]
    pub fn active(&self) -> u32 {
        self.active
    }

    /// Get idle connection count.
    #[must_use]
    pub fn idle(&self) -> u32 {
        self.idle
    }

    /// Get pool utilization percentage.
    #[must_use]
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.active as f64 / self.max_size as f64) * 100.0
        }
    }

    /// Check if pool is exhausted.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.active >= self.max_size && self.idle == 0
    }

    /// Get wait count (failed acquisitions).
    #[must_use]
    pub fn wait_count(&self) -> u64 {
        self.wait_count
    }

    /// Reset pool state.
    pub fn reset(&mut self) {
        self.active = 0;
        self.idle = 0;
        self.created = 0;
        self.destroyed = 0;
        self.wait_count = 0;
    }
}

#[cfg(test)]
mod connection_pool_tests {
    use super::*;

    /// F-CPOOL-001: New pool starts empty
    #[test]
    fn f_cpool_001_new() {
        let cp = ConnectionPool::new(10);
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-002: Default uses database size
    #[test]
    fn f_cpool_002_default() {
        let cp = ConnectionPool::default();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-003: Acquire creates connection
    #[test]
    fn f_cpool_003_acquire() {
        let mut cp = ConnectionPool::new(10);
        assert!(cp.acquire());
        assert_eq!(cp.active(), 1);
    }

    /// F-CPOOL-004: Release returns to idle
    #[test]
    fn f_cpool_004_release() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.release();
        assert_eq!(cp.idle(), 1);
    }

    /// F-CPOOL-005: Acquire from idle
    #[test]
    fn f_cpool_005_acquire_idle() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.release();
        cp.acquire();
        assert_eq!(cp.active(), 1);
        assert_eq!(cp.idle(), 0);
    }

    /// F-CPOOL-006: Exhausted when full
    #[test]
    fn f_cpool_006_exhausted() {
        let mut cp = ConnectionPool::new(2);
        cp.acquire();
        cp.acquire();
        assert!(cp.is_exhausted());
    }

    /// F-CPOOL-007: Factory for_database
    #[test]
    fn f_cpool_007_for_database() {
        let cp = ConnectionPool::for_database();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-008: Factory for_http
    #[test]
    fn f_cpool_008_for_http() {
        let cp = ConnectionPool::for_http();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-009: Utilization calculated
    #[test]
    fn f_cpool_009_utilization() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.acquire();
        assert!((cp.utilization() - 20.0).abs() < 0.01);
    }

    /// F-CPOOL-010: Wait count on exhaustion
    #[test]
    fn f_cpool_010_wait_count() {
        let mut cp = ConnectionPool::new(1);
        cp.acquire();
        cp.acquire(); // fails
        assert_eq!(cp.wait_count(), 1);
    }

    /// F-CPOOL-011: Reset clears state
    #[test]
    fn f_cpool_011_reset() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        cp.reset();
        assert_eq!(cp.active(), 0);
    }

    /// F-CPOOL-012: Clone preserves state
    #[test]
    fn f_cpool_012_clone() {
        let mut cp = ConnectionPool::new(10);
        cp.acquire();
        let cloned = cp.clone();
        assert_eq!(cp.active(), cloned.active());
    }
}

// ============================================================================
// RequestTracker - O(1) request lifecycle tracking
// ============================================================================

/// O(1) request lifecycle tracking.
///
/// Tracks request counts, latencies, and error rates.
#[derive(Debug, Clone)]
pub struct RequestTracker {
    total: u64,
    success: u64,
    errors: u64,
    total_latency_us: u64,
    max_latency_us: u64,
    in_flight: u32,
}

impl Default for RequestTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl RequestTracker {
    /// Create a new request tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            total: 0,
            success: 0,
            errors: 0,
            total_latency_us: 0,
            max_latency_us: 0,
            in_flight: 0,
        }
    }

    /// Factory for API requests.
    #[must_use]
    pub fn for_api() -> Self {
        Self::new()
    }

    /// Factory for database queries.
    #[must_use]
    pub fn for_queries() -> Self {
        Self::new()
    }

    /// Start tracking a request.
    pub fn start(&mut self) {
        self.in_flight += 1;
    }

    /// Complete a successful request.
    pub fn complete(&mut self, latency_us: u64) {
        self.total += 1;
        self.success += 1;
        self.total_latency_us += latency_us;
        if latency_us > self.max_latency_us {
            self.max_latency_us = latency_us;
