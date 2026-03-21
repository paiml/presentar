    }

    /// F-PAIR-006: Success rate calculation
    #[test]
    fn f_pair_006_success_rate() {
        let mut cp = CounterPair::new();
        cp.add_successes(80);
        cp.add_failures(20);
        assert_eq!(cp.success_rate(), 80.0);
    }

    /// F-PAIR-007: Failure rate calculation
    #[test]
    fn f_pair_007_failure_rate() {
        let mut cp = CounterPair::new();
        cp.add_successes(80);
        cp.add_failures(20);
        assert_eq!(cp.failure_rate(), 20.0);
    }

    /// F-PAIR-008: Empty counter is 100% success
    #[test]
    fn f_pair_008_empty_healthy() {
        let cp = CounterPair::new();
        assert_eq!(cp.success_rate(), 100.0);
    }

    /// F-PAIR-009: is_healthy check
    #[test]
    fn f_pair_009_is_healthy() {
        let mut cp = CounterPair::new();
        cp.add_successes(95);
        cp.add_failures(5);
        assert!(cp.is_healthy(90.0));
        assert!(!cp.is_healthy(99.0));
    }

    /// F-PAIR-010: Reset clears state
    #[test]
    fn f_pair_010_reset() {
        let mut cp = CounterPair::new();
        cp.success();
        cp.failure();
        cp.reset();
        assert_eq!(cp.total(), 0);
    }

    /// F-PAIR-011: Debug format works
    #[test]
    fn f_pair_011_debug() {
        let cp = CounterPair::new();
        let debug = format!("{:?}", cp);
        assert!(debug.contains("CounterPair"));
    }

    /// F-PAIR-012: Clone preserves state
    #[test]
    fn f_pair_012_clone() {
        let mut cp = CounterPair::new();
        cp.success();
        let cloned = cp.clone();
        assert_eq!(cp.successes(), cloned.successes());
    }

    // =========================================================================
    // HEALTH SCORE TESTS (F-HEALTH-001 to F-HEALTH-012)
    // =========================================================================

    /// F-HEALTH-001: New score starts at 100
    #[test]
    fn f_health_001_new_100() {
        let hs = HealthScore::new();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-002: Default same as new
    #[test]
    fn f_health_002_default() {
        let hs = HealthScore::default();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-003: Set component score
    #[test]
    fn f_health_003_set() {
        let mut hs = HealthScore::new();
        hs.set(0, 80.0);
        assert_eq!(hs.score(), 80.0);
    }

    /// F-HEALTH-004: Weighted average
    #[test]
    fn f_health_004_weighted() {
        let mut hs = HealthScore::new();
        hs.set(0, 100.0);
        hs.set_weight(0, 2.0);
        hs.set(1, 50.0);
        hs.set_weight(1, 1.0);
        // (100*2 + 50*1) / 3 = 83.33
        let score = hs.score();
        assert!(score > 80.0 && score < 90.0);
    }

    /// F-HEALTH-005: Status healthy
    #[test]
    fn f_health_005_status_healthy() {
        let hs = HealthScore::new();
        assert_eq!(hs.status(), HealthStatus::Healthy);
    }

    /// F-HEALTH-006: Status degraded
    #[test]
    fn f_health_006_status_degraded() {
        let mut hs = HealthScore::new();
        hs.set(0, 75.0);
        assert_eq!(hs.status(), HealthStatus::Degraded);
    }

    /// F-HEALTH-007: Status warning
    #[test]
    fn f_health_007_status_warning() {
        let mut hs = HealthScore::new();
        hs.set(0, 55.0);
        assert_eq!(hs.status(), HealthStatus::Warning);
    }

    /// F-HEALTH-008: Status critical
    #[test]
    fn f_health_008_status_critical() {
        let mut hs = HealthScore::new();
        hs.set(0, 30.0);
        assert_eq!(hs.status(), HealthStatus::Critical);
    }

    /// F-HEALTH-009: Min score tracking
    #[test]
    fn f_health_009_min_score() {
        let mut hs = HealthScore::new();
        hs.set(0, 90.0);
        hs.set(1, 60.0);
        hs.set(2, 80.0);
        assert_eq!(hs.min_score(), 60.0);
    }

    /// F-HEALTH-010: Reset to 100
    #[test]
    fn f_health_010_reset() {
        let mut hs = HealthScore::new();
        hs.set(0, 50.0);
        hs.reset();
        assert_eq!(hs.score(), 100.0);
    }

    /// F-HEALTH-011: Debug format works
    #[test]
    fn f_health_011_debug() {
        let hs = HealthScore::new();
        let debug = format!("{:?}", hs);
        assert!(debug.contains("HealthScore"));
    }

    /// F-HEALTH-012: Clone preserves state
    #[test]
    fn f_health_012_clone() {
        let mut hs = HealthScore::new();
        hs.set(0, 75.0);
        let cloned = hs.clone();
        assert_eq!(hs.score(), cloned.score());
    }

    // ========================================================================
    // BatchProcessor Falsification Tests (F-BATCH-001 to F-BATCH-012)
    // ========================================================================

    /// F-BATCH-001: New with batch size
    #[test]
    fn f_batch_001_new() {
        let bp = BatchProcessor::new(10);
        assert_eq!(bp.batches_completed(), 0);
        assert_eq!(bp.total_items(), 0);
    }

    /// F-BATCH-002: Default batch size 100
    #[test]
    fn f_batch_002_default() {
        let bp = BatchProcessor::default();
        assert_eq!(bp.remaining(), 100);
    }

    /// F-BATCH-003: Add returns false until batch complete
    #[test]
    fn f_batch_003_add_partial() {
        let mut bp = BatchProcessor::new(3);
        assert!(!bp.add());
        assert!(!bp.add());
        assert!(bp.add()); // 3rd item completes batch
    }

    /// F-BATCH-004: Batch completes resets count
    #[test]
    fn f_batch_004_batch_complete() {
        let mut bp = BatchProcessor::new(2);
        bp.add();
        bp.add();
        assert_eq!(bp.batches_completed(), 1);
        assert_eq!(bp.remaining(), 2);
    }

    /// F-BATCH-005: Add many returns correct batches
    #[test]
    fn f_batch_005_add_many() {
        let mut bp = BatchProcessor::new(10);
        let batches = bp.add_many(25);
        assert_eq!(batches, 2);
        assert_eq!(bp.remaining(), 5);
    }

    /// F-BATCH-006: Fill percentage calculation
    #[test]
    fn f_batch_006_fill_percentage() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        assert!((bp.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-BATCH-007: Factory for_network batch 1000
    #[test]
    fn f_batch_007_for_network() {
        let bp = BatchProcessor::for_network();
        assert_eq!(bp.remaining(), 1000);
    }

    /// F-BATCH-008: Factory for_disk batch 100
    #[test]
    fn f_batch_008_for_disk() {
        let bp = BatchProcessor::for_disk();
        assert_eq!(bp.remaining(), 100);
    }

    /// F-BATCH-009: Factory for_metrics batch 50
    #[test]
    fn f_batch_009_for_metrics() {
        let bp = BatchProcessor::for_metrics();
        assert_eq!(bp.remaining(), 50);
    }

    /// F-BATCH-010: Flush completes partial batch
    #[test]
    fn f_batch_010_flush() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        bp.flush();
        assert_eq!(bp.batches_completed(), 1);
        assert_eq!(bp.remaining(), 10);
    }

    /// F-BATCH-011: Reset clears all counters
    #[test]
    fn f_batch_011_reset() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(25);
        bp.reset();
        assert_eq!(bp.batches_completed(), 0);
        assert_eq!(bp.total_items(), 0);
    }

    /// F-BATCH-012: Clone preserves state
    #[test]
    fn f_batch_012_clone() {
        let mut bp = BatchProcessor::new(10);
        bp.add_many(5);
        let cloned = bp.clone();
        assert_eq!(bp.remaining(), cloned.remaining());
    }

    // ========================================================================
    // PipelineStage Falsification Tests (F-PIPE-001 to F-PIPE-012)
    // ========================================================================

    /// F-PIPE-001: New creates empty stage
    #[test]
    fn f_pipe_001_new() {
        let ps = PipelineStage::new();
        assert!(ps.is_idle());
        assert_eq!(ps.depth(), 0);
    }

    /// F-PIPE-002: Default same as new
    #[test]
    fn f_pipe_002_default() {
        let ps = PipelineStage::default();
        assert!(ps.is_idle());
    }

    /// F-PIPE-003: Enter increases depth
    #[test]
    fn f_pipe_003_enter() {
        let mut ps = PipelineStage::new();
        ps.enter();
        assert_eq!(ps.depth(), 1);
        assert!(!ps.is_idle());
    }

    /// F-PIPE-004: Exit decreases depth
    #[test]
    fn f_pipe_004_exit() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        assert_eq!(ps.depth(), 0);
    }

    /// F-PIPE-005: Peak depth tracked
    #[test]
    fn f_pipe_005_peak() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.peak_depth(), 3);
    }

    /// F-PIPE-006: Average latency calculation
    #[test]
    fn f_pipe_006_avg_latency() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        ps.enter();
        ps.exit(2000);
        assert!((ps.avg_latency_us() - 1500.0).abs() < 0.01);
    }

    /// F-PIPE-007: Latency ms conversion
    #[test]
    fn f_pipe_007_latency_ms() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        assert!((ps.avg_latency_ms() - 1.0).abs() < 0.01);
    }

    /// F-PIPE-008: Throughput equals exits
    #[test]
    fn f_pipe_008_throughput() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit_simple();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.throughput(), 2);
    }

    /// F-PIPE-009: Total entered tracked
    #[test]
    fn f_pipe_009_total_entered() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.exit_simple();
        assert_eq!(ps.total_entered(), 2);
    }

    /// F-PIPE-010: Backlog detection
    #[test]
    fn f_pipe_010_backlogged() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.enter();
        ps.enter();
        assert!(ps.is_backlogged(2));
    }

    /// F-PIPE-011: Reset clears all
    #[test]
    fn f_pipe_011_reset() {
        let mut ps = PipelineStage::new();
        ps.enter();
        ps.exit(1000);
        ps.reset();
        assert!(ps.is_idle());
        assert_eq!(ps.throughput(), 0);
    }

    /// F-PIPE-012: Clone preserves state
    #[test]
    fn f_pipe_012_clone() {
        let mut ps = PipelineStage::new();
        ps.enter();
        let cloned = ps.clone();
        assert_eq!(ps.depth(), cloned.depth());
    }

    // ========================================================================
    // WorkQueue Falsification Tests (F-QUEUE-001 to F-QUEUE-012)
    // ========================================================================

    /// F-QUEUE-001: New creates empty queue
    #[test]
    fn f_queue_001_new() {
        let wq = WorkQueue::new();
        assert!(wq.is_empty());
        assert_eq!(wq.size(), 0);
    }

    /// F-QUEUE-002: Default same as new
    #[test]
    fn f_queue_002_default() {
        let wq = WorkQueue::default();
        assert!(wq.is_empty());
    }

    /// F-QUEUE-003: With capacity sets limit
    #[test]
    fn f_queue_003_with_capacity() {
        let wq = WorkQueue::with_capacity(10);
        assert_eq!(wq.remaining_capacity(), 10);
    }

    /// F-QUEUE-004: Enqueue increases size
    #[test]
    fn f_queue_004_enqueue() {
        let mut wq = WorkQueue::new();
        assert!(wq.enqueue());
        assert_eq!(wq.size(), 1);
    }

    /// F-QUEUE-005: Dequeue decreases size
    #[test]
    fn f_queue_005_dequeue() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        assert!(wq.dequeue(100));
        assert!(wq.is_empty());
    }

    /// F-QUEUE-006: Full queue rejects enqueue
    #[test]
    fn f_queue_006_full() {
        let mut wq = WorkQueue::with_capacity(1);
        wq.enqueue();
        assert!(!wq.enqueue());
        assert!(wq.is_full());
    }

    /// F-QUEUE-007: Empty queue rejects dequeue
    #[test]
    fn f_queue_007_empty_dequeue() {
        let mut wq = WorkQueue::new();
        assert!(!wq.dequeue_simple());
    }

    /// F-QUEUE-008: Peak size tracked
    #[test]
    fn f_queue_008_peak() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.enqueue();
        wq.dequeue_simple();
        assert_eq!(wq.peak_size(), 2);
    }

    /// F-QUEUE-009: Average wait time
    #[test]
    fn f_queue_009_avg_wait() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.dequeue(1000);
        wq.enqueue();
        wq.dequeue(2000);
        assert!((wq.avg_wait_us() - 1500.0).abs() < 0.01);
    }

    /// F-QUEUE-010: Utilization percentage
    #[test]
    fn f_queue_010_utilization() {
        let mut wq = WorkQueue::with_capacity(10);
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        wq.enqueue();
        assert!((wq.utilization() - 50.0).abs() < 0.01);
    }

    /// F-QUEUE-011: Reset clears all
    #[test]
    fn f_queue_011_reset() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        wq.dequeue(1000);
        wq.reset();
        assert!(wq.is_empty());
        assert_eq!(wq.total_dequeued(), 0);
    }

    /// F-QUEUE-012: Clone preserves state
    #[test]
    fn f_queue_012_clone() {
        let mut wq = WorkQueue::new();
        wq.enqueue();
        let cloned = wq.clone();
        assert_eq!(wq.size(), cloned.size());
    }

    // ========================================================================
    // LeakyBucket Falsification Tests (F-LEAK-001 to F-LEAK-012)
    // ========================================================================

    /// F-LEAK-001: New creates empty bucket
    #[test]
    fn f_leak_001_new() {
        let lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.is_empty());
        assert_eq!(lb.overflows(), 0);
    }

    /// F-LEAK-002: Default 100 capacity, 10 rate
    #[test]
    fn f_leak_002_default() {
        let lb = LeakyBucket::default();
        assert!(lb.is_empty());
    }

    /// F-LEAK-003: Add increases level
    #[test]
    fn f_leak_003_add() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.add(50.0, 0));
        assert!((lb.level() - 50.0).abs() < 0.01);
    }

    /// F-LEAK-004: Overflow rejected
    #[test]
    fn f_leak_004_overflow() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        assert!(lb.add(80.0, 0));
        assert!(!lb.add(50.0, 0)); // Would exceed
        assert_eq!(lb.overflows(), 1);
    }

    /// F-LEAK-005: Leaking over time
    #[test]
    fn f_leak_005_leak() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 1000); // Init with timestamp 1000
        lb.update_with_time(1_001_000); // 1 second later
                                        // Leaked 10 tokens: 50 - 10 = 40
        assert!(lb.level() < 45.0);
    }

    /// F-LEAK-006: Fill percentage
    #[test]
    fn f_leak_006_fill_percentage() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        assert!((lb.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-LEAK-007: Factory for_api
    #[test]
    fn f_leak_007_for_api() {
        let lb = LeakyBucket::for_api();
        assert!(lb.is_empty());
    }

    /// F-LEAK-008: Factory for_network
    #[test]
    fn f_leak_008_for_network() {
        let lb = LeakyBucket::for_network();
        assert!(lb.is_empty());
    }

    /// F-LEAK-009: Full leak empties bucket
    #[test]
    fn f_leak_009_full_leak() {
        let mut lb = LeakyBucket::new(100.0, 100.0);
        lb.add(50.0, 1000); // Init with timestamp 1000
        lb.update_with_time(1_001_000); // 1 second later, 100 leaked
        assert!(lb.is_empty());
    }

    /// F-LEAK-010: Reset clears bucket
    #[test]
    fn f_leak_010_reset() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        lb.add(200.0, 0); // overflow
        lb.reset();
        assert!(lb.is_empty());
        assert_eq!(lb.overflows(), 0);
    }

    /// F-LEAK-011: Debug format works
    #[test]
    fn f_leak_011_debug() {
        let lb = LeakyBucket::new(100.0, 10.0);
        let debug = format!("{:?}", lb);
        assert!(debug.contains("LeakyBucket"));
    }

    /// F-LEAK-012: Clone preserves state
    #[test]
    fn f_leak_012_clone() {
        let mut lb = LeakyBucket::new(100.0, 10.0);
        lb.add(50.0, 0);
        let cloned = lb.clone();
        assert!((lb.level() - cloned.level()).abs() < 0.01);
    }

    // ========================================================================
    // SlidingWindowRate Falsification Tests (F-SLIDE-001 to F-SLIDE-012)
    // ========================================================================

    /// F-SLIDE-001: New creates empty windows
    #[test]
    fn f_slide_001_new() {
        let sw = SlidingWindowRate::new(1_000_000, 100);
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.exceeded(), 0);
    }

    /// F-SLIDE-002: Default 1s window, 100 limit
    #[test]
    fn f_slide_002_default() {
        let sw = SlidingWindowRate::default();
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-003: Record increases count
    #[test]
    fn f_slide_003_record() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        assert!(sw.record(0));
        assert_eq!(sw.count(), 1);
    }

    /// F-SLIDE-004: Exceed limit rejected
    #[test]
    fn f_slide_004_exceed() {
        let mut sw = SlidingWindowRate::new(1_000_000, 3);
        sw.record(0);
        sw.record(0);
        sw.record(0);
        assert!(!sw.record(0)); // Would exceed
        assert_eq!(sw.exceeded(), 1);
    }

    /// F-SLIDE-005: Window rotation clears old counts
    #[test]
    fn f_slide_005_rotation() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(1000); // Init with timestamp 1000
        sw.record(1000);
        // Rotate through all windows (each sub-window is 100ms)
        sw.update_with_time(2_001_000); // 2 seconds later
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-006: Rate percentage
    #[test]
    fn f_slide_006_rate_percentage() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        for _ in 0..50 {
            sw.record(0);
        }
        assert!((sw.rate_percentage() - 50.0).abs() < 0.01);
    }

    /// F-SLIDE-007: Would exceed check
    #[test]
    fn f_slide_007_would_exceed() {
        let mut sw = SlidingWindowRate::new(1_000_000, 2);
        sw.record(0);
        sw.record(0);
        assert!(sw.would_exceed());
    }

    /// F-SLIDE-008: Factory per_second
    #[test]
    fn f_slide_008_per_second() {
        let sw = SlidingWindowRate::per_second(100);
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-009: Factory per_minute
    #[test]
    fn f_slide_009_per_minute() {
        let sw = SlidingWindowRate::per_minute(100);
        assert_eq!(sw.count(), 0);
    }

    /// F-SLIDE-010: Reset clears all
    #[test]
    fn f_slide_010_reset() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(0);
        sw.reset();
        assert_eq!(sw.count(), 0);
        assert_eq!(sw.exceeded(), 0);
    }

    /// F-SLIDE-011: Debug format works
    #[test]
    fn f_slide_011_debug() {
        let sw = SlidingWindowRate::new(1_000_000, 100);
        let debug = format!("{:?}", sw);
        assert!(debug.contains("SlidingWindowRate"));
    }

    /// F-SLIDE-012: Clone preserves state
    #[test]
    fn f_slide_012_clone() {
        let mut sw = SlidingWindowRate::new(1_000_000, 100);
        sw.record(0);
        let cloned = sw.clone();
        assert_eq!(sw.count(), cloned.count());
    }

    // ========================================================================
    // ResourcePool Falsification Tests (F-POOL-001 to F-POOL-012)
    // ========================================================================

    /// F-POOL-001: New creates empty pool
    #[test]
    fn f_pool_001_new() {
        let pool = ResourcePool::new(10);
        assert!(pool.is_idle());
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-002: Default capacity 10
    #[test]
    fn f_pool_002_default() {
        let pool = ResourcePool::default();
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-003: Acquire reduces available
    #[test]
    fn f_pool_003_acquire() {
        let mut pool = ResourcePool::new(10);
        assert!(pool.acquire(100));
        assert_eq!(pool.available(), 9);
    }

    /// F-POOL-004: Release increases available
    #[test]
    fn f_pool_004_release() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.release();
        assert_eq!(pool.available(), 10);
    }

    /// F-POOL-005: Exhausted pool rejects acquire
    #[test]
    fn f_pool_005_exhausted() {
        let mut pool = ResourcePool::new(1);
        pool.acquire(100);
        assert!(!pool.acquire(100));
        assert!(pool.is_exhausted());
    }

    /// F-POOL-006: Utilization percentage
    #[test]
    fn f_pool_006_utilization() {
        let mut pool = ResourcePool::new(10);
        for _ in 0..5 {
            pool.acquire(100);
        }
        assert!((pool.utilization() - 50.0).abs() < 0.01);
    }

    /// F-POOL-007: Average wait time
    #[test]
    fn f_pool_007_avg_wait() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(1000);
        pool.acquire(2000);
        assert!((pool.avg_wait_us() - 1500.0).abs() < 0.01);
    }

    /// F-POOL-008: Timeout rate
    #[test]
    fn f_pool_008_timeout_rate() {
        let mut pool = ResourcePool::new(1);
        pool.acquire(100);
        pool.acquire(100); // timeout
        pool.acquire(100); // timeout
                           // 1 success, 2 timeouts = 66.67% timeout rate
        assert!(pool.timeout_rate() > 60.0);
    }

    /// F-POOL-009: Peak utilization
    #[test]
    fn f_pool_009_peak() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.acquire(100);
        pool.acquire(100);
        pool.release();
        assert!((pool.peak_utilization() - 30.0).abs() < 0.01);
    }

    /// F-POOL-010: Factory for_database
    #[test]
    fn f_pool_010_for_database() {
        let pool = ResourcePool::for_database();
        assert_eq!(pool.available(), 20);
    }

    /// F-POOL-011: Factory for_http
    #[test]
    fn f_pool_011_for_http() {
        let pool = ResourcePool::for_http();
        assert_eq!(pool.available(), 100);
    }

    /// F-POOL-012: Reset clears counters
    #[test]
    fn f_pool_012_reset() {
        let mut pool = ResourcePool::new(10);
        pool.acquire(100);
        pool.reset();
        assert!(pool.is_idle());
    }

    // ========================================================================
    // Histogram2D Falsification Tests (F-HIST2D-001 to F-HIST2D-012)
    // ========================================================================

    /// F-HIST2D-001: New creates empty histogram
    #[test]
    fn f_hist2d_001_new() {
        let h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-002: Default 0-100 range
    #[test]
    fn f_hist2d_002_default() {
        let h = Histogram2D::default();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-003: Add increases count
    #[test]
    fn f_hist2d_003_add() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        assert_eq!(h.count(), 1);
    }

    /// F-HIST2D-004: Get returns cell count
    #[test]
    fn f_hist2d_004_get() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        assert_eq!(h.get(5, 5), 1);
    }

    /// F-HIST2D-005: Density percentage
    #[test]
    fn f_hist2d_005_density() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        assert!((h.density(5, 5) - 100.0).abs() < 0.01);
    }

    /// F-HIST2D-006: Max count
    #[test]
    fn f_hist2d_006_max_count() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        h.add(10.0, 10.0);
        assert_eq!(h.max_count(), 2);
    }

    /// F-HIST2D-007: Hotspot detection
    #[test]
    fn f_hist2d_007_hotspot() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.add(50.0, 50.0);
        assert_eq!(h.hotspot(), (5, 5));
    }

    /// F-HIST2D-008: Factory for_latency_throughput
    #[test]
    fn f_hist2d_008_for_latency() {
        let h = Histogram2D::for_latency_throughput();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-009: Factory for_cpu_memory
    #[test]
    fn f_hist2d_009_for_cpu() {
        let h = Histogram2D::for_cpu_memory();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-010: Reset clears cells
    #[test]
    fn f_hist2d_010_reset() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        h.reset();
        assert_eq!(h.count(), 0);
    }

    /// F-HIST2D-011: Debug format works
    #[test]
    fn f_hist2d_011_debug() {
        let h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        let debug = format!("{:?}", h);
        assert!(debug.contains("Histogram2D"));
    }

    /// F-HIST2D-012: Clone preserves state
    #[test]
    fn f_hist2d_012_clone() {
        let mut h = Histogram2D::new(0.0, 100.0, 0.0, 100.0);
        h.add(50.0, 50.0);
        let cloned = h.clone();
        assert_eq!(h.count(), cloned.count());
    }

    // ========================================================================
    // ReservoirSampler Falsification Tests (F-RESERVOIR-001 to F-RESERVOIR-012)
    // ========================================================================

    /// F-RESERVOIR-001: New creates empty sampler
    #[test]
    fn f_reservoir_001_new() {
        let s = ReservoirSampler::new(10);
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
    }

    /// F-RESERVOIR-002: Default capacity 16
    #[test]
    fn f_reservoir_002_default() {
        let s = ReservoirSampler::default();
        assert!(s.is_empty());
    }

    /// F-RESERVOIR-003: Add fills sample
    #[test]
    fn f_reservoir_003_add() {
        let mut s = ReservoirSampler::new(10);
        s.add(1.0);
        s.add(2.0);
        assert_eq!(s.len(), 2);
    }

    /// F-RESERVOIR-004: Get returns sample
    #[test]
    fn f_reservoir_004_get() {
        let mut s = ReservoirSampler::new(10);
        s.add(42.0);
        assert_eq!(s.get(0), Some(42.0));
    }

    /// F-RESERVOIR-005: Total seen tracks all
    #[test]
    fn f_reservoir_005_total_seen() {
        let mut s = ReservoirSampler::new(2);
        s.add(1.0);
        s.add(2.0);
        s.add(3.0);
        assert_eq!(s.total_seen(), 3);
        assert_eq!(s.len(), 2);
    }

    /// F-RESERVOIR-006: Mean calculation
    #[test]
    fn f_reservoir_006_mean() {
        let mut s = ReservoirSampler::new(10);
        s.add(10.0);
        s.add(20.0);
        assert!((s.mean() - 15.0).abs() < 0.01);
    }

    /// F-RESERVOIR-007: Min tracking
    #[test]
    fn f_reservoir_007_min() {
        let mut s = ReservoirSampler::new(10);
        s.add(30.0);
        s.add(10.0);
        s.add(20.0);
        assert!((s.min() - 10.0).abs() < 0.01);
    }

    /// F-RESERVOIR-008: Max tracking
    #[test]
    fn f_reservoir_008_max() {
        let mut s = ReservoirSampler::new(10);
        s.add(10.0);
        s.add(30.0);
        s.add(20.0);
        assert!((s.max() - 30.0).abs() < 0.01);
    }

    /// F-RESERVOIR-009: Get out of bounds returns None
    #[test]
    fn f_reservoir_009_oob() {
        let s = ReservoirSampler::new(10);
        assert_eq!(s.get(0), None);
    }

    /// F-RESERVOIR-010: Reset clears samples
    #[test]
    fn f_reservoir_010_reset() {
        let mut s = ReservoirSampler::new(10);
        s.add(1.0);
        s.reset();
        assert!(s.is_empty());
    }

    /// F-RESERVOIR-011: Debug format works
    #[test]
    fn f_reservoir_011_debug() {
        let s = ReservoirSampler::new(10);
        let debug = format!("{:?}", s);
        assert!(debug.contains("ReservoirSampler"));
    }

    /// F-RESERVOIR-012: Clone preserves state
    #[test]
    fn f_reservoir_012_clone() {
        let mut s = ReservoirSampler::new(10);
        s.add(42.0);
        let cloned = s.clone();
        assert_eq!(s.len(), cloned.len());
    }

    // ========================================================================
    // ExponentialHistogram Falsification Tests (F-EXPHIST-001 to F-EXPHIST-012)
    // ========================================================================

    /// F-EXPHIST-001: New creates empty histogram
    #[test]
    fn f_exphist_001_new() {
        let h = ExponentialHistogram::new(1.0);
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-002: Default base 1.0
    #[test]
    fn f_exphist_002_default() {
        let h = ExponentialHistogram::default();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-003: Add increases count
    #[test]
    fn f_exphist_003_add() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        assert_eq!(h.count(), 1);
    }

    /// F-EXPHIST-004: Bucket assignment
    #[test]
    fn f_exphist_004_bucket() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(0.5); // bucket 0
        h.add(1.5); // bucket 0
        h.add(3.0); // bucket 1
        h.add(5.0); // bucket 2
        assert!(h.bucket_count(0) >= 1);
    }

    /// F-EXPHIST-005: Mean calculation
    #[test]
    fn f_exphist_005_mean() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(10.0);
        h.add(20.0);
        assert!((h.mean() - 15.0).abs() < 0.01);
    }

    /// F-EXPHIST-006: Mode bucket
    #[test]
    fn f_exphist_006_mode() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(0.5);
        h.add(0.6);
        h.add(0.7);
        h.add(10.0);
        assert_eq!(h.mode_bucket(), 0);
    }

    /// F-EXPHIST-007: Factory for_latency_ms
    #[test]
    fn f_exphist_007_for_latency() {
        let h = ExponentialHistogram::for_latency_ms();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-008: Factory for_bytes_kb
    #[test]
    fn f_exphist_008_for_bytes() {
        let h = ExponentialHistogram::for_bytes_kb();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-009: Bucket upper bound
    #[test]
    fn f_exphist_009_upper_bound() {
        let h = ExponentialHistogram::new(1.0);
        assert!((h.bucket_upper_bound(0) - 2.0).abs() < 0.01);
        assert!((h.bucket_upper_bound(1) - 4.0).abs() < 0.01);
    }

    /// F-EXPHIST-010: Reset clears histogram
    #[test]
    fn f_exphist_010_reset() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        h.reset();
        assert_eq!(h.count(), 0);
    }

    /// F-EXPHIST-011: Debug format works
    #[test]
    fn f_exphist_011_debug() {
        let h = ExponentialHistogram::new(1.0);
        let debug = format!("{:?}", h);
        assert!(debug.contains("ExponentialHistogram"));
    }

    /// F-EXPHIST-012: Clone preserves state
    #[test]
    fn f_exphist_012_clone() {
        let mut h = ExponentialHistogram::new(1.0);
        h.add(5.0);
        let cloned = h.clone();
        assert_eq!(h.count(), cloned.count());
    }

    // ========================================================================
    // CacheStats Falsification Tests (F-CACHE-001 to F-CACHE-012)
    // ========================================================================

    /// F-CACHE-001: New creates empty stats
    #[test]
    fn f_cache_001_new() {
        let cs = CacheStats::new(1024);
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-002: Default zero capacity
    #[test]
    fn f_cache_002_default() {
        let cs = CacheStats::default();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-003: Hit increases count
    #[test]
    fn f_cache_003_hit() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        assert_eq!(cs.total_requests(), 1);
    }

    /// F-CACHE-004: Miss increases count
    #[test]
    fn f_cache_004_miss() {
        let mut cs = CacheStats::new(1024);
        cs.miss();
        assert_eq!(cs.total_requests(), 1);
    }

    /// F-CACHE-005: Hit rate calculation
    #[test]
    fn f_cache_005_hit_rate() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.hit();
        cs.miss();
        // 2 hits / 3 total = 66.67%
        assert!(cs.hit_rate() > 60.0);
    }

    /// F-CACHE-006: Miss rate calculation
    #[test]
    fn f_cache_006_miss_rate() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.miss();
        assert!((cs.miss_rate() - 50.0).abs() < 0.01);
    }

    /// F-CACHE-007: Eviction tracking
    #[test]
    fn f_cache_007_eviction() {
        let mut cs = CacheStats::new(1024);
        cs.insert(512);
        cs.evict(256);
        assert!(cs.eviction_rate() > 0.0);
    }

    /// F-CACHE-008: Fill percentage
    #[test]
    fn f_cache_008_fill() {
        let mut cs = CacheStats::new(1024);
        cs.insert(512);
        assert!((cs.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-CACHE-009: Factory for_l1_cache
    #[test]
    fn f_cache_009_for_l1() {
        let cs = CacheStats::for_l1_cache();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-010: Factory for_app_cache
    #[test]
    fn f_cache_010_for_app() {
        let cs = CacheStats::for_app_cache();
        assert_eq!(cs.total_requests(), 0);
    }

    /// F-CACHE-011: Is effective check
    #[test]
    fn f_cache_011_effective() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.hit();
        cs.miss();
        assert!(cs.is_effective(60.0));
    }

    /// F-CACHE-012: Reset clears stats
    #[test]
    fn f_cache_012_reset() {
        let mut cs = CacheStats::new(1024);
        cs.hit();
        cs.reset();
        assert_eq!(cs.total_requests(), 0);
    }

    // ========================================================================
    // BloomFilter Falsification Tests (F-BLOOM-001 to F-BLOOM-012)
    // ========================================================================

    /// F-BLOOM-001: New creates empty filter
    #[test]
    fn f_bloom_001_new() {
        let bf = BloomFilter::new(3);
        assert!(bf.is_empty());
    }

    /// F-BLOOM-002: Default 3 hashes
    #[test]
    fn f_bloom_002_default() {
        let bf = BloomFilter::default();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-003: Add increases len
    #[test]
    fn f_bloom_003_add() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        assert_eq!(bf.len(), 1);
    }

    /// F-BLOOM-004: Might contain returns true for added
    #[test]
    fn f_bloom_004_contains() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        assert!(bf.might_contain(42));
    }

    /// F-BLOOM-005: Might contain returns false for not added
    #[test]
    fn f_bloom_005_not_contains() {
        let bf = BloomFilter::new(3);
        // Empty filter should not contain anything
        assert!(!bf.might_contain(12345));
    }

    /// F-BLOOM-006: Fill percentage increases
    #[test]
    fn f_bloom_006_fill() {
        let mut bf = BloomFilter::new(3);
        bf.add(1);
        bf.add(2);
        bf.add(3);
        assert!(bf.fill_percentage() > 0.0);
    }

    /// F-BLOOM-007: False positive rate estimation
    #[test]
    fn f_bloom_007_fpr() {
        let mut bf = BloomFilter::new(3);
        for i in 0..100 {
            bf.add(i);
        }
        // With 100 items in 1024 bits, FPR should be low but positive
        assert!(bf.false_positive_rate() > 0.0);
    }

    /// F-BLOOM-008: Factory for_small
    #[test]
    fn f_bloom_008_for_small() {
        let bf = BloomFilter::for_small();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-009: Factory for_medium
    #[test]
    fn f_bloom_009_for_medium() {
        let bf = BloomFilter::for_medium();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-010: Reset clears filter
    #[test]
    fn f_bloom_010_reset() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        bf.reset();
        assert!(bf.is_empty());
    }

    /// F-BLOOM-011: Debug format works
    #[test]
    fn f_bloom_011_debug() {
        let bf = BloomFilter::new(3);
        let debug = format!("{:?}", bf);
        assert!(debug.contains("BloomFilter"));
    }

    /// F-BLOOM-012: Clone preserves state
    #[test]
    fn f_bloom_012_clone() {
        let mut bf = BloomFilter::new(3);
        bf.add(42);
        let cloned = bf.clone();
        assert_eq!(bf.len(), cloned.len());
    }

    // ========================================================================
    // LoadBalancer Falsification Tests (F-LB-001 to F-LB-012)
    // ========================================================================

    /// F-LB-001: New creates empty balancer
    #[test]
    fn f_lb_001_new() {
        let lb = LoadBalancer::new();
        assert_eq!(lb.backend_count(), 0);
    }

    /// F-LB-002: Default same as new
    #[test]
    fn f_lb_002_default() {
        let lb = LoadBalancer::default();
        assert_eq!(lb.backend_count(), 0);
    }

    /// F-LB-003: Add backend increases count
    #[test]
    fn f_lb_003_add_backend() {
        let mut lb = LoadBalancer::new();
        lb.add_backend(1);
        assert_eq!(lb.backend_count(), 1);
    }

    /// F-LB-004: Next returns backend
    #[test]
    fn f_lb_004_next() {
        let mut lb = LoadBalancer::new();
        lb.add_backend(1);
        assert_eq!(lb.select_backend(), Some(0));
    }

    /// F-LB-005: Empty balancer returns None
    #[test]
    fn f_lb_005_empty_next() {
        let mut lb = LoadBalancer::new();
        assert_eq!(lb.select_backend(), None);
    }

    /// F-LB-006: Equal weights distributes evenly
    #[test]
    fn f_lb_006_equal_weights() {
        let mut lb = LoadBalancer::equal_weights(2);
        for _ in 0..10 {
            let _ = lb.select_backend();
        }
        // Both backends should get ~50%
        assert!(lb.distribution(0) > 40.0);
        assert!(lb.distribution(1) > 40.0);
    }

    /// F-LB-007: Total dispatched tracked
    #[test]
    fn f_lb_007_dispatched() {
        let mut lb = LoadBalancer::equal_weights(2);
        let _ = lb.select_backend();
        let _ = lb.select_backend();
        let _ = lb.select_backend();
        assert_eq!(lb.total_dispatched(), 3);
    }

    /// F-LB-008: Is balanced check
    #[test]
    fn f_lb_008_balanced() {
        let mut lb = LoadBalancer::equal_weights(2);
        for _ in 0..100 {
            let _ = lb.select_backend();
        }
        assert!(lb.is_balanced(20.0));
    }

    /// F-LB-009: Distribution percentage
    #[test]
    fn f_lb_009_distribution() {
        let mut lb = LoadBalancer::equal_weights(1);
        let _ = lb.select_backend();
        assert!((lb.distribution(0) - 100.0).abs() < 0.01);
    }

    /// F-LB-010: Reset clears counters
    #[test]
    fn f_lb_010_reset() {
        let mut lb = LoadBalancer::equal_weights(2);
        let _ = lb.select_backend();
        lb.reset();
        assert_eq!(lb.total_dispatched(), 0);
    }

    /// F-LB-011: Debug format works
    #[test]
    fn f_lb_011_debug() {
        let lb = LoadBalancer::new();
        let debug = format!("{:?}", lb);
        assert!(debug.contains("LoadBalancer"));
    }

    /// F-LB-012: Clone preserves state
    #[test]
    fn f_lb_012_clone() {
        let mut lb = LoadBalancer::equal_weights(2);
        let _ = lb.select_backend();
        let cloned = lb.clone();
        assert_eq!(lb.total_dispatched(), cloned.total_dispatched());
    }

    // ========================================================================
    // BurstTracker Falsification Tests (F-BURST-001 to F-BURST-012)
    // ========================================================================

    /// F-BURST-001: New creates full bucket
    #[test]
    fn f_burst_001_new() {
        let bt = BurstTracker::new(100.0, 10.0);
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-002: Default 100 capacity
    #[test]
    fn f_burst_002_default() {
        let bt = BurstTracker::default();
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-003: Consume reduces tokens
    #[test]
    fn f_burst_003_consume() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        assert!(bt.consume(10.0, 1000));
        assert!((bt.tokens() - 90.0).abs() < 0.01);
    }

    /// F-BURST-004: Consume returns false when empty
    #[test]
    fn f_burst_004_empty() {
        let mut bt = BurstTracker::new(10.0, 1.0);
        bt.consume(10.0, 1000);
        assert!(!bt.consume(10.0, 1000));
    }

    /// F-BURST-005: Max burst tracked
    #[test]
    fn f_burst_005_max_burst() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(1.0, 1000);
        bt.consume(1.0, 1000);
        bt.consume(1.0, 1000);
        assert_eq!(bt.max_burst(), 3);
    }

    /// F-BURST-006: Fill percentage
    #[test]
    fn f_burst_006_fill() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        assert!((bt.fill_percentage() - 50.0).abs() < 0.01);
    }

    /// F-BURST-007: Factory for_api
    #[test]
    fn f_burst_007_for_api() {
        let bt = BurstTracker::for_api();
        assert!(bt.tokens() > 0.0);
    }

    /// F-BURST-008: Factory for_network
    #[test]
    fn f_burst_008_for_network() {
        let bt = BurstTracker::for_network();
        assert!(bt.tokens() > 0.0);
    }

    /// F-BURST-009: Refill over time
    #[test]
    fn f_burst_009_refill() {
        let mut bt = BurstTracker::new(100.0, 100.0);
        bt.consume(50.0, 1000);
        bt.consume(0.0, 1_001_000); // 1 second later
                                    // Should have refilled 100 tokens (capped at capacity)
        assert!(bt.tokens() > 50.0);
    }

    /// F-BURST-010: Reset restores capacity
    #[test]
    fn f_burst_010_reset() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        bt.reset();
        assert!((bt.tokens() - 100.0).abs() < 0.01);
    }

    /// F-BURST-011: Debug format works
    #[test]
    fn f_burst_011_debug() {
        let bt = BurstTracker::new(100.0, 10.0);
        let debug = format!("{:?}", bt);
        assert!(debug.contains("BurstTracker"));
    }

    /// F-BURST-012: Clone preserves state
    #[test]
    fn f_burst_012_clone() {
        let mut bt = BurstTracker::new(100.0, 10.0);
        bt.consume(50.0, 1000);
        let cloned = bt.clone();
        assert!((bt.tokens() - cloned.tokens()).abs() < 0.01);
    }

    // ========================================================================
    // TopKTracker Falsification Tests (F-TOPK-001 to F-TOPK-012)
    // ========================================================================

    /// F-TOPK-001: New creates empty tracker
    #[test]
    fn f_topk_001_new() {
        let tk = TopKTracker::new(5);
        assert_eq!(tk.count(), 0);
    }

    /// F-TOPK-002: Default creates k=10
    #[test]
    fn f_topk_002_default() {
        let tk = TopKTracker::default();
        assert_eq!(tk.k(), 10);
    }

    /// F-TOPK-003: Add value increases count
    #[test]
    fn f_topk_003_add() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        assert_eq!(tk.count(), 1);
    }

    /// F-TOPK-004: Top returns sorted values
    #[test]
    fn f_topk_004_top() {
        let mut tk = TopKTracker::new(3);
        tk.add(10.0);
        tk.add(30.0);
        tk.add(20.0);
        let top = tk.top();
        assert!((top[0] - 30.0).abs() < 0.01);
    }

    /// F-TOPK-005: Limited to k values
    #[test]
    fn f_topk_005_limit() {
        let mut tk = TopKTracker::new(3);
        for i in 0..10 {
            tk.add(i as f64);
        }
        assert_eq!(tk.top().len(), 3);
    }

    /// F-TOPK-006: Minimum returns smallest in top-k
    #[test]
    fn f_topk_006_minimum() {
        let mut tk = TopKTracker::new(3);
        tk.add(100.0);
        tk.add(200.0);
        tk.add(300.0);
        assert!((tk.minimum().unwrap() - 100.0).abs() < 0.01);
    }

    /// F-TOPK-007: Maximum returns largest
    #[test]
    fn f_topk_007_maximum() {
        let mut tk = TopKTracker::new(3);
        tk.add(100.0);
        tk.add(200.0);
        tk.add(300.0);
        assert!((tk.maximum().unwrap() - 300.0).abs() < 0.01);
    }

    /// F-TOPK-008: Factory for_metrics
    #[test]
    fn f_topk_008_for_metrics() {
        let tk = TopKTracker::for_metrics();
        assert_eq!(tk.k(), 10);
    }

    /// F-TOPK-009: Factory for_processes
    #[test]
    fn f_topk_009_for_processes() {
        let tk = TopKTracker::for_processes();
        assert_eq!(tk.k(), 20);
    }

    /// F-TOPK-010: Reset clears values
    #[test]
    fn f_topk_010_reset() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        tk.reset();
        assert_eq!(tk.count(), 0);
    }

    /// F-TOPK-011: Debug format works
    #[test]
    fn f_topk_011_debug() {
        let tk = TopKTracker::new(5);
        let debug = format!("{:?}", tk);
        assert!(debug.contains("TopKTracker"));
    }

    /// F-TOPK-012: Clone preserves state
    #[test]
    fn f_topk_012_clone() {
        let mut tk = TopKTracker::new(5);
        tk.add(10.0);
        let cloned = tk.clone();
        assert_eq!(tk.count(), cloned.count());
    }

    // ========================================================================
    // QuotaTracker Falsification Tests (F-QUOTA-001 to F-QUOTA-012)
    // ========================================================================

    /// F-QUOTA-001: New creates tracker with limit
    #[test]
    fn f_quota_001_new() {
        let qt = QuotaTracker::new(1000);
        assert_eq!(qt.limit(), 1000);
    }

    /// F-QUOTA-002: Default creates 1000 limit
    #[test]
    fn f_quota_002_default() {
        let qt = QuotaTracker::default();
        assert_eq!(qt.limit(), 1000);
    }

    /// F-QUOTA-003: Use reduces remaining
    #[test]
    fn f_quota_003_use() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(30);
        assert_eq!(qt.remaining(), 70);
    }

    /// F-QUOTA-004: Use returns false when exceeded
    #[test]
    fn f_quota_004_exceeded() {
        let mut qt = QuotaTracker::new(100);
        assert!(!qt.use_quota(150));
    }

    /// F-QUOTA-005: Usage percentage
    #[test]
    fn f_quota_005_usage() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        assert!((qt.usage_percentage() - 50.0).abs() < 0.01);
    }

    /// F-QUOTA-006: Is exhausted check
    #[test]
    fn f_quota_006_exhausted() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(100);
        assert!(qt.is_exhausted());
    }

    /// F-QUOTA-007: Factory for_api_daily
    #[test]
    fn f_quota_007_for_api() {
        let qt = QuotaTracker::for_api_daily();
        assert_eq!(qt.limit(), 10000);
    }

    /// F-QUOTA-008: Factory for_storage_gb
    #[test]
    fn f_quota_008_for_storage() {
        let qt = QuotaTracker::for_storage_gb();
        assert_eq!(qt.limit(), 100);
    }

    /// F-QUOTA-009: Release returns quota
    #[test]
    fn f_quota_009_release() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        qt.release(20);
        assert_eq!(qt.remaining(), 70);
    }

    /// F-QUOTA-010: Reset restores full quota
    #[test]
    fn f_quota_010_reset() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(50);
        qt.reset();
        assert_eq!(qt.remaining(), 100);
    }

    /// F-QUOTA-011: Debug format works
    #[test]
    fn f_quota_011_debug() {
        let qt = QuotaTracker::new(100);
        let debug = format!("{:?}", qt);
        assert!(debug.contains("QuotaTracker"));
    }

    /// F-QUOTA-012: Clone preserves state
    #[test]
    fn f_quota_012_clone() {
        let mut qt = QuotaTracker::new(100);
        qt.use_quota(30);
        let cloned = qt.clone();
        assert_eq!(qt.remaining(), cloned.remaining());
    }

    // ========================================================================
    // FrequencyCounter Falsification Tests (F-FREQ-001 to F-FREQ-012)
    // ========================================================================

    /// F-FREQ-001: New creates empty counter
    #[test]
    fn f_freq_001_new() {
        let fc = FrequencyCounter::new();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-002: Default same as new
    #[test]
    fn f_freq_002_default() {
        let fc = FrequencyCounter::default();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-003: Increment increases count
    #[test]
    fn f_freq_003_increment() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        assert_eq!(fc.count(0), 1);
    }

    /// F-FREQ-004: Frequency calculation
    #[test]
    fn f_freq_004_frequency() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(0);
        fc.increment(1);
        assert!((fc.frequency(0) - 66.666).abs() < 1.0);
    }

    /// F-FREQ-005: Most frequent returns max
    #[test]
    fn f_freq_005_most_frequent() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(1);
        fc.increment(1);
        assert_eq!(fc.most_frequent(), Some(1));
    }

    /// F-FREQ-006: 16 slots available
    #[test]
    fn f_freq_006_slots() {
        let mut fc = FrequencyCounter::new();
        for i in 0..16 {
            fc.increment(i);
        }
        assert_eq!(fc.total(), 16);
    }

    /// F-FREQ-007: Non-zero slots counted
    #[test]
    fn f_freq_007_non_zero() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.increment(5);
        assert_eq!(fc.non_zero_count(), 2);
    }

    /// F-FREQ-008: Add multiple at once
    #[test]
    fn f_freq_008_add() {
        let mut fc = FrequencyCounter::new();
        fc.add(0, 10);
        assert_eq!(fc.count(0), 10);
    }

    /// F-FREQ-009: Entropy calculation
    #[test]
    fn f_freq_009_entropy() {
        let mut fc = FrequencyCounter::new();
        // Uniform distribution across all 16 categories for max entropy
        for i in 0..16 {
            fc.add(i, 10);
        }
        // 16 uniform categories = log2(16) / log2(16) = 1.0 normalized
        assert!(fc.entropy() > 0.9);
    }

    /// F-FREQ-010: Reset clears counts
    #[test]
    fn f_freq_010_reset() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        fc.reset();
        assert_eq!(fc.total(), 0);
    }

    /// F-FREQ-011: Debug format works
    #[test]
    fn f_freq_011_debug() {
        let fc = FrequencyCounter::new();
        let debug = format!("{:?}", fc);
        assert!(debug.contains("FrequencyCounter"));
    }

    /// F-FREQ-012: Clone preserves state
    #[test]
    fn f_freq_012_clone() {
        let mut fc = FrequencyCounter::new();
        fc.increment(0);
        let cloned = fc.clone();
        assert_eq!(fc.total(), cloned.total());
    }

    // ========================================================================
    // MovingRange Falsification Tests (F-RANGE-001 to F-RANGE-012)
    // ========================================================================

    /// F-RANGE-001: New creates empty tracker
    #[test]
    fn f_range_001_new() {
        let mr = MovingRange::new(10);
        assert_eq!(mr.count(), 0);
    }

    /// F-RANGE-002: Default window of 10
    #[test]
    fn f_range_002_default() {
        let mr = MovingRange::default();
        assert_eq!(mr.window_size(), 10);
    }

    /// F-RANGE-003: Add updates min/max
    #[test]
    fn f_range_003_add() {
        let mut mr = MovingRange::new(10);
        mr.add(50.0);
