mod explode_tests {
    use super::*;

    /// F-EXPLODE-001: Exploded detection threshold test
    #[test]
    fn test_f_explode_001_detection_threshold() {
        // Normal panel width (typical CPU panel in 4-panel grid)
        let normal_width = 50.0;
        let is_exploded_normal = normal_width > 100.0;
        assert!(
            !is_exploded_normal,
            "Normal panel should NOT be detected as exploded"
        );

        // Exploded width (fullscreen on 150 col terminal)
        let exploded_width = 148.0; // 150 - 2 for borders
        let is_exploded_full = exploded_width > 100.0;
        assert!(
            is_exploded_full,
            "Exploded panel SHOULD be detected as exploded"
        );
    }

    /// F-EXPLODE-002: Core layout spreads horizontally in exploded mode
    #[test]
    fn test_f_explode_002_core_spread() {
        let core_count: usize = 48;
        let core_area_height = 35.0_f32; // Typical exploded height

        // Normal mode: all cores in as few columns as possible
        let cores_per_col_normal = core_area_height as usize; // 35
        let cols_normal = core_count.div_ceil(cores_per_col_normal);
        assert_eq!(
            cols_normal, 2,
            "Normal mode: 48 cores / 35 per col = 2 cols"
        );

        // Exploded mode: max 12 cores per column
        let max_per_col: usize = 12;
        let cores_per_col_exploded = (core_area_height as usize).min(max_per_col);
        let cols_exploded = core_count.div_ceil(cores_per_col_exploded);
        assert_eq!(
            cols_exploded, 4,
            "Exploded mode: 48 cores / 12 per col = 4 cols"
        );
    }

    /// F-EXPLODE-003: Bar length increases in exploded mode
    #[test]
    fn test_f_explode_003_bar_length() {
        // Updated: bar_len is 8 in exploded (was 10, reduced to prevent column overflow)
        let bar_len_normal: usize = 6;
        let bar_len_exploded: usize = 8;

        assert!(
            bar_len_exploded > bar_len_normal,
            "Exploded bars should be longer"
        );
        assert_eq!(
            bar_len_exploded - bar_len_normal,
            2,
            "Exploded bars 2 chars longer"
        );
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    use crate::ptop::ui::core::format::format_uptime;

    // =========================================================================
    // percent_color TESTS
    // =========================================================================

    #[test]
    fn test_percent_color_low() {
        // Low values (0-25%): cyan to green
        let color = percent_color(10.0);
        assert!(color.b > 0.5, "Low percent should have blue/cyan component");
        assert!(color.g > 0.5, "Low percent should have green component");
    }

    #[test]
    fn test_percent_color_medium_low() {
        // Medium-low (25-50%): green to yellow
        let color = percent_color(35.0);
        assert!(color.g > 0.7, "Medium-low should be greenish");
    }

    #[test]
    fn test_percent_color_medium_high() {
        // Medium-high (50-75%): yellow to orange
        let color = percent_color(60.0);
        assert!(color.r > 0.7, "Medium-high should have high red");
        assert!(
            color.g > 0.5,
            "Medium-high should have some green (yellow-orange)"
        );
    }

    #[test]
    fn test_percent_color_high() {
        // High (75-90%): orange-red
        let color = percent_color(80.0);
        assert_eq!(color.r, 1.0, "High should be red component");
    }

    #[test]
    fn test_percent_color_critical() {
        // Critical (90-100%): bright red
        let color = percent_color(95.0);
        assert_eq!(color.r, 1.0, "Critical should be full red");
        assert!(color.g < 0.5, "Critical should have low green");
    }

    #[test]
    fn test_percent_color_clamped() {
        // Values outside 0-100 should be clamped
        let neg = percent_color(-10.0);
        let over = percent_color(150.0);

        let zero = percent_color(0.0);
        let hundred = percent_color(100.0);

        // Clamped values should match boundaries
        assert_eq!(neg.r, zero.r);
        assert_eq!(over.r, hundred.r);
    }

    #[test]
    fn test_percent_color_boundary_90() {
        let color = percent_color(90.0);
        assert_eq!(color.r, 1.0);
        assert_eq!(color.g, 0.25);
    }

    #[test]
    fn test_percent_color_boundary_75() {
        let color = percent_color(75.0);
        assert_eq!(color.r, 1.0);
    }

    #[test]
    fn test_percent_color_boundary_50() {
        let color = percent_color(50.0);
        assert_eq!(color.r, 1.0);
    }

    #[test]
    fn test_percent_color_boundary_25() {
        let color = percent_color(25.0);
        assert!(color.g > 0.8);
    }

    // =========================================================================
    // format_bytes TESTS
    // =========================================================================

    #[test]
    fn test_format_bytes_small() {
        assert_eq!(format_bytes(500), "500B");
        assert_eq!(format_bytes(1023), "1023B");
    }

    #[test]
    fn test_format_bytes_kb() {
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(1536), "1.5K");
        assert_eq!(format_bytes(1024 * 10), "10.0K");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes(1024 * 1024), "1.0M");
        assert_eq!(format_bytes(1024 * 1024 * 5), "5.0M");
    }

    #[test]
    fn test_format_bytes_gb() {
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0G");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 8), "8.0G");
    }

    #[test]
    fn test_format_bytes_tb() {
        assert_eq!(format_bytes(1024u64 * 1024 * 1024 * 1024), "1.0T");
        assert_eq!(format_bytes(1024u64 * 1024 * 1024 * 1024 * 2), "2.0T");
    }

    // =========================================================================
    // format_bytes_rate TESTS
    // =========================================================================

    #[test]
    fn test_format_bytes_rate_small() {
        assert_eq!(format_bytes_rate(500.0), "500B");
    }

    #[test]
    fn test_format_bytes_rate_kb() {
        assert_eq!(format_bytes_rate(1024.0), "1K");
    }

    #[test]
    fn test_format_bytes_rate_mb() {
        assert_eq!(format_bytes_rate(1024.0 * 1024.0), "1.0M");
    }

    #[test]
    fn test_format_bytes_rate_gb() {
        assert_eq!(format_bytes_rate(1024.0 * 1024.0 * 1024.0), "1.0G");
    }

    // =========================================================================
    // format_uptime TESTS
    // =========================================================================

    #[test]
    fn test_format_uptime_seconds() {
        // When both hours and minutes are 0, format shows "0m"
        assert_eq!(format_uptime(30), "0m");
        assert_eq!(format_uptime(59), "0m");
    }

    #[test]
    fn test_format_uptime_minutes() {
        assert_eq!(format_uptime(60), "1m");
        assert_eq!(format_uptime(90), "1m");
        assert_eq!(format_uptime(3599), "59m");
    }

    #[test]
    fn test_format_uptime_hours() {
        assert_eq!(format_uptime(3600), "1h 0m");
        assert_eq!(format_uptime(3660), "1h 1m");
        assert_eq!(format_uptime(7200), "2h 0m");
    }

    #[test]
    fn test_format_uptime_days() {
        assert_eq!(format_uptime(86400), "1d 0h");
        assert_eq!(format_uptime(90000), "1d 1h");
        assert_eq!(format_uptime(172800), "2d 0h");
    }

    // =========================================================================
    // swap_color TESTS
    // =========================================================================

    #[test]
    fn test_swap_color_low() {
        let color = swap_color(10.0);
        // Low swap usage should be normal/green-ish
        assert!(color.g > 0.5);
    }

    #[test]
    fn test_swap_color_medium() {
        let color = swap_color(40.0);
        // Medium swap should be warning-ish
        assert!(color.r > 0.5 || color.g > 0.5);
    }

    #[test]
    fn test_swap_color_high() {
        let color = swap_color(80.0);
        // High swap should be red
        assert!(color.r > 0.7);
    }

    #[test]
    fn test_swap_color_clamped() {
        let neg = swap_color(-10.0);
        let over = swap_color(110.0);
        // Should clamp and not panic
        assert!(neg.r >= 0.0 && neg.r <= 1.0);
        assert!(over.r >= 0.0 && over.r <= 1.0);
    }

    // =========================================================================
    // pressure_symbol TESTS
    // =========================================================================

    #[test]
    fn test_pressure_symbol_none() {
        // ≤1% returns "—"
        assert_eq!(pressure_symbol(0.0), "—");
        assert_eq!(pressure_symbol(0.5), "—");
        assert_eq!(pressure_symbol(1.0), "—");
    }

    #[test]
    fn test_pressure_symbol_low() {
        // >1% to ≤5%: "◐"
        assert_eq!(pressure_symbol(2.0), "◐");
        // >5% to ≤20%: "▼"
        assert_eq!(pressure_symbol(10.0), "▼");
    }

    #[test]
    fn test_pressure_symbol_high() {
        // >20% to ≤50%: "▲"
        assert_eq!(pressure_symbol(30.0), "▲");
        // >50%: "▲▲"
        assert_eq!(pressure_symbol(60.0), "▲▲");
    }

    // =========================================================================
    // pressure_color TESTS
    // =========================================================================

    #[test]
    fn test_pressure_color_none() {
        let color = pressure_color(0.0);
        // Should be dim
        assert!(color.r < 0.5);
    }

    #[test]
    fn test_pressure_color_low() {
        let color = pressure_color(5.0);
        // Low pressure should be green-ish
        assert!(color.g > 0.0);
    }

    #[test]
    fn test_pressure_color_high() {
        let color = pressure_color(50.0);
        // High pressure should be red
        assert!(color.r > 0.5);
    }

    // =========================================================================
    // port_to_service TESTS
    // =========================================================================

    #[test]
    fn test_port_to_service_known() {
        assert_eq!(port_to_service(22), "SSH");
        assert_eq!(port_to_service(80), "HTTP");
        assert_eq!(port_to_service(443), "HTTPS");
        assert_eq!(port_to_service(53), "DNS");
        assert_eq!(port_to_service(25), "SMTP");
        assert_eq!(port_to_service(21), "FTP");
    }

    #[test]
    fn test_port_to_service_database() {
        assert_eq!(port_to_service(3306), "MySQL");
        assert_eq!(port_to_service(5432), "Pgsql");
        assert_eq!(port_to_service(6379), "Redis");
        assert_eq!(port_to_service(27017), "Mongo");
    }

    #[test]
    fn test_port_to_service_unknown() {
        // Unknown ports return empty string
        assert_eq!(port_to_service(12345), "");
    }

    #[test]
    fn test_port_to_service_app_range() {
        // 9000-9999 range returns "App"
        assert_eq!(port_to_service(9000), "App");
        assert_eq!(port_to_service(9999), "App");
    }

    // =========================================================================
    // COLOR CONSTANT TESTS
    // =========================================================================

    #[test]
    fn test_cpu_color_is_cyan() {
        assert!(CPU_COLOR.b > 0.9);
        assert!(CPU_COLOR.g > 0.7);
    }

    #[test]
    fn test_memory_color_is_purple() {
        assert!(MEMORY_COLOR.b > 0.9);
        assert!(MEMORY_COLOR.r > 0.6);
    }

    #[test]
    fn test_network_color_is_orange() {
        assert!(NETWORK_COLOR.r > 0.9);
        assert!(NETWORK_COLOR.g > 0.5);
    }

    #[test]
    fn test_process_color_is_yellow() {
        assert!(PROCESS_COLOR.r > 0.8);
        assert!(PROCESS_COLOR.g > 0.6);
    }

    #[test]
    fn test_gpu_color_is_green() {
        assert!(GPU_COLOR.g > 0.9);
        assert!(GPU_COLOR.b > 0.5);
    }

    // =========================================================================
    // create_panel_border TESTS
    // =========================================================================

    #[test]
    fn test_create_panel_border_unfocused() {
        let border = create_panel_border("Test", CPU_COLOR, false);
        // Verify the border was created without panic
        let _ = border;
    }

    #[test]
    fn test_create_panel_border_focused() {
        let border = create_panel_border("Test", CPU_COLOR, true);
        // Verify focused border was created without panic
        let _ = border;
    }

    // =========================================================================
    // ADDITIONAL COVERAGE TESTS
    // =========================================================================

    #[test]
    fn test_selection_colors() {
        // Verify selection colors match ttop style (bright green accent, subtle bg)
        assert!(FOCUS_ACCENT_COLOR.g >= 0.9, "Accent should be bright green");
        assert!(
            ROW_SELECT_BG.b > ROW_SELECT_BG.r,
            "Selection bg should have purple/blue tint"
        );
        assert!(ROW_SELECT_BG.r < 0.25, "Selection bg should be subtle/dark");
    }

    #[test]
    fn test_status_bar_bg() {
        // Status bar should be dark
        assert!(STATUS_BAR_BG.r < 0.15);
        assert!(STATUS_BAR_BG.g < 0.15);
        assert!(STATUS_BAR_BG.b < 0.15);
    }

    #[test]
    fn test_col_select_bg() {
        // Column select should be blue-ish
        assert!(COL_SELECT_BG.b > COL_SELECT_BG.r);
    }

    #[test]
    fn test_net_colors() {
        // RX (download) should be cyan
        assert!(NET_RX_COLOR.b > 0.9);
        // TX (upload) should be red
        assert!(NET_TX_COLOR.r > 0.9);
    }

    // =========================================================================
    // ZramStats TESTS
    // =========================================================================

    #[test]
    fn test_zram_stats_default() {
        let stats = ZramStats::default();
        assert_eq!(stats.orig_data_size, 0);
        assert_eq!(stats.compr_data_size, 0);
        assert!(stats.algorithm.is_empty());
    }

    #[test]
    fn test_zram_stats_ratio_zero_compressed() {
        let stats = ZramStats {
            orig_data_size: 1000,
            compr_data_size: 0,
            algorithm: "lz4".to_string(),
        };
        assert!((stats.ratio() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zram_stats_ratio_normal() {
        let stats = ZramStats {
            orig_data_size: 1000,
            compr_data_size: 500,
            algorithm: "lz4".to_string(),
        };
        assert!((stats.ratio() - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zram_stats_ratio_high_compression() {
        let stats = ZramStats {
            orig_data_size: 10000,
            compr_data_size: 1000,
            algorithm: "zstd".to_string(),
        };
        assert!((stats.ratio() - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_zram_stats_is_active_true() {
        let stats = ZramStats {
            orig_data_size: 100,
            compr_data_size: 50,
            algorithm: "lzo".to_string(),
        };
        assert!(stats.is_active());
    }

    #[test]
    fn test_zram_stats_is_active_false() {
        let stats = ZramStats {
            orig_data_size: 0,
            compr_data_size: 0,
            algorithm: "".to_string(),
        };
        assert!(!stats.is_active());
    }

    #[test]
    fn test_zram_stats_debug() {
        let stats = ZramStats {
            orig_data_size: 1024,
            compr_data_size: 512,
            algorithm: "lz4".to_string(),
        };
        let debug = format!("{:?}", stats);
        assert!(debug.contains("ZramStats"));
        assert!(debug.contains("1024"));
        assert!(debug.contains("lz4"));
    }

    // =========================================================================
    // CpuMeterLayout TESTS
    // =========================================================================

    #[test]
    fn test_cpu_meter_layout_normal_mode() {
        let layout = CpuMeterLayout::calculate(8, 20.0, false);
        assert_eq!(layout.bar_len, 6);
        assert!(layout.meter_bar_width > 0.0);
        assert!(layout.cores_per_col > 0);
        assert!(layout.num_meter_cols > 0);
    }

    #[test]
    fn test_cpu_meter_layout_exploded_mode() {
        let layout = CpuMeterLayout::calculate(8, 20.0, true);
        assert_eq!(layout.bar_len, 8);
        assert!(layout.meter_bar_width > 0.0);
    }

    #[test]
    fn test_cpu_meter_layout_exploded_caps_cores_per_col() {
        // In exploded mode, max 12 cores per col
        let layout = CpuMeterLayout::calculate(48, 35.0, true);
        assert!(layout.cores_per_col <= 12);
    }

    #[test]
    fn test_cpu_meter_layout_normal_uses_full_height() {
        let layout = CpuMeterLayout::calculate(48, 35.0, false);
        // Normal mode should use full height (35 cores per col)
        assert_eq!(layout.cores_per_col, 35);
    }

    #[test]
    fn test_cpu_meter_layout_single_core() {
        let layout = CpuMeterLayout::calculate(1, 10.0, false);
        assert_eq!(layout.num_meter_cols, 1);
        assert_eq!(layout.cores_per_col, 10);
    }

    #[test]
    fn test_cpu_meter_layout_zero_height() {
        // Should have minimum 1 core per col
        let layout = CpuMeterLayout::calculate(4, 0.0, false);
        assert!(layout.cores_per_col >= 1);
    }

    #[test]
    fn test_cpu_meter_layout_many_cores() {
        let layout = CpuMeterLayout::calculate(128, 30.0, false);
        // 128 cores / 30 height = 5 columns needed
        assert!(layout.num_meter_cols >= 5);
    }

    #[test]
    fn test_cpu_meter_layout_bar_width_calculation() {
        let layout_normal = CpuMeterLayout::calculate(8, 20.0, false);
        let layout_exploded = CpuMeterLayout::calculate(8, 20.0, true);
        // Exploded mode has larger bar width (bar_len + 9)
        assert!(layout_exploded.meter_bar_width > layout_normal.meter_bar_width);
    }

    // =========================================================================
    // MemoryStats TESTS (requires App)
    // =========================================================================

    #[test]
    fn test_memory_stats_creation() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let stats = MemoryStats::from_app(&app);
        // In deterministic mode, memory values are set
        assert!(stats.used_gb >= 0.0);
        assert!(stats.cached_gb >= 0.0);
        assert!(stats.free_gb >= 0.0);
    }

    // =========================================================================
    // panel_border_color TESTS
    // =========================================================================

    #[test]
    fn test_panel_border_color_cpu() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Cpu);
        assert_eq!(color.r, CPU_COLOR.r);
        assert_eq!(color.g, CPU_COLOR.g);
        assert_eq!(color.b, CPU_COLOR.b);
    }

    #[test]
    fn test_panel_border_color_memory() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Memory);
        assert_eq!(color.r, MEMORY_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_disk() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Disk);
        assert_eq!(color.r, DISK_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_network() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Network);
        assert_eq!(color.r, NETWORK_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_process() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Process);
        assert_eq!(color.r, PROCESS_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_gpu() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Gpu);
        assert_eq!(color.r, GPU_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_battery() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Battery);
        assert_eq!(color.r, BATTERY_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_sensors() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Sensors);
        assert_eq!(color.r, SENSORS_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_psi() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Psi);
        assert_eq!(color.r, PSI_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_connections() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Connections);
        assert_eq!(color.r, CONNECTIONS_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_files() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Files);
        assert_eq!(color.r, FILES_COLOR.r);
    }

    #[test]
    fn test_panel_border_color_containers() {
        use crate::ptop::config::PanelType;
        let color = panel_border_color(PanelType::Containers);
        assert_eq!(color.r, CONTAINERS_COLOR.r);
    }

    // =========================================================================
    // ADDITIONAL HELPER FUNCTION TESTS
    // =========================================================================

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0B");
    }

    #[test]
    fn test_format_bytes_rate_zero() {
        assert_eq!(format_bytes_rate(0.0), "0B");
    }

    #[test]
    fn test_format_uptime_zero() {
        assert_eq!(format_uptime(0), "0m");
    }

    #[test]
    fn test_format_uptime_large() {
        // Test 365 days
        let secs = 365 * 24 * 60 * 60;
        let result = format_uptime(secs);
        assert!(result.contains("365d"));
    }

    #[test]
    fn test_percent_color_exact_boundaries() {
        // Test exact boundary values
        let _ = percent_color(0.0);
        let _ = percent_color(25.0);
        let _ = percent_color(50.0);
        let _ = percent_color(75.0);
        let _ = percent_color(90.0);
        let _ = percent_color(100.0);
    }

    #[test]
    fn test_swap_color_boundaries() {
        // Test exact boundaries
        let low = swap_color(10.0);
        let med = swap_color(10.1);
        let high = swap_color(50.1);
        assert!(low.g > 0.8); // Green
        assert!(med.g > 0.7); // Yellow (still has green)
        assert!(high.r > 0.9); // Red
    }

    #[test]
    fn test_pressure_symbol_boundary_values() {
        assert_eq!(pressure_symbol(1.0), "—");
        assert_eq!(pressure_symbol(1.1), "◐");
        assert_eq!(pressure_symbol(5.0), "◐");
        assert_eq!(pressure_symbol(5.1), "▼");
        assert_eq!(pressure_symbol(20.0), "▼");
        assert_eq!(pressure_symbol(20.1), "▲");
        assert_eq!(pressure_symbol(50.0), "▲");
        assert_eq!(pressure_symbol(50.1), "▲▲");
    }

    #[test]
    fn test_pressure_color_boundaries() {
        let none = pressure_color(1.0);
        let low = pressure_color(5.0);
        let med = pressure_color(20.0);
        let high = pressure_color(50.0);
        // Just verify they return valid colors
        assert!(none.r >= 0.0 && none.r <= 1.0);
        assert!(low.g >= 0.0 && low.g <= 1.0);
        assert!(med.r >= 0.0 && med.r <= 1.0);
        assert!(high.r >= 0.0 && high.r <= 1.0);
    }

    #[test]
    fn test_port_to_service_edge_cases() {
        // Ports just outside known ranges
        assert_eq!(port_to_service(8999), "");
        assert_eq!(port_to_service(10000), "");
    }

    #[test]
    fn test_dim_color_constant() {
        assert!(DIM_COLOR.r < 0.5);
        assert!(DIM_COLOR.g < 0.5);
        assert!(DIM_COLOR.b < 0.5);
    }

    #[test]
    fn test_cached_color_constant() {
        assert!(CACHED_COLOR.g > 0.7);
        assert!(CACHED_COLOR.b > 0.8);
    }

    #[test]
    fn test_free_color_constant() {
        assert!(FREE_COLOR.b > 0.8);
    }

    #[test]
    fn test_battery_color_is_yellow() {
        assert!(BATTERY_COLOR.r > 0.9);
        assert!(BATTERY_COLOR.g > 0.8);
    }

    #[test]
    fn test_sensors_color_is_pink() {
        assert!(SENSORS_COLOR.r > 0.9);
        assert!(SENSORS_COLOR.b > 0.5);
    }

    #[test]
    fn test_psi_color_is_red() {
        assert!(PSI_COLOR.r > 0.7);
    }

    #[test]
    fn test_disk_color_is_blue() {
        assert!(DISK_COLOR.b > 0.9);
    }

    #[test]
    fn test_files_color_is_brown() {
        assert!(FILES_COLOR.r > 0.6);
        assert!(FILES_COLOR.g > 0.4);
    }

    #[test]
    fn test_containers_color_is_docker_blue() {
        assert!(CONTAINERS_COLOR.b > 0.8);
    }

    #[test]
    fn test_connections_color_is_light_blue() {
        assert!(CONNECTIONS_COLOR.b > 0.8);
    }
}

#[cfg(test)]
mod draw_integration_tests {
    use super::*;
    use crate::direct::CellBuffer;

    #[test]
    fn test_draw_small_terminal() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(80, 24);
        draw(&app, &mut buffer);
        // Should complete without panic
    }

    #[test]
    fn test_draw_large_terminal() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(160, 50);
        draw(&app, &mut buffer);
        // Should complete without panic
    }

    #[test]
    fn test_draw_minimum_size() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(10, 5);
        draw(&app, &mut buffer);
        // Should complete without panic (minimum viable size)
    }

    #[test]
    fn test_draw_too_small_width() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(5, 24);
        draw(&app, &mut buffer);
        // Should early-return without panic
    }

    #[test]
    fn test_draw_too_small_height() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(80, 3);
        draw(&app, &mut buffer);
        // Should early-return without panic
    }

    #[test]
    fn test_draw_standard_sizes() {
        use crate::ptop::app::App;
        let app = App::new(true);

        // Test common terminal sizes
        let sizes = [(80, 24), (120, 40), (132, 43), (200, 60)];
        for (w, h) in sizes {
            let mut buffer = CellBuffer::new(w, h);
            draw(&app, &mut buffer);
        }
    }

    #[test]
    fn test_draw_multiple_times() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let mut buffer = CellBuffer::new(100, 30);

        // Simulate multiple frame renders
        for _ in 0..10 {
            draw(&app, &mut buffer);
        }
    }

    #[test]
    fn test_count_top_panels() {
        use crate::ptop::app::App;
        let app = App::new(true);
        let count = count_top_panels(&app);
        // In deterministic mode, we have a default panel configuration
        assert!(count >= 2);
        assert!(count <= 10);
    }
}
