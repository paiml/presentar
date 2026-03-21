mod tests {
    use super::*;

    // RingBuffer tests
    #[test]
    fn test_ring_buffer() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        buf.push(1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.as_slice(), &[1, 2, 3]);

        buf.push(4);
        assert_eq!(buf.as_slice(), &[2, 3, 4]);

        assert_eq!(buf.last(), Some(&4));
    }

    #[test]
    fn test_ring_buffer_empty() {
        let buf: RingBuffer<i32> = RingBuffer::new(5);
        assert!(buf.as_slice().is_empty());
        assert_eq!(buf.last(), None);
    }

    #[test]
    fn test_ring_buffer_single_element() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(1);
        buf.push(42);
        assert_eq!(buf.as_slice(), &[42]);

        buf.push(100);
        assert_eq!(buf.as_slice(), &[100]);
    }

    #[test]
    fn test_ring_buffer_many_pushes() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        for i in 0..100 {
            buf.push(i);
        }
        assert_eq!(buf.as_slice(), &[97, 98, 99]);
    }

    // ProcessSortColumn tests
    #[test]
    fn test_process_sort_column_next() {
        assert_eq!(ProcessSortColumn::Pid.next(), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::User.next(), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::Cpu.next(), ProcessSortColumn::Mem);
        assert_eq!(ProcessSortColumn::Mem.next(), ProcessSortColumn::Command);
        assert_eq!(ProcessSortColumn::Command.next(), ProcessSortColumn::Pid);
    }

    #[test]
    fn test_process_sort_column_prev() {
        assert_eq!(ProcessSortColumn::Pid.prev(), ProcessSortColumn::Command);
        assert_eq!(ProcessSortColumn::User.prev(), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::Cpu.prev(), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::Mem.prev(), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::Command.prev(), ProcessSortColumn::Mem);
    }

    #[test]
    fn test_process_sort_column_count() {
        assert_eq!(ProcessSortColumn::COUNT, 5);
    }

    #[test]
    fn test_process_sort_column_next_cycle() {
        let mut col = ProcessSortColumn::Pid;
        for _ in 0..5 {
            col = col.next();
        }
        assert_eq!(col, ProcessSortColumn::Pid); // Full cycle
    }

    #[test]
    fn test_process_sort_column_prev_cycle() {
        let mut col = ProcessSortColumn::Pid;
        for _ in 0..5 {
            col = col.prev();
        }
        assert_eq!(col, ProcessSortColumn::Pid); // Full cycle
    }

    // PanelVisibility tests
    #[test]
    fn test_panel_visibility_default() {
        let panels = PanelVisibility::default();
        assert!(panels.cpu);
        assert!(panels.memory);
        assert!(panels.disk);
        assert!(panels.network);
        assert!(panels.process);
        assert!(!panels.gpu);
        assert!(!panels.sensors);
        assert!(!panels.psi);
        assert!(!panels.connections);
        assert!(!panels.battery);
        assert!(!panels.sensors_compact);
        assert!(!panels.system);
        assert!(!panels.treemap);
        assert!(!panels.files);
    }

    #[test]
    fn test_panel_visibility_all_fields() {
        let panels = PanelVisibility {
            cpu: true,
            memory: true,
            disk: true,
            network: true,
            process: true,
            gpu: true,
            sensors: true,
            psi: true,
            connections: true,
            battery: true,
            sensors_compact: true,
            system: true,
            treemap: true,
            files: true,
        };
        assert!(panels.cpu && panels.gpu && panels.treemap && panels.files);
    }

    // App tests
    #[test]
    fn test_app_normal_mode() {
        let app = App::new(false);
        assert!(!app.deterministic);
        assert!(!app.show_fps);
    }

    #[test]
    fn test_app_deterministic_mode() {
        let app = App::new(true);
        assert!(app.deterministic);

        // Check fixed values - ttop deterministic mode uses 48 cores, all zeros
        assert_eq!(app.per_core_percent.len(), 48);
        // ttop deterministic: all memory values are 0
        assert_eq!(app.mem_total, 0);
        assert_eq!(app.mem_used, 0);
        assert_eq!(app.swap_total, 0);

        // Check fixed uptime (5 days, 3 hours, 47 minutes)
        assert_eq!(app.uptime(), 5 * 86400 + 3 * 3600 + 47 * 60);

        // Check history is pre-populated with 60 zeros
        assert_eq!(app.cpu_history.as_slice().len(), 60);
        assert_eq!(app.mem_history.as_slice().len(), 60);
    }

    #[test]
    fn test_deterministic_mode_collect_metrics_noop() {
        let mut app = App::new(true);
        let initial_frame_id = app.frame_id;
        let initial_cpu_history_len = app.cpu_history.as_slice().len();

        // Collect should only increment frame_id, not change data
        app.collect_metrics();

        assert_eq!(app.frame_id, initial_frame_id + 1);
        // History should NOT grow (deterministic mode skips collection)
        assert_eq!(app.cpu_history.as_slice().len(), initial_cpu_history_len);
    }

    #[test]
    fn test_app_process_count_deterministic() {
        let app = App::new(true);
        // Deterministic mode returns 0 processes
        assert_eq!(app.process_count(), 0);
    }

    #[test]
    fn test_app_sorted_processes_deterministic() {
        let app = App::new(true);
        // Deterministic mode returns empty process list
        assert!(app.sorted_processes().is_empty());
    }

    #[test]
    fn test_app_focus_panel() {
        let mut app = App::new(true);
        // Default is CPU focused
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));

        app.focused_panel = Some(PanelType::Memory);
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        app.focused_panel = None;
        assert!(app.focused_panel.is_none());
    }

    #[test]
    fn test_app_is_panel_focused() {
        let mut app = App::new(true);
        // Default is CPU focused
        assert!(app.is_panel_focused(PanelType::Cpu));
        assert!(!app.is_panel_focused(PanelType::Memory));

        app.focused_panel = Some(PanelType::Memory);
        assert!(!app.is_panel_focused(PanelType::Cpu));
        assert!(app.is_panel_focused(PanelType::Memory));
    }

    #[test]
    fn test_app_sort_column_toggle() {
        let mut app = App::new(true);
        assert_eq!(app.sort_column, ProcessSortColumn::Cpu);

        app.sort_column = app.sort_column.next();
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
    }

    #[test]
    fn test_app_sort_descending_toggle() {
        let mut app = App::new(true);
        // Default is descending (highest first)
        assert!(app.sort_descending);

        app.sort_descending = false;
        assert!(!app.sort_descending);
    }

    #[test]
    fn test_app_filter_field_assignment() {
        // NOTE: This only tests the filter FIELD, not actual process filtering.
        // For actual filtering tests, see falsification_tests.rs:
        // - falsify_filter_does_not_reduce_count
        // - falsify_filter_does_not_match_known_process
        let mut app = App::new(true);
        assert!(app.filter.is_empty());

        app.filter = "test".to_string();
        assert_eq!(app.filter, "test");
    }

    #[test]
    fn test_app_update_frame_stats() {
        let mut app = App::new(true);
        app.update_frame_stats(&[
            Duration::from_micros(1000),
            Duration::from_micros(2000),
            Duration::from_micros(3000),
        ]);
        assert_eq!(app.avg_frame_time_us, 2000);
    }

    #[test]
    fn test_app_update_frame_stats_empty() {
        let mut app = App::new(true);
        app.avg_frame_time_us = 1234;
        app.update_frame_stats(&[]);
        // Should not change when empty
        assert_eq!(app.avg_frame_time_us, 1234);
    }

    #[test]
    fn test_app_request_signal_deterministic_noop() {
        // NOTE: This test only verifies no-op behavior in deterministic mode (no processes).
        // For actual signal request testing, see falsification_tests.rs:
        // - falsify_request_signal_sets_pending
        let mut app = App::new(true);
        // Deterministic mode has no processes, so this should be a no-op
        app.request_signal(SignalType::Term);
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_app_cancel_signal() {
        let mut app = App::new(true);
        app.pending_signal = Some((123, "test".to_string(), SignalType::Term));
        app.cancel_signal();
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_app_data_availability_deterministic() {
        let app = App::new(true);
        let avail = app.data_availability();
        // Deterministic mode has no optional data
        assert!(!avail.psi_available);
        assert!(!avail.gpu_available);
        assert!(!avail.treemap_ready);
    }

    #[test]
    fn test_app_apply_snapshot() {
        let mut app = App::new(true);
        let initial_frame = app.frame_id;

        let snapshot = MetricsSnapshot {
            per_core_percent: vec![25.0; 4],
            per_core_freq: vec![2000; 4],
            per_core_temp: vec![50.0; 4],
            cpu_avg: 25.0,
            load_avg: sysinfo::LoadAvg {
                one: 1.0,
                five: 0.5,
                fifteen: 0.25,
            },
            mem_total: 16 * 1024 * 1024 * 1024,
            mem_used: 8 * 1024 * 1024 * 1024,
            mem_available: 8 * 1024 * 1024 * 1024,
            mem_cached: 2 * 1024 * 1024 * 1024,
            swap_total: 4 * 1024 * 1024 * 1024,
            swap_used: 0,
            net_rx: 1000,
            net_tx: 500,
            gpu_info: None,
            processes: vec![],
            disk_info: vec![],
            network_info: vec![],
            psi_data: None,
            connections_data: None,
            treemap_data: None,
            sensor_health_data: None,
            disk_io_data: None,
            disk_entropy_data: None,
            file_analyzer_data: None,
        };

        app.apply_snapshot(snapshot);

        assert_eq!(app.frame_id, initial_frame + 1);
        assert_eq!(app.per_core_percent.len(), 4);
        assert_eq!(app.mem_total, 16 * 1024 * 1024 * 1024);
    }

    #[test]
    fn test_detail_level_for_panel() {
        let app = App::new(true);
        let level = app.detail_level_for_panel(PanelType::Cpu, 20);
        // Height 20 should give some detail level
        assert!(!matches!(level, DetailLevel::Minimal));
    }

    // =========================================================================
    // MetricsSnapshot TESTS
    // =========================================================================

    #[test]
    fn test_metrics_snapshot_empty() {
        let snap = MetricsSnapshot::empty();
        assert!((snap.cpu_avg - 0.0).abs() < f64::EPSILON);
        assert!(snap.per_core_percent.is_empty());
        assert!(snap.per_core_freq.is_empty());
        assert!(snap.per_core_temp.is_empty());
        assert_eq!(snap.mem_total, 0);
        assert_eq!(snap.mem_used, 0);
        assert_eq!(snap.swap_total, 0);
        assert_eq!(snap.net_rx, 0);
        assert_eq!(snap.net_tx, 0);
        assert!(snap.gpu_info.is_none());
        assert!(snap.processes.is_empty());
        assert!(snap.disk_info.is_empty());
        assert!(snap.network_info.is_empty());
        assert!(snap.psi_data.is_none());
        assert!(snap.connections_data.is_none());
        assert!(snap.treemap_data.is_none());
    }

    #[test]
    fn test_metrics_snapshot_clone() {
        let snap = MetricsSnapshot::empty();
        let snap2 = snap.clone();
        assert_eq!(snap2.mem_total, 0);
    }

    #[test]
    fn test_metrics_snapshot_with_values() {
        let snap = MetricsSnapshot {
            cpu_avg: 50.0,
            per_core_percent: vec![25.0, 50.0, 75.0, 100.0],
            per_core_freq: vec![3000, 3200, 3400, 3600],
            per_core_temp: vec![40.0, 45.0, 50.0, 55.0],
            load_avg: sysinfo::LoadAvg {
                one: 1.5,
                five: 1.0,
                fifteen: 0.5,
            },
            mem_total: 16 * 1024 * 1024 * 1024,
            mem_used: 8 * 1024 * 1024 * 1024,
            mem_available: 8 * 1024 * 1024 * 1024,
            mem_cached: 2 * 1024 * 1024 * 1024,
            swap_total: 4 * 1024 * 1024 * 1024,
            swap_used: 1024 * 1024 * 1024,
            net_rx: 1_000_000,
            net_tx: 500_000,
            gpu_info: None,
            processes: vec![],
            disk_info: vec![],
            network_info: vec![],
            psi_data: None,
            connections_data: None,
            treemap_data: None,
            sensor_health_data: None,
            disk_io_data: None,
            disk_entropy_data: None,
            file_analyzer_data: None,
        };
        assert!((snap.cpu_avg - 50.0).abs() < f64::EPSILON);
        assert_eq!(snap.per_core_percent.len(), 4);
        assert_eq!(snap.per_core_freq[3], 3600);
        assert!((snap.per_core_temp[0] - 40.0).abs() < f32::EPSILON);
    }

    // =========================================================================
    // ProcessInfo TESTS
    // =========================================================================

    #[test]
    fn test_process_info_clone() {
        let info = ProcessInfo {
            pid: 1234,
            name: "test".to_string(),
            cpu_usage: 25.5,
            memory: 1024 * 1024,
            user: "root".to_string(),
            cmd: "/usr/bin/test".to_string(),
        };
        let info2 = info.clone();
        assert_eq!(info2.pid, 1234);
        assert_eq!(info2.name, "test");
        assert!((info2.cpu_usage - 25.5).abs() < f32::EPSILON);
    }

    // =========================================================================
    // DiskInfo TESTS
    // =========================================================================

    #[test]
    fn test_disk_info_clone() {
        let info = DiskInfo {
            name: "sda1".to_string(),
            mount_point: "/".to_string(),
            total_space: 500 * 1024 * 1024 * 1024,
            available_space: 200 * 1024 * 1024 * 1024,
            file_system: "ext4".to_string(),
        };
        let info2 = info.clone();
        assert_eq!(info2.name, "sda1");
        assert_eq!(info2.mount_point, "/");
        assert_eq!(info2.file_system, "ext4");
    }

    // =========================================================================
    // NetworkInfo TESTS
    // =========================================================================

    #[test]
    fn test_network_info_clone() {
        let info = NetworkInfo {
            name: "eth0".to_string(),
            received: 1_000_000,
            transmitted: 500_000,
        };
        let info2 = info.clone();
        assert_eq!(info2.name, "eth0");
        assert_eq!(info2.received, 1_000_000);
        assert_eq!(info2.transmitted, 500_000);
    }

    // =========================================================================
    // MetricsCollector TESTS
    // =========================================================================

    #[test]
    fn test_metrics_collector_new_deterministic() {
        let collector = MetricsCollector::new(true);
        assert!(collector.deterministic);
        assert_eq!(collector.frame_id, 0);
    }

    #[test]
    fn test_metrics_collector_has_psi_returns_bool() {
        // NOTE: has_psi() returns true if /proc/pressure/cpu exists on host.
        // Deterministic mode still detects real system capabilities.
        // For actual PSI falsification, see falsification_tests.rs.
        let collector = MetricsCollector::new(true);
        let has_psi: bool = collector.has_psi();
        // Just verify it returns a bool and doesn't panic
        let _: bool = has_psi; // type check
    }

    #[test]
    fn test_metrics_collector_has_gpu_returns_bool() {
        // NOTE: has_gpu() returns true if GPU detected on host.
        // For actual GPU falsification, see falsification_tests.rs.
        let collector = MetricsCollector::new(true);
        let has_gpu: bool = collector.has_gpu();
        let _: bool = has_gpu; // type check
    }

    #[test]
    fn test_metrics_collector_has_sensors_returns_bool() {
        // NOTE: has_sensors() returns true if hwmon detected on host.
        let collector = MetricsCollector::new(true);
        let has_sensors: bool = collector.has_sensors();
        let _: bool = has_sensors; // type check
    }

    #[test]
    fn test_metrics_collector_has_connections_returns_bool() {
        // NOTE: has_connections() returns true if /proc/net/tcp readable.
        let collector = MetricsCollector::new(true);
        let has_connections: bool = collector.has_connections();
        let _: bool = has_connections; // type check
    }

    #[test]
    fn test_metrics_collector_has_treemap_returns_bool() {
        // NOTE: has_treemap() returns true if treemap analyzer available.
        let collector = MetricsCollector::new(true);
        let has_treemap: bool = collector.has_treemap();
        let _: bool = has_treemap; // type check
    }

    // =========================================================================
    // RingBuffer ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_ring_buffer_len() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(5);
        assert_eq!(buf.as_slice().len(), 0);
        buf.push(1);
        assert_eq!(buf.as_slice().len(), 1);
        buf.push(2);
        buf.push(3);
        assert_eq!(buf.as_slice().len(), 3);
    }

    #[test]
    fn test_ring_buffer_capacity_maintained() {
        let mut buf: RingBuffer<i32> = RingBuffer::new(3);
        for i in 0..10 {
            buf.push(i);
            assert!(buf.as_slice().len() <= 3);
        }
    }

    #[test]
    fn test_ring_buffer_with_floats() {
        let mut buf: RingBuffer<f64> = RingBuffer::new(3);
        buf.push(1.5);
        buf.push(2.5);
        buf.push(3.5);
        assert!((buf.as_slice()[0] - 1.5).abs() < f64::EPSILON);
        assert!((buf.last().unwrap() - 3.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ring_buffer_with_strings() {
        let mut buf: RingBuffer<String> = RingBuffer::new(2);
        buf.push("hello".to_string());
        buf.push("world".to_string());
        assert_eq!(buf.as_slice(), &["hello", "world"]);

        buf.push("rust".to_string());
        assert_eq!(buf.as_slice(), &["world", "rust"]);
    }

    // =========================================================================
    // App ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_app_uptime() {
        let app = App::new(true);
        // Deterministic mode has fixed uptime (5d 3h 47m)
        let uptime = app.uptime();
        assert_eq!(uptime, 5 * 86400 + 3 * 3600 + 47 * 60);
    }

    #[test]
    fn test_app_frame_id_starts_at_zero() {
        let app = App::new(true);
        assert_eq!(app.frame_id, 0);
    }

    #[test]
    fn test_app_show_filter_input() {
        let mut app = App::new(true);
        assert!(!app.show_filter_input);
        app.show_filter_input = true;
        assert!(app.show_filter_input);
    }

    #[test]
    fn test_app_show_help() {
        let mut app = App::new(true);
        assert!(!app.show_help);
        app.show_help = true;
        assert!(app.show_help);
    }

    #[test]
    fn test_app_show_fps() {
        let mut app = App::new(true);
        assert!(!app.show_fps);
        app.show_fps = true;
        assert!(app.show_fps);
    }

    #[test]
    fn test_app_cpu_history_initial() {
        let app = App::new(true);
        // Deterministic mode pre-fills with 60 zeros
        assert_eq!(app.cpu_history.as_slice().len(), 60);
        for &val in app.cpu_history.as_slice() {
            assert!((val - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn test_app_mem_history_initial() {
        let app = App::new(true);
        // Deterministic mode pre-fills with 60 zeros
        assert_eq!(app.mem_history.as_slice().len(), 60);
    }

    #[test]
    fn test_app_panels_visibility() {
        let app = App::new(true);
        assert!(app.panels.cpu);
        assert!(app.panels.memory);
        assert!(app.panels.disk);
        assert!(app.panels.network);
        assert!(app.panels.process);
    }

    #[test]
    fn test_app_core_count_deterministic() {
        let app = App::new(true);
        // Deterministic mode has 48 cores
        assert_eq!(app.per_core_percent.len(), 48);
    }

    #[test]
    fn test_app_with_config() {
        let config = PtopConfig::default();
        let app = App::with_config(true, config);
        assert!(app.deterministic);
    }

    #[test]
    fn test_app_with_config_lightweight() {
        let config = PtopConfig::default();
        let app = App::with_config_lightweight(true, config);
        assert!(app.deterministic);
    }

    #[test]
    fn test_app_multiple_collect_metrics() {
        let mut app = App::new(true);
        for _ in 0..5 {
            app.collect_metrics();
        }
        assert_eq!(app.frame_id, 5);
    }

    #[test]
    fn test_app_data_availability_fields() {
        let app = App::new(true);
        let avail = app.data_availability();
        // Just verify all fields exist
        let _psi = avail.psi_available;
        let _gpu = avail.gpu_available;
        let _treemap = avail.treemap_ready;
    }

    #[test]
    fn test_app_process_selected() {
        let mut app = App::new(true);
        assert_eq!(app.process_selected, 0);
        app.process_selected = 5;
        assert_eq!(app.process_selected, 5);
    }

    #[test]
    fn test_app_process_scroll_offset() {
        let mut app = App::new(true);
        assert_eq!(app.process_scroll_offset, 0);
        app.process_scroll_offset = 10;
        assert_eq!(app.process_scroll_offset, 10);
    }

    #[test]
    fn test_panel_visibility_fields() {
        let panels = PanelVisibility::default();
        // Test field access for all fields
        let _ = panels.cpu;
        let _ = panels.memory;
        let _ = panels.disk;
        let _ = panels.network;
        let _ = panels.process;
        let _ = panels.gpu;
        let _ = panels.sensors;
        let _ = panels.psi;
        let _ = panels.connections;
        let _ = panels.battery;
        let _ = panels.sensors_compact;
        let _ = panels.system;
        let _ = panels.treemap;
        let _ = panels.files;
    }

    #[test]
    fn test_app_net_history() {
        let app = App::new(true);
        // Check network history exists
        let _ = app.net_rx_history.as_slice().len();
        let _ = app.net_tx_history.as_slice().len();
    }

    #[test]
    fn test_process_sort_column_label() {
        // Test that the column enum has proper variants
        let col = ProcessSortColumn::Pid;
        assert!(matches!(col, ProcessSortColumn::Pid));

        let col = ProcessSortColumn::User;
        assert!(matches!(col, ProcessSortColumn::User));

        let col = ProcessSortColumn::Cpu;
        assert!(matches!(col, ProcessSortColumn::Cpu));

        let col = ProcessSortColumn::Mem;
        assert!(matches!(col, ProcessSortColumn::Mem));

        let col = ProcessSortColumn::Command;
        assert!(matches!(col, ProcessSortColumn::Command));
    }

    #[test]
    fn test_app_load_avg_deterministic() {
        let app = App::new(true);
        // Deterministic mode has zero load average
        assert!((app.load_avg.one - 0.0).abs() < f64::EPSILON);
        assert!((app.load_avg.five - 0.0).abs() < f64::EPSILON);
        assert!((app.load_avg.fifteen - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_app_snapshot_disks() {
        let app = App::new(true);
        // Check snapshot_disks field exists
        let _ = app.snapshot_disks.len();
    }

    #[test]
    fn test_app_snapshot_networks() {
        let app = App::new(true);
        // Check snapshot_networks field exists
        let _ = app.snapshot_networks.len();
    }

    #[test]
    fn test_app_snapshot_processes() {
        let app = App::new(true);
        // Check snapshot_processes field exists
        let _ = app.snapshot_processes.len();
    }

    #[test]
    fn test_app_hostname() {
        let app = App::new(true);
        // Hostname field should exist
        let _ = app.hostname.len();
    }

    #[test]
    fn test_app_kernel_version() {
        let app = App::new(true);
        // Kernel version field should exist
        let _ = app.kernel_version.len();
    }

    #[test]
    fn test_app_in_container() {
        let app = App::new(true);
        // Just verify field exists
        let _ = app.in_container;
    }

    #[test]
    fn test_app_running_state() {
        let mut app = App::new(true);
        assert!(app.running);
        app.running = false;
        assert!(!app.running);
    }

    #[test]
    fn test_app_exploded_panel() {
        let mut app = App::new(true);
        assert!(app.exploded_panel.is_none());
        app.exploded_panel = Some(PanelType::Cpu);
        assert!(app.exploded_panel.is_some());
    }

    #[test]
    fn test_app_selected_column() {
        let mut app = App::new(true);
        assert_eq!(app.selected_column, 0);
        app.selected_column = 3;
        assert_eq!(app.selected_column, 3);
    }

    // =========================================================================
    // ProcessSortColumn ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_process_sort_column_from_index() {
        assert_eq!(ProcessSortColumn::from_index(0), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::from_index(1), ProcessSortColumn::User);
        assert_eq!(ProcessSortColumn::from_index(2), ProcessSortColumn::Cpu);
        assert_eq!(ProcessSortColumn::from_index(3), ProcessSortColumn::Mem);
        assert_eq!(ProcessSortColumn::from_index(4), ProcessSortColumn::Command);
        // Wrap around
        assert_eq!(ProcessSortColumn::from_index(5), ProcessSortColumn::Pid);
        assert_eq!(ProcessSortColumn::from_index(10), ProcessSortColumn::Pid);
    }

    #[test]
    fn test_process_sort_column_to_index() {
        assert_eq!(ProcessSortColumn::Pid.to_index(), 0);
        assert_eq!(ProcessSortColumn::User.to_index(), 1);
        assert_eq!(ProcessSortColumn::Cpu.to_index(), 2);
        assert_eq!(ProcessSortColumn::Mem.to_index(), 3);
        assert_eq!(ProcessSortColumn::Command.to_index(), 4);
    }

    #[test]
    fn test_process_sort_column_header_not_sorted() {
        assert_eq!(ProcessSortColumn::Pid.header(false, true), "PID");
        assert_eq!(ProcessSortColumn::User.header(false, true), "USER");
        assert_eq!(ProcessSortColumn::Cpu.header(false, true), "CPU%");
        assert_eq!(ProcessSortColumn::Mem.header(false, true), "MEM%");
        assert_eq!(ProcessSortColumn::Command.header(false, true), "COMMAND");
    }

    #[test]
    fn test_process_sort_column_header_sorted_desc() {
        assert_eq!(ProcessSortColumn::Pid.header(true, true), "PID▼");
        assert_eq!(ProcessSortColumn::Cpu.header(true, true), "CPU%▼");
    }

    #[test]
    fn test_process_sort_column_header_sorted_asc() {
        assert_eq!(ProcessSortColumn::Pid.header(true, false), "PID▲");
        assert_eq!(ProcessSortColumn::Command.header(true, false), "COMMAND▲");
    }

    // =========================================================================
    // handle_key() TESTS
    // =========================================================================

    #[test]
    fn test_handle_key_quit_q() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_quit_ctrl_c() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
    }

    #[test]
    fn test_handle_key_escape_quits() {
        let mut app = App::new(true);
        assert!(app.handle_key(KeyCode::Esc, KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_help_toggle() {
        let mut app = App::new(true);
        assert!(!app.show_help);

        // '?' toggles help
        app.handle_key(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(app.show_help);

        // '?' again toggles off
        app.handle_key(KeyCode::Char('?'), KeyModifiers::empty());
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_help_f1() {
        let mut app = App::new(true);
        app.handle_key(KeyCode::F(1), KeyModifiers::empty());
        assert!(app.show_help);
    }

    #[test]
    fn test_handle_key_h_toggles_help() {
        let mut app = App::new(true);
        app.handle_key(KeyCode::Char('h'), KeyModifiers::empty());
        assert!(app.show_help);
    }

    #[test]
    fn test_handle_key_in_help_mode_esc_closes() {
        let mut app = App::new(true);
        app.show_help = true;

        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(!app.show_help);
    }

    #[test]
    fn test_handle_key_in_help_mode_q_quits() {
        let mut app = App::new(true);
        app.show_help = true;

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_in_help_mode_swallows_other() {
        let mut app = App::new(true);
        app.show_help = true;

        // Random key should be swallowed, not quit
        assert!(!app.handle_key(KeyCode::Char('x'), KeyModifiers::empty()));
        assert!(app.show_help); // Still in help
    }

    #[test]
    fn test_handle_key_panel_toggles() {
        let mut app = App::new(true);

        // In deterministic mode: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files are on
        // PSI is off

        // Toggle CPU off
        assert!(app.panels.cpu);
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(!app.panels.cpu);

        // Toggle memory off
        assert!(app.panels.memory);
        app.handle_key(KeyCode::Char('2'), KeyModifiers::empty());
        assert!(!app.panels.memory);

        // Toggle disk off
        assert!(app.panels.disk);
        app.handle_key(KeyCode::Char('3'), KeyModifiers::empty());
        assert!(!app.panels.disk);

        // Toggle network off
        assert!(app.panels.network);
        app.handle_key(KeyCode::Char('4'), KeyModifiers::empty());
        assert!(!app.panels.network);

        // Toggle process off
        assert!(app.panels.process);
        app.handle_key(KeyCode::Char('5'), KeyModifiers::empty());
        assert!(!app.panels.process);

        // Toggle GPU off (it's on in deterministic mode)
        assert!(app.panels.gpu);
        app.handle_key(KeyCode::Char('6'), KeyModifiers::empty());
        assert!(!app.panels.gpu);

        // Toggle sensors off (it's on in deterministic mode)
        assert!(app.panels.sensors);
        app.handle_key(KeyCode::Char('7'), KeyModifiers::empty());
        assert!(!app.panels.sensors);

        // Toggle connections off (it's on in deterministic mode)
        assert!(app.panels.connections);
        app.handle_key(KeyCode::Char('8'), KeyModifiers::empty());
        assert!(!app.panels.connections);

        // Toggle PSI on (it's off in deterministic mode)
        assert!(!app.panels.psi);
        app.handle_key(KeyCode::Char('9'), KeyModifiers::empty());
        assert!(app.panels.psi);
    }

    #[test]
    fn test_handle_key_reset_panels() {
        let mut app = App::new(true);
        app.panels.cpu = false;
        app.panels.gpu = true;

        // '0' resets to defaults
        app.handle_key(KeyCode::Char('0'), KeyModifiers::empty());

        assert!(app.panels.cpu);
        assert!(!app.panels.gpu);
    }

    #[test]
    fn test_handle_key_sort_keys() {
        let mut app = App::new(true);

        // 'c' sorts by CPU
        app.handle_key(KeyCode::Char('c'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Cpu);
        assert!(app.sort_descending);

        // 'm' sorts by Memory
        app.handle_key(KeyCode::Char('m'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
        assert!(app.sort_descending);

        // 'p' sorts by PID
        app.handle_key(KeyCode::Char('p'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Pid);
        assert!(!app.sort_descending);

        // 'r' reverses sort
        app.handle_key(KeyCode::Char('r'), KeyModifiers::empty());
        assert!(app.sort_descending);

        // 's' cycles to next column
        app.sort_column = ProcessSortColumn::Cpu;
        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Mem);
    }

    #[test]
    fn test_handle_key_filter_mode() {
        let mut app = App::new(true);

        // '/' enters filter mode
        app.handle_key(KeyCode::Char('/'), KeyModifiers::empty());
        assert!(app.show_filter_input);

        // Type some characters
        app.handle_key(KeyCode::Char('t'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('e'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('s'), KeyModifiers::empty());
        app.handle_key(KeyCode::Char('t'), KeyModifiers::empty());
        assert_eq!(app.filter, "test");

        // Backspace removes character
        app.handle_key(KeyCode::Backspace, KeyModifiers::empty());
        assert_eq!(app.filter, "tes");

        // Enter exits filter mode
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(!app.show_filter_input);
        assert_eq!(app.filter, "tes"); // Filter remains
    }

    #[test]
    fn test_handle_key_filter_escape() {
        let mut app = App::new(true);
        app.show_filter_input = true;
        app.filter = "test".to_string();

        // Esc clears filter and exits
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(!app.show_filter_input);
        assert!(app.filter.is_empty());
    }

    #[test]
    fn test_handle_key_delete_clears_filter() {
        let mut app = App::new(true);
        app.filter = "test".to_string();

        app.handle_key(KeyCode::Delete, KeyModifiers::empty());
        assert!(app.filter.is_empty());
    }

    #[test]
    fn test_handle_key_explode_panel() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Enter explodes focused panel
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(app.exploded_panel, Some(PanelType::Cpu));

        // Esc collapses
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(app.exploded_panel.is_none());
    }

    #[test]
    fn test_handle_key_explode_z() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Memory);

        // 'z' explodes focused panel
        app.handle_key(KeyCode::Char('z'), KeyModifiers::empty());
        assert_eq!(app.exploded_panel, Some(PanelType::Memory));

        // 'z' again collapses
        app.handle_key(KeyCode::Char('z'), KeyModifiers::empty());
        assert!(app.exploded_panel.is_none());
    }

    #[test]
    fn test_handle_key_tab_navigation() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Tab navigates forward
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        // BackTab navigates backward
        app.handle_key(KeyCode::BackTab, KeyModifiers::empty());
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_handle_key_process_navigation() {
        let mut app = App::new(true);

        // In deterministic mode, no processes, so navigation is noop
        app.handle_key(KeyCode::Down, KeyModifiers::empty());
        assert_eq!(app.process_selected, 0);

        app.handle_key(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(app.process_selected, 0);
    }

    #[test]
    fn test_handle_key_signal_request_deterministic_noop() {
        // NOTE: This test only verifies no-op behavior in deterministic mode (no processes).
        // For actual 'x' key signal request testing, see falsification_tests.rs:
        // - falsify_x_key_creates_pending_signal
        let mut app = App::new(true);

        // In deterministic mode, no selected process, so request does nothing
        app.handle_key(KeyCode::Char('x'), KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        // 'n' cancels
        app.handle_key(KeyCode::Char('n'), KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation_esc() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        // Esc cancels
        app.handle_key(KeyCode::Esc, KeyModifiers::empty());
        assert!(app.pending_signal.is_none());
    }

    #[test]
    fn test_handle_key_in_signal_confirmation_q_quits() {
        let mut app = App::new(true);
        app.pending_signal = Some((1234, "test".to_string(), SignalType::Term));

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    #[test]
    fn test_handle_key_in_exploded_column_navigation() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.selected_column = 2;

        // Left moves column left
        app.handle_key(KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.selected_column, 1);

        // Right moves column right
        app.handle_key(KeyCode::Right, KeyModifiers::empty());
        assert_eq!(app.selected_column, 2);

        // At 0, left wraps to last
        app.selected_column = 0;
        app.handle_key(KeyCode::Left, KeyModifiers::empty());
        assert_eq!(app.selected_column, ProcessSortColumn::COUNT - 1);
    }

    #[test]
    fn test_handle_key_in_exploded_sort_toggle() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.sort_column = ProcessSortColumn::Cpu;
        app.selected_column = 2; // CPU column
        app.sort_descending = true;

        // Enter on same column toggles direction
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(!app.sort_descending);

        // Enter again toggles back
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert!(app.sort_descending);
    }

    #[test]
    fn test_handle_key_in_exploded_sort_new_column() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);
        app.sort_column = ProcessSortColumn::Cpu;
        app.selected_column = 0; // PID column
        app.sort_descending = true;

        // Enter on different column changes sort
        app.handle_key(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(app.sort_column, ProcessSortColumn::Pid);
        // PID is not numeric for descending default
        assert!(!app.sort_descending);
    }

    #[test]
    fn test_handle_key_in_exploded_quit() {
        let mut app = App::new(true);
        app.exploded_panel = Some(PanelType::Process);

        assert!(app.handle_key(KeyCode::Char('q'), KeyModifiers::empty()));
    }

    // =========================================================================
    // visible_panels() TESTS
    // =========================================================================

    #[test]
    fn test_visible_panels_default() {
        let app = App::new(true);
        let visible = app.visible_panels();

        // In deterministic mode: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files
        assert_eq!(visible.len(), 9);
        assert!(visible.contains(&PanelType::Cpu));
        assert!(visible.contains(&PanelType::Memory));
        assert!(visible.contains(&PanelType::Disk));
        assert!(visible.contains(&PanelType::Network));
        assert!(visible.contains(&PanelType::Process));
        assert!(visible.contains(&PanelType::Gpu));
        assert!(visible.contains(&PanelType::Sensors));
        assert!(visible.contains(&PanelType::Connections));
        assert!(visible.contains(&PanelType::Files));
    }

    #[test]
    fn test_visible_panels_with_psi() {
        let mut app = App::new(true);
        app.panels.psi = true;

        let visible = app.visible_panels();
        assert_eq!(visible.len(), 10);
        assert!(visible.contains(&PanelType::Psi));
    }

    #[test]
    fn test_visible_panels_order() {
        let app = App::new(true);
        let visible = app.visible_panels();

        // Order should be: CPU, Memory, Disk, Network, Process, GPU, Sensors, Connections, Files
        assert_eq!(visible[0], PanelType::Cpu);
        assert_eq!(visible[1], PanelType::Memory);
        assert_eq!(visible[2], PanelType::Disk);
        assert_eq!(visible[3], PanelType::Network);
        assert_eq!(visible[4], PanelType::Process);
        assert_eq!(visible[5], PanelType::Gpu);
        assert_eq!(visible[6], PanelType::Sensors);
        assert_eq!(visible[7], PanelType::Connections);
        assert_eq!(visible[8], PanelType::Files);
    }

    #[test]
    fn test_visible_panels_empty() {
        let mut app = App::new(true);
        // Turn off all panels
        app.panels.cpu = false;
        app.panels.memory = false;
        app.panels.disk = false;
        app.panels.network = false;
        app.panels.process = false;
        app.panels.gpu = false;
        app.panels.sensors = false;
        app.panels.connections = false;
        app.panels.files = false;

        let visible = app.visible_panels();
        assert!(visible.is_empty());
    }

    // =========================================================================
    // navigate_panel_*() TESTS
    // =========================================================================

    #[test]
    fn test_navigate_panel_forward() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Disk));
    }

    #[test]
    fn test_navigate_panel_forward_wraps() {
        let mut app = App::new(true);
        // In deterministic mode, Files is the last visible panel
        app.focused_panel = Some(PanelType::Files);

        // After Files (last), wraps to CPU
        app.navigate_panel_forward();
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_navigate_panel_backward() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Memory);

        app.navigate_panel_backward();
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
    }

    #[test]
    fn test_navigate_panel_backward_wraps() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        // Before CPU (first), wraps to Files (last in deterministic mode)
        app.navigate_panel_backward();
        assert_eq!(app.focused_panel, Some(PanelType::Files));
    }

    #[test]
    fn test_navigate_panel_empty_is_noop() {
        let mut app = App::new(true);
        // Turn off all panels
        app.panels.cpu = false;
        app.panels.memory = false;
        app.panels.disk = false;
        app.panels.network = false;
        app.panels.process = false;
        app.panels.gpu = false;
        app.panels.sensors = false;
        app.panels.connections = false;
        app.panels.files = false;
        app.focused_panel = None;

        app.navigate_panel_forward();
        assert!(app.focused_panel.is_none());

        app.navigate_panel_backward();
        assert!(app.focused_panel.is_none());
    }

    // =========================================================================
    // navigate_process() TESTS
    // =========================================================================

    #[test]
    fn test_navigate_process_down() {
        let mut app = App::new(true);
        // Deterministic mode has 0 processes, so navigation is bounded
        app.navigate_process(1);
        assert_eq!(app.process_selected, 0);
    }

    #[test]
    fn test_navigate_process_up() {
        let mut app = App::new(true);
        app.process_selected = 5;
        // With 0 processes, navigate is a no-op (early return)
        app.navigate_process(-1);
        // process_selected unchanged since count is 0
        assert_eq!(app.process_selected, 5);
    }

    // =========================================================================
    // evaluate_panel_display() TESTS
    // =========================================================================

    #[test]
    fn test_evaluate_panel_display_cpu() {
        let app = App::new(true);
        let action = app.evaluate_panel_display(PanelType::Cpu);
        // CPU should always show
        assert!(matches!(action, crate::widgets::DisplayAction::Show));
    }

    #[test]
    fn test_evaluate_panel_display_psi() {
        let app = App::new(true);
        let action = app.evaluate_panel_display(PanelType::Psi);
        // PSI not available in deterministic mode
        // Should be HideNoData or similar
        let _ = action; // Just verify it doesn't panic
    }

    #[test]
    fn test_data_availability_with_connections() {
        let mut app = App::new(true);
        app.snapshot_connections = Some(ConnectionsData {
            connections: vec![],
            state_counts: std::collections::HashMap::new(),
            count_history: vec![],
        });

        let avail = app.data_availability();
        assert!(avail.connections_available);
        assert_eq!(avail.connection_count, 0);
    }

    #[test]
    fn test_data_availability_with_treemap() {
        let mut app = App::new(true);
        app.snapshot_treemap = Some(TreemapData {
            root_path: std::path::PathBuf::from("/"),
            root: None,
            top_items: vec![],
            total_size: 0,
            total_files: 0,
            total_dirs: 0,
            depth: 0,
            last_scan: None,
            scan_duration: std::time::Duration::from_secs(0),
        });

        let avail = app.data_availability();
        // Empty top_items means not ready
        assert!(!avail.treemap_ready);
    }

    // =========================================================================
    // Signal handling ADDITIONAL TESTS
    // =========================================================================

    #[test]
    fn test_confirm_signal_with_no_pending() {
        let mut app = App::new(true);
        assert!(app.pending_signal.is_none());

        app.confirm_signal();
        // Should be no-op with no pending signal
        assert!(app.signal_result.is_none());
    }

    #[test]
    fn test_signal_type_name_and_number() {
        assert_eq!(SignalType::Term.name(), "TERM");
        assert_eq!(SignalType::Term.number(), 15);

        assert_eq!(SignalType::Kill.name(), "KILL");
        assert_eq!(SignalType::Kill.number(), 9);

        assert_eq!(SignalType::Hup.name(), "HUP");
        assert_eq!(SignalType::Hup.number(), 1);

        assert_eq!(SignalType::Int.name(), "INT");
        assert_eq!(SignalType::Int.number(), 2);

        assert_eq!(SignalType::Stop.name(), "STOP");
        assert_eq!(SignalType::Stop.number(), 19);
    }

    // =========================================================================
    // Signal result auto-clear tests (PMAT-GAP-033)
    // =========================================================================

    #[test]
    fn test_clear_old_signal_result_none() {
        let mut app = App::new(true);
        app.signal_result = None;
        app.clear_old_signal_result();
        assert!(app.signal_result.is_none());
    }

    #[test]
    fn test_clear_old_signal_result_recent() {
        let mut app = App::new(true);
        app.signal_result = Some((true, "test".to_string(), std::time::Instant::now()));
        app.clear_old_signal_result();
        assert!(app.signal_result.is_some()); // Not old enough to clear
    }

    #[test]
    fn test_signal_result_tuple_structure() {
        let mut app = App::new(true);
        let now = std::time::Instant::now();
        app.signal_result = Some((true, "Success message".to_string(), now));

        if let Some((success, message, timestamp)) = &app.signal_result {
            assert!(*success);
            assert_eq!(message, "Success message");
            assert!(timestamp.elapsed().as_secs() < 1);
        } else {
            panic!("Expected signal_result to be Some");
        }
    }

    #[test]
    fn test_signal_result_failure() {
        let mut app = App::new(true);
        let now = std::time::Instant::now();
        app.signal_result = Some((false, "Failed to send signal".to_string(), now));

        if let Some((success, message, _timestamp)) = &app.signal_result {
            assert!(!*success);
            assert!(message.contains("Failed"));
        } else {
            panic!("Expected signal_result to be Some");
        }
    }

    // =========================================================================
    // PMAT-GAP-031: Network interface cycling tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_selected_interface_index_field_exists() {
        let app = App::new(true);
        // Field must exist and default to 0
        assert_eq!(app.selected_interface_index, 0);
    }

    #[test]
    fn test_cycle_interface_no_interfaces() {
        let mut app = App::new(true);
        // No interfaces available - should stay at 0
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0);
    }

    #[test]
    fn test_cycle_interface_wraps_around() {
        let mut app = App::new(true);
        // Simulate 3 interfaces
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "lo".to_string(),
                received: 0,
                transmitted: 0,
            },
        ];

        assert_eq!(app.selected_interface_index, 0);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 1);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 2);
        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0); // Wraps around
    }

    #[test]
    fn test_selected_interface_name() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 100,
                transmitted: 200,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 50,
                transmitted: 25,
            },
        ];

        assert_eq!(app.selected_interface_name(), Some("eth0"));
        app.selected_interface_index = 1;
        assert_eq!(app.selected_interface_name(), Some("wlan0"));
        app.selected_interface_index = 2; // Out of bounds
        assert_eq!(app.selected_interface_name(), None);
    }

    #[test]
    fn test_selected_interface_data() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![NetworkInfo {
            name: "eth0".to_string(),
            received: 1000,
            transmitted: 500,
        }];

        let data = app.selected_interface_data();
        assert!(data.is_some());
        let info = data.unwrap();
        assert_eq!(info.name, "eth0");
        assert_eq!(info.received, 1000);
    }

    #[test]
    fn test_cycle_interface_single_interface() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![NetworkInfo {
            name: "lo".to_string(),
            received: 0,
            transmitted: 0,
        }];

        app.cycle_interface();
        assert_eq!(app.selected_interface_index, 0); // Stays at 0 (wraps from 1 to 0)
    }

    #[test]
    fn test_tab_cycles_interface_when_network_focused() {
        let mut app = App::new(true);
        app.snapshot_networks = vec![
            NetworkInfo {
                name: "eth0".to_string(),
                received: 0,
                transmitted: 0,
            },
            NetworkInfo {
                name: "wlan0".to_string(),
                received: 0,
                transmitted: 0,
            },
        ];
        app.focused_panel = Some(PanelType::Network);

        assert_eq!(app.selected_interface_index, 0);
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.selected_interface_index, 1);
        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        assert_eq!(app.selected_interface_index, 0); // Wraps
    }

    #[test]
    fn test_tab_navigates_panels_when_not_network_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.memory = true;

        app.handle_key(KeyCode::Tab, KeyModifiers::empty());
        // Should navigate to next panel, not cycle interfaces
        assert_ne!(app.focused_panel, Some(PanelType::Cpu));
        assert_eq!(app.selected_interface_index, 0); // Unchanged
    }

    // =========================================================================
    // PMAT-GAP-034: Files view mode toggle tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_files_view_mode_field_exists() {
        let app = App::new(true);
        // Field must exist and default to Size
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_files_view_mode_next_cycle() {
        assert_eq!(FilesViewMode::Size.next(), FilesViewMode::Tree);
        assert_eq!(FilesViewMode::Tree.next(), FilesViewMode::Flat);
        assert_eq!(FilesViewMode::Flat.next(), FilesViewMode::Size);
    }

    #[test]
    fn test_files_view_mode_names() {
        assert_eq!(FilesViewMode::Tree.name(), "tree");
        assert_eq!(FilesViewMode::Flat.name(), "flat");
        assert_eq!(FilesViewMode::Size.name(), "size");
    }

    #[test]
    fn test_cycle_files_view_mode() {
        let mut app = App::new(true);
        assert_eq!(app.files_view_mode, FilesViewMode::Size);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Tree);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Flat);

        app.cycle_files_view_mode();
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_v_key_cycles_view_mode_when_files_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Files);
        app.panels.files = true;

        assert_eq!(app.files_view_mode, FilesViewMode::Size);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Tree);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Flat);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Size);
    }

    #[test]
    fn test_v_key_does_nothing_when_files_not_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);

        assert_eq!(app.files_view_mode, FilesViewMode::Size);
        app.handle_key(KeyCode::Char('v'), KeyModifiers::empty());
        assert_eq!(app.files_view_mode, FilesViewMode::Size); // Unchanged
    }

    // =========================================================================
    // PMAT-GAP-035: Panel collapse memory tests (ttop parity)
    // =========================================================================

    #[test]
    fn test_collapse_memory_field_exists() {
        let app = App::new(true);
        // Field must exist and default to None
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_toggle_panel_hides_focused_stores_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Toggle off CPU while focused
        app.toggle_panel(PanelType::Cpu);

        // CPU should be hidden
        assert!(!app.panels.cpu);
        // Focus should move to first visible (Memory)
        assert_eq!(app.focused_panel, Some(PanelType::Memory));
        // Collapse memory should store CPU
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));
    }

    #[test]
    fn test_toggle_panel_restore_focus_from_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Hide CPU (stores in memory)
        app.toggle_panel(PanelType::Cpu);
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));
        assert_eq!(app.focused_panel, Some(PanelType::Memory));

        // Show CPU again (should restore focus)
        app.toggle_panel(PanelType::Cpu);
        assert!(app.panels.cpu);
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        assert!(app.collapse_memory.is_none()); // Memory cleared
    }

    #[test]
    fn test_toggle_panel_no_memory_when_not_focused() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Toggle off Memory (not focused)
        app.toggle_panel(PanelType::Memory);

        // Memory should be hidden
        assert!(!app.panels.memory);
        // Focus should stay on CPU
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        // Collapse memory should be empty (Memory wasn't focused)
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_toggle_panel_key_binding_with_memory() {
        let mut app = App::new(true);
        app.focused_panel = Some(PanelType::Cpu);
        app.panels.cpu = true;
        app.panels.memory = true;

        // Press '1' to hide CPU
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(!app.panels.cpu);
        assert_eq!(app.collapse_memory, Some(PanelType::Cpu));

        // Press '1' again to show CPU and restore focus
        app.handle_key(KeyCode::Char('1'), KeyModifiers::empty());
        assert!(app.panels.cpu);
        assert_eq!(app.focused_panel, Some(PanelType::Cpu));
        assert!(app.collapse_memory.is_none());
    }

    #[test]
    fn test_is_panel_visible() {
        let mut app = App::new(true);
        app.panels.cpu = true;
        app.panels.memory = false;

        assert!(app.is_panel_visible(PanelType::Cpu));
        assert!(!app.is_panel_visible(PanelType::Memory));
    }

    #[test]
    fn test_set_panel_visible() {
        let mut app = App::new(true);
        app.panels.cpu = true;

        app.set_panel_visible(PanelType::Cpu, false);
        assert!(!app.panels.cpu);

        app.set_panel_visible(PanelType::Cpu, true);
        assert!(app.panels.cpu);
    }
}
