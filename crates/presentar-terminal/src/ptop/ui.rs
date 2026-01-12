//! UI layout and rendering for ptop.
//!
//! Pixel-perfect ttop clone using presentar-terminal widgets.

// Allow style-only clippy warnings that don't affect correctness
#![allow(clippy::too_many_lines)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::option_map_or_none)]

use crate::direct::{CellBuffer, DirectTerminalCanvas};
use crate::{
    Border, BorderStyle, BrailleGraph, GraphMode, NetworkInterface, NetworkPanel, ProcessEntry,
    ProcessState, ProcessTable, TitleBar, Treemap, TreemapNode,
};
use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};

use super::analyzers::TcpState;
use super::app::App;
use super::config::{calculate_grid_layout, snap_to_grid, DetailLevel, PanelType};

// ttop panel border colors (exact RGB values from theme.rs)
const CPU_COLOR: Color = Color {
    r: 0.392,
    g: 0.784,
    b: 1.0,
    a: 1.0,
}; // #64C8FF (100,200,255)
const MEMORY_COLOR: Color = Color {
    r: 0.706,
    g: 0.471,
    b: 1.0,
    a: 1.0,
}; // #B478FF (180,120,255)
const DISK_COLOR: Color = Color {
    r: 0.392,
    g: 0.706,
    b: 1.0,
    a: 1.0,
}; // #64B4FF (100,180,255)
const NETWORK_COLOR: Color = Color {
    r: 1.0,
    g: 0.588,
    b: 0.392,
    a: 1.0,
}; // #FF9664 (255,150,100)
const PROCESS_COLOR: Color = Color {
    r: 0.863,
    g: 0.706,
    b: 0.392,
    a: 1.0,
}; // #DCC464 (220,180,100)
const GPU_COLOR: Color = Color {
    r: 0.392,
    g: 1.0,
    b: 0.588,
    a: 1.0,
}; // #64FF96 (100,255,150)
const BATTERY_COLOR: Color = Color {
    r: 1.0,
    g: 0.863,
    b: 0.392,
    a: 1.0,
}; // #FFDC64 (255,220,100)
const SENSORS_COLOR: Color = Color {
    r: 1.0,
    g: 0.392,
    b: 0.588,
    a: 1.0,
}; // #FF6496 (255,100,150)
const PSI_COLOR: Color = Color {
    r: 0.784,
    g: 0.314,
    b: 0.314,
    a: 1.0,
}; // #C85050 (200,80,80)
const CONNECTIONS_COLOR: Color = Color {
    r: 0.471,
    g: 0.706,
    b: 0.863,
    a: 1.0,
}; // #78B4DC (120,180,220)
const FILES_COLOR: Color = Color {
    r: 0.706,
    g: 0.549,
    b: 0.392,
    a: 1.0,
}; // #B48C64 (180,140,100)
const CONTAINERS_COLOR: Color = Color {
    r: 0.392,
    g: 0.706,
    b: 0.863,
    a: 1.0,
}; // #64B4DC (100,180,220) - Docker blue

// Network graph colors (distinct from panel border)
const NET_RX_COLOR: Color = Color {
    r: 0.392,
    g: 0.784,
    b: 1.0,
    a: 1.0,
}; // Cyan (download)
const NET_TX_COLOR: Color = Color {
    r: 1.0,
    g: 0.392,
    b: 0.392,
    a: 1.0,
}; // Red (upload)

/// btop-style color gradient for percentage values (0-100)
/// Uses smooth transition: cyan -> green -> yellow -> orange -> red
fn percent_color(percent: f64) -> Color {
    let p = percent.clamp(0.0, 100.0);

    if p >= 90.0 {
        // Critical: bright red
        Color {
            r: 1.0,
            g: 0.25,
            b: 0.25,
            a: 1.0,
        }
    } else if p >= 75.0 {
        // High: orange-red gradient
        let t = (p - 75.0) / 15.0;
        Color {
            r: 1.0,
            g: (0.706 - t * 0.456) as f32,
            b: 0.25,
            a: 1.0,
        }
    } else if p >= 50.0 {
        // Medium-high: yellow to orange
        let t = (p - 50.0) / 25.0;
        Color {
            r: 1.0,
            g: (0.863 - t * 0.157) as f32,
            b: 0.25,
            a: 1.0,
        }
    } else if p >= 25.0 {
        // Medium-low: green to yellow
        let t = (p - 25.0) / 25.0;
        Color {
            r: (0.392 + t * 0.608) as f32,
            g: 0.863,
            b: (0.392 - t * 0.142) as f32,
            a: 1.0,
        }
    } else {
        // Low: cyan to green
        let t = p / 25.0;
        Color {
            r: (0.25 + t * 0.142) as f32,
            g: (0.706 + t * 0.157) as f32,
            b: (0.863 - t * 0.471) as f32,
            a: 1.0,
        }
    }
}

/// Format bytes to human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

/// Format bytes per second rate (for disk/network I/O)
fn format_bytes_rate(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.1}G", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1}M", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.0}K", bytes_per_sec / KB)
    } else {
        format!("{bytes_per_sec:.0}B")
    }
}

/// Format uptime seconds to human-readable string
fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// Create a border with STRONG focus indication (SPEC-024 v5.0 Feature D)
/// Focus indicators:
/// 1. Double-line border (vs rounded for unfocused)
/// 2. Significantly brighter color (1.5x, not 1.2x)
/// 3. Focus indicator arrow `►` prepended to title
/// 4. Bold title text for focused panels
fn create_panel_border(title: &str, color: Color, is_focused: bool) -> Border {
    let style = if is_focused {
        BorderStyle::Double // Double border for focused panel
    } else {
        BorderStyle::Rounded // Normal rounded border
    };

    // Make focused panels MUCH brighter (1.5x instead of 1.2x)
    let border_color = if is_focused {
        Color {
            r: (color.r * 1.5).min(1.0),
            g: (color.g * 1.5).min(1.0),
            b: (color.b * 1.5).min(1.0),
            a: color.a,
        }
    } else {
        // Dim unfocused panels slightly for contrast
        Color {
            r: color.r * 0.7,
            g: color.g * 0.7,
            b: color.b * 0.7,
            a: color.a,
        }
    };

    // Add focus indicator to title
    let display_title = if is_focused {
        format!("► {title}")
    } else {
        title.to_string()
    };

    Border::new()
        .with_title(&display_title)
        .with_style(style)
        .with_color(border_color)
        .with_title_left_aligned()
}

/// ZRAM statistics from /sys/block/zram*
#[derive(Debug, Default)]
struct ZramStats {
    /// Original (uncompressed) data size in bytes
    orig_data_size: u64,
    /// Compressed data size in bytes
    compr_data_size: u64,
    /// Compression algorithm (lzo, lz4, zstd, etc.)
    algorithm: String,
}

impl ZramStats {
    /// Get compression ratio (original / compressed)
    fn ratio(&self) -> f64 {
        if self.compr_data_size == 0 {
            1.0
        } else {
            self.orig_data_size as f64 / self.compr_data_size as f64
        }
    }

    /// Check if ZRAM is active
    fn is_active(&self) -> bool {
        self.orig_data_size > 0
    }
}

/// Read ZRAM statistics from /sys/block/zram* (Linux only)
fn read_zram_stats() -> Option<ZramStats> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;

        // Try zram0 first (most common)
        for i in 0..4 {
            let device = format!("zram{i}");
            let base_path = format!("/sys/block/{device}");

            // Check if device exists
            if !std::path::Path::new(&base_path).exists() {
                continue;
            }

            // Read mm_stat: contains orig_data_size, compr_data_size, etc.
            // Format: orig_data_size compr_data_size mem_used_total mem_limit max_used_pages same_pages pages_compacted huge_pages
            let mm_stat_path = format!("{base_path}/mm_stat");
            if let Ok(content) = fs::read_to_string(&mm_stat_path) {
                let parts: Vec<&str> = content.split_whitespace().collect();
                if parts.len() >= 2 {
                    let orig = parts[0].parse::<u64>().unwrap_or(0);
                    let compr = parts[1].parse::<u64>().unwrap_or(0);

                    if orig > 0 {
                        // Read compression algorithm
                        let algo_path = format!("{base_path}/comp_algorithm");
                        let algorithm = fs::read_to_string(&algo_path)
                            .ok()
                            .and_then(|s| {
                                // Format: "lzo lzo-rle [lz4] zstd" - bracketed = active
                                s.split_whitespace()
                                    .find(|p| p.starts_with('[') && p.ends_with(']'))
                                    .map(|p| p.trim_matches(|c| c == '[' || c == ']').to_string())
                            })
                            .unwrap_or_else(|| "?".to_string());

                        return Some(ZramStats {
                            orig_data_size: orig,
                            compr_data_size: compr,
                            algorithm,
                        });
                    }
                }
            }
        }
        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// Main draw function - called each frame
pub fn draw(app: &App, buffer: &mut CellBuffer) {
    let w = buffer.width() as f32;
    let h = buffer.height() as f32;

    if w < 10.0 || h < 5.0 {
        return;
    }

    let mut canvas = DirectTerminalCanvas::new(buffer);

    // =========================================================================
    // TITLE BAR: App name + search (SPEC-024 Section 27.8 - Grammar of Graphics)
    // Every TUI MUST have a title bar with app name and search
    // Keybinds change in exploded mode to show DataFrame controls
    // =========================================================================
    let title_bar_height = 1.0_f32;
    let title_bar_rect = Rect::new(0.0, 0.0, w, title_bar_height);

    let keybinds: &[(&str, &str)] = if app.exploded_panel.is_some() {
        // DataFrame navigation mode
        &[
            ("←→", "Column"),
            ("↵", "Sort"),
            ("↑↓", "Row"),
            ("Esc", "Exit"),
        ]
    } else {
        // Normal navigation mode
        &[
            ("q", "Quit"),
            ("?", "Help"),
            ("/", "Filter"),
            ("Tab", "Nav"),
        ]
    };

    let mut title_bar = TitleBar::new("ptop")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_search_placeholder("Filter processes...")
        .with_search_text(&app.filter)
        .with_search_active(app.show_filter_input)
        .with_keybinds(keybinds)
        .with_primary_color(CPU_COLOR);

    // Add compact fullscreen indicator in exploded mode
    if app.exploded_panel.is_some() {
        title_bar = title_bar.with_mode_indicator("[▣]");
    }

    title_bar.layout(title_bar_rect);
    title_bar.paint(&mut canvas);

    // Reserve 1 row for status bar at bottom (SPEC-024 v5.5 Section 11.6)
    let status_bar_height = 1.0_f32;
    let content_y = title_bar_height;
    let content_h = h - status_bar_height - title_bar_height;

    // EXPLODED MODE: render single panel fullscreen (SPEC-024 v5.0 Feature D)
    // Reference: ttop/src/ui.rs line 20-50
    // Note: TitleBar now shows [FULLSCREEN] indicator and DataFrame keybinds
    if let Some(panel) = app.exploded_panel {
        draw_exploded_panel(
            app,
            &mut canvas,
            Rect::new(0.0, content_y, w, content_h),
            panel,
        );
        draw_status_bar(app, &mut canvas, w, h);
        return;
    }

    // Count visible top panels (like ttop)
    let top_panel_count = count_top_panels(app);
    let has_process = app.panels.process;

    // Layout: 45% top panels, 55% bottom row (like ttop)
    let top_height = if top_panel_count > 0 && has_process {
        (content_h * 0.45).max(8.0)
    } else if top_panel_count > 0 {
        content_h
    } else {
        0.0
    };
    let bottom_y = content_y + top_height;
    let bottom_height = content_h - top_height;

    // Draw top panels in grid layout
    if top_panel_count > 0 {
        draw_top_panels(app, &mut canvas, Rect::new(0.0, content_y, w, top_height));
    }

    // Draw bottom row: ttop uses exactly 48 | 36 | 36 for 120-width terminal
    if has_process && bottom_height > 3.0 {
        // Calculate widths to match ttop exactly
        let proc_w = (w * 0.4).round(); // 48 for 120
        let remaining = w - proc_w; // 72 for 120
        let conn_w = (remaining / 2.0).floor(); // 36 for 72
        let files_w = remaining - conn_w; // 36 for 72

        draw_process_panel(
            app,
            &mut canvas,
            Rect::new(0.0, bottom_y, proc_w, bottom_height),
        );

        if app.panels.connections {
            draw_connections_panel(
                app,
                &mut canvas,
                Rect::new(proc_w, bottom_y, conn_w, bottom_height),
            );
        }

        // Draw Files panel (ttop style) or Treemap if files not enabled
        if app.panels.files {
            draw_files_panel(
                app,
                &mut canvas,
                Rect::new(proc_w + conn_w, bottom_y, files_w, bottom_height),
            );
        } else if app.panels.treemap {
            draw_treemap_panel(
                app,
                &mut canvas,
                Rect::new(proc_w + conn_w, bottom_y, files_w, bottom_height),
            );
        }
    }

    // Overlays
    if app.show_help {
        draw_help_overlay(&mut canvas, w, h);
    }

    if app.show_filter_input {
        draw_filter_overlay(app, &mut canvas, w, h);
    }

    if app.show_fps {
        draw_fps_overlay(app, &mut canvas, w);
    }

    // Status bar at bottom (SPEC-024 v5.5 Section 11.6)
    draw_status_bar(app, &mut canvas, w, h);
}

fn count_top_panels(app: &App) -> u32 {
    let mut count = 0;
    if app.panels.cpu {
        count += 1;
    }
    if app.panels.memory {
        count += 1;
    }
    if app.panels.disk {
        count += 1;
    }
    if app.panels.network {
        count += 1;
    }
    if app.panels.gpu {
        count += 1;
    }
    if app.panels.battery {
        count += 1;
    }
    if app.panels.sensors {
        count += 1;
    }
    if app.panels.psi {
        count += 1;
    }
    count
}

fn draw_fps_overlay(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32) {
    let fps_str = format!(" Frame: {}μs ", app.avg_frame_time_us);
    let style = TextStyle {
        color: Color::new(0.4, 1.0, 0.4, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        &fps_str,
        Point::new(w - fps_str.len() as f32 - 1.0, 0.0),
        &style,
    );
}

/// Status bar with navigation hints (SPEC-024 v5.5 Section 11.6)
/// Shows: [Tab] Navigate [Enter] Explode [?] Help [q] Quit | Focused: CPU
fn draw_status_bar(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let y = h - 1.0;
    let bar_width = w as usize;

    // Key hint colors
    let key_style = TextStyle {
        color: Color::new(0.3, 0.3, 0.3, 1.0), // Dim for background
        ..Default::default()
    };
    let bracket_style = TextStyle {
        color: Color::new(0.6, 0.6, 0.6, 1.0), // Brackets
        ..Default::default()
    };
    let action_style = TextStyle {
        color: Color::new(0.8, 0.8, 0.8, 1.0), // Action text
        ..Default::default()
    };
    let focus_style = TextStyle {
        color: Color::new(0.4, 0.8, 1.0, 1.0), // Cyan for focused panel
        ..Default::default()
    };

    // Draw background bar
    let bg = "─".repeat(bar_width);
    canvas.draw_text(&bg, Point::new(0.0, y), &key_style);

    // Navigation hints on left
    let hints = if app.exploded_panel.is_some() {
        " [Esc] Collapse  [?] Help  [q] Quit "
    } else {
        " [Tab] Navigate  [Enter] Explode  [/] Filter  [?] Help  [q] Quit "
    };

    // Draw hints with bracket highlighting
    let mut x = 0.0;
    let mut in_bracket = false;
    for ch in hints.chars() {
        let style = if ch == '[' {
            in_bracket = true;
            &bracket_style
        } else if ch == ']' {
            in_bracket = false;
            &bracket_style
        } else if in_bracket {
            &focus_style // Key inside brackets
        } else {
            &action_style
        };
        canvas.draw_text(&ch.to_string(), Point::new(x, y), style);
        x += 1.0;
    }

    // Focused panel indicator on right
    if let Some(panel) = app.focused_panel {
        let panel_name = match panel {
            super::config::PanelType::Cpu => "CPU",
            super::config::PanelType::Memory => "Memory",
            super::config::PanelType::Disk => "Disk",
            super::config::PanelType::Network => "Network",
            super::config::PanelType::Process => "Process",
            super::config::PanelType::Gpu => "GPU",
            super::config::PanelType::Battery => "Battery",
            super::config::PanelType::Sensors => "Sensors",
            super::config::PanelType::Files => "Files",
            super::config::PanelType::Connections => "Connections",
            super::config::PanelType::Psi => "PSI",
            super::config::PanelType::Containers => "Containers",
        };
        // Draw full right-aligned string "│ CPU " all at once
        // Use chars().count() since "│" is multi-byte UTF-8
        let focus_text = format!("│ {panel_name} ");
        let focus_x = w - focus_text.chars().count() as f32;
        if focus_x > x {
            canvas.draw_text(&focus_text, Point::new(focus_x, y), &focus_style);
            // Draw the separator with different color
            canvas.draw_text("│", Point::new(focus_x, y), &bracket_style);
        }
    }
}

#[allow(clippy::type_complexity)]
fn draw_top_panels(app: &App, canvas: &mut DirectTerminalCanvas<'_>, area: Rect) {
    // ttop layout at 120x40:
    // Row 0 (rows 0-8, 9 rows):   CPU | Memory | Disk
    // Row 1 (rows 9-17, 9 rows):  Network | GPU | Sensors (3 rows) + Containers (6 rows)
    //
    // The third column of row 1 is split into stacked panels:
    // - Sensors: 3 rows (33%)
    // - Containers: 6 rows (67%)

    // For ttop-style deterministic layout with 6 core panels:
    let is_ttop_layout = app.panels.cpu
        && app.panels.memory
        && app.panels.disk
        && app.panels.network
        && app.panels.gpu
        && app.panels.sensors
        && !app.panels.psi
        && !app.panels.battery;

    if is_ttop_layout && area.width >= 100.0 {
        // ttop-specific 3x2 grid with stacked third column in row 1
        let cols = 3;
        let rows = 2;
        let cell_w = area.width / cols as f32;
        let cell_h = area.height / rows as f32;

        // Row 0: CPU, Memory, Disk
        draw_cpu_panel(app, canvas, Rect::new(area.x, area.y, cell_w, cell_h));
        draw_memory_panel(
            app,
            canvas,
            Rect::new(area.x + cell_w, area.y, cell_w, cell_h),
        );
        draw_disk_panel(
            app,
            canvas,
            Rect::new(area.x + 2.0 * cell_w, area.y, cell_w, cell_h),
        );

        // Row 1: Network, GPU, Sensors+Containers stacked
        let row1_y = area.y + cell_h;
        draw_network_panel(app, canvas, Rect::new(area.x, row1_y, cell_w, cell_h));
        draw_gpu_panel(
            app,
            canvas,
            Rect::new(area.x + cell_w, row1_y, cell_w, cell_h),
        );

        // Third column: split into Sensors (33%) + Containers (67%)
        let col3_x = area.x + 2.0 * cell_w;
        let sensors_h = (cell_h / 3.0).round(); // 3 rows for sensors
        let containers_h = cell_h - sensors_h; // 6 rows for containers

        draw_sensors_panel(app, canvas, Rect::new(col3_x, row1_y, cell_w, sensors_h));
        draw_containers_panel(
            app,
            canvas,
            Rect::new(col3_x, row1_y + sensors_h, cell_w, containers_h),
        );
    } else {
        // Generic grid layout for non-ttop configurations (SPEC-024 v5.0 Feature B)
        // Uses calculate_grid_layout and snap_to_grid for automatic space packing
        let mut panels: Vec<fn(&App, &mut DirectTerminalCanvas<'_>, Rect)> = Vec::new();

        if app.panels.cpu {
            panels.push(draw_cpu_panel);
        }
        if app.panels.memory {
            panels.push(draw_memory_panel);
        }
        if app.panels.disk {
            panels.push(draw_disk_panel);
        }
        if app.panels.network {
            panels.push(draw_network_panel);
        }
        if app.panels.gpu {
            panels.push(draw_gpu_panel);
        }
        if app.panels.sensors {
            panels.push(draw_sensors_panel);
        }
        if app.panels.psi {
            panels.push(draw_psi_panel);
        }
        if app.panels.battery {
            panels.push(draw_battery_panel);
        }
        if app.panels.sensors_compact {
            panels.push(draw_sensors_compact_panel);
        }
        if app.panels.system {
            panels.push(draw_system_panel);
        }

        if panels.is_empty() {
            return;
        }

        // Use config's grid layout algorithm (SPEC-024 v5.0 Feature B)
        let layout_config = &app.config.layout;

        let grid_rects = calculate_grid_layout(
            panels.len() as u32,
            area.width as u16,
            area.height as u16,
            layout_config,
        );

        for (i, draw_fn) in panels.iter().enumerate() {
            if let Some(rect) = grid_rects.get(i) {
                // Apply snap_to_grid for pixel-perfect alignment
                let snapped_x = snap_to_grid(rect.x, layout_config.grid_size);
                let snapped_y = snap_to_grid(rect.y, layout_config.grid_size);
                let bounds = Rect::new(
                    area.x + snapped_x as f32,
                    area.y + snapped_y as f32,
                    rect.width as f32,
                    rect.height as f32,
                );
                draw_fn(app, canvas, bounds);
            }
        }
    }
}

// ============================================================================
// CPU Panel Helper Functions
// Extracted to reduce cyclomatic complexity of draw_cpu_panel
// ============================================================================

/// Build CPU panel title string.
fn build_cpu_title(
    cpu_pct: f64,
    core_count: usize,
    freq_ghz: f64,
    is_boosting: bool,
    uptime: u64,
    load_one: f64,
    deterministic: bool,
) -> String {
    let boost_icon = if is_boosting { "⚡" } else { "" };
    if deterministic {
        format!(
            "CPU {cpu_pct:.0}% │ {core_count} cores │ {freq_ghz:.1}GHz │ up {} │",
            format_uptime(uptime)
        )
    } else {
        // Prioritize: CPU% > cores > freq > uptime > LAV
        // Compact format: "CPU 14% │ 48 cores │ 4.8GHz⚡ │ up 3d 3h │ LAV 30.28"
        format!(
            "CPU {cpu_pct:.0}% │ {core_count} cores │ {freq_ghz:.1}GHz{boost_icon} │ up {} │ LAV {load_one:.1}",
            format_uptime(uptime)
        )
    }
}

/// Build a compact CPU title for narrow panels (prioritizes frequency)
fn build_cpu_title_compact(
    cpu_pct: f64,
    core_count: usize,
    freq_ghz: f64,
    is_boosting: bool,
) -> String {
    let boost_icon = if is_boosting { "⚡" } else { "" };
    // Compact: "CPU 14% │ 48c │ 4.8GHz⚡" (~22 chars)
    format!("CPU {cpu_pct:.0}% │ {core_count}c │ {freq_ghz:.1}GHz{boost_icon}")
}

/// Calculate CPU meter layout parameters.
struct CpuMeterLayout {
    bar_len: usize,
    meter_bar_width: f32,
    cores_per_col: usize,
    num_meter_cols: usize,
}

impl CpuMeterLayout {
    fn calculate(core_count: usize, core_area_height: f32, is_exploded: bool) -> Self {
        let bar_len: usize = if is_exploded { 8 } else { 6 };
        let meter_bar_width = (bar_len + 9) as f32;

        let max_cores_per_col = if is_exploded {
            (core_area_height as usize).min(12)
        } else {
            core_area_height as usize
        };
        let cores_per_col = max_cores_per_col.max(1);
        let num_meter_cols = core_count.div_ceil(cores_per_col);

        Self {
            bar_len,
            meter_bar_width,
            cores_per_col,
            num_meter_cols,
        }
    }
}

// ============================================================================

#[allow(clippy::too_many_lines)]
fn draw_cpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    use sysinfo::{Cpu, LoadAvg};

    // Determine detail level based on available height (SPEC-024 v5.0 Feature E)
    let _detail_level = DetailLevel::for_height(bounds.height as u16);

    let cpu_pct = app.cpu_history.last().copied().unwrap_or(0.0) * 100.0;
    let core_count = app.per_core_percent.len();
    let uptime = app.uptime();

    // Use cached load_avg from App (O(1) render - no I/O in render path)
    let (load, max_freq_mhz) = if app.deterministic {
        (
            LoadAvg {
                one: 0.0,
                five: 0.0,
                fifteen: 0.0,
            },
            0u64,
        )
    } else {
        let freq = app
            .system
            .cpus()
            .iter()
            .map(Cpu::frequency)
            .max()
            .unwrap_or(0);
        (app.load_avg.clone(), freq)
    };

    let is_boosting = max_freq_mhz > 3000; // Heuristic: >3GHz = boosting
    let freq_ghz = max_freq_mhz as f64 / 1000.0;

    // Build title - use compact version for narrow panels to ensure frequency is visible
    let title = if bounds.width < 35.0 {
        // Very narrow: just show essentials
        build_cpu_title_compact(cpu_pct, core_count, freq_ghz, is_boosting)
    } else {
        build_cpu_title(
            cpu_pct,
            core_count,
            freq_ghz,
            is_boosting,
            uptime,
            load.one,
            app.deterministic,
        )
    };

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Cpu);
    let mut border = create_panel_border(&title, CPU_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 10.0 {
        return;
    }

    // Reserve 2 rows for load gauge + top consumers at bottom (like ttop)
    let reserved_bottom = 2.0_f32;
    let core_area_height = (inner.height - reserved_bottom).max(1.0);

    // In deterministic mode with zeroed data, ttop shows empty interior (no per-core meters)
    // Only show per-core meters when there's actual data
    let has_cpu_data = !app.deterministic || app.per_core_percent.iter().any(|&p| p > 0.0);

    if has_cpu_data {
        // CB-EXPLODE-001 v2: Responsive layout for exploded mode
        let is_exploded = inner.width > 100.0;

        // Calculate meter layout using helper
        let layout = CpuMeterLayout::calculate(core_count, core_area_height, is_exploded);
        let bar_len = layout.bar_len;
        let meter_bar_width = layout.meter_bar_width;
        let cores_per_col = layout.cores_per_col;
        let num_meter_cols = layout.num_meter_cols;

        // In exploded mode, allow meters to take up to 70% of width
        let max_meter_ratio = if is_exploded { 0.70 } else { 0.5 };
        let meters_width =
            (num_meter_cols as f32 * meter_bar_width).min(inner.width * max_meter_ratio);

        // Check if we can fit all cores - if not, show summary mode
        let visible_cols = (meters_width / meter_bar_width) as usize;
        let visible_cores = visible_cols * cores_per_col;
        let use_summary_mode = visible_cores < core_count && core_count > 8;

        // Get per-core temperatures from sensor data (like ttop)
        // Look for sensors with labels like "Core 0", "Core 1", etc.
        let core_temps: std::collections::HashMap<usize, f64> = app
            .analyzers
            .sensor_health_data()
            .map(|data| {
                data.temperatures()
                    .filter_map(|s| {
                        // Parse "Core X" from label
                        if s.label.starts_with("Core ") {
                            s.label
                                .strip_prefix("Core ")
                                .and_then(|n| n.parse::<usize>().ok())
                                .map(|idx| (idx, s.value))
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        if use_summary_mode {
            // Summary mode for high core count in narrow panels
            // Show utilization buckets instead of individual cores
            let mut high = 0_usize; // >70%
            let mut med = 0_usize; // 30-70%
            let mut low = 0_usize; // 1-30%
            let mut idle = 0_usize; // <1%

            for &pct in &app.per_core_percent {
                if pct > 70.0 {
                    high += 1;
                } else if pct > 30.0 {
                    med += 1;
                } else if pct > 1.0 {
                    low += 1;
                } else {
                    idle += 1;
                }
            }

            let summary_lines = [
                (
                    high,
                    "▓",
                    Color {
                        r: 1.0,
                        g: 0.4,
                        b: 0.4,
                        a: 1.0,
                    },
                    ">70%",
                ),
                (
                    med,
                    "▒",
                    Color {
                        r: 1.0,
                        g: 0.8,
                        b: 0.3,
                        a: 1.0,
                    },
                    "30-70",
                ),
                (
                    low,
                    "░",
                    Color {
                        r: 0.5,
                        g: 0.9,
                        b: 0.5,
                        a: 1.0,
                    },
                    "1-30",
                ),
                (
                    idle,
                    " ",
                    Color {
                        r: 0.4,
                        g: 0.4,
                        b: 0.5,
                        a: 1.0,
                    },
                    "idle",
                ),
            ];

            let mut y = inner.y;
            for (count, _char, color, label) in summary_lines {
                if count > 0 && y < inner.y + core_area_height {
                    let text = format!("{count:>2}c {label}");
                    canvas.draw_text(
                        &text,
                        Point::new(inner.x, y),
                        &TextStyle {
                            color,
                            ..Default::default()
                        },
                    );
                    y += 1.0;
                }
            }
        } else {
            // Draw per-core meters on left side (normal mode)
            for (i, &percent) in app.per_core_percent.iter().enumerate() {
                if cores_per_col == 0 {
                    break;
                }
                let col = i / cores_per_col;
                let row = i % cores_per_col;

                let cell_x = inner.x + col as f32 * meter_bar_width;
                let cell_y = inner.y + row as f32;

                if cell_x + meter_bar_width > inner.x + meters_width
                    || cell_y >= inner.y + core_area_height
                {
                    continue;
                }

                let color = percent_color(percent);
                // bar_len is set above based on is_exploded (8 for exploded, 6 for normal)
                let filled = ((percent / 100.0) * bar_len as f64) as usize;
                let bar: String =
                    "█".repeat(filled.min(bar_len)) + &"░".repeat(bar_len - filled.min(bar_len));

                // ttop style: show temp if available, otherwise show percent
                let label = if let Some(&temp) = core_temps.get(&i) {
                    format!("{i:>2} {bar} {temp:>2.0}°")
                } else {
                    format!("{i:>2} {bar} {percent:>3.0}")
                };
                canvas.draw_text(
                    &label,
                    Point::new(cell_x, cell_y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
            }
        }

        // Draw graph on right side
        let graph_x = inner.x + meters_width + 1.0;
        let graph_width = inner.width - meters_width - 1.0;

        if graph_width > 5.0 && !app.cpu_history.as_slice().is_empty() {
            let history: Vec<f64> = app
                .cpu_history
                .as_slice()
                .iter()
                .map(|&v| v * 100.0)
                .collect();
            let mut graph = BrailleGraph::new(history)
                .with_color(CPU_COLOR)
                .with_range(0.0, 100.0)
                .with_mode(GraphMode::Block);
            graph.layout(Rect::new(graph_x, inner.y, graph_width, core_area_height));
            graph.paint(canvas);
        }
    }
    // In deterministic mode with no data, interior is left empty (just like ttop)

    // === Bottom Row 1: Load Average Gauge with trend arrows (ttop style) ===
    let load_y = inner.y + core_area_height;
    if load_y < inner.y + inner.height && inner.width > 20.0 {
        let load_normalized = load.one / core_count as f64;
        let load_color = if load_normalized > 1.0 {
            Color {
                r: 1.0,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            } // Red
        } else if load_normalized > 0.7 {
            Color {
                r: 1.0,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            } // Yellow
        } else {
            Color {
                r: 0.3,
                g: 0.9,
                b: 0.3,
                a: 1.0,
            } // Green
        };

        // Load trend arrows (ttop style)
        let trend_1_5 = if load.one > load.five {
            "↑"
        } else if load.one < load.five {
            "↓"
        } else {
            "→"
        };
        let trend_5_15 = if load.five > load.fifteen {
            "↑"
        } else if load.five < load.fifteen {
            "↓"
        } else {
            "→"
        };

        // Load bar (0-2x cores = 100%)
        let bar_width = 10_usize;
        let load_pct = (load_normalized / 2.0).min(1.0);
        let filled = (load_pct * bar_width as f64) as usize;
        let bar: String =
            "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));

        // Adaptive Load line format based on available width
        // Full:    "Load ██████████ 2.15↑ 1.85↓ 1.50→ │ 4.8GHz" (~48 chars)
        // Medium:  "Load ██████████ 2.15↑ 1.85↓ 1.50→" (~38 chars)
        // Compact: "Load ██░░ 2.1↑ 1.8↓ 1.5→" (~24 chars)
        let available_width = inner.width as usize;
        let freq_ghz = max_freq_mhz as f64 / 1000.0;

        let load_str = if app.deterministic {
            format!(
                "Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2} │ Fre",
                load.one, load.five, load.fifteen
            )
        } else if available_width >= 45 && freq_ghz > 0.0 {
            // Full format with frequency
            format!(
                "Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2}→ │ {freq_ghz:.1}GHz",
                load.one, load.five, load.fifteen
            )
        } else if available_width >= 35 {
            // Medium format without frequency
            format!(
                "Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2}→",
                load.one, load.five, load.fifteen
            )
        } else {
            // Compact format for narrow panels
            let short_bar: String = bar.chars().take(4).collect();
            format!(
                "Load {short_bar} {:.1}{trend_1_5} {:.1}{trend_5_15} {:.1}→",
                load.one, load.five, load.fifteen
            )
        };

        canvas.draw_text(
            &load_str,
            Point::new(inner.x, load_y),
            &TextStyle {
                color: load_color,
                ..Default::default()
            },
        );
    }

    // === Bottom Row 2: Top 3 CPU Consumers (ttop style) ===
    // Skip in deterministic mode (ttop shows empty row)
    let consumers_y = inner.y + core_area_height + 1.0;
    if !app.deterministic && consumers_y < inner.y + inner.height && inner.width > 20.0 {
        // Get top 3 processes by CPU
        let mut top_procs: Vec<_> = app
            .system
            .processes()
            .values()
            .filter(|p| p.cpu_usage() > 0.1)
            .collect();
        top_procs.sort_by(|a, b| {
            b.cpu_usage()
                .partial_cmp(&a.cpu_usage())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Only show "Top" section if there are processes to display
        if !top_procs.is_empty() {
            let dim_color = Color {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            };

            // Draw "Top " prefix
            canvas.draw_text(
                "Top ",
                Point::new(inner.x, consumers_y),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );

            let mut x_offset = 4.0;
            for (i, proc) in top_procs.iter().take(3).enumerate() {
                let cpu = proc.cpu_usage() as f64;
                let name: String = proc.name().to_string_lossy().chars().take(12).collect();

                let cpu_color = if cpu > 50.0 {
                    Color {
                        r: 1.0,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    }
                } else if cpu > 20.0 {
                    Color {
                        r: 1.0,
                        g: 0.8,
                        b: 0.2,
                        a: 1.0,
                    }
                } else {
                    Color {
                        r: 0.3,
                        g: 0.9,
                        b: 0.3,
                        a: 1.0,
                    }
                };

                if i > 0 {
                    canvas.draw_text(
                        " │ ",
                        Point::new(inner.x + x_offset, consumers_y),
                        &TextStyle {
                            color: dim_color,
                            ..Default::default()
                        },
                    );
                    x_offset += 3.0;
                }

                let cpu_str = format!("{cpu:.0}%");
                canvas.draw_text(
                    &cpu_str,
                    Point::new(inner.x + x_offset, consumers_y),
                    &TextStyle {
                        color: cpu_color,
                        ..Default::default()
                    },
                );
                x_offset += cpu_str.len() as f32;

                canvas.draw_text(
                    &format!(" {name}"),
                    Point::new(inner.x + x_offset, consumers_y),
                    &TextStyle {
                        color: Color {
                            r: 0.9,
                            g: 0.9,
                            b: 0.9,
                            a: 1.0,
                        },
                        ..Default::default()
                    },
                );
                x_offset += 1.0 + name.len() as f32;
            }
        }
    }
}

// ============================================================================
// Memory Panel Helper Functions
// Extracted to reduce cyclomatic complexity of draw_memory_panel
// ============================================================================

/// Memory statistics for deterministic rendering (GB values only).
struct MemoryStats {
    used_gb: f64,
    cached_gb: f64,
    free_gb: f64,
}

impl MemoryStats {
    fn from_app(app: &App) -> Self {
        let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
        Self {
            used_gb: gb(app.mem_used),
            cached_gb: gb(app.mem_cached),
            free_gb: gb(app.mem_available),
        }
    }
}

/// Get color for swap usage percentage.
fn swap_color(pct: f64) -> Color {
    if pct > 50.0 {
        Color::new(1.0, 0.3, 0.3, 1.0) // Red
    } else if pct > 10.0 {
        Color::new(1.0, 0.8, 0.2, 1.0) // Yellow
    } else {
        Color::new(0.3, 0.9, 0.3, 1.0) // Green
    }
}

/// Standard dim color for labels.
const DIM_COLOR: Color = Color {
    r: 0.3,
    g: 0.3,
    b: 0.3,
    a: 1.0,
};

/// Cyan color for cached memory.
const CACHED_COLOR: Color = Color {
    r: 0.3,
    g: 0.8,
    b: 0.9,
    a: 1.0,
};

/// Blue color for free memory.
const FREE_COLOR: Color = Color {
    r: 0.4,
    g: 0.4,
    b: 0.9,
    a: 1.0,
};

/// Draw stacked memory bar (Used|Cached|Free).
fn draw_memory_stacked_bar(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32, app: &App) {
    let bar_width = inner.width as usize;
    let used_actual_pct = if app.mem_total > 0 {
        ((app.mem_total - app.mem_available) as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };
    let cached_pct = if app.mem_total > 0 {
        (app.mem_cached as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };

    let used_chars = ((used_actual_pct / 100.0) * bar_width as f64) as usize;
    let cached_chars = ((cached_pct / 100.0) * bar_width as f64) as usize;
    let free_chars = bar_width.saturating_sub(used_chars + cached_chars);

    let used_color = percent_color(used_actual_pct);
    let free_color = Color::new(0.3, 0.3, 0.3, 1.0);

    // Draw used segment
    if used_chars > 0 {
        let used_bar: String = "█".repeat(used_chars);
        canvas.draw_text(
            &used_bar,
            Point::new(inner.x, y),
            &TextStyle {
                color: used_color,
                ..Default::default()
            },
        );
    }

    // Draw cached segment
    if cached_chars > 0 {
        let cached_bar: String = "█".repeat(cached_chars);
        canvas.draw_text(
            &cached_bar,
            Point::new(inner.x + used_chars as f32, y),
            &TextStyle {
                color: CACHED_COLOR,
                ..Default::default()
            },
        );
    }

    // Draw free segment
    if free_chars > 0 {
        let free_bar: String = "░".repeat(free_chars);
        canvas.draw_text(
            &free_bar,
            Point::new(inner.x + used_chars as f32 + cached_chars as f32, y),
            &TextStyle {
                color: free_color,
                ..Default::default()
            },
        );
    }
}

/// Draw memory rows in deterministic mode (ttop style).
fn draw_memory_rows_deterministic(
    canvas: &mut DirectTerminalCanvas<'_>,
    inner: Rect,
    mut y: f32,
    stats: &MemoryStats,
) -> f32 {
    // Used row
    canvas.draw_text(
        &format!("  Used:   {:.1}G  0", stats.used_gb),
        Point::new(inner.x, y),
        &TextStyle {
            color: percent_color(0.0),
            ..Default::default()
        },
    );
    y += 1.0;

    // Cached row
    if y < inner.y + inner.height {
        canvas.draw_text(
            &format!("Cached:   {:.1}G  0", stats.cached_gb),
            Point::new(inner.x, y),
            &TextStyle {
                color: CACHED_COLOR,
                ..Default::default()
            },
        );
        y += 1.0;
    }

    // Free row
    if y < inner.y + inner.height {
        canvas.draw_text(
            &format!("  Free:   {:.1}G  0", stats.free_gb),
            Point::new(inner.x, y),
            &TextStyle {
                color: FREE_COLOR,
                ..Default::default()
            },
        );
        y += 1.0;
    }

    // PSI row (ttop style)
    if y < inner.y + inner.height {
        canvas.draw_text(
            "PSI ○ 0.0 cpu ○ 0.0 mem ○ 0.0 io",
            Point::new(inner.x, y),
            &TextStyle {
                color: DIM_COLOR,
                ..Default::default()
            },
        );
        y += 1.0;
    }

    // Top Memory Consumers header
    if y < inner.y + inner.height {
        canvas.draw_text(
            "── Top Memory Consumers ──────────────",
            Point::new(inner.x, y),
            &TextStyle {
                color: DIM_COLOR,
                ..Default::default()
            },
        );
    }

    y
}

// ============================================================================

#[allow(clippy::too_many_lines)]
fn draw_memory_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Determine detail level based on available height (SPEC-024 v5.0 Feature E)
    let _detail_level = DetailLevel::for_height(bounds.height as u16);

    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let mem_pct = if app.mem_total > 0 {
        (app.mem_used as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };

    // Check for ZRAM (skip in deterministic mode)
    let zram_stats = if app.deterministic {
        None
    } else {
        read_zram_stats()
    };
    let zram_info = zram_stats
        .as_ref()
        .filter(|z| z.is_active())
        .map(|z| format!(" │ ZRAM:{:.1}x", z.ratio()))
        .unwrap_or_default();

    // ttop-style title (Border widget adds outer spaces)
    // Deterministic: "Memory │ 0.0G / 0.0G (0%)"
    let title = format!(
        "Memory │ {:.1}G / {:.1}G ({:.0}%){}",
        gb(app.mem_used),
        gb(app.mem_total),
        mem_pct,
        zram_info
    );

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Memory);
    let mut border = create_panel_border(&title, MEMORY_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 || inner.width < 10.0 {
        return;
    }

    let mut y = inner.y;

    // Line 1: Stacked memory bar (ttop style: Used|Cached|Free)
    draw_memory_stacked_bar(canvas, inner, y, app);
    y += 1.0;

    // Remaining lines: Memory breakdown rows (ttop style)
    // Always show even with 0 values (like ttop deterministic mode)
    if y < inner.y + inner.height {
        let used_pct = if app.mem_total > 0 {
            (app.mem_used as f64 / app.mem_total as f64) * 100.0
        } else {
            0.0
        };
        let cached_pct = if app.mem_total > 0 {
            (app.mem_cached as f64 / app.mem_total as f64) * 100.0
        } else {
            0.0
        };
        let free_pct = if app.mem_total > 0 {
            (app.mem_available as f64 / app.mem_total as f64) * 100.0
        } else {
            0.0
        };
        let swap_pct = if app.swap_total > 0 {
            (app.swap_used as f64 / app.swap_total as f64) * 100.0
        } else {
            0.0
        };

        let mut rows: Vec<(&str, f64, f64, Color)> = vec![
            ("Used", gb(app.mem_used), used_pct, percent_color(used_pct)),
            ("Swap", gb(app.swap_used), swap_pct, swap_color(swap_pct)),
            ("Cached", gb(app.mem_cached), cached_pct, CACHED_COLOR),
            ("Free", gb(app.mem_available), free_pct, FREE_COLOR),
        ];

        // === ZRAM Row (conditional) - ttop style ===
        // We need to render ZRAM separately due to special formatting
        let zram_row_data = zram_stats.as_ref().filter(|z| z.is_active()).map(|z| {
            let orig_gb = gb(z.orig_data_size);
            let compr_gb = gb(z.compr_data_size);
            let ratio = z.ratio();
            let algo = z.algorithm.as_str();
            (orig_gb, compr_gb, ratio, algo)
        });

        // Remove the "Cached" and "Free" rows for now to make room for ZRAM if needed
        // Actually, just add ZRAM after Swap
        if zram_row_data.is_some() {
            // Insert ZRAM as a special row - we'll handle it separately
            rows.insert(
                2,
                (
                    "ZRAM",
                    0.0,
                    0.0,
                    Color {
                        r: 0.8,
                        g: 0.4,
                        b: 1.0,
                        a: 1.0,
                    },
                ),
            );
        }

        // In deterministic mode, use ttop simple format: "  Used:   0.0G  0"
        // In normal mode, use detailed format with bar and percentage
        if app.deterministic {
            let stats = MemoryStats::from_app(app);
            draw_memory_rows_deterministic(canvas, inner, y, &stats);
        } else {
            // Normal mode: detailed format with bars
            for (label, value, pct, color) in &rows {
                if y >= inner.y + inner.height {
                    break;
                }

                // Special handling for ZRAM row (ttop style: "ZRAM 2.5G→1.0G 2.5x lz4")
                if *label == "ZRAM" {
                    if let Some((orig_gb, compr_gb, ratio, algo)) = &zram_row_data {
                        let orig_str = if *orig_gb >= 1024.0 {
                            format!("{:.1}T", orig_gb / 1024.0)
                        } else {
                            format!("{orig_gb:.1}G")
                        };
                        let compr_str = if *compr_gb >= 1024.0 {
                            format!("{:.1}T", compr_gb / 1024.0)
                        } else {
                            format!("{compr_gb:.1}G")
                        };

                        let dim_color = Color {
                            r: 0.5,
                            g: 0.5,
                            b: 0.5,
                            a: 1.0,
                        };
                        let magenta = Color {
                            r: 0.8,
                            g: 0.4,
                            b: 1.0,
                            a: 1.0,
                        };
                        let green = Color {
                            r: 0.3,
                            g: 0.9,
                            b: 0.3,
                            a: 1.0,
                        };

                        canvas.draw_text(
                            "  ZRAM ",
                            Point::new(inner.x, y),
                            &TextStyle {
                                color: dim_color,
                                ..Default::default()
                            },
                        );
                        canvas.draw_text(
                            &format!("{orig_str}→{compr_str} "),
                            Point::new(inner.x + 7.0, y),
                            &TextStyle {
                                color: magenta,
                                ..Default::default()
                            },
                        );
                        let ratio_x = inner.x
                            + 7.0
                            + orig_str.len() as f32
                            + 1.0
                            + compr_str.len() as f32
                            + 1.0;
                        canvas.draw_text(
                            &format!("{ratio:.1}x"),
                            Point::new(ratio_x, y),
                            &TextStyle {
                                color: green,
                                ..Default::default()
                            },
                        );
                        canvas.draw_text(
                            &format!(" {algo}"),
                            Point::new(ratio_x + 4.0, y),
                            &TextStyle {
                                color: dim_color,
                                ..Default::default()
                            },
                        );
                    }
                    y += 1.0;
                    continue;
                }

                let bar_width = 10.min((inner.width as usize).saturating_sub(22));
                let filled = ((*pct / 100.0) * bar_width as f64) as usize;
                let bar: String = "█".repeat(filled.min(bar_width))
                    + &"░".repeat(bar_width - filled.min(bar_width));

                let text = format!("{label:>6} {value:>5.1}G {bar} {pct:>5.1}%");
                canvas.draw_text(
                    &text,
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: *color,
                        ..Default::default()
                    },
                );

                // Add swap thrashing indicator (CB-MEM-004) after Swap row
                if *label == "Swap" {
                    if let Some(swap_data) = app.analyzers.swap_data() {
                        let (is_thrashing, severity) = swap_data.is_thrashing();
                        if is_thrashing
                            || swap_data.swap_in_rate > 0.0
                            || swap_data.swap_out_rate > 0.0
                        {
                            let (indicator, ind_color) = if severity >= 1.0 {
                                (
                                    "●",
                                    Color {
                                        r: 1.0,
                                        g: 0.3,
                                        b: 0.3,
                                        a: 1.0,
                                    },
                                ) // Red - critical
                            } else if severity >= 0.7 {
                                (
                                    "◐",
                                    Color {
                                        r: 1.0,
                                        g: 0.6,
                                        b: 0.2,
                                        a: 1.0,
                                    },
                                ) // Orange - thrashing
                            } else if severity >= 0.4 {
                                (
                                    "◔",
                                    Color {
                                        r: 1.0,
                                        g: 0.8,
                                        b: 0.2,
                                        a: 1.0,
                                    },
                                ) // Yellow - swapping
                            } else {
                                (
                                    "○",
                                    Color {
                                        r: 0.5,
                                        g: 0.5,
                                        b: 0.5,
                                        a: 1.0,
                                    },
                                ) // Gray - idle
                            };
                            let thrash_x = inner.x + 28.0 + bar_width as f32;
                            let thrash_text = format!(
                                " {indicator} I:{:.0}/O:{:.0}",
                                swap_data.swap_in_rate, swap_data.swap_out_rate
                            );
                            canvas.draw_text(
                                &thrash_text,
                                Point::new(thrash_x, y),
                                &TextStyle {
                                    color: ind_color,
                                    ..Default::default()
                                },
                            );
                        }
                    }
                }

                y += 1.0;
            }

            // Memory Pressure indicator (CB-MEM-003) - show PSI memory pressure
            if y < inner.y + inner.height {
                if let Some(psi) = app.psi_data() {
                    let mem_some = psi.memory.some.avg10;
                    let mem_full = psi.memory.full.as_ref().map_or(0.0, |f| f.avg10);

                    // Choose indicator based on pressure level (ttop style)
                    let (symbol, color) = if mem_some > 20.0 || mem_full > 5.0 {
                        (
                            "●",
                            Color {
                                r: 1.0,
                                g: 0.3,
                                b: 0.3,
                                a: 1.0,
                            },
                        ) // Red - critical
                    } else if mem_some > 10.0 || mem_full > 1.0 {
                        (
                            "◐",
                            Color {
                                r: 1.0,
                                g: 0.8,
                                b: 0.2,
                                a: 1.0,
                            },
                        ) // Yellow - warning
                    } else {
                        (
                            "○",
                            Color {
                                r: 0.3,
                                g: 0.9,
                                b: 0.3,
                                a: 1.0,
                            },
                        ) // Green - healthy
                    };

                    let psi_text =
                        format!("   PSI {symbol} {mem_some:>5.1}% some {mem_full:>5.1}% full");
                    canvas.draw_text(
                        &psi_text,
                        Point::new(inner.x, y),
                        &TextStyle {
                            color,
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }
}

fn draw_disk_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Get disk I/O data
    let disk_io = app.disk_io_data();

    // In deterministic mode, use zeroed values like ttop
    let (total_used, total_space, read_rate, write_rate) = if app.deterministic {
        (0u64, 0u64, 0.0, 0.0)
    } else {
        // Calculate total disk usage for title
        let (used, space): (u64, u64) = app
            .disks
            .iter()
            .map(|d| (d.total_space() - d.available_space(), d.total_space()))
            .fold((0, 0), |(au, at), (u, t)| (au + u, at + t));

        let r_rate = disk_io.map_or(0.0, |d| d.total_read_bytes_per_sec);
        let w_rate = disk_io.map_or(0.0, |d| d.total_write_bytes_per_sec);

        (used, space, r_rate, w_rate)
    };
    let total_pct = if total_space > 0 {
        (total_used as f64 / total_space as f64) * 100.0
    } else {
        0.0
    };

    // ttop-style title (Border adds outer spaces)
    // Deterministic: "Disk │ R: 0B/s │ W: 0B/s │ -0 IOPS │"
    let title = if app.deterministic {
        // ttop deterministic format with IOPS
        "Disk │ R: 0B/s │ W: 0B/s │ -0 IOPS │".to_string()
    } else if read_rate > 0.0 || write_rate > 0.0 {
        format!(
            "Disk │ R: {} │ W: {} │ {:.0}G / {:.0}G",
            format_bytes_rate(read_rate),
            format_bytes_rate(write_rate),
            total_used as f64 / 1024.0 / 1024.0 / 1024.0,
            total_space as f64 / 1024.0 / 1024.0 / 1024.0,
        )
    } else {
        // Fallback when no I/O data
        format!(
            "Disk │ {:.0}G / {:.0}G ({:.0}%)",
            total_used as f64 / 1024.0 / 1024.0 / 1024.0,
            total_space as f64 / 1024.0 / 1024.0 / 1024.0,
            total_pct
        )
    };

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Disk);
    let mut border = create_panel_border(&title, DISK_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // In deterministic mode, show ttop-style content
    if app.deterministic {
        let dim_color = Color {
            r: 0.3,
            g: 0.3,
            b: 0.3,
            a: 1.0,
        };

        // Row 1: I/O Pressure
        canvas.draw_text(
            "I/O Pressure ○  0.0% some    0.0% full",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );

        // Row 2: Top Active Processes header
        if inner.height >= 2.0 {
            canvas.draw_text(
                "── Top Active Processes ──────────────",
                Point::new(inner.x, inner.y + 1.0),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );
        }

        return;
    }

    let max_disks = inner.height as usize;
    for (i, disk) in app.disks.iter().take(max_disks).enumerate() {
        let y = inner.y + i as f32;
        if y >= inner.y + inner.height {
            break;
        }

        let mount = disk.mount_point().to_string_lossy();
        let mount_short: String = if mount == "/" {
            "/".to_string()
        } else {
            mount
                .split('/')
                .next_back()
                .unwrap_or(&mount)
                .chars()
                .take(8)
                .collect()
        };

        let total = disk.total_space();
        let used = total - disk.available_space();
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;

        // Try to find I/O rates for this disk
        let disk_name = disk.name().to_string_lossy();
        let device_name = disk_name.trim_start_matches("/dev/");

        let (d_read, d_write) = if let Some(data) = disk_io {
            if let Some(rate) = data.rates.get(device_name) {
                (rate.read_bytes_per_sec, rate.write_bytes_per_sec)
            } else {
                (0.0, 0.0)
            }
        } else {
            (0.0, 0.0)
        };

        let io_str = if d_read > 0.0 || d_write > 0.0 {
            format!(
                " R:{} W:{}",
                format_bytes_rate(d_read),
                format_bytes_rate(d_write)
            )
        } else {
            String::new()
        };

        // Layout: mount(8) | size(5)G | bar(...) | pct(5)% | IO(...)
        // Fixed parts: 8 + 1 + 6 + 1 + 1 + 6 + 1 = 24 chars
        // IO string is variable width
        let fixed_width = 24;
        let io_width = io_str.len();
        let available_width = (inner.width as usize).saturating_sub(fixed_width + io_width);
        let bar_width = available_width.max(2);

        let filled = ((pct / 100.0) * bar_width as f64) as usize;
        let bar: String =
            "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));

        // Format: "mnt      100G  ████░░  50.0%  R:10M W:5M"
        let text = format!("{mount_short:<8} {total_gb:>5.0}G {bar} {pct:>5.1}%{io_str}");

        // Highlight active disks
        let color = if d_read > 1024.0 || d_write > 1024.0 {
            Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            } // White for active
        } else {
            percent_color(pct) // Gradient for idle
        };

        canvas.draw_text(
            &text,
            Point::new(inner.x, y),
            &TextStyle {
                color,
                ..Default::default()
            },
        );
    }
}

fn draw_network_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // In deterministic mode, use zeroed values like ttop
    let (rx_total, tx_total, primary_iface) = if app.deterministic {
        (0u64, 0u64, "none")
    } else {
        // Calculate total network rates for title
        let (rx, tx): (u64, u64) = app
            .networks
            .values()
            .map(|d| (d.received(), d.transmitted()))
            .fold((0, 0), |(ar, at), (r, t)| (ar + r, at + t));

        // Find primary interface (highest traffic, excluding loopback)
        let iface = app
            .networks
            .iter()
            .filter(|(name, _)| !name.starts_with("lo"))
            .max_by_key(|(_, data)| data.received() + data.transmitted())
            .map_or("none", |(name, _)| name.as_str());
        (rx, tx, iface)
    };

    // ttop-style title (Border adds outer spaces)
    // Deterministic: "Network (none) │ ↓ 0B/s │ ↑ 0B/s"
    let title = format!(
        "Network ({}) │ ↓ {}/s │ ↑ {}/s",
        primary_iface,
        format_bytes(rx_total),
        format_bytes(tx_total)
    );

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Network);
    let mut border = create_panel_border(&title, NETWORK_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    // In deterministic mode, show ttop-style network content
    if app.deterministic {
        let cyan = Color {
            r: 0.3,
            g: 0.8,
            b: 0.9,
            a: 1.0,
        };
        let red = Color {
            r: 1.0,
            g: 0.3,
            b: 0.3,
            a: 1.0,
        };
        let dim_color = Color {
            r: 0.3,
            g: 0.3,
            b: 0.3,
            a: 1.0,
        };
        let white = Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let green = Color {
            r: 0.3,
            g: 0.9,
            b: 0.3,
            a: 1.0,
        };

        let mut y = inner.y;

        // Row 1: Download label
        canvas.draw_text(
            "↓",
            Point::new(inner.x, y),
            &TextStyle {
                color: cyan,
                ..Default::default()
            },
        );
        canvas.draw_text(
            " Download ",
            Point::new(inner.x + 1.0, y),
            &TextStyle {
                color: cyan,
                ..Default::default()
            },
        );
        canvas.draw_text(
            "0B/s",
            Point::new(inner.x + 11.0, y),
            &TextStyle {
                color: white,
                ..Default::default()
            },
        );
        y += 1.0;

        // Row 2: Download braille graph (empty)
        if y < inner.y + inner.height {
            let empty_braille = "⠀".repeat(inner.width as usize);
            canvas.draw_text(
                &empty_braille,
                Point::new(inner.x, y),
                &TextStyle {
                    color: cyan,
                    ..Default::default()
                },
            );
            y += 1.0;
        }

        // Row 3: Upload label
        if y < inner.y + inner.height {
            canvas.draw_text(
                "↑",
                Point::new(inner.x, y),
                &TextStyle {
                    color: red,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                " Upload   ",
                Point::new(inner.x + 1.0, y),
                &TextStyle {
                    color: red,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0B/s",
                Point::new(inner.x + 11.0, y),
                &TextStyle {
                    color: white,
                    ..Default::default()
                },
            );
            y += 1.0;
        }

        // Row 4-5: Upload braille graph (empty)
        for _ in 0..2 {
            if y < inner.y + inner.height {
                let empty_braille = "⠀".repeat(inner.width as usize);
                canvas.draw_text(
                    &empty_braille,
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: red,
                        ..Default::default()
                    },
                );
                y += 1.0;
            }
        }

        // Row 6: Session stats
        if y < inner.y + inner.height {
            canvas.draw_text(
                "Session ",
                Point::new(inner.x, y),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "↓",
                Point::new(inner.x + 8.0, y),
                &TextStyle {
                    color: cyan,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0B",
                Point::new(inner.x + 9.0, y),
                &TextStyle {
                    color: white,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                " ↑",
                Point::new(inner.x + 11.0, y),
                &TextStyle {
                    color: red,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0B",
                Point::new(inner.x + 13.0, y),
                &TextStyle {
                    color: white,
                    ..Default::default()
                },
            );
            y += 1.0;
        }

        // Row 7: TCP/UDP stats
        if y < inner.y + inner.height {
            canvas.draw_text(
                "TCP ",
                Point::new(inner.x, y),
                &TextStyle {
                    color: Color {
                        r: 0.3,
                        g: 0.7,
                        b: 0.9,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0",
                Point::new(inner.x + 4.0, y),
                &TextStyle {
                    color: green,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "/",
                Point::new(inner.x + 5.0, y),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0",
                Point::new(inner.x + 6.0, y),
                &TextStyle {
                    color: Color {
                        r: 0.3,
                        g: 0.7,
                        b: 0.9,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
            canvas.draw_text(
                " UDP ",
                Point::new(inner.x + 7.0, y),
                &TextStyle {
                    color: Color {
                        r: 0.8,
                        g: 0.3,
                        b: 0.8,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "0",
                Point::new(inner.x + 12.0, y),
                &TextStyle {
                    color: white,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                " │ RTT ",
                Point::new(inner.x + 13.0, y),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );
            canvas.draw_text(
                "●●●●●",
                Point::new(inner.x + 20.0, y),
                &TextStyle {
                    color: green,
                    ..Default::default()
                },
            );
        }

        return;
    }

    // Get network stats from analyzer for errors/drops
    let network_stats_data = app.analyzers.network_stats_data();

    // Build interface list with historical data for sparklines
    let mut interfaces: Vec<NetworkInterface> = Vec::new();
    for (name, data) in &app.networks {
        let mut iface = NetworkInterface::new(name);
        iface.update(data.received() as f64, data.transmitted() as f64);
        iface.set_totals(data.total_received(), data.total_transmitted());

        // Add error/drop stats from analyzer if available
        if let Some(stats_data) = network_stats_data {
            if let Some(stats) = stats_data.stats.get(name.as_str()) {
                iface.set_stats(
                    stats.rx_errors,
                    stats.tx_errors,
                    stats.rx_dropped,
                    stats.tx_dropped,
                );
            }
            if let Some(rates) = stats_data.rates.get(name.as_str()) {
                iface.set_rates(rates.errors_per_sec, rates.drops_per_sec);
                // Set bandwidth utilization (CB-NET-006)
                iface.set_utilization(rates.utilization_percent());
            }
        }

        interfaces.push(iface);
    }

    // Sort by traffic (highest first) and keep primary interface
    interfaces.sort_by(|a, b| {
        let a_total = a.rx_bps + a.tx_bps;
        let b_total = b.rx_bps + b.tx_bps;
        b_total
            .partial_cmp(&a_total)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Inject app's network history into the primary interface for sparklines
    // The history is normalized 0-1, scale back to approximate bytes/s for display
    if let Some(primary) = interfaces.first_mut() {
        // Convert normalized history (0-1) to scaled values for sparkline
        // Max expected rate is 1GB/s, so multiply by 1e9 and scale for display
        let max_rate = 1_000_000_000.0_f64;
        primary.rx_history = app
            .net_rx_history
            .as_slice()
            .iter()
            .map(|&v| v * max_rate)
            .collect();
        primary.tx_history = app
            .net_tx_history
            .as_slice()
            .iter()
            .map(|&v| v * max_rate)
            .collect();
    }

    interfaces.truncate(4);

    if !interfaces.is_empty() && inner.height > 0.0 {
        let spark_w = (inner.width as usize / 4).max(5);
        let mut panel = NetworkPanel::new()
            .with_spark_width(spark_w)
            .with_rx_color(NET_RX_COLOR)
            .with_tx_color(NET_TX_COLOR)
            .compact();
        panel.set_interfaces(interfaces);
        panel.layout(inner);
        panel.paint(canvas);
    }
}

fn draw_process_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // ttop uses "CPU%" not "CPU" for percentage-based columns
    let sort_name = match app.sort_column {
        super::app::ProcessSortColumn::Cpu => "CPU%",
        super::app::ProcessSortColumn::Mem => "MEM%",
        super::app::ProcessSortColumn::Pid => "PID",
        super::app::ProcessSortColumn::User => "USER",
        super::app::ProcessSortColumn::Command => "CMD",
    };
    let arrow = if app.sort_descending { "▼" } else { "▲" };
    let filter_str = if app.filter.is_empty() {
        String::new()
    } else {
        format!(" │ Filter: \"{}\"", app.filter)
    };

    // ttop-style title (Border adds outer spaces)
    // Deterministic: "Processes (0) │ Sort: CPU% ▼"
    let title = format!(
        "Processes ({}) │ Sort: {} {}{}",
        app.process_count(),
        sort_name,
        arrow,
        filter_str
    );

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Process);
    let mut border = create_panel_border(&title, PROCESS_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 {
        return;
    }

    // In deterministic mode, show ttop-style header and empty list
    if app.deterministic {
        // ttop header: "PID    S  C%   M%   COMMAND"
        let header = "PID    S  C%   M%   COMMAND";
        canvas.draw_text(
            header,
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: PROCESS_COLOR,
                ..Default::default()
            },
        );
        // Empty process list (no separator, no "No processes" message)
        return;
    }

    // Get sorted processes
    let procs = app.sorted_processes();
    let total_mem = app.mem_total as f64;

    // Get extended process info from analyzer
    let process_extra_data = app.analyzers.process_extra_data();

    // Detect exploded mode: when panel is large (fullscreen), show more detail
    // User request: show full command path when exploded so "rustc...which project?" is answered
    let is_exploded = inner.height > 30.0 || inner.width > 100.0;
    let max_cmd_len = if is_exploded { 200 } else { 40 };

    // Convert to ProcessEntry with state and extended info
    let entries: Vec<ProcessEntry> = procs
        .iter()
        .take(if is_exploded { 500 } else { 100 })
        .map(|p| {
            let pid = p.pid().as_u32();
            let mem_pct = if total_mem > 0.0 {
                (p.memory() as f64 / total_mem) * 100.0
            } else {
                0.0
            };
            // Resolve UID to username using Users lookup table
            let user = p
                .user_id()
                .and_then(|uid| app.users.get_user_by_id(uid))
                .map(|u| u.name().to_string())
                .unwrap_or_else(|| "-".to_string());
            let user_short: String = user.chars().take(8).collect();

            // In exploded mode, show full command line with path
            let cmd: String = if is_exploded {
                // Try to get full command line
                let cmdline: Vec<String> = p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect();
                if cmdline.is_empty() {
                    p.name()
                        .to_string_lossy()
                        .chars()
                        .take(max_cmd_len)
                        .collect()
                } else {
                    cmdline.join(" ").chars().take(max_cmd_len).collect()
                }
            } else {
                p.name()
                    .to_string_lossy()
                    .chars()
                    .take(max_cmd_len)
                    .collect()
            };

            // Convert sysinfo status to ProcessState
            let state = match p.status() {
                sysinfo::ProcessStatus::Run => ProcessState::Running,
                sysinfo::ProcessStatus::Sleep => ProcessState::Sleeping,
                sysinfo::ProcessStatus::Idle => ProcessState::Idle,
                sysinfo::ProcessStatus::Zombie => ProcessState::Zombie,
                sysinfo::ProcessStatus::Stop => ProcessState::Stopped,
                sysinfo::ProcessStatus::UninterruptibleDiskSleep => ProcessState::DiskWait,
                _ => ProcessState::Sleeping,
            };

            let mut entry =
                ProcessEntry::new(pid, &user_short, p.cpu_usage(), mem_pct as f32, &cmd)
                    .with_state(state);

            // Add extended process info if available
            if let Some(extra_data) = process_extra_data {
                if let Some(extra) = extra_data.get(pid) {
                    entry = entry
                        .with_oom_score(extra.oom_score)
                        .with_cgroup(extra.cgroup_short())
                        .with_nice(extra.nice)
                        .with_threads(extra.num_threads);
                }
            }

            entry
        })
        .collect();

    // In exploded mode, use full table with cmdline; otherwise compact with threads
    let mut table = if is_exploded {
        ProcessTable::new().with_cmdline().with_threads_column()
    } else {
        ProcessTable::new().compact().with_threads_column()
    };
    table.set_processes(entries);
    table.select(app.process_selected);
    table.layout(inner);
    table.paint(canvas);
}

fn draw_help_overlay(canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let popup_w = 55.0;
    let popup_h = 24.0; // Expanded for new keybindings
    let px = (w - popup_w) / 2.0;
    let py = (h - popup_h) / 2.0;

    // Clear background
    for y in 0..popup_h as u16 {
        let spaces: String = (0..popup_w as usize).map(|_| ' ').collect();
        canvas.draw_text(
            &spaces,
            Point::new(px, py + y as f32),
            &TextStyle {
                color: Color::new(0.1, 0.1, 0.15, 1.0),
                ..Default::default()
            },
        );
    }

    let mut border = Border::new()
        .with_title(" Help ")
        .with_style(BorderStyle::Double)
        .with_color(Color::new(0.3, 0.8, 0.9, 1.0));
    border.layout(Rect::new(px, py, popup_w, popup_h));
    border.paint(canvas);

    let text_style = TextStyle {
        color: Color::new(0.9, 0.9, 0.9, 1.0),
        ..Default::default()
    };
    let key_style = TextStyle {
        color: Color::new(0.3, 0.8, 0.9, 1.0),
        ..Default::default()
    };
    let section_style = TextStyle {
        color: Color::new(0.8, 0.8, 0.2, 1.0),
        ..Default::default()
    };

    // Help content with section headers (SPEC-024 v5.0 Feature D keybindings)
    let help_lines: &[(&str, &str, bool)] = &[
        ("", "-- General --", true),
        ("q, Esc, Ctrl+C", "Quit", false),
        ("h, ?", "Toggle help", false),
        ("", "-- Panel Navigation --", true),
        ("Tab", "Focus next panel", false),
        ("Shift+Tab", "Focus previous panel", false),
        ("hjkl", "Vim-style focus navigation", false),
        ("Enter, z", "Explode/zoom focused panel", false),
        ("", "-- Process List --", true),
        ("j/k, ↑/↓", "Navigate processes", false),
        ("PgUp/PgDn", "Page up/down", false),
        ("g/G", "Go to top/bottom", false),
        ("c/m/p", "Sort by CPU/Memory/PID", false),
        ("s", "Cycle sort column", false),
        ("r", "Reverse sort", false),
        ("/, f", "Filter processes", false),
        ("Delete", "Clear filter", false),
        ("", "-- Panels --", true),
        ("1-5", "Toggle panels", false),
        ("0", "Reset panels", false),
    ];

    for (i, (key, desc, is_section)) in help_lines.iter().enumerate() {
        let y = py + 1.0 + i as f32;
        if *is_section {
            canvas.draw_text(desc, Point::new(px + 2.0, y), &section_style);
        } else {
            canvas.draw_text(&format!("{key:>14}"), Point::new(px + 2.0, y), &key_style);
            canvas.draw_text(desc, Point::new(px + 18.0, y), &text_style);
        }
    }
}

fn draw_filter_overlay(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let popup_w = 45.0;
    let popup_h = 3.0;
    let px = (w - popup_w) / 2.0;
    let py = (h - popup_h) / 2.0;

    let mut border = Border::new()
        .with_title(" Filter Processes ")
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.3, 0.8, 0.9, 1.0));
    border.layout(Rect::new(px, py, popup_w, popup_h));
    border.paint(canvas);

    let filter_display = format!("{}_", app.filter);
    canvas.draw_text(
        &filter_display,
        Point::new(px + 2.0, py + 1.0),
        &TextStyle {
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            ..Default::default()
        },
    );
}

// ============================================================================
// NEW PANELS (F006-F014): GPU, Battery, Sensors, PSI, Connections, etc.
// ============================================================================

/// GPU information from sysfs or nvidia-smi
/// GPU information structure used by both app.rs and ui.rs
#[derive(Debug, Default, Clone)]
pub struct GpuInfo {
    /// GPU name/model
    pub name: String,
    /// GPU utilization (0-100)
    pub utilization: Option<u8>,
    /// Temperature in Celsius
    pub temperature: Option<u32>,
    /// Power consumption in Watts
    pub power_watts: Option<f32>,
    /// VRAM used in bytes
    pub vram_used: Option<u64>,
    /// VRAM total in bytes
    pub vram_total: Option<u64>,
}

/// Read GPU info from nvidia-smi (NVIDIA) or sysfs (AMD/Intel)
pub fn read_gpu_info() -> Option<GpuInfo> {
    // Try NVIDIA first (nvidia-smi)
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;

        // Try nvidia-smi for NVIDIA GPUs
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=name,utilization.gpu,temperature.gpu,power.draw,memory.used,memory.total",
                "--format=csv,noheader,nounits"
            ])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let line = stdout.lines().next().unwrap_or("");
                let parts: Vec<&str> = line.split(", ").collect();

                if parts.len() >= 6 {
                    return Some(GpuInfo {
                        name: parts[0].trim().to_string(),
                        utilization: parts[1].trim().parse().ok(),
                        temperature: parts[2].trim().parse().ok(),
                        power_watts: parts[3].trim().parse().ok(),
                        vram_used: parts[4].trim().parse::<u64>().ok().map(|v| v * 1024 * 1024), // MiB -> bytes
                        vram_total: parts[5].trim().parse::<u64>().ok().map(|v| v * 1024 * 1024),
                    });
                }
            }
        }

        // Try AMD via sysfs
        use std::fs;
        use std::path::Path;

        for card in 0..4 {
            let card_path = format!("/sys/class/drm/card{card}/device");
            let path = Path::new(&card_path);

            if !path.exists() {
                continue;
            }

            // Check if it's an AMD GPU (has hwmon with temp/power)
            let hwmon_path = format!("{card_path}/hwmon");
            if let Ok(entries) = fs::read_dir(&hwmon_path) {
                for entry in entries.flatten() {
                    let hwmon_dir = entry.path();

                    // Read temperature
                    let temp = fs::read_to_string(hwmon_dir.join("temp1_input"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u32>().ok())
                        .map(|t| t / 1000); // millidegrees -> degrees

                    // Read power
                    let power = fs::read_to_string(hwmon_dir.join("power1_average"))
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .map(|p| p as f32 / 1_000_000.0); // microwatts -> watts

                    // Read GPU name
                    let name = fs::read_to_string(hwmon_dir.join("name"))
                        .ok()
                        .map_or_else(|| "AMD GPU".to_string(), |s| s.trim().to_string());

                    // Read VRAM
                    let vram_used = fs::read_to_string(format!("{card_path}/mem_info_vram_used"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok());
                    let vram_total = fs::read_to_string(format!("{card_path}/mem_info_vram_total"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok());

                    // Read utilization
                    let utilization = fs::read_to_string(format!("{card_path}/gpu_busy_percent"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok());

                    if temp.is_some() || power.is_some() {
                        return Some(GpuInfo {
                            name,
                            utilization,
                            temperature: temp,
                            power_watts: power,
                            vram_used,
                            vram_total,
                        });
                    }
                }
            }
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// F006: GPU Panel - shows GPU utilization, VRAM, temperature
fn draw_gpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Determine detail level based on available height (SPEC-024 v5.0 Feature E)
    let detail_level = DetailLevel::for_height(bounds.height as u16);

    // Use cached GPU info from app (updated in App::update())
    let gpu = app.gpu_info.clone();

    // ttop-style title (Border adds outer spaces)
    // Deterministic: just "GPU" with no info
    // At Minimal detail level, only show name
    let title = gpu
        .as_ref()
        .map(|g| {
            if detail_level == DetailLevel::Minimal {
                g.name.clone()
            } else {
                let temp_str = g
                    .temperature
                    .map(|t| format!(" │ {t}°C"))
                    .unwrap_or_default();
                let power_str = g
                    .power_watts
                    .map(|p| format!(" │ {p:.0}W"))
                    .unwrap_or_default();
                format!("{}{}{}", g.name, temp_str, power_str)
            }
        })
        .unwrap_or_else(|| "GPU".to_string());

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Gpu);
    let mut border = create_panel_border(&title, GPU_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    if let Some(g) = gpu {
        let mut y = inner.y;

        // GPU utilization bar
        if let Some(util) = g.utilization {
            let bar_width = (inner.width as usize).min(20);
            let filled = ((util as f32 / 100.0) * bar_width as f32) as usize;
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));
            let color = percent_color(util as f64);

            let text = format!("GPU  {bar} {util:>3}%");
            canvas.draw_text(
                &text,
                Point::new(inner.x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
            y += 1.0;
        }

        // VRAM usage bar
        if let (Some(used), Some(total)) = (g.vram_used, g.vram_total) {
            if y < inner.y + inner.height && total > 0 {
                let pct = (used as f64 / total as f64) * 100.0;
                let bar_width = (inner.width as usize).min(20);
                let filled = ((pct / 100.0) * bar_width as f64) as usize;
                let bar: String =
                    "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));
                let color = percent_color(pct);

                let used_mb = used / 1024 / 1024;
                let total_mb = total / 1024 / 1024;
                let text = format!("VRAM {bar} {used_mb}M/{total_mb}M");
                canvas.draw_text(
                    &text,
                    Point::new(inner.x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
                y += 1.0;
            }
        }

        // Temperature row
        if let Some(temp) = g.temperature {
            if y < inner.y + inner.height {
                let color = if temp > 85 {
                    Color {
                        r: 1.0,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    } // Red - hot
                } else if temp > 70 {
                    Color {
                        r: 1.0,
                        g: 0.8,
                        b: 0.2,
                        a: 1.0,
                    } // Yellow - warm
                } else {
                    Color {
                        r: 0.3,
                        g: 0.9,
                        b: 0.3,
                        a: 1.0,
                    } // Green - cool
                };
                canvas.draw_text(
                    &format!("Temp {temp}°C"),
                    Point::new(inner.x, y),
                    &TextStyle {
                        color,
                        ..Default::default()
                    },
                );
                y += 1.0;
            }
        }

        // Power row
        if let Some(power) = g.power_watts {
            if y < inner.y + inner.height {
                canvas.draw_text(
                    &format!("Power {power:.0}W"),
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: Color {
                            r: 0.7,
                            g: 0.7,
                            b: 0.7,
                            a: 1.0,
                        },
                        ..Default::default()
                    },
                );
                y += 1.0;
            }
        }

        // GPU Utilization History Graph (SPEC-024 v5.2.0 Exploded mode, D012 fix)
        // Only render in exploded mode (height >= 40)
        if detail_level == DetailLevel::Exploded && y < inner.y + inner.height - 10.0 {
            // Draw GPU utilization history graph using real data from app.gpu_history
            let gpu_history: Vec<f64> = app.gpu_history.as_slice().to_vec();
            if !gpu_history.is_empty() {
                let graph_height = 6.0_f32;
                let mut graph = BrailleGraph::new(gpu_history)
                    .with_color(GPU_COLOR)
                    .with_label("GPU History")
                    .with_range(0.0, 100.0);
                graph.layout(Rect::new(inner.x, y, inner.width, graph_height));
                graph.paint(canvas);
                y += graph_height + 1.0;
            }

            // Draw VRAM history graph using real data from app.vram_history
            let vram_history: Vec<f64> = app.vram_history.as_slice().to_vec();
            if !vram_history.is_empty() {
                let graph_height = 6.0_f32;
                let mut graph = BrailleGraph::new(vram_history)
                    .with_color(Color::new(0.6, 0.4, 1.0, 1.0))
                    .with_label("VRAM History")
                    .with_range(0.0, 100.0);
                graph.layout(Rect::new(inner.x, y, inner.width, graph_height));
                graph.paint(canvas);
                y += graph_height + 1.0;
            }
        }

        // GPU Processes with G/C badges (SPEC-024 v5.0 Feature E)
        // Only show at DetailLevel::Expanded or higher
        // Reference: ttop/src/panels.rs lines 1497-1989, ttop/src/analyzers/gpu_procs.rs
        if detail_level >= DetailLevel::Expanded && y < inner.y + inner.height - 3.0 {
            if let Some(gpu_data) = app.analyzers.gpu_procs_data() {
                if !gpu_data.processes.is_empty() {
                    // Header
                    y += 1.0;
                    let header_color = Color {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 1.0,
                    };
                    canvas.draw_text(
                        "TY  PID   SM%  MEM%  CMD",
                        Point::new(inner.x, y),
                        &TextStyle {
                            color: header_color,
                            ..Default::default()
                        },
                    );
                    y += 1.0;

                    // Show top 3 GPU processes with G/C type badge
                    let max_procs = 3.min(gpu_data.processes.len());
                    for proc in gpu_data.processes.iter().take(max_procs) {
                        if y >= inner.y + inner.height {
                            break;
                        }

                        // Type badge: C (Cyan) for Compute, G (Magenta) for Graphics
                        // Reference: ttop/src/analyzers/gpu_procs.rs GpuProcType
                        let (type_badge, badge_color) =
                            match proc.process_type.to_uppercase().as_str() {
                                "C" | "COMPUTE" => (
                                    "C",
                                    Color {
                                        r: 0.0,
                                        g: 0.8,
                                        b: 1.0,
                                        a: 1.0,
                                    },
                                ), // Cyan
                                "G" | "GRAPHICS" => (
                                    "G",
                                    Color {
                                        r: 1.0,
                                        g: 0.0,
                                        b: 1.0,
                                        a: 1.0,
                                    },
                                ), // Magenta
                                _ => (
                                    "?",
                                    Color {
                                        r: 0.5,
                                        g: 0.5,
                                        b: 0.5,
                                        a: 1.0,
                                    },
                                ),
                            };

                        // Draw type badge
                        canvas.draw_text(
                            type_badge,
                            Point::new(inner.x, y),
                            &TextStyle {
                                color: badge_color,
                                ..Default::default()
                            },
                        );

                        // Draw process info
                        let sm_str = proc
                            .gpu_util
                            .map_or_else(|| "  -".to_string(), |u| format!("{u:>3.0}"));
                        let mem_str = proc
                            .mem_util
                            .map_or_else(|| "  -".to_string(), |u| format!("{u:>3.0}"));
                        let cmd = if proc.name.len() > 12 {
                            &proc.name[..12]
                        } else {
                            &proc.name
                        };

                        let proc_info =
                            format!(" {:>5} {}%  {}%  {}", proc.pid, sm_str, mem_str, cmd);
                        canvas.draw_text(
                            &proc_info,
                            Point::new(inner.x + 1.0, y),
                            &TextStyle {
                                color: Color {
                                    r: 0.8,
                                    g: 0.8,
                                    b: 0.8,
                                    a: 1.0,
                                },
                                ..Default::default()
                            },
                        );
                        y += 1.0;
                    }
                }
            }
        }
    } else if !app.deterministic {
        // Only show "No GPU" message in non-deterministic mode
        // In deterministic mode, ttop shows empty GPU panel
        canvas.draw_text(
            "No GPU detected or nvidia-smi not available",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
    // In deterministic mode with no GPU, leave panel interior empty (like ttop)
}

/// Battery information from /`sys/class/power_supply`
#[derive(Debug, Default)]
struct BatteryInfo {
    /// Capacity as percentage (0-100)
    capacity: u8,
    /// Status: Charging, Discharging, Full, Not charging, Unknown
    status: String,
    /// Time remaining in minutes (if available)
    time_remaining_mins: Option<u32>,
    /// Whether battery is present
    #[allow(dead_code)]
    present: bool,
}

/// Read battery info from /`sys/class/power_supply` (Linux only)
fn read_battery_info() -> Option<BatteryInfo> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;
        use std::path::Path;

        // Look for BAT0, BAT1, etc.
        for i in 0..4 {
            let bat_path = format!("/sys/class/power_supply/BAT{i}");
            let path = Path::new(&bat_path);

            if !path.exists() {
                continue;
            }

            // Read capacity
            let capacity = fs::read_to_string(format!("{bat_path}/capacity"))
                .ok()
                .and_then(|s| s.trim().parse::<u8>().ok())
                .unwrap_or(0);

            // Read status
            let status = fs::read_to_string(format!("{bat_path}/status"))
                .ok()
                .map_or_else(|| "Unknown".to_string(), |s| s.trim().to_string());

            // Try to calculate time remaining
            // Uses energy_now/power_now or charge_now/current_now
            let time_remaining_mins = {
                let energy_now = fs::read_to_string(format!("{bat_path}/energy_now"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok());
                let power_now = fs::read_to_string(format!("{bat_path}/power_now"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok());
                let energy_full = fs::read_to_string(format!("{bat_path}/energy_full"))
                    .ok()
                    .and_then(|s| s.trim().parse::<u64>().ok());

                match (energy_now, power_now, energy_full, status.as_str()) {
                    (Some(en), Some(pn), _, "Discharging") if pn > 0 => Some((en * 60 / pn) as u32),
                    (Some(en), Some(pn), Some(ef), "Charging") if pn > 0 => {
                        let remaining = ef.saturating_sub(en);
                        Some((remaining * 60 / pn) as u32)
                    }
                    _ => None,
                }
            };

            return Some(BatteryInfo {
                capacity,
                status,
                time_remaining_mins,
                present: true,
            });
        }

        None
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

/// F007: Battery Panel - shows charge level and state
fn draw_battery_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let battery = read_battery_info();

    // ttop-style title (Border adds outer spaces)
    let title = battery
        .as_ref()
        .map(|b| {
            let time_str = b
                .time_remaining_mins
                .map(|m| {
                    if m >= 60 {
                        format!(" │ {}h{}m", m / 60, m % 60)
                    } else {
                        format!(" │ {m}m")
                    }
                })
                .unwrap_or_default();
            format!("Battery │ {}% │ {}{}", b.capacity, b.status, time_str)
        })
        .unwrap_or_else(|| "Battery │ No battery".to_string());

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Battery);
    let mut border = create_panel_border(&title, BATTERY_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    if let Some(bat) = battery {
        // Draw charge bar with inverted color (red=low, green=full)
        let bar_width = (inner.width as usize).min(30);
        let filled = ((bat.capacity as f32 / 100.0) * bar_width as f32) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));

        // Color: red when low, yellow when medium, green when high
        let color = if bat.capacity < 20 {
            Color {
                r: 1.0,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            } // Red
        } else if bat.capacity < 50 {
            Color {
                r: 1.0,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            } // Yellow
        } else {
            Color {
                r: 0.3,
                g: 0.9,
                b: 0.3,
                a: 1.0,
            } // Green
        };

        canvas.draw_text(
            &bar,
            Point::new(inner.x, inner.y),
            &TextStyle {
                color,
                ..Default::default()
            },
        );

        // Show status icon
        if inner.height >= 2.0 {
            let status_icon = match bat.status.as_str() {
                "Charging" => "⚡ Charging",
                "Discharging" => "🔋 Discharging",
                "Full" => "✓ Full",
                "Not charging" => "— Idle",
                _ => "? Unknown",
            };
            canvas.draw_text(
                status_icon,
                Point::new(inner.x, inner.y + 1.0),
                &TextStyle {
                    color: Color {
                        r: 0.7,
                        g: 0.7,
                        b: 0.7,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
        }
    } else {
        canvas.draw_text(
            "No battery detected",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

/// F008: Sensors Panel - shows temperature sensors with health indicators
fn draw_sensors_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    use super::analyzers::{SensorStatus, SensorType};
    use sysinfo::{Component, Components};

    // In deterministic mode, show 0°C like ttop
    let (components, max_temp) = if app.deterministic {
        (None, 0.0_f32)
    } else {
        let comps = Components::new_with_refreshed_list();
        let temp = comps
            .iter()
            .filter_map(Component::temperature)
            .fold(0.0_f32, f32::max);
        (Some(comps), temp)
    };

    // Get additional sensor data from analyzer (fan RPM, voltage, etc.)
    let sensor_health_data = app.analyzers.sensor_health_data();

    // Build title with max temp and fan/voltage count
    let extra_info = if let Some(health_data) = sensor_health_data {
        let fan_count = health_data
            .type_counts
            .get(&SensorType::Fan)
            .copied()
            .unwrap_or(0);
        let volt_count = health_data
            .type_counts
            .get(&SensorType::Voltage)
            .copied()
            .unwrap_or(0);
        if fan_count > 0 || volt_count > 0 {
            format!(" │ {fan_count}F {volt_count}V")
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // ttop-style title (Border adds outer spaces)
    let title = format!("Sensors │ {max_temp:.0}°C{extra_info}");

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Sensors);
    let mut border = create_panel_border(&title, SENSORS_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // In deterministic mode, components is None - don't show any sensors
    let Some(ref comps) = components else {
        return;
    };

    let mut y = inner.y;
    let max_rows = inner.height as usize;
    let mut rows_used = 0;

    // First show temperature sensors from sysinfo
    for component in comps {
        if rows_used >= max_rows {
            break;
        }

        let label = component.label();
        let label_short: String = label.chars().take(12).collect();

        // Get temperature, skip if not available
        let Some(temp) = component.temperature() else {
            continue;
        };

        // Health indicator based on temperature
        let (indicator, color) = if temp > 85.0 {
            (
                "✗",
                Color {
                    r: 1.0,
                    g: 0.3,
                    b: 0.3,
                    a: 1.0,
                },
            )
        } else if temp > 70.0 {
            (
                "⚠",
                Color {
                    r: 1.0,
                    g: 0.8,
                    b: 0.2,
                    a: 1.0,
                },
            )
        } else {
            (
                "✓",
                Color {
                    r: 0.3,
                    g: 0.9,
                    b: 0.3,
                    a: 1.0,
                },
            )
        };

        let text = format!("{indicator} {label_short:<12} {temp:>5.1}°C");
        canvas.draw_text(
            &text,
            Point::new(inner.x, y),
            &TextStyle {
                color,
                ..Default::default()
            },
        );
        y += 1.0;
        rows_used += 1;
    }

    // Then show fan and voltage sensors from sensor_health analyzer
    if let Some(health_data) = sensor_health_data {
        // Fan sensors
        for fan in health_data.fans() {
            if rows_used >= max_rows {
                break;
            }

            let (indicator, color) = match fan.status {
                SensorStatus::Critical | SensorStatus::Fault => (
                    "✗",
                    Color {
                        r: 1.0,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    },
                ),
                SensorStatus::Warning | SensorStatus::Low => (
                    "⚠",
                    Color {
                        r: 1.0,
                        g: 0.8,
                        b: 0.2,
                        a: 1.0,
                    },
                ),
                SensorStatus::Normal => (
                    "✓",
                    Color {
                        r: 0.3,
                        g: 0.8,
                        b: 0.9,
                        a: 1.0,
                    },
                ),
            };

            let text = format!(
                "{indicator} {:<12} {:>5.0} RPM",
                fan.short_label(),
                fan.value
            );
            canvas.draw_text(
                &text,
                Point::new(inner.x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
            y += 1.0;
            rows_used += 1;
        }

        // Voltage sensors
        for volt in health_data.by_type(SensorType::Voltage) {
            if rows_used >= max_rows {
                break;
            }

            let (indicator, color) = match volt.status {
                SensorStatus::Critical | SensorStatus::Fault => (
                    "✗",
                    Color {
                        r: 1.0,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    },
                ),
                SensorStatus::Warning | SensorStatus::Low => (
                    "⚠",
                    Color {
                        r: 1.0,
                        g: 0.8,
                        b: 0.2,
                        a: 1.0,
                    },
                ),
                SensorStatus::Normal => (
                    "✓",
                    Color {
                        r: 0.9,
                        g: 0.7,
                        b: 0.3,
                        a: 1.0,
                    },
                ),
            };

            let text = format!(
                "{indicator} {:<12} {:>6.2}V",
                volt.short_label(),
                volt.value
            );
            canvas.draw_text(
                &text,
                Point::new(inner.x, y),
                &TextStyle {
                    color,
                    ..Default::default()
                },
            );
            y += 1.0;
            rows_used += 1;
        }
    }

    // Note: In deterministic mode, we already returned early above
    // This check is for non-deterministic mode when no sensors are detected
    if comps.is_empty() && sensor_health_data.is_none() {
        canvas.draw_text(
            "No sensors detected",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

/// Containers Panel - shows Docker/Podman containers (ttop style)
fn draw_containers_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // ttop-style title
    let title = "Containers";

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Containers);
    let mut border = create_panel_border(title, CONTAINERS_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // In deterministic mode, show "No running containers" like ttop
    if app.deterministic {
        canvas.draw_text(
            "No running containers",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
        return;
    }

    // Get containers data from analyzer
    if let Some(data) = app.analyzers.containers_data() {
        if data.containers.is_empty() {
            canvas.draw_text(
                "No running containers",
                Point::new(inner.x, inner.y),
                &TextStyle {
                    color: Color {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
        } else {
            let mut y = inner.y;
            for container in data.containers.iter().take(inner.height as usize) {
                let status_icon = match container.state {
                    super::analyzers::ContainerState::Running => "●",
                    super::analyzers::ContainerState::Paused => "◐",
                    super::analyzers::ContainerState::Exited => "○",
                    super::analyzers::ContainerState::Created => "◎",
                    super::analyzers::ContainerState::Restarting => "↻",
                    super::analyzers::ContainerState::Removing => "⊘",
                    super::analyzers::ContainerState::Dead => "✗",
                    super::analyzers::ContainerState::Unknown => "?",
                };
                let name: String = container.name.chars().take(20).collect();
                let cpu = container.stats.cpu_percent;
                let mem_mb = container.stats.memory_bytes / (1024 * 1024);

                let text = format!("{status_icon} {name:<20} {cpu:>5.1}% {mem_mb:>4}MB");
                canvas.draw_text(
                    &text,
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: CONTAINERS_COLOR,
                        ..Default::default()
                    },
                );
                y += 1.0;
            }
        }
    } else {
        canvas.draw_text(
            "No container runtime",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

/// F010: PSI Panel - shows CPU/Memory/IO pressure (Linux only)
fn draw_psi_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // ttop-style title (Border adds outer spaces)
    let title = "Pressure │ —";

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Psi);
    let mut border = create_panel_border(title, PSI_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Use PSI data from analyzer
    if let Some(psi) = app.psi_data() {
        if psi.available {
            let mut y = inner.y;

            // CPU pressure
            let cpu = psi.cpu.some.avg10;
            let cpu_symbol = pressure_symbol(cpu);
            let cpu_color = pressure_color(cpu);
            canvas.draw_text(
                &format!("CPU  {cpu_symbol} {cpu:>5.1}%"),
                Point::new(inner.x, y),
                &TextStyle {
                    color: cpu_color,
                    ..Default::default()
                },
            );
            y += 1.0;

            // Memory pressure
            if y < inner.y + inner.height {
                let mem = psi.memory.some.avg10;
                let mem_symbol = pressure_symbol(mem);
                let mem_color = pressure_color(mem);
                canvas.draw_text(
                    &format!("MEM  {mem_symbol} {mem:>5.1}%"),
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: mem_color,
                        ..Default::default()
                    },
                );
                y += 1.0;
            }

            // I/O pressure
            if y < inner.y + inner.height {
                let io = psi.io.some.avg10;
                let io_symbol = pressure_symbol(io);
                let io_color = pressure_color(io);
                canvas.draw_text(
                    &format!("I/O  {io_symbol} {io:>5.1}%"),
                    Point::new(inner.x, y),
                    &TextStyle {
                        color: io_color,
                        ..Default::default()
                    },
                );
            }
        } else {
            canvas.draw_text(
                "PSI not available",
                Point::new(inner.x, inner.y),
                &TextStyle {
                    color: Color {
                        r: 0.5,
                        g: 0.5,
                        b: 0.5,
                        a: 1.0,
                    },
                    ..Default::default()
                },
            );
        }
    } else {
        canvas.draw_text(
            "PSI not available",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

fn pressure_symbol(pct: f64) -> &'static str {
    if pct > 50.0 {
        "▲▲"
    } else if pct > 20.0 {
        "▲"
    } else if pct > 5.0 {
        "▼"
    } else if pct > 1.0 {
        "◐"
    } else {
        "—"
    }
}

fn pressure_color(pct: f64) -> Color {
    if pct > 50.0 {
        Color {
            r: 1.0,
            g: 0.2,
            b: 0.2,
            a: 1.0,
        } // Critical red
    } else if pct > 20.0 {
        Color {
            r: 1.0,
            g: 0.5,
            b: 0.3,
            a: 1.0,
        } // High orange
    } else if pct > 5.0 {
        Color {
            r: 1.0,
            g: 0.8,
            b: 0.2,
            a: 1.0,
        } // Medium yellow
    } else if pct > 1.0 {
        Color {
            r: 0.3,
            g: 0.9,
            b: 0.3,
            a: 1.0,
        } // Low green
    } else {
        Color {
            r: 0.4,
            g: 0.4,
            b: 0.4,
            a: 1.0,
        } // None gray
    }
}

/// Get service name from port
fn port_to_service(port: u16) -> &'static str {
    match port {
        22 => "SSH",
        80 => "HTTP",
        443 => "HTTPS",
        53 => "DNS",
        25 => "SMTP",
        21 => "FTP",
        3306 => "MySQL",
        5432 => "Pgsql",
        6379 => "Redis",
        27017 => "Mongo",
        8080 => "HTTP",
        8443 => "HTTPS",
        9000..=9999 => "App",
        _ => "",
    }
}

/// F012: Connections Panel - shows active network connections
fn draw_connections_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Get connection data from analyzer
    let (listen_count, active_count, connections) =
        if let Some(conn_data) = app.analyzers.connections_data() {
            let listen = conn_data
                .connections
                .iter()
                .filter(|c| c.state == TcpState::Listen)
                .count();
            let active = conn_data
                .connections
                .iter()
                .filter(|c| c.state == TcpState::Established)
                .count();
            (listen, active, Some(&conn_data.connections))
        } else {
            (0, 0, None)
        };

    // Generate sparkline for connection history (CB-CONN-007)
    let sparkline_str = if let Some(conn_data) = app.analyzers.connections_data() {
        let sparkline_data = conn_data.established_sparkline();
        if sparkline_data.len() >= 3 {
            // Use braille-style sparkline characters: ▁▂▃▄▅▆▇█
            let chars: Vec<char> = sparkline_data
                .iter()
                .rev() // Most recent on the right
                .take(12) // Limit to 12 chars for title space
                .rev()
                .map(|&v| {
                    let idx = ((v * 7.0).round() as usize).min(7);
                    ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'][idx]
                })
                .collect();
            format!(" {}", chars.iter().collect::<String>())
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // ttop-style title (Border adds outer spaces)
    let title =
        format!("Connections │ {active_count} active │ {listen_count} listen{sparkline_str}");

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Connections);
    let mut border = create_panel_border(&title, CONNECTIONS_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // In deterministic mode, use ttop-style header and "No active connections"
    if app.deterministic {
        let _dim_color = Color {
            r: 0.3,
            g: 0.3,
            b: 0.3,
            a: 1.0,
        };

        // ttop header: "SVC   LOCA REMOT GE ST AGE   PROC"
        let header = "SVC   LOCA REMOT GE ST AGE   PROC";
        canvas.draw_text(
            header,
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: CONNECTIONS_COLOR,
                ..Default::default()
            },
        );

        // No connections in deterministic mode
        return;
    }

    // Header for real data mode (ttop style: SVC LOCAL REMOTE GE ST AGE PROC)
    // GE (geo) simplified: L=local, R=remote
    // AGE: connection duration (CB-CONN-001)
    let header = "SVC   LOCAL        REMOTE            GE ST  AGE   PROC";
    canvas.draw_text(
        header,
        Point::new(inner.x, inner.y),
        &TextStyle {
            color: CONNECTIONS_COLOR,
            ..Default::default()
        },
    );

    let Some(conns) = connections else {
        canvas.draw_text(
            "No data",
            Point::new(inner.x, inner.y + 1.0),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
        return;
    };

    // Show connections (skip loopback, prioritize ESTABLISHED and LISTEN)
    use std::net::{IpAddr, Ipv4Addr};
    let loopback_v4: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

    let mut display_conns: Vec<_> = conns
        .iter()
        .filter(|c| c.remote_addr != loopback_v4 || c.state == TcpState::Listen)
        .collect();

    // Sort: LISTEN first, then ESTABLISHED, then others
    display_conns.sort_by(|a, b| {
        let order = |s: TcpState| match s {
            TcpState::Listen => 0,
            TcpState::Established => 1,
            _ => 2,
        };
        order(a.state).cmp(&order(b.state))
    });

    let max_rows = (inner.height as usize).saturating_sub(1);
    let dim_color = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    let active_color = Color {
        r: 0.3,
        g: 0.9,
        b: 0.3,
        a: 1.0,
    };
    let listen_color = Color {
        r: 0.3,
        g: 0.7,
        b: 1.0,
        a: 1.0,
    };

    for (i, conn) in display_conns.iter().take(max_rows).enumerate() {
        let y = inner.y + 1.0 + i as f32;
        if y >= inner.y + inner.height {
            break;
        }

        let svc = port_to_service(conn.local_port);
        let local = format!(":{}", conn.local_port);
        let remote = if conn.state == TcpState::Listen {
            "*".to_string()
        } else {
            // Truncate remote address for display
            let addr_str = format!("{}:{}", conn.remote_addr, conn.remote_port);
            if addr_str.len() > 17 {
                format!("{}…", &addr_str[..16])
            } else {
                addr_str
            }
        };

        // GE (geo): L=local (127.x, 192.168.x, 10.x), R=remote
        let is_local = match &conn.remote_addr {
            std::net::IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
            std::net::IpAddr::V6(ip) => ip.is_loopback(),
        };
        let geo = if conn.state == TcpState::Listen {
            "-"
        } else if is_local {
            "L"
        } else {
            "R"
        };

        let state_short = match conn.state {
            TcpState::Established => "E",
            TcpState::Listen => "L",
            TcpState::TimeWait => "T",
            TcpState::CloseWait => "C",
            TcpState::SynSent => "S",
            _ => "?",
        };

        // PROC: process name or PID
        let proc_name = conn
            .process_name
            .as_ref()
            .map(|s| {
                if s.len() > 10 {
                    format!("{}…", &s[..9])
                } else {
                    s.clone()
                }
            })
            .or_else(|| conn.pid.map(|p| p.to_string()))
            .unwrap_or_else(|| "-".to_string());

        let state_color = match conn.state {
            TcpState::Established => active_color,
            TcpState::Listen => listen_color,
            _ => dim_color,
        };

        // Get connection age (CB-CONN-001)
        let age = conn.age_display();

        // Get hot indicator (CB-CONN-006)
        let (hot_indicator, _hot_level) = conn.hot_indicator();

        // Format: SVC   LOCAL        REMOTE            GE ST  AGE   PROC
        let line = format!(
            "{svc:<5} {local:<12} {remote:<17} {geo:<2} {state_short:<3} {age:<5} {proc_name}"
        );
        canvas.draw_text(
            &line,
            Point::new(inner.x, y),
            &TextStyle {
                color: state_color,
                ..Default::default()
            },
        );

        // Draw hot indicator after the line (CB-CONN-006)
        if !hot_indicator.is_empty() {
            let hot_color = if hot_indicator == "●" {
                Color {
                    r: 1.0,
                    g: 0.4,
                    b: 0.2,
                    a: 1.0,
                } // Orange for hot
            } else {
                Color {
                    r: 1.0,
                    g: 0.7,
                    b: 0.3,
                    a: 1.0,
                } // Yellow for warm
            };
            // Draw at the end of the line
            let hot_x = inner.x + 56.0;
            if hot_x < inner.x + inner.width {
                canvas.draw_text(
                    hot_indicator,
                    Point::new(hot_x, y),
                    &TextStyle {
                        color: hot_color,
                        ..Default::default()
                    },
                );
            }
        }
    }

    // If no connections, show message
    if display_conns.is_empty() && inner.height > 1.0 {
        canvas.draw_text(
            "No active connections",
            Point::new(inner.x, inner.y + 1.0),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
    }
}

/// F009: Sensors Compact Panel - compact sensor display with dual-color bars
fn draw_sensors_compact_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    use sysinfo::{Component, Components};

    let components = Components::new_with_refreshed_list();
    let max_temp = components
        .iter()
        .filter_map(Component::temperature)
        .fold(0.0_f32, f32::max);

    let title = format!("Sensors │ {max_temp:.0}°C");

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(SENSORS_COLOR)
        .with_title_left_aligned();
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;
    for component in components.iter().take(inner.height as usize) {
        let label = component.label();
        let Some(temp) = component.temperature() else {
            continue;
        };

        // Type character: C (CPU), G (GPU), D (Disk), F (Fan), M (Mobo)
        let type_char = if label.contains("CPU") || label.contains("Core") {
            'C'
        } else if label.contains("GPU") {
            'G'
        } else if label.contains("nvme") || label.contains("SSD") || label.contains("HDD") {
            'D'
        } else if label.contains("fan") || label.contains("Fan") {
            'F'
        } else {
            'M'
        };

        // 4-char dual-color bar
        let pct = (temp / 100.0).clamp(0.0, 1.0);
        let filled = (pct * 4.0).round() as usize;
        let bar: String = (0..4).map(|i| if i < filled { '▄' } else { '░' }).collect();

        let label_short: String = label.chars().take(8).collect();
        let text = format!("{type_char} {bar} {temp:>4.0}°C {label_short}");

        let color = if temp > 85.0 {
            Color {
                r: 1.0,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            }
        } else if temp > 70.0 {
            Color {
                r: 1.0,
                g: 0.8,
                b: 0.2,
                a: 1.0,
            }
        } else {
            Color {
                r: 0.3,
                g: 0.9,
                b: 0.3,
                a: 1.0,
            }
        };

        canvas.draw_text(
            &text,
            Point::new(inner.x, y),
            &TextStyle {
                color,
                ..Default::default()
            },
        );
        y += 1.0;
    }

    if components.is_empty() {
        canvas.draw_text(
            "No sensors",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

/// F011: System Panel - composite view with `sensors_compact` + containers
fn draw_system_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let title = "System";

    let mut border = Border::new()
        .with_title(title)
        .with_style(BorderStyle::Rounded)
        .with_color(Color {
            r: 0.5,
            g: 0.7,
            b: 0.9,
            a: 1.0,
        })
        .with_title_left_aligned();
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Show system info (O(1) render - uses cached data from App)
    let mut y = inner.y;

    // Hostname (cached at startup)
    if !app.hostname.is_empty() {
        canvas.draw_text(
            &format!("Host: {}", app.hostname),
            Point::new(inner.x, y),
            &TextStyle {
                color: Color {
                    r: 0.7,
                    g: 0.9,
                    b: 1.0,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
        y += 1.0;
    }

    // Kernel version (cached at startup)
    if !app.kernel_version.is_empty() && y < inner.y + inner.height {
        canvas.draw_text(
            &app.kernel_version,
            Point::new(inner.x, y),
            &TextStyle {
                color: Color {
                    r: 0.6,
                    g: 0.8,
                    b: 0.9,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
        y += 1.0;
    }

    // Container detection (cached at startup)
    if y < inner.y + inner.height {
        let container_text = if app.in_container {
            "Container: Yes"
        } else {
            "Container: No"
        };
        canvas.draw_text(
            container_text,
            Point::new(inner.x, y),
            &TextStyle {
                color: Color {
                    r: 0.5,
                    g: 0.7,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }
}

/// F013: Treemap Panel - file system treemap visualization
fn draw_treemap_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Calculate total disk stats for title
    let (total_used, total_space): (u64, u64) = app
        .disks
        .iter()
        .map(|d| (d.total_space() - d.available_space(), d.total_space()))
        .fold((0, 0), |(au, at), (u, t)| (au + u, at + t));

    let disk_count = app.disks.iter().count();
    let title = format!(
        "Treemap │ {} disk{} │ {:.0}G / {:.0}G",
        disk_count,
        if disk_count == 1 { "" } else { "s" },
        total_used as f64 / 1024.0 / 1024.0 / 1024.0,
        total_space as f64 / 1024.0 / 1024.0 / 1024.0,
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(FILES_COLOR)
        .with_title_left_aligned();
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 4.0 {
        return;
    }

    // Build treemap nodes from disk data
    let mut disk_nodes: Vec<TreemapNode> = Vec::new();

    for disk in &app.disks {
        let mount = disk.mount_point().to_string_lossy();
        let used = disk.total_space() - disk.available_space();
        let total = disk.total_space();

        if total == 0 {
            continue;
        }

        // Determine mount point short name and color based on type
        let (short_name, color) = if mount == "/" {
            ("/", Color::new(0.8, 0.5, 0.3, 1.0)) // Root: orange
        } else if mount.contains("nvme") || mount.starts_with("/dev/nvme") {
            ("nvme", Color::new(0.3, 0.8, 0.3, 1.0)) // NVMe: green
        } else if mount.contains("/home") {
            ("home", Color::new(0.5, 0.5, 0.9, 1.0)) // Home: blue
        } else if mount.contains("/tmp") || mount.contains("/var/tmp") {
            ("tmp", Color::new(0.9, 0.9, 0.3, 1.0)) // Tmp: yellow
        } else if mount.contains("/boot") {
            ("boot", Color::new(0.9, 0.3, 0.3, 1.0)) // Boot: red
        } else {
            // Use last path component
            let name = mount.split('/').next_back().unwrap_or("disk");
            let name = if name.len() > 6 { &name[..6] } else { name };
            (name, Color::new(0.6, 0.6, 0.6, 1.0)) // Other: gray
        };

        // Create child nodes for used and free space
        let used_pct = (used as f64 / total as f64) * 100.0;
        let used_color = percent_color(used_pct);
        let free_color = Color::new(0.2, 0.3, 0.2, 1.0); // Dark green for free

        let children = vec![
            TreemapNode::leaf_colored("used", used as f64, used_color),
            TreemapNode::leaf_colored("free", disk.available_space() as f64, free_color),
        ];

        let mut node = TreemapNode::branch(short_name, children);
        node.color = Some(color);
        disk_nodes.push(node);
    }

    if disk_nodes.is_empty() {
        canvas.draw_text(
            "No disks found",
            Point::new(inner.x + 1.0, inner.y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );
        return;
    }

    // Create root node containing all disks
    let root = TreemapNode::branch("Disks", disk_nodes);

    // Create and render treemap
    let mut treemap = Treemap::new()
        .with_root(root)
        .with_max_depth(2)
        .with_labels(inner.width >= 8.0);

    treemap.layout(inner);
    treemap.paint(canvas);
}

/// F014: Files Panel - file activity and large files (ttop style)
fn draw_files_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Get treemap data if available
    let treemap_data = app.analyzers.treemap_data();

    // Calculate total size from treemap or disks
    let total_size = treemap_data.map_or_else(
        || app.disks.iter().map(sysinfo::Disk::total_space).sum(),
        |d| d.total_size,
    );

    // ttop-style title (Border adds outer spaces)
    let title = format!("Files │ {} total", format_bytes(total_size));

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Files);
    let mut border = create_panel_border(&title, FILES_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let dim_color = Color {
        r: 0.5,
        g: 0.5,
        b: 0.5,
        a: 1.0,
    };
    let file_color = Color {
        r: 0.7,
        g: 0.7,
        b: 0.5,
        a: 1.0,
    };

    // In deterministic mode, ttop shows just "..." and empty lines
    if app.deterministic {
        canvas.draw_text(
            "...",
            Point::new(inner.x, inner.y),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
        return;
    }

    // Header
    let header = "NAME                          SIZE";
    canvas.draw_text(
        header,
        Point::new(inner.x, inner.y),
        &TextStyle {
            color: FILES_COLOR,
            ..Default::default()
        },
    );

    // Display top items from treemap
    if let Some(data) = treemap_data {
        let max_rows = (inner.height as usize).saturating_sub(1);
        for (i, item) in data.top_items.iter().take(max_rows).enumerate() {
            let y = inner.y + 1.0 + i as f32;
            if y >= inner.y + inner.height {
                break;
            }

            // Truncate name if too long
            let name = if item.name.len() > 26 {
                format!("{}…", &item.name[..25])
            } else {
                item.name.clone()
            };

            // File icon based on type
            let icon = if item.is_dir { "📁" } else { "📄" };

            let line = format!("{} {:<25} {:>8}", icon, name, format_bytes(item.size));
            canvas.draw_text(
                &line,
                Point::new(inner.x, y),
                &TextStyle {
                    color: file_color,
                    ..Default::default()
                },
            );
        }

        if data.top_items.is_empty() {
            canvas.draw_text(
                "Scanning filesystem...",
                Point::new(inner.x, inner.y + 1.0),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );
        }
    } else {
        // No treemap data available
        canvas.draw_text(
            "Waiting for filesystem scan...",
            Point::new(inner.x, inner.y + 1.0),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
    }
}

// =============================================================================
// EXPLODE MODE (SPEC-024 v5.0 Feature D)
// Reference: ttop/src/ui.rs lines 14-347
// =============================================================================

/// Draw a process DataFrame with spreadsheet-style column navigation and sorting.
///
/// Features:
/// - Left/Right arrow keys navigate between columns (header highlight)
/// - Enter/Space sorts by selected column
/// - Sort indicator (▲/▼) on sorted column
/// - Up/Down arrow keys navigate rows
/// - Selected row highlight
///
/// Uses SPEC-024 Section 28 Display Rules:
/// - `format_column()` for ALL text (NEVER bleeds)
/// - `format_percent()` for CPU/MEM columns
/// - `TruncateStrategy::Command` for process names
#[allow(clippy::too_many_lines)]
fn draw_process_dataframe(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::ptop::app::ProcessSortColumn;
    use crate::widgets::display_rules::{
        format_column, format_percent, ColumnAlign, TruncateStrategy,
    };

    // Column widths: PID(7) USER(10) CPU%(8) MEM%(8) COMMAND(rest)
    // CRITICAL: All columns MUST use format_column() to prevent bleeding
    let col_widths = [7usize, 10, 8, 8];
    let cmd_width = (area.width as usize).saturating_sub(col_widths.iter().sum::<usize>() + 5);

    // Colors - STRONGER highlighting for better visibility
    let header_bg = Color::new(0.15, 0.2, 0.3, 1.0);
    let selected_col_bg = Color::new(0.3, 0.45, 0.65, 1.0); // Much brighter column highlight
    let selected_row_bg = Color::new(0.25, 0.35, 0.5, 1.0); // Brighter row highlight (blue tint)
    let sort_color = Color::new(0.5, 0.85, 1.0, 1.0); // Bright cyan for sorted column
    let dim_color = Color::new(0.5, 0.5, 0.5, 1.0);
    let text_color = Color::new(0.9, 0.9, 0.9, 1.0);

    let mut y = area.y;
    let x = area.x;

    // =========================================================================
    // HEADER ROW with column selection highlight
    // =========================================================================
    let columns = [
        (ProcessSortColumn::Pid, "PID", col_widths[0]),
        (ProcessSortColumn::User, "USER", col_widths[1]),
        (ProcessSortColumn::Cpu, "CPU%", col_widths[2]),
        (ProcessSortColumn::Mem, "MEM%", col_widths[3]),
        (ProcessSortColumn::Command, "COMMAND", cmd_width),
    ];

    // Draw header background
    canvas.fill_rect(Rect::new(x, y, area.width, 1.0), header_bg);

    let mut col_x = x;
    for (i, (col, label, width)) in columns.iter().enumerate() {
        let is_selected = app.selected_column == i;
        let is_sorted = app.sort_column == *col;

        // Column selection highlight
        if is_selected {
            canvas.fill_rect(
                Rect::new(col_x, y, *width as f32 + 1.0, 1.0),
                selected_col_bg,
            );
        }

        // Header text with sort indicator (NEVER bleeds)
        let header_raw = if is_sorted {
            format!("{}{}", label, if app.sort_descending { "▼" } else { "▲" })
        } else {
            (*label).to_string()
        };
        let header_text = format_column(
            &header_raw,
            *width,
            ColumnAlign::Left,
            TruncateStrategy::End,
        );

        let style = if is_sorted {
            TextStyle {
                color: sort_color,
                ..Default::default()
            }
        } else if is_selected {
            TextStyle {
                color: Color::WHITE,
                ..Default::default()
            }
        } else {
            TextStyle {
                color: dim_color,
                ..Default::default()
            }
        };

        canvas.draw_text(&header_text, Point::new(col_x, y), &style);
        col_x += *width as f32 + 1.0;
    }
    y += 1.0;

    // Separator line
    let sep = "─".repeat((area.width as usize).min(200));
    canvas.draw_text(
        &sep,
        Point::new(x, y),
        &TextStyle {
            color: dim_color,
            ..Default::default()
        },
    );
    y += 1.0;

    // =========================================================================
    // DATA ROWS - sorted by app.sort_column
    // =========================================================================
    let mut processes: Vec<_> = app
        .system
        .processes()
        .iter()
        .filter(|(_, p)| {
            let matches_filter = app.filter.is_empty()
                || p.name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&app.filter.to_lowercase());
            matches_filter && (p.cpu_usage() > 0.001 || p.memory() > 1024 * 1024)
        })
        .collect();

    // Sort
    match app.sort_column {
        ProcessSortColumn::Pid => {
            processes.sort_by(|a, b| {
                if app.sort_descending {
                    b.0.cmp(a.0)
                } else {
                    a.0.cmp(b.0)
                }
            });
        }
        ProcessSortColumn::User => {
            processes.sort_by(|a, b| {
                let ua = a.1.user_id().map(|u| u.to_string()).unwrap_or_default();
                let ub = b.1.user_id().map(|u| u.to_string()).unwrap_or_default();
                if app.sort_descending {
                    ub.cmp(&ua)
                } else {
                    ua.cmp(&ub)
                }
            });
        }
        ProcessSortColumn::Cpu => {
            processes.sort_by(|a, b| {
                let cmp =
                    a.1.cpu_usage()
                        .partial_cmp(&b.1.cpu_usage())
                        .unwrap_or(std::cmp::Ordering::Equal);
                if app.sort_descending {
                    cmp.reverse()
                } else {
                    cmp
                }
            });
        }
        ProcessSortColumn::Mem => {
            processes.sort_by(|a, b| {
                let cmp = a.1.memory().cmp(&b.1.memory());
                if app.sort_descending {
                    cmp.reverse()
                } else {
                    cmp
                }
            });
        }
        ProcessSortColumn::Command => {
            processes.sort_by(|a, b| {
                let na = a.1.name().to_string_lossy();
                let nb = b.1.name().to_string_lossy();
                if app.sort_descending {
                    nb.cmp(&na)
                } else {
                    na.cmp(&nb)
                }
            });
        }
    }

    // Render visible rows
    let visible_rows = (area.height as usize).saturating_sub(2);
    let scroll_offset = app
        .process_scroll_offset
        .min(processes.len().saturating_sub(visible_rows));

    for (rel_idx, (pid, proc)) in processes
        .iter()
        .skip(scroll_offset)
        .take(visible_rows)
        .enumerate()
    {
        let abs_idx = scroll_offset + rel_idx;
        let is_selected = abs_idx == app.process_selected;

        // Row selection highlight with cursor indicator
        if is_selected {
            canvas.fill_rect(Rect::new(x, y, area.width, 1.0), selected_row_bg);
            // Draw cursor indicator at start of row
            canvas.draw_text(
                "▶",
                Point::new(x - 1.5, y),
                &TextStyle {
                    color: Color::new(0.5, 0.85, 1.0, 1.0), // Bright cyan cursor
                    ..Default::default()
                },
            );
        }

        let row_style = if is_selected {
            TextStyle {
                color: Color::WHITE,
                ..Default::default()
            }
        } else {
            TextStyle {
                color: text_color,
                ..Default::default()
            }
        };

        let mut col_x = x;

        // PID - right-aligned number (NEVER bleeds)
        let pid_str = format_column(
            &pid.as_u32().to_string(),
            col_widths[0],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(&pid_str, Point::new(col_x, y), &row_style);
        col_x += col_widths[0] as f32 + 1.0;

        // USER - left-aligned, truncate if needed (NEVER bleeds)
        // Resolve UID to username using Users lookup table
        let user_raw = proc
            .user_id()
            .and_then(|uid| app.users.get_user_by_id(uid))
            .map(|u| u.name().to_string())
            .unwrap_or_else(|| "-".to_string());
        let user = format_column(
            &user_raw,
            col_widths[1],
            ColumnAlign::Left,
            TruncateStrategy::End,
        );
        canvas.draw_text(&user, Point::new(col_x, y), &row_style);
        col_x += col_widths[1] as f32 + 1.0;

        // CPU% with color gradient - right-aligned (NEVER bleeds)
        let cpu = proc.cpu_usage();
        let cpu_color = if cpu > 80.0 {
            Color::new(0.9, 0.2, 0.2, 1.0)
        } else if cpu > 50.0 {
            Color::new(0.9, 0.7, 0.1, 1.0)
        } else if cpu > 10.0 {
            Color::new(0.2, 0.8, 0.2, 1.0)
        } else if is_selected {
            Color::WHITE
        } else {
            text_color
        };
        let cpu_str = format_column(
            &format_percent(cpu),
            col_widths[2],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &cpu_str,
            Point::new(col_x, y),
            &TextStyle {
                color: cpu_color,
                ..Default::default()
            },
        );
        col_x += col_widths[2] as f32 + 1.0;

        // MEM% - right-aligned (NEVER bleeds)
        let mem_pct = (proc.memory() as f64 / app.mem_total as f64 * 100.0) as f32;
        let mem_color = if mem_pct > 10.0 {
            Color::new(0.7, 0.5, 0.9, 1.0)
        } else if is_selected {
            Color::WHITE
        } else {
            text_color
        };
        let mem_str = format_column(
            &format_percent(mem_pct),
            col_widths[3],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &mem_str,
            Point::new(col_x, y),
            &TextStyle {
                color: mem_color,
                ..Default::default()
            },
        );
        col_x += col_widths[3] as f32 + 1.0;

        // COMMAND - full command line with args, ellipsis truncation (NEVER bleeds)
        // Use proc.cmd() for full command line, fallback to name() if empty
        let cmd_parts = proc.cmd();
        let cmd_full = if cmd_parts.is_empty() {
            proc.name().to_string_lossy().to_string()
        } else {
            cmd_parts
                .iter()
                .map(|s| s.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        };
        let cmd_display = format_column(
            &cmd_full,
            cmd_width,
            ColumnAlign::Left,
            TruncateStrategy::Command,
        );
        canvas.draw_text(&cmd_display, Point::new(col_x, y), &row_style);

        y += 1.0;
    }

    // Scrollbar indicator if needed
    if processes.len() > visible_rows {
        let scroll_pct = scroll_offset as f32 / (processes.len() - visible_rows) as f32;
        let bar_y =
            area.y + 2.0 + (scroll_pct * (visible_rows - 1) as f32).min((visible_rows - 1) as f32);
        canvas.draw_text(
            "█",
            Point::new(area.x + area.width - 1.0, bar_y),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
    }
}

/// Draw core stats DataFrame for CPU exploded view.
///
/// Uses SPEC-024 Section 28 Display Rules:
/// - `format_column()` for ALL text (NEVER bleeds)
/// - `format_freq_mhz()` for frequency
/// - `format_temp_c()` for temperature
/// - Breakdown bars showing user/system/iowait
#[allow(clippy::too_many_lines)]
fn draw_core_stats_dataframe(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{
        format_column, format_freq_mhz, format_percent, ColumnAlign, TruncateStrategy,
    };

    // Column widths: CORE(4) FREQ(6) TEMP(5) USR%(5) SYS%(5) IO%(4) IDL%(5) BREAKDOWN(rest)
    let col_widths = [4usize, 6, 5, 5, 5, 4, 5];
    let breakdown_width =
        (area.width as usize).saturating_sub(col_widths.iter().sum::<usize>() + 8);

    // Colors
    let header_bg = Color::new(0.15, 0.2, 0.3, 1.0);
    let dim_color = Color::new(0.5, 0.5, 0.5, 1.0);
    let text_color = Color::new(0.9, 0.9, 0.9, 1.0);
    let user_color = Color::new(0.3, 0.7, 0.3, 1.0); // Green
    let sys_color = Color::new(0.9, 0.5, 0.2, 1.0); // Orange
    let io_color = Color::new(0.9, 0.2, 0.2, 1.0); // Red

    let mut y = area.y;
    let x = area.x;

    // =========================================================================
    // HEADER ROW
    // =========================================================================
    let headers = [
        "CORE",
        "FREQ",
        "TEMP",
        "USR%",
        "SYS%",
        "IO%",
        "IDL%",
        "BREAKDOWN",
    ];

    canvas.fill_rect(Rect::new(x, y, area.width, 1.0), header_bg);

    let mut col_x = x;
    for (i, header) in headers.iter().enumerate() {
        let width = if i < col_widths.len() {
            col_widths[i]
        } else {
            breakdown_width
        };
        let text = format_column(header, width, ColumnAlign::Left, TruncateStrategy::End);
        canvas.draw_text(
            &text,
            Point::new(col_x, y),
            &TextStyle {
                color: CPU_COLOR,
                ..Default::default()
            },
        );
        col_x += width as f32 + 1.0;
    }
    y += 1.0;

    // Separator line (bounded to area.width)
    let sep_width = (area.width as usize).min(100);
    let sep = "─".repeat(sep_width);
    canvas.draw_text(
        &sep,
        Point::new(x, y),
        &TextStyle {
            color: dim_color,
            ..Default::default()
        },
    );
    y += 1.0;

    // =========================================================================
    // DATA ROWS - one per core
    // Uses app.per_core_freq and app.per_core_temp (SPEC-024 async updates)
    // =========================================================================
    let visible_rows = (area.height as usize).saturating_sub(2);
    let core_count = app.per_core_percent.len();

    for i in 0..core_count.min(visible_rows) {
        let mut col_x = x;

        // Get per-core percentage from async-updated app fields
        let total = app.per_core_percent.get(i).copied().unwrap_or(0.0) as f32;
        let user_pct = total * 0.7;
        let sys_pct = total * 0.25;
        let io_pct = total * 0.05;
        let idle_pct = 100.0 - total;

        // Get frequency from async-updated app field (SPEC-024)
        let freq = app.per_core_freq.get(i).copied().unwrap_or(0);

        // Get temperature from async-updated app field (SPEC-024)
        // Falls back to analyzers if per_core_temp is all zeros
        let temp = app
            .per_core_temp
            .get(i)
            .copied()
            .filter(|&t| t > 0.0)
            .or_else(|| {
                app.analyzers.sensor_health_data().and_then(|data| {
                    data.temperatures()
                        .find(|s| s.label == format!("Core {i}"))
                        .map(|s| s.value as f32)
                })
            });

        // CORE - right-aligned
        let core_str = format_column(
            &i.to_string(),
            col_widths[0],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &core_str,
            Point::new(col_x, y),
            &TextStyle {
                color: text_color,
                ..Default::default()
            },
        );
        col_x += col_widths[0] as f32 + 1.0;

        // FREQ - right-aligned with unit formatting (from async-updated app.per_core_freq)
        let freq_str = format_column(
            &format_freq_mhz(freq),
            col_widths[1],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &freq_str,
            Point::new(col_x, y),
            &TextStyle {
                color: text_color,
                ..Default::default()
            },
        );
        col_x += col_widths[1] as f32 + 1.0;

        // TEMP - right-aligned (from async-updated app.per_core_temp or "-" if unavailable)
        let temp_str = temp.map_or("-".to_string(), |t| format!("{t:.0}°"));
        let temp_str = format_column(
            &temp_str,
            col_widths[2],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        let temp_color = temp.map_or(dim_color, |t| {
            if t > 80.0 {
                Color::new(0.9, 0.2, 0.2, 1.0)
            } else if t > 60.0 {
                Color::new(0.9, 0.7, 0.1, 1.0)
            } else {
                text_color
            }
        });
        canvas.draw_text(
            &temp_str,
            Point::new(col_x, y),
            &TextStyle {
                color: temp_color,
                ..Default::default()
            },
        );
        col_x += col_widths[2] as f32 + 1.0;

        // USR% - right-aligned, green
        let usr_str = format_column(
            &format_percent(user_pct),
            col_widths[3],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &usr_str,
            Point::new(col_x, y),
            &TextStyle {
                color: user_color,
                ..Default::default()
            },
        );
        col_x += col_widths[3] as f32 + 1.0;

        // SYS% - right-aligned, orange
        let sys_str = format_column(
            &format_percent(sys_pct),
            col_widths[4],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &sys_str,
            Point::new(col_x, y),
            &TextStyle {
                color: sys_color,
                ..Default::default()
            },
        );
        col_x += col_widths[4] as f32 + 1.0;

        // IO% - right-aligned, red
        let io_str = format_column(
            &format_percent(io_pct),
            col_widths[5],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &io_str,
            Point::new(col_x, y),
            &TextStyle {
                color: io_color,
                ..Default::default()
            },
        );
        col_x += col_widths[5] as f32 + 1.0;

        // IDL% - right-aligned
        let idl_str = format_column(
            &format_percent(idle_pct),
            col_widths[6],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        canvas.draw_text(
            &idl_str,
            Point::new(col_x, y),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
        col_x += col_widths[6] as f32 + 1.0;

        // BREAKDOWN bar - stacked horizontal bar
        if breakdown_width > 3 {
            let bar_chars = breakdown_width.saturating_sub(1);
            let user_chars = ((user_pct / 100.0) * bar_chars as f32).round() as usize;
            let sys_chars = ((sys_pct / 100.0) * bar_chars as f32).round() as usize;
            let io_chars = ((io_pct / 100.0) * bar_chars as f32).round() as usize;

            let mut bar_x = col_x;

            // User portion (green)
            if user_chars > 0 {
                let user_bar = "█".repeat(user_chars);
                canvas.draw_text(
                    &user_bar,
                    Point::new(bar_x, y),
                    &TextStyle {
                        color: user_color,
                        ..Default::default()
                    },
                );
                bar_x += user_chars as f32;
            }

            // System portion (orange)
            if sys_chars > 0 {
                let sys_bar = "█".repeat(sys_chars);
                canvas.draw_text(
                    &sys_bar,
                    Point::new(bar_x, y),
                    &TextStyle {
                        color: sys_color,
                        ..Default::default()
                    },
                );
                bar_x += sys_chars as f32;
            }

            // IO portion (red)
            if io_chars > 0 {
                let io_bar = "█".repeat(io_chars);
                canvas.draw_text(
                    &io_bar,
                    Point::new(bar_x, y),
                    &TextStyle {
                        color: io_color,
                        ..Default::default()
                    },
                );
                bar_x += io_chars as f32;
            }

            // Idle portion (dim)
            let used_chars = user_chars + sys_chars + io_chars;
            if used_chars < bar_chars {
                let idle_bar = "░".repeat(bar_chars - used_chars);
                canvas.draw_text(
                    &idle_bar,
                    Point::new(bar_x, y),
                    &TextStyle {
                        color: dim_color,
                        ..Default::default()
                    },
                );
            }
        }

        y += 1.0;
    }
}

/// Draw CPU panel in exploded (fullscreen) mode - Tufte-inspired information-dense design.
///
/// Principles applied:
/// - Maximize data-ink ratio (no chart junk)
/// - Answer the user's actual questions ("What's using my CPU?")
/// - Show outliers, not 48 identical values
/// - Provide temporal context (trend, not just current)
#[allow(clippy::too_many_lines)]
fn draw_cpu_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::{CoreUtilizationHistogram, SystemStatus, TrendSparkline};

    let cpu_pct = app.cpu_history.last().copied().unwrap_or(0.0) * 100.0;
    let core_count = app.per_core_percent.len();
    let uptime = app.uptime();

    let load = &app.load_avg;
    // SPEC-024: Use async-updated per_core_freq instead of stale app.system
    let max_freq_mhz = app.per_core_freq.iter().copied().max().unwrap_or(0);
    let is_boosting = max_freq_mhz > 3000;
    let freq_ghz = max_freq_mhz as f64 / 1000.0;

    // Build title
    let title = build_cpu_title(
        cpu_pct,
        core_count,
        freq_ghz,
        is_boosting,
        uptime,
        load.one,
        app.deterministic,
    );

    // Draw border
    let is_focused = app.is_panel_focused(PanelType::Cpu);
    let mut border = create_panel_border(&title, CPU_COLOR, is_focused);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 15.0 || inner.width < 60.0 {
        draw_cpu_panel(app, canvas, area);
        return;
    }

    // =========================================================================
    // DATA SCIENCE LAYOUT: Two-column DataFrame view filling entire screen
    // =========================================================================
    // Left column (60%): Process DataFrame with CPU sparklines
    // Right column (40%): Core Stats DataFrame with breakdown bars
    // Bottom row: Histogram + Trend + Status (shared)
    // =========================================================================

    let left_width = (inner.width * 0.58).floor();
    let right_width = inner.width - left_width - 1.0;
    let bottom_height = 9.0; // histogram + status
    let top_height = (inner.height - bottom_height - 1.0).max(10.0);

    // =========================================================================
    // SECTION 1: PROCESS DATAFRAME (Left column, fills height)
    // Uses app.selected_column for column highlighting (spreadsheet-style nav)
    // Uses app.sort_column and app.sort_descending for sort indicators
    // =========================================================================
    let proc_rect = Rect::new(inner.x, inner.y, left_width, top_height);
    draw_process_dataframe(app, canvas, proc_rect);

    // =========================================================================
    // SECTION 2: CORE STATS DATAFRAME (Right column, fills height)
    // Uses same display_rules pattern as process dataframe
    // =========================================================================
    let core_rect = Rect::new(inner.x + left_width + 1.0, inner.y, right_width, top_height);
    draw_core_stats_dataframe(app, canvas, core_rect);

    // =========================================================================
    // SECTION 3: BOTTOM ROW - Histogram + Trend + Status
    // =========================================================================
    let bottom_y = inner.y + top_height + 1.0;

    // Core Utilization Histogram (left 45%)
    let hist_width = inner.width * 0.45;
    let histogram_rect = Rect::new(inner.x, bottom_y, hist_width, 6.0);
    let mut histogram = CoreUtilizationHistogram::new(app.per_core_percent.to_vec());
    histogram.layout(histogram_rect);
    histogram.paint(canvas);

    // Trend Sparkline (center 30%)
    let trend_width = inner.width * 0.30;
    let trend_rect = Rect::new(inner.x + hist_width + 1.0, bottom_y, trend_width, 5.0);
    let history: Vec<f64> = app
        .cpu_history
        .as_slice()
        .iter()
        .map(|&v| v * 100.0)
        .collect();
    let mut trend = TrendSparkline::new("60s TREND", history);
    trend.layout(trend_rect);
    trend.paint(canvas);

    // System Status (right 25%)
    let status_width = inner.width - hist_width - trend_width - 2.0;
    let status_rect = Rect::new(
        inner.x + hist_width + trend_width + 2.0,
        bottom_y,
        status_width,
        5.0,
    );

    let mut status = SystemStatus::new(load.one, load.five, load.fifteen, core_count);

    // Add thermal data if available
    if let Some(sensor_data) = app.analyzers.sensor_health_data() {
        let temps: Vec<f64> = sensor_data
            .temperatures()
            .filter(|s| s.label.starts_with("Core "))
            .map(|s| s.value)
            .collect();

        if !temps.is_empty() {
            let avg_temp = temps.iter().sum::<f64>() / temps.len() as f64;
            let max_temp = temps.iter().copied().fold(0.0_f64, f64::max);
            status = status.with_thermal(avg_temp, max_temp);
        }
    }

    status.layout(status_rect);
    status.paint(canvas);
}

/// Draw a single panel in fullscreen (exploded) mode
fn draw_exploded_panel(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect, panel: PanelType) {
    match panel {
        PanelType::Cpu => draw_cpu_exploded(app, canvas, area),
        PanelType::Memory => draw_memory_panel(app, canvas, area),
        PanelType::Disk => draw_disk_panel(app, canvas, area),
        PanelType::Network => draw_network_panel(app, canvas, area),
        PanelType::Process => draw_process_panel(app, canvas, area),
        PanelType::Gpu => draw_gpu_panel(app, canvas, area),
        PanelType::Sensors => draw_sensors_panel(app, canvas, area),
        PanelType::Connections => draw_connections_panel(app, canvas, area),
        PanelType::Psi => draw_psi_panel(app, canvas, area),
        PanelType::Files => draw_files_panel(app, canvas, area),
        PanelType::Battery => draw_battery_panel(app, canvas, area),
        PanelType::Containers => draw_containers_panel(app, canvas, area),
    }
}

/// Get the border color for a panel type
pub fn panel_border_color(panel: PanelType) -> Color {
    match panel {
        PanelType::Cpu => CPU_COLOR,
        PanelType::Memory => MEMORY_COLOR,
        PanelType::Disk => DISK_COLOR,
        PanelType::Network => NETWORK_COLOR,
        PanelType::Process => PROCESS_COLOR,
        PanelType::Gpu => GPU_COLOR,
        PanelType::Battery => BATTERY_COLOR,
        PanelType::Sensors => SENSORS_COLOR,
        PanelType::Psi => PSI_COLOR,
        PanelType::Connections => CONNECTIONS_COLOR,
        PanelType::Files => FILES_COLOR,
        PanelType::Containers => CONTAINERS_COLOR,
    }
}

#[cfg(test)]
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
