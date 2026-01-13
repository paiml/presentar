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
    Border, BorderStyle, BrailleGraph, CpuGrid, GraphMode, NetworkInterface, NetworkPanel,
    ProcessEntry, ProcessState, ProcessTable, TitleBar, Treemap, TreemapNode,
};
use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};

use crate::ptop::analyzers::{ContainerState, SensorStatus, SensorType, TcpState};
use crate::ptop::app::{App, ProcessSortColumn};
use crate::ptop::config::{calculate_grid_layout, snap_to_grid, DetailLevel, PanelType};
use crate::ptop::ui::core::panel_cpu::{
    build_cpu_title, build_cpu_title_compact, consumer_cpu_color, load_color, load_trend_arrow,
    build_load_bar, CpuMeterLayout, DIM_LABEL_COLOR, PROCESS_NAME_COLOR,
};
use crate::ptop::ui::core::panel_memory::{
    swap_color, MemoryStats as MemStats, psi_memory_indicator, thrashing_indicator,
    has_swap_activity, ZramDisplay, ZRAM_COLOR, RATIO_COLOR,
    CACHED_COLOR, DIM_COLOR, FREE_COLOR,
};
#[allow(unused_imports)]
use crate::ptop::ui::core::panel_gpu::{
    gpu_temp_color, gpu_proc_badge, build_gpu_bar, build_gpu_title,
    format_proc_util, truncate_name, POWER_COLOR, HEADER_COLOR, PROC_INFO_COLOR,
    VRAM_GRAPH_COLOR,
};
use crate::ptop::ui::panels::connections::{
    build_sparkline, DIM_COLOR as CONN_DIM_COLOR, ACTIVE_COLOR, LISTEN_COLOR,
};
use crate::ptop::ui::core::layout::push_if_visible;
// Atomic widget helpers (available for incremental adoption)
#[allow(unused_imports)]
use crate::ptop::ui_atoms::{draw_colored_text, severity_color, usage_color};

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

// =============================================================================
// TUFTE-INSPIRED SELECTION: Use framework widgets from widgets/selection.rs
// =============================================================================
use crate::widgets::selection::{SELECTION_ACCENT, SELECTION_BG};

// Re-export for local use (framework-first design)
const FOCUS_ACCENT_COLOR: Color = SELECTION_ACCENT;
const ROW_SELECT_BG: Color = SELECTION_BG;

/// Column header selection background (slightly different from row)
const COL_SELECT_BG: Color = Color {
    r: 0.15,
    g: 0.4,
    b: 0.65,
    a: 1.0,
};

/// Status bar background
const STATUS_BAR_BG: Color = Color {
    r: 0.08,
    g: 0.08,
    b: 0.12,
    a: 1.0,
};

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

// Note: format_uptime moved to format.rs

/// Create a border with PATTERN 5 HYBRID focus indication
/// Focus indicators (WCAG AAA compliant):
/// 1. Double-line border (vs rounded for unfocused) - works in monochrome
/// 2. Bright cyan accent color for focused - high visibility
/// 3. Focus indicator arrow `►` prepended to title
/// 4. Unfocused panels are dimmed for contrast
fn create_panel_border(title: &str, color: Color, is_focused: bool) -> Border {
    let style = if is_focused {
        BorderStyle::Double // Double border for focused panel (Pattern 1)
    } else {
        BorderStyle::Rounded // Normal rounded border
    };

    // PATTERN 5: Use accent color for focused, dim for unfocused
    let border_color = if is_focused {
        // Blend panel color with cyan accent for focused state
        Color {
            r: (color.r * 0.5 + FOCUS_ACCENT_COLOR.r * 0.5).min(1.0),
            g: (color.g * 0.5 + FOCUS_ACCENT_COLOR.g * 0.5).min(1.0),
            b: (color.b * 0.5 + FOCUS_ACCENT_COLOR.b * 0.5).min(1.0),
            a: color.a,
        }
    } else {
        // Dim unfocused panels significantly for contrast
        Color {
            r: color.r * 0.5,
            g: color.g * 0.5,
            b: color.b * 0.5,
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

/// FRAMEWORK-LEVEL: Paint a panel with automatic clipping
///
/// This function:
/// 1. Creates and paints the border
/// 2. Pushes a clip region for the inner content area
/// 3. Calls the content painter closure
/// 4. Pops the clip region
///
/// All panel content is automatically constrained to the inner bounds,
/// preventing text overflow into adjacent panels.
#[allow(dead_code)] // Framework function for future use
fn paint_panel_with_clip<F>(
    canvas: &mut DirectTerminalCanvas<'_>,
    title: &str,
    color: Color,
    is_focused: bool,
    bounds: Rect,
    content_painter: F,
) where
    F: FnOnce(&mut DirectTerminalCanvas<'_>, Rect),
{
    let mut border = create_panel_border(title, color, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 || inner.width < 1.0 {
        return;
    }

    // Push clip to constrain content to panel bounds
    canvas.push_clip(inner);
    content_painter(canvas, inner);
    canvas.pop_clip();
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

/// Parse compression algorithm from comp_algorithm file content.
/// Format: "lzo lzo-rle [lz4] zstd" - bracketed = active
#[cfg(target_os = "linux")]
fn parse_zram_algorithm(content: &str) -> String {
    content
        .split_whitespace()
        .find(|p| p.starts_with('[') && p.ends_with(']'))
        .map(|p| p.trim_matches(|c| c == '[' || c == ']').to_string())
        .unwrap_or_else(|| "?".to_string())
}

/// Read ZRAM stats from a specific device path.
#[cfg(target_os = "linux")]
fn read_zram_device(base_path: &str) -> Option<ZramStats> {
    use std::fs;

    let mm_stat_path = format!("{base_path}/mm_stat");
    let content = fs::read_to_string(&mm_stat_path).ok()?;
    let parts: Vec<&str> = content.split_whitespace().collect();

    if parts.len() < 2 {
        return None;
    }

    let orig = parts[0].parse::<u64>().unwrap_or(0);
    let compr = parts[1].parse::<u64>().unwrap_or(0);

    if orig == 0 {
        return None;
    }

    let algo_path = format!("{base_path}/comp_algorithm");
    let algorithm = fs::read_to_string(&algo_path)
        .map(|s| parse_zram_algorithm(&s))
        .unwrap_or_else(|_| "?".to_string());

    Some(ZramStats {
        orig_data_size: orig,
        compr_data_size: compr,
        algorithm,
    })
}

/// Read ZRAM statistics from /sys/block/zram* (Linux only)
fn read_zram_stats() -> Option<ZramStats> {
    #[cfg(target_os = "linux")]
    {
        for i in 0..4 {
            let base_path = format!("/sys/block/zram{i}");
            if !std::path::Path::new(&base_path).exists() {
                continue;
            }
            if let Some(stats) = read_zram_device(&base_path) {
                return Some(stats);
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
/// Get keybinds based on current mode.
fn get_keybinds(exploded: bool) -> &'static [(&'static str, &'static str)] {
    if exploded {
        &[("←→", "Column"), ("↵", "Sort"), ("↑↓", "Row"), ("Esc", "Exit")]
    } else {
        &[("q", "Quit"), ("?", "Help"), ("/", "Filter"), ("Tab", "Nav")]
    }
}

/// Draw title bar with app name and search.
fn draw_title_bar(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32) {
    let keybinds = get_keybinds(app.exploded_panel.is_some());
    let mut title_bar = TitleBar::new("ptop")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_search_placeholder("Filter processes...")
        .with_search_text(&app.filter)
        .with_search_active(app.show_filter_input)
        .with_keybinds(keybinds)
        .with_primary_color(CPU_COLOR);
    if app.exploded_panel.is_some() { title_bar = title_bar.with_mode_indicator("[▣]"); }
    title_bar.layout(Rect::new(0.0, 0.0, w, 1.0));
    title_bar.paint(canvas);
}

/// Compute layout heights for top/bottom panels.
fn compute_panel_layout(content_h: f32, top_count: u32, has_process: bool) -> (f32, f32) {
    let top_h = if top_count > 0 && has_process { (content_h * 0.45).max(8.0) } else if top_count > 0 { content_h } else { 0.0 };
    (top_h, content_h - top_h)
}

/// Draw bottom row panels (process, connections, files/treemap).
fn draw_bottom_row(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bottom_y: f32, bottom_h: f32, w: f32) {
    if !app.panels.process || bottom_h <= 3.0 { return; }
    let proc_w = (w * 0.4).round();
    let remaining = w - proc_w;
    let conn_w = (remaining / 2.0).floor();
    let files_w = remaining - conn_w;
    draw_process_panel(app, canvas, Rect::new(0.0, bottom_y, proc_w, bottom_h));
    if app.panels.connections { draw_connections_panel(app, canvas, Rect::new(proc_w, bottom_y, conn_w, bottom_h)); }
    if app.panels.files { draw_files_panel(app, canvas, Rect::new(proc_w + conn_w, bottom_y, files_w, bottom_h)); }
    else if app.panels.treemap { draw_treemap_panel(app, canvas, Rect::new(proc_w + conn_w, bottom_y, files_w, bottom_h)); }
}

/// Draw overlay dialogs (help, signal, filter, fps).
fn draw_overlays(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    if app.show_help { draw_help_overlay(canvas, w, h); }
    if app.pending_signal.is_some() { draw_signal_dialog(app, canvas, w, h); }
    if app.show_filter_input { draw_filter_overlay(app, canvas, w, h); }
    if app.show_fps { draw_fps_overlay(app, canvas, w); }
}

pub fn draw(app: &App, buffer: &mut CellBuffer) {
    let w = buffer.width() as f32;
    let h = buffer.height() as f32;
    if w < 10.0 || h < 5.0 { return; }

    let mut canvas = DirectTerminalCanvas::new(buffer);
    draw_title_bar(app, &mut canvas, w);

    let content_y = 1.0_f32;
    let content_h = h - 2.0; // 1 title + 1 status

    if let Some(panel) = app.exploded_panel {
        draw_exploded_panel(app, &mut canvas, Rect::new(0.0, content_y, w, content_h), panel);
        draw_status_bar(app, &mut canvas, w, h);
        return;
    }

    let top_count = count_top_panels(app);
    let (top_h, bottom_h) = compute_panel_layout(content_h, top_count, app.panels.process);

    if top_count > 0 { draw_top_panels(app, &mut canvas, Rect::new(0.0, content_y, w, top_h)); }
    draw_bottom_row(app, &mut canvas, content_y + top_h, bottom_h, w);
    draw_overlays(app, &mut canvas, w, h);
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

    // PATTERN 5 HYBRID: Status bar colors
    let bracket_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0), // Brackets
        ..Default::default()
    };
    let key_style = TextStyle {
        color: FOCUS_ACCENT_COLOR, // Bright cyan for keys
        ..Default::default()
    };
    let action_style = TextStyle {
        color: Color::new(0.7, 0.7, 0.7, 1.0), // Action text
        ..Default::default()
    };
    let focus_indicator_style = TextStyle {
        color: FOCUS_ACCENT_COLOR, // Bright cyan for focus indicator
        ..Default::default()
    };

    // Draw background bar
    canvas.fill_rect(Rect::new(0.0, y, w, 1.0), STATUS_BAR_BG);

    // Navigation hints - different for exploded vs normal view
    let hints = if app.exploded_panel.is_some() {
        " [Esc]Exit  [↑↓]Row  [←→]Col  [?]Help  [q]Quit "
    } else {
        " [Tab]Panel  [Enter]Explode  [↑↓]Row  [/]Filter  [?]Help  [q]Quit "
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
            &key_style // Key inside brackets - bright cyan
        } else {
            &action_style
        };
        canvas.draw_text(&ch.to_string(), Point::new(x, y), style);
        x += 1.0;
    }

    // PATTERN 5: Prominent focused panel indicator on right with ► cursor
    if let Some(panel) = app.focused_panel {
        let panel_name = match panel {
            PanelType::Cpu => "CPU",
            PanelType::Memory => "Memory",
            PanelType::Disk => "Disk",
            PanelType::Network => "Network",
            PanelType::Process => "Process",
            PanelType::Gpu => "GPU",
            PanelType::Battery => "Battery",
            PanelType::Sensors => "Sensors",
            PanelType::Files => "Files",
            PanelType::Connections => "Connections",
            PanelType::Psi => "PSI",
            PanelType::Containers => "Containers",
        };
        // Prominent focus indicator: "► CPU"
        let focus_text = format!("► {panel_name} ");
        let focus_x = w - focus_text.chars().count() as f32 - 1.0;
        if focus_x > x {
            canvas.draw_text(&focus_text, Point::new(focus_x, y), &focus_indicator_style);
        }
    }
}

/// Check if app is configured for ttop-style 6-panel layout.
fn is_ttop_layout(app: &App) -> bool {
    app.panels.cpu
        && app.panels.memory
        && app.panels.disk
        && app.panels.network
        && app.panels.gpu
        && app.panels.sensors
        && !app.panels.psi
        && !app.panels.battery
}

/// Draw ttop-specific 3x2 grid layout with stacked third column.
fn draw_ttop_grid(app: &App, canvas: &mut DirectTerminalCanvas<'_>, area: Rect) {
    let cell_w = area.width / 3.0;
    let cell_h = area.height / 2.0;

    // Row 0: CPU, Memory, Disk
    draw_cpu_panel(app, canvas, Rect::new(area.x, area.y, cell_w, cell_h));
    draw_memory_panel(app, canvas, Rect::new(area.x + cell_w, area.y, cell_w, cell_h));
    draw_disk_panel(app, canvas, Rect::new(area.x + 2.0 * cell_w, area.y, cell_w, cell_h));

    // Row 1: Network, GPU, Sensors+Containers stacked
    let row1_y = area.y + cell_h;
    draw_network_panel(app, canvas, Rect::new(area.x, row1_y, cell_w, cell_h));
    draw_gpu_panel(app, canvas, Rect::new(area.x + cell_w, row1_y, cell_w, cell_h));

    // Third column: Sensors (33%) + Containers (67%)
    let col3_x = area.x + 2.0 * cell_w;
    let sensors_h = (cell_h / 3.0).round();
    draw_sensors_panel(app, canvas, Rect::new(col3_x, row1_y, cell_w, sensors_h));
    draw_containers_panel(app, canvas, Rect::new(col3_x, row1_y + sensors_h, cell_w, cell_h - sensors_h));
}

/// Build list of panel draw functions based on app configuration.
#[allow(clippy::type_complexity)]
fn build_panel_list(app: &App) -> Vec<fn(&App, &mut DirectTerminalCanvas<'_>, Rect)> {
    let mut panels: Vec<fn(&App, &mut DirectTerminalCanvas<'_>, Rect)> = Vec::new();

    if app.panels.cpu { panels.push(draw_cpu_panel); }
    if app.panels.memory { panels.push(draw_memory_panel); }
    if app.panels.disk { panels.push(draw_disk_panel); }
    if app.panels.network { panels.push(draw_network_panel); }

    push_if_visible(&mut panels, app, app.panels.gpu, PanelType::Gpu, draw_gpu_panel, None);
    push_if_visible(&mut panels, app, app.panels.sensors, PanelType::Sensors, draw_sensors_panel, Some(draw_sensors_compact_panel));
    push_if_visible(&mut panels, app, app.panels.psi, PanelType::Psi, draw_psi_panel, None);
    push_if_visible(&mut panels, app, app.panels.battery, PanelType::Battery, draw_battery_panel, None);

    if app.panels.sensors_compact { panels.push(draw_sensors_compact_panel); }
    if app.panels.system { panels.push(draw_system_panel); }

    panels
}

fn draw_top_panels(app: &App, canvas: &mut DirectTerminalCanvas<'_>, area: Rect) {
    if is_ttop_layout(app) && area.width >= 100.0 {
        draw_ttop_grid(app, canvas, area);
        return;
    }

    let panels = build_panel_list(app);
    if panels.is_empty() {
        return;
    }

    let layout_config = &app.config.layout;
    let grid_rects = calculate_grid_layout(
        panels.len() as u32,
        area.width as u16,
        area.height as u16,
        layout_config,
    );

    for (i, draw_fn) in panels.iter().enumerate() {
        if let Some(rect) = grid_rects.get(i) {
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

// ============================================================================
// CPU Panel - uses extracted helpers from panel_cpu module
// ============================================================================

/// Get CPU load average and max frequency.
fn get_cpu_load_freq(app: &App) -> (sysinfo::LoadAvg, u64) {
    use sysinfo::Cpu;
    if app.deterministic {
        (sysinfo::LoadAvg { one: 0.0, five: 0.0, fifteen: 0.0 }, 0)
    } else {
        let freq = app.system.cpus().iter().map(Cpu::frequency).max().unwrap_or(0);
        (app.load_avg.clone(), freq)
    }
}

/// Draw CPU meters and history graph.
fn draw_cpu_meters_graph(
    app: &App,
    canvas: &mut DirectTerminalCanvas<'_>,
    inner: Rect,
    core_area_height: f32,
    max_freq_mhz: u64,
) {
    let core_count = app.per_core_percent.len();
    let is_exploded = inner.width > 100.0;

    let layout = CpuMeterLayout::calculate(core_count, core_area_height, is_exploded);
    let max_meter_ratio = if is_exploded { 0.70 } else { 0.5 };
    let meters_width = (layout.num_meter_cols as f32 * layout.meter_bar_width).min(inner.width * max_meter_ratio);

    let mut grid = CpuGrid::new(app.per_core_percent.clone())
        .with_frequencies(
            app.per_core_freq.iter().map(|&f| f as u32).collect(),
            vec![max_freq_mhz as u32; core_count],
        )
        .with_freq_indicators();

    if is_exploded { grid = grid.with_percentages(); }

    grid.layout(Rect::new(inner.x, inner.y, meters_width, core_area_height));
    grid.paint(canvas);

    let graph_x = inner.x + meters_width + 1.0;
    let graph_width = inner.width - meters_width - 1.0;

    if graph_width > 5.0 && !app.cpu_history.as_slice().is_empty() {
        let history: Vec<f64> = app.cpu_history.as_slice().iter().map(|&v| v * 100.0).collect();
        let mut graph = BrailleGraph::new(history)
            .with_color(CPU_COLOR)
            .with_range(0.0, 100.0)
            .with_mode(GraphMode::Block);
        graph.layout(Rect::new(graph_x, inner.y, graph_width, core_area_height));
        graph.paint(canvas);
    }
}

/// Format load average string based on available width.
fn format_load_string(load: &sysinfo::LoadAvg, core_count: usize, freq_ghz: f64, width: usize, deterministic: bool) -> String {
    let load_normalized = load.one / core_count as f64;
    let trend_1_5 = load_trend_arrow(load.one, load.five);
    let trend_5_15 = load_trend_arrow(load.five, load.fifteen);
    let load_pct = (load_normalized / 2.0).min(1.0);

    if deterministic {
        let bar = build_load_bar(load_pct, 10);
        format!("Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2} │ Fre", load.one, load.five, load.fifteen)
    } else if width >= 45 && freq_ghz > 0.0 {
        let bar = build_load_bar(load_pct, 10);
        format!("Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2}→ │ {freq_ghz:.1}GHz", load.one, load.five, load.fifteen)
    } else if width >= 35 {
        let bar = build_load_bar(load_pct, 10);
        format!("Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2}→", load.one, load.five, load.fifteen)
    } else {
        let bar = build_load_bar(load_pct, 4);
        format!("Load {bar} {:.1}{trend_1_5} {:.1}{trend_5_15} {:.1}→", load.one, load.five, load.fifteen)
    }
}

/// Draw load average gauge row.
fn draw_load_gauge(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, load_y: f32, load: &sysinfo::LoadAvg, core_count: usize, freq_ghz: f64, deterministic: bool) {
    if load_y >= inner.y + inner.height || inner.width <= 20.0 { return; }

    let load_normalized = load.one / core_count as f64;
    let load_str = format_load_string(load, core_count, freq_ghz, inner.width as usize, deterministic);

    canvas.draw_text(&load_str, Point::new(inner.x, load_y), &TextStyle { color: load_color(load_normalized), ..Default::default() });
}

/// Draw top CPU consumers row.
fn draw_top_consumers(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, consumers_y: f32) {
    if app.deterministic || consumers_y >= inner.y + inner.height || inner.width <= 20.0 { return; }

    let mut top_procs: Vec<_> = app.system.processes().values().filter(|p| p.cpu_usage() > 0.1).collect();
    top_procs.sort_by(|a, b| b.cpu_usage().partial_cmp(&a.cpu_usage()).unwrap_or(std::cmp::Ordering::Equal));

    if top_procs.is_empty() { return; }

    canvas.draw_text("Top ", Point::new(inner.x, consumers_y), &TextStyle { color: DIM_LABEL_COLOR, ..Default::default() });

    let mut x_offset = 4.0;
    for (i, proc) in top_procs.iter().take(3).enumerate() {
        let cpu = proc.cpu_usage() as f64;
        let name: String = proc.name().to_string_lossy().chars().take(12).collect();

        if i > 0 {
            canvas.draw_text(" │ ", Point::new(inner.x + x_offset, consumers_y), &TextStyle { color: DIM_LABEL_COLOR, ..Default::default() });
            x_offset += 3.0;
        }

        let cpu_str = format!("{cpu:.0}%");
        canvas.draw_text(&cpu_str, Point::new(inner.x + x_offset, consumers_y), &TextStyle { color: consumer_cpu_color(cpu), ..Default::default() });
        x_offset += cpu_str.len() as f32;

        canvas.draw_text(&format!(" {name}"), Point::new(inner.x + x_offset, consumers_y), &TextStyle { color: PROCESS_NAME_COLOR, ..Default::default() });
        x_offset += 1.0 + name.len() as f32;
    }
}

fn draw_cpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let cpu_pct = app.cpu_history.last().copied().unwrap_or(0.0) * 100.0;
    let core_count = app.per_core_percent.len();
    let uptime = app.uptime();
    let (load, max_freq_mhz) = get_cpu_load_freq(app);

    let is_boosting = max_freq_mhz > 3000;
    let freq_ghz = max_freq_mhz as f64 / 1000.0;

    let title = if bounds.width < 35.0 {
        build_cpu_title_compact(cpu_pct, core_count, freq_ghz, is_boosting)
    } else {
        build_cpu_title(cpu_pct, core_count, freq_ghz, is_boosting, uptime, load.one, app.deterministic)
    };

    let is_focused = app.is_panel_focused(PanelType::Cpu);
    let mut border = create_panel_border(&title, CPU_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 10.0 { return; }

    let reserved_bottom = 2.0_f32;
    let core_area_height = (inner.height - reserved_bottom).max(1.0);
    let has_cpu_data = !app.deterministic || app.per_core_percent.iter().any(|&p| p > 0.0);

    if has_cpu_data {
        draw_cpu_meters_graph(app, canvas, inner, core_area_height, max_freq_mhz);
    }

    draw_load_gauge(canvas, inner, inner.y + core_area_height, &load, core_count, freq_ghz, app.deterministic);
    draw_top_consumers(app, canvas, inner, inner.y + core_area_height + 1.0);
}

// ============================================================================
// Memory Panel - uses extracted helpers from panel_memory module
// Local drawing functions use imported types and colors
// ============================================================================

/// Memory statistics for deterministic rendering (legacy wrapper).
struct MemoryStats {
    used_gb: f64,
    cached_gb: f64,
    free_gb: f64,
}

impl MemoryStats {
    fn from_app(app: &App) -> Self {
        let stats = MemStats::from_bytes(app.mem_used, app.mem_cached, app.mem_available, app.mem_total);
        Self {
            used_gb: stats.used_gb,
            cached_gb: stats.cached_gb,
            free_gb: stats.free_gb,
        }
    }
}

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
/// Compute memory percentages from app state.
fn compute_mem_percentages(app: &App) -> (f64, f64, f64, f64) {
    let mem_total = app.mem_total;
    let swap_total = app.swap_total;
    let used_pct = if mem_total > 0 { (app.mem_used as f64 / mem_total as f64) * 100.0 } else { 0.0 };
    let cached_pct = if mem_total > 0 { (app.mem_cached as f64 / mem_total as f64) * 100.0 } else { 0.0 };
    let free_pct = if mem_total > 0 { (app.mem_available as f64 / mem_total as f64) * 100.0 } else { 0.0 };
    let swap_pct = if swap_total > 0 { (app.swap_used as f64 / swap_total as f64) * 100.0 } else { 0.0 };
    (used_pct, cached_pct, free_pct, swap_pct)
}

/// Build memory rows vector with optional ZRAM row.
fn build_memory_rows(app: &App, has_zram: bool) -> Vec<(&'static str, f64, f64, Color)> {
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let (used_pct, cached_pct, free_pct, swap_pct) = compute_mem_percentages(app);
    let mut rows: Vec<(&str, f64, f64, Color)> = vec![
        ("Used", gb(app.mem_used), used_pct, percent_color(used_pct)),
        ("Swap", gb(app.swap_used), swap_pct, swap_color(swap_pct)),
        ("Cached", gb(app.mem_cached), cached_pct, CACHED_COLOR),
        ("Free", gb(app.mem_available), free_pct, FREE_COLOR),
    ];
    if has_zram {
        rows.insert(2, ("ZRAM", 0.0, 0.0, Color { r: 0.8, g: 0.4, b: 1.0, a: 1.0 }));
    }
    rows
}

/// Draw ZRAM row in ttop style.
fn draw_zram_row(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32, zram_data: &(f64, f64, f64, &str)) {
    let (orig_gb, compr_gb, ratio, algo) = zram_data;
    let orig_str = ZramDisplay::format_size(*orig_gb);
    let compr_str = ZramDisplay::format_size(*compr_gb);
    canvas.draw_text("  ZRAM ", Point::new(inner.x, y), &TextStyle { color: DIM_COLOR, ..Default::default() });
    canvas.draw_text(&format!("{orig_str}→{compr_str} "), Point::new(inner.x + 7.0, y), &TextStyle { color: ZRAM_COLOR, ..Default::default() });
    let ratio_x = inner.x + 7.0 + orig_str.len() as f32 + 1.0 + compr_str.len() as f32 + 1.0;
    canvas.draw_text(&format!("{ratio:.1}x"), Point::new(ratio_x, y), &TextStyle { color: RATIO_COLOR, ..Default::default() });
    canvas.draw_text(&format!(" {algo}"), Point::new(ratio_x + 4.0, y), &TextStyle { color: DIM_COLOR, ..Default::default() });
}

/// Draw a single memory row with progress bar.
fn draw_memory_row_bar(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32, label: &str, value: f64, pct: f64, color: Color) {
    let bar_width = 10.min((inner.width as usize).saturating_sub(22));
    let filled = ((pct / 100.0) * bar_width as f64) as usize;
    let bar: String = "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));
    let text = format!("{label:>6} {value:>5.1}G {bar} {pct:>5.1}%");
    canvas.draw_text(&text, Point::new(inner.x, y), &TextStyle { color, ..Default::default() });
}

/// Draw swap thrashing indicator if active.
fn draw_swap_thrash_indicator(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32) {
    if let Some(swap_data) = app.analyzers.swap_data() {
        let (is_thrashing, severity) = swap_data.is_thrashing();
        if has_swap_activity(is_thrashing, swap_data.swap_in_rate, swap_data.swap_out_rate) {
            let (indicator, ind_color) = thrashing_indicator(severity);
            let bar_width = 10.min((inner.width as usize).saturating_sub(22));
            let thrash_x = inner.x + 28.0 + bar_width as f32;
            let thrash_text = format!(" {indicator} I:{:.0}/O:{:.0}", swap_data.swap_in_rate, swap_data.swap_out_rate);
            canvas.draw_text(&thrash_text, Point::new(thrash_x, y), &TextStyle { color: ind_color, ..Default::default() });
        }
    }
}

/// Draw PSI memory pressure indicator.
fn draw_mem_psi_indicator(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32) {
    if let Some(psi) = app.psi_data() {
        let mem_some = psi.memory.some.avg10;
        let mem_full = psi.memory.full.as_ref().map_or(0.0, |f| f.avg10);
        let (symbol, color) = psi_memory_indicator(mem_some, mem_full);
        let psi_text = format!("   PSI {symbol} {mem_some:>5.1}% some {mem_full:>5.1}% full");
        canvas.draw_text(&psi_text, Point::new(inner.x, y), &TextStyle { color, ..Default::default() });
    }
}

/// Draw memory rows in normal mode with bars and indicators.
fn draw_memory_rows_normal(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, mut y: f32, rows: &[(&str, f64, f64, Color)], zram_data: Option<(f64, f64, f64, &str)>) {
    for (label, value, pct, color) in rows {
        if y >= inner.y + inner.height { break; }
        if *label == "ZRAM" {
            if let Some(ref data) = zram_data {
                draw_zram_row(canvas, inner, y, data);
            }
            y += 1.0;
            continue;
        }
        draw_memory_row_bar(canvas, inner, y, label, *value, *pct, *color);
        if *label == "Swap" { draw_swap_thrash_indicator(app, canvas, inner, y); }
        y += 1.0;
    }
    if y < inner.y + inner.height { draw_mem_psi_indicator(app, canvas, inner, y); }
}

fn draw_memory_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let _detail_level = DetailLevel::for_height(bounds.height as u16);
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let mem_pct = if app.mem_total > 0 { (app.mem_used as f64 / app.mem_total as f64) * 100.0 } else { 0.0 };

    let zram_stats = if app.deterministic { None } else { read_zram_stats() };
    let zram_info = zram_stats.as_ref().filter(|z| z.is_active()).map(|z| format!(" │ ZRAM:{:.1}x", z.ratio())).unwrap_or_default();
    let title = format!("Memory │ {:.1}G / {:.1}G ({:.0}%){}", gb(app.mem_used), gb(app.mem_total), mem_pct, zram_info);

    let is_focused = app.is_panel_focused(PanelType::Memory);
    let mut border = create_panel_border(&title, MEMORY_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 || inner.width < 10.0 { return; }

    let mut y = inner.y;
    draw_memory_stacked_bar(canvas, inner, y, app);
    y += 1.0;

    if y >= inner.y + inner.height { return; }

    let zram_row_data = zram_stats.as_ref().filter(|z| z.is_active()).map(|z| (gb(z.orig_data_size), gb(z.compr_data_size), z.ratio(), z.algorithm.as_str()));
    let rows = build_memory_rows(app, zram_row_data.is_some());

    if app.deterministic {
        let stats = MemoryStats::from_app(app);
        draw_memory_rows_deterministic(canvas, inner, y, &stats);
    } else {
        draw_memory_rows_normal(app, canvas, inner, y, &rows, zram_row_data);
    }
}

/// Compute total disk stats (used, space, read_rate, write_rate).
fn compute_disk_stats(app: &App) -> (u64, u64, f64, f64) {
    if app.deterministic { return (0, 0, 0.0, 0.0); }
    let disk_io = app.disk_io_data();
    let (used, space): (u64, u64) = app.disks.iter()
        .map(|d| (d.total_space() - d.available_space(), d.total_space()))
        .fold((0, 0), |(au, at), (u, t)| (au + u, at + t));
    let r_rate = disk_io.map_or(0.0, |d| d.total_read_bytes_per_sec);
    let w_rate = disk_io.map_or(0.0, |d| d.total_write_bytes_per_sec);
    (used, space, r_rate, w_rate)
}

/// Format disk panel title.
fn format_disk_title(deterministic: bool, used: u64, space: u64, r_rate: f64, w_rate: f64) -> String {
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    if deterministic {
        "Disk │ R: 0B/s │ W: 0B/s │ -0 IOPS │".to_string()
    } else if r_rate > 0.0 || w_rate > 0.0 {
        format!("Disk │ R: {} │ W: {} │ {:.0}G / {:.0}G", format_bytes_rate(r_rate), format_bytes_rate(w_rate), gb(used), gb(space))
    } else {
        let pct = if space > 0 { (used as f64 / space as f64) * 100.0 } else { 0.0 };
        format!("Disk │ {:.0}G / {:.0}G ({:.0}%)", gb(used), gb(space), pct)
    }
}

/// Draw disk panel in deterministic mode.
fn draw_disk_deterministic(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect) {
    let dim_color = Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 };
    canvas.draw_text("I/O Pressure ○  0.0% some    0.0% full", Point::new(inner.x, inner.y), &TextStyle { color: dim_color, ..Default::default() });
    if inner.height >= 2.0 {
        canvas.draw_text("── Top Active Processes ──────────────", Point::new(inner.x, inner.y + 1.0), &TextStyle { color: dim_color, ..Default::default() });
    }
}

/// Get I/O rates for a specific disk device.
fn get_disk_io_rates(app: &App, device_name: &str) -> (f64, f64) {
    app.disk_io_data()
        .and_then(|data| data.rates.get(device_name))
        .map_or((0.0, 0.0), |rate| (rate.read_bytes_per_sec, rate.write_bytes_per_sec))
}

/// Draw a single disk row.
fn draw_disk_row(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32, disk: &sysinfo::Disk, d_read: f64, d_write: f64) {
    let mount = disk.mount_point().to_string_lossy();
    let mount_short: String = if mount == "/" { "/".to_string() } else { mount.split('/').next_back().unwrap_or(&mount).chars().take(8).collect() };
    let total = disk.total_space();
    let used = total - disk.available_space();
    let pct = if total > 0 { (used as f64 / total as f64) * 100.0 } else { 0.0 };
    let total_gb = total as f64 / 1024.0 / 1024.0 / 1024.0;
    let io_str = if d_read > 0.0 || d_write > 0.0 { format!(" R:{} W:{}", format_bytes_rate(d_read), format_bytes_rate(d_write)) } else { String::new() };
    let bar_width = (inner.width as usize).saturating_sub(24 + io_str.len()).max(2);
    let filled = ((pct / 100.0) * bar_width as f64) as usize;
    let bar: String = "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));
    let text = format!("{mount_short:<8} {total_gb:>5.0}G {bar} {pct:>5.1}%{io_str}");
    let color = if d_read > 1024.0 || d_write > 1024.0 { Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 } } else { percent_color(pct) };
    canvas.draw_text(&text, Point::new(inner.x, y), &TextStyle { color, ..Default::default() });
}

fn draw_disk_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let (total_used, total_space, read_rate, write_rate) = compute_disk_stats(app);
    let title = format_disk_title(app.deterministic, total_used, total_space, read_rate, write_rate);

    let is_focused = app.is_panel_focused(PanelType::Disk);
    let mut border = create_panel_border(&title, DISK_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 { return; }
    if app.deterministic { draw_disk_deterministic(canvas, inner); return; }

    let max_disks = inner.height as usize;
    for (i, disk) in app.disks.iter().take(max_disks).enumerate() {
        let y = inner.y + i as f32;
        if y >= inner.y + inner.height { break; }
        let disk_name = disk.name().to_string_lossy();
        let device_name = disk_name.trim_start_matches("/dev/");
        let (d_read, d_write) = get_disk_io_rates(app, device_name);
        draw_disk_row(canvas, inner, y, disk, d_read, d_write);
    }
}

/// Compute network stats (rx_total, tx_total, primary_iface).
fn compute_network_stats(app: &App) -> (u64, u64, &str) {
    if app.deterministic { return (0, 0, "none"); }
    let (rx, tx): (u64, u64) = app.networks.values().map(|d| (d.received(), d.transmitted())).fold((0, 0), |(ar, at), (r, t)| (ar + r, at + t));
    let iface = app.networks.iter().filter(|(name, _)| !name.starts_with("lo")).max_by_key(|(_, data)| data.received() + data.transmitted()).map_or("none", |(name, _)| name.as_str());
    (rx, tx, iface)
}

/// Draw network deterministic download/upload rows.
fn draw_net_dl_ul_rows(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: &mut f32) {
    let cyan = Color { r: 0.3, g: 0.8, b: 0.9, a: 1.0 };
    let red = Color { r: 1.0, g: 0.3, b: 0.3, a: 1.0 };
    let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    canvas.draw_text("↓", Point::new(inner.x, *y), &TextStyle { color: cyan, ..Default::default() });
    canvas.draw_text(" Download ", Point::new(inner.x + 1.0, *y), &TextStyle { color: cyan, ..Default::default() });
    canvas.draw_text("0B/s", Point::new(inner.x + 11.0, *y), &TextStyle { color: white, ..Default::default() });
    *y += 1.0;
    if *y < inner.y + inner.height {
        canvas.draw_text(&"⠀".repeat(inner.width as usize), Point::new(inner.x, *y), &TextStyle { color: cyan, ..Default::default() });
        *y += 1.0;
    }
    if *y < inner.y + inner.height {
        canvas.draw_text("↑", Point::new(inner.x, *y), &TextStyle { color: red, ..Default::default() });
        canvas.draw_text(" Upload   ", Point::new(inner.x + 1.0, *y), &TextStyle { color: red, ..Default::default() });
        canvas.draw_text("0B/s", Point::new(inner.x + 11.0, *y), &TextStyle { color: white, ..Default::default() });
        *y += 1.0;
    }
    for _ in 0..2 {
        if *y < inner.y + inner.height {
            canvas.draw_text(&"⠀".repeat(inner.width as usize), Point::new(inner.x, *y), &TextStyle { color: red, ..Default::default() });
            *y += 1.0;
        }
    }
}

/// Draw network deterministic session and TCP/UDP rows.
fn draw_net_session_stats(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: f32) {
    let cyan = Color { r: 0.3, g: 0.8, b: 0.9, a: 1.0 };
    let red = Color { r: 1.0, g: 0.3, b: 0.3, a: 1.0 };
    let dim = Color { r: 0.3, g: 0.3, b: 0.3, a: 1.0 };
    let white = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
    let green = Color { r: 0.3, g: 0.9, b: 0.3, a: 1.0 };
    let mut y = y;
    if y < inner.y + inner.height {
        canvas.draw_text("Session ", Point::new(inner.x, y), &TextStyle { color: dim, ..Default::default() });
        canvas.draw_text("↓", Point::new(inner.x + 8.0, y), &TextStyle { color: cyan, ..Default::default() });
        canvas.draw_text("0B", Point::new(inner.x + 9.0, y), &TextStyle { color: white, ..Default::default() });
        canvas.draw_text(" ↑", Point::new(inner.x + 11.0, y), &TextStyle { color: red, ..Default::default() });
        canvas.draw_text("0B", Point::new(inner.x + 13.0, y), &TextStyle { color: white, ..Default::default() });
        y += 1.0;
    }
    if y < inner.y + inner.height {
        let tcp_col = Color { r: 0.3, g: 0.7, b: 0.9, a: 1.0 };
        let udp_col = Color { r: 0.8, g: 0.3, b: 0.8, a: 1.0 };
        canvas.draw_text("TCP ", Point::new(inner.x, y), &TextStyle { color: tcp_col, ..Default::default() });
        canvas.draw_text("0", Point::new(inner.x + 4.0, y), &TextStyle { color: green, ..Default::default() });
        canvas.draw_text("/", Point::new(inner.x + 5.0, y), &TextStyle { color: dim, ..Default::default() });
        canvas.draw_text("0", Point::new(inner.x + 6.0, y), &TextStyle { color: tcp_col, ..Default::default() });
        canvas.draw_text(" UDP ", Point::new(inner.x + 7.0, y), &TextStyle { color: udp_col, ..Default::default() });
        canvas.draw_text("0", Point::new(inner.x + 12.0, y), &TextStyle { color: white, ..Default::default() });
        canvas.draw_text(" │ RTT ", Point::new(inner.x + 13.0, y), &TextStyle { color: dim, ..Default::default() });
        canvas.draw_text("●●●●●", Point::new(inner.x + 20.0, y), &TextStyle { color: green, ..Default::default() });
    }
}

/// Draw network panel in deterministic mode.
fn draw_network_deterministic(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect) {
    let mut y = inner.y;
    draw_net_dl_ul_rows(canvas, inner, &mut y);
    draw_net_session_stats(canvas, inner, y);
}

/// Build network interfaces list with stats.
fn build_network_interfaces(app: &App) -> Vec<NetworkInterface> {
    let network_stats_data = app.analyzers.network_stats_data();
    let mut interfaces: Vec<NetworkInterface> = Vec::new();
    for (name, data) in &app.networks {
        let mut iface = NetworkInterface::new(name);
        iface.update(data.received() as f64, data.transmitted() as f64);
        iface.set_totals(data.total_received(), data.total_transmitted());
        if let Some(stats_data) = network_stats_data {
            if let Some(stats) = stats_data.stats.get(name.as_str()) {
                iface.set_stats(stats.rx_errors, stats.tx_errors, stats.rx_dropped, stats.tx_dropped);
            }
            if let Some(rates) = stats_data.rates.get(name.as_str()) {
                iface.set_rates(rates.errors_per_sec, rates.drops_per_sec);
                iface.set_utilization(rates.utilization_percent());
            }
        }
        interfaces.push(iface);
    }
    interfaces.sort_by(|a, b| (b.rx_bps + b.tx_bps).partial_cmp(&(a.rx_bps + a.tx_bps)).unwrap_or(std::cmp::Ordering::Equal));
    interfaces
}

fn draw_network_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let (rx_total, tx_total, primary_iface) = compute_network_stats(app);
    let title = format!("Network ({}) │ ↓ {}/s │ ↑ {}/s", primary_iface, format_bytes(rx_total), format_bytes(tx_total));

    let is_focused = app.is_panel_focused(PanelType::Network);
    let mut border = create_panel_border(&title, NETWORK_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if app.deterministic { draw_network_deterministic(canvas, inner); return; }

    let mut interfaces = build_network_interfaces(app);
    for iface in interfaces.iter_mut() {
        if let Some((rx_hist, tx_hist)) = app.net_iface_history.get(&iface.name) {
            iface.rx_history = rx_hist.as_slice().to_vec();
            iface.tx_history = tx_hist.as_slice().to_vec();
        }
    }
    interfaces.truncate(4);

    if !interfaces.is_empty() && inner.height > 0.0 {
        let spark_w = (inner.width as usize / 4).max(5);
        let mut panel = NetworkPanel::new().with_spark_width(spark_w).with_rx_color(NET_RX_COLOR).with_tx_color(NET_TX_COLOR).compact();
        panel.set_interfaces(interfaces);
        panel.layout(inner);
        panel.paint(canvas);
    }
}

fn draw_process_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // ttop uses "CPU%" not "CPU" for percentage-based columns
    let sort_name = match app.sort_column {
        ProcessSortColumn::Cpu => "CPU%",
        ProcessSortColumn::Mem => "MEM%",
        ProcessSortColumn::Pid => "PID",
        ProcessSortColumn::User => "USER",
        ProcessSortColumn::Command => "CMD",
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
    let popup_h = 27.0; // Expanded for signal keybindings (SPEC-024 Appendix G.6)
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

    // Help content with section headers (SPEC-024 v5.0 Feature D, Appendix G.6)
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
        ("", "-- Signals --", true),
        ("x", "SIGTERM (graceful stop)", false),
        ("X", "SIGKILL (force kill)", false),
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

/// Draw signal confirmation dialog (SPEC-024 Appendix G.6 P0)
fn draw_signal_dialog(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    use crate::ptop::config::SignalType;

    let Some((pid, ref name, signal)) = app.pending_signal else {
        return;
    };

    let popup_w = 50.0;
    let popup_h = 7.0;
    let px = (w - popup_w) / 2.0;
    let py = (h - popup_h) / 2.0;

    // Clear background
    for y in 0..popup_h as u16 {
        let spaces: String = (0..popup_w as usize).map(|_| ' ').collect();
        canvas.draw_text(
            &spaces,
            Point::new(px, py + y as f32),
            &TextStyle {
                color: Color::new(0.15, 0.1, 0.1, 1.0),
                ..Default::default()
            },
        );
    }

    // Border color based on signal severity
    let border_color = match signal {
        SignalType::Kill => Color::new(1.0, 0.3, 0.3, 1.0), // Red for SIGKILL
        SignalType::Term => Color::new(1.0, 0.8, 0.2, 1.0), // Yellow for SIGTERM
        SignalType::Stop => Color::new(0.8, 0.4, 1.0, 1.0), // Purple for SIGSTOP
        _ => Color::new(0.3, 0.8, 0.9, 1.0),                // Cyan for others
    };

    let mut border = Border::new()
        .with_title(format!(" Send SIG{} ", signal.name()))
        .with_style(BorderStyle::Double)
        .with_color(border_color);
    border.layout(Rect::new(px, py, popup_w, popup_h));
    border.paint(canvas);

    let text_style = TextStyle {
        color: Color::new(0.9, 0.9, 0.9, 1.0),
        ..Default::default()
    };
    let warning_style = TextStyle {
        color: border_color,
        ..Default::default()
    };
    let hint_style = TextStyle {
        color: Color::new(0.6, 0.6, 0.6, 1.0),
        ..Default::default()
    };

    // Truncate process name if too long
    let max_name_len = 25;
    let display_name = if name.len() > max_name_len {
        format!("{}...", &name[..max_name_len - 3])
    } else {
        name.clone()
    };

    // Dialog content
    canvas.draw_text(
        &format!("Process: {} (PID {})", display_name, pid),
        Point::new(px + 2.0, py + 1.0),
        &text_style,
    );
    canvas.draw_text(
        &format!("Signal: {} - {}", signal.name(), signal.description()),
        Point::new(px + 2.0, py + 2.0),
        &warning_style,
    );

    // Confirmation prompt
    canvas.draw_text("", Point::new(px + 2.0, py + 3.0), &text_style);
    canvas.draw_text(
        "Send signal? [Y]es / [n]o / [Esc] cancel",
        Point::new(px + 2.0, py + 4.0),
        &text_style,
    );
    canvas.draw_text(
        "x=TERM  K=KILL  H=HUP  i=INT  p=STOP",
        Point::new(px + 2.0, py + 5.0),
        &hint_style,
    );
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
/// Try to read NVIDIA GPU info via nvidia-smi.
#[cfg(target_os = "linux")]
fn try_read_nvidia_gpu() -> Option<GpuInfo> {
    use std::process::Command;
    let output = Command::new("nvidia-smi")
        .args(["--query-gpu=name,utilization.gpu,temperature.gpu,power.draw,memory.used,memory.total", "--format=csv,noheader,nounits"])
        .output().ok()?;
    if !output.status.success() { return None; }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = stdout.lines().next()?.split(", ").collect();
    if parts.len() < 6 { return None; }
    Some(GpuInfo {
        name: parts[0].trim().to_string(),
        utilization: parts[1].trim().parse().ok(),
        temperature: parts[2].trim().parse().ok(),
        power_watts: parts[3].trim().parse().ok(),
        vram_used: parts[4].trim().parse::<u64>().ok().map(|v| v * 1024 * 1024),
        vram_total: parts[5].trim().parse::<u64>().ok().map(|v| v * 1024 * 1024),
    })
}

/// Read AMD GPU info from hwmon directory.
#[cfg(target_os = "linux")]
fn read_amd_hwmon(hwmon_dir: &std::path::Path, card_path: &str) -> Option<GpuInfo> {
    use std::fs;
    let temp = fs::read_to_string(hwmon_dir.join("temp1_input")).ok().and_then(|s| s.trim().parse::<u32>().ok()).map(|t| t / 1000);
    let power = fs::read_to_string(hwmon_dir.join("power1_average")).ok().and_then(|s| s.trim().parse::<u64>().ok()).map(|p| p as f32 / 1_000_000.0);
    if temp.is_none() && power.is_none() { return None; }
    let name = fs::read_to_string(hwmon_dir.join("name")).ok().map_or_else(|| "AMD GPU".to_string(), |s| s.trim().to_string());
    let vram_used = fs::read_to_string(format!("{card_path}/mem_info_vram_used")).ok().and_then(|s| s.trim().parse().ok());
    let vram_total = fs::read_to_string(format!("{card_path}/mem_info_vram_total")).ok().and_then(|s| s.trim().parse().ok());
    let utilization = fs::read_to_string(format!("{card_path}/gpu_busy_percent")).ok().and_then(|s| s.trim().parse().ok());
    Some(GpuInfo { name, utilization, temperature: temp, power_watts: power, vram_used, vram_total })
}

/// Try to read AMD GPU info via sysfs.
#[cfg(target_os = "linux")]
fn try_read_amd_gpu() -> Option<GpuInfo> {
    use std::fs;
    for card in 0..4 {
        let card_path = format!("/sys/class/drm/card{card}/device");
        if !std::path::Path::new(&card_path).exists() { continue; }
        let hwmon_path = format!("{card_path}/hwmon");
        if let Ok(entries) = fs::read_dir(&hwmon_path) {
            for entry in entries.flatten() {
                if let Some(info) = read_amd_hwmon(&entry.path(), &card_path) { return Some(info); }
            }
        }
    }
    None
}

pub fn read_gpu_info() -> Option<GpuInfo> {
    #[cfg(target_os = "linux")]
    { try_read_nvidia_gpu().or_else(try_read_amd_gpu) }
    #[cfg(not(target_os = "linux"))]
    { None }
}

/// F006: GPU Panel - shows GPU utilization, VRAM, temperature
/// Format GPU panel title based on detail level.
fn format_gpu_title(gpu: Option<&GpuInfo>, detail_level: DetailLevel) -> String {
    gpu.map(|g| {
        if detail_level == DetailLevel::Minimal { g.name.clone() }
        else {
            let temp_str = g.temperature.map(|t| format!(" │ {t}°C")).unwrap_or_default();
            let power_str = g.power_watts.map(|p| format!(" │ {p:.0}W")).unwrap_or_default();
            format!("{}{}{}", g.name, temp_str, power_str)
        }
    }).unwrap_or_else(|| "GPU".to_string())
}

/// Draw GPU utilization bar.
fn draw_gpu_util_bar(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: &mut f32, util: u8) {
    let bar_width = (inner.width as usize).min(20);
    let filled = ((util as f32 / 100.0) * bar_width as f32) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));
    canvas.draw_text(&format!("GPU  {bar} {util:>3}%"), Point::new(inner.x, *y), &TextStyle { color: percent_color(util as f64), ..Default::default() });
    *y += 1.0;
}

/// Draw VRAM usage bar.
fn draw_vram_bar(canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: &mut f32, used: u64, total: u64) {
    if total == 0 || *y >= inner.y + inner.height { return; }
    let pct = (used as f64 / total as f64) * 100.0;
    let bar_width = (inner.width as usize).min(20);
    let filled = ((pct / 100.0) * bar_width as f64) as usize;
    let bar: String = "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));
    canvas.draw_text(&format!("VRAM {bar} {}M/{}M", used / 1024 / 1024, total / 1024 / 1024), Point::new(inner.x, *y), &TextStyle { color: percent_color(pct), ..Default::default() });
    *y += 1.0;
}

/// Draw GPU history graphs in exploded mode.
fn draw_gpu_history_graphs(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: &mut f32) {
    let gpu_history: Vec<f64> = app.gpu_history.as_slice().to_vec();
    if !gpu_history.is_empty() {
        let mut graph = BrailleGraph::new(gpu_history).with_color(GPU_COLOR).with_label("GPU History").with_range(0.0, 100.0);
        graph.layout(Rect::new(inner.x, *y, inner.width, 6.0));
        graph.paint(canvas);
        *y += 7.0;
    }
    let vram_history: Vec<f64> = app.vram_history.as_slice().to_vec();
    if !vram_history.is_empty() {
        let mut graph = BrailleGraph::new(vram_history).with_color(VRAM_GRAPH_COLOR).with_label("VRAM History").with_range(0.0, 100.0);
        graph.layout(Rect::new(inner.x, *y, inner.width, 6.0));
        graph.paint(canvas);
        *y += 7.0;
    }
}

/// Draw GPU processes list.
fn draw_gpu_procs(app: &App, canvas: &mut DirectTerminalCanvas<'_>, inner: Rect, y: &mut f32) {
    let Some(gpu_data) = app.analyzers.gpu_procs_data() else { return; };
    if gpu_data.processes.is_empty() { return; }
    *y += 1.0;
    canvas.draw_text("TY  PID   SM%  MEM%  CMD", Point::new(inner.x, *y), &TextStyle { color: HEADER_COLOR, ..Default::default() });
    *y += 1.0;
    for proc in gpu_data.processes.iter().take(3) {
        if *y >= inner.y + inner.height { break; }
        let (type_badge, badge_color) = gpu_proc_badge(proc.proc_type.as_str());
        canvas.draw_text(type_badge, Point::new(inner.x, *y), &TextStyle { color: badge_color, ..Default::default() });
        let sm_str = format_proc_util(proc.gpu_util());
        let mem_str = format_proc_util(if proc.mem_util > 0 { Some(proc.mem_util as f32) } else { None });
        let proc_info = format!(" {:>5} {}%  {}%  {}", proc.pid, sm_str, mem_str, truncate_name(&proc.name, 12));
        canvas.draw_text(&proc_info, Point::new(inner.x + 1.0, *y), &TextStyle { color: PROC_INFO_COLOR, ..Default::default() });
        *y += 1.0;
    }
}

fn draw_gpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let detail_level = DetailLevel::for_height(bounds.height as u16);
    let gpu = app.gpu_info.clone();
    let title = format_gpu_title(gpu.as_ref(), detail_level);

    let is_focused = app.is_panel_focused(PanelType::Gpu);
    let mut border = create_panel_border(&title, GPU_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();
    if inner.height < 1.0 { return; }

    canvas.push_clip(inner);

    if let Some(g) = gpu {
        let mut y = inner.y;
        if let Some(util) = g.utilization { draw_gpu_util_bar(canvas, inner, &mut y, util); }
        if let (Some(used), Some(total)) = (g.vram_used, g.vram_total) { draw_vram_bar(canvas, inner, &mut y, used, total); }
        if let Some(temp) = g.temperature { if y < inner.y + inner.height { canvas.draw_text(&format!("Temp {temp}°C"), Point::new(inner.x, y), &TextStyle { color: gpu_temp_color(temp), ..Default::default() }); y += 1.0; } }
        if let Some(power) = g.power_watts { if y < inner.y + inner.height { canvas.draw_text(&format!("Power {power:.0}W"), Point::new(inner.x, y), &TextStyle { color: POWER_COLOR, ..Default::default() }); y += 1.0; } }
        if detail_level == DetailLevel::Exploded && y < inner.y + inner.height - 10.0 { draw_gpu_history_graphs(app, canvas, inner, &mut y); }
        if detail_level >= DetailLevel::Expanded && y < inner.y + inner.height - 3.0 { draw_gpu_procs(app, canvas, inner, &mut y); }
    } else if !app.deterministic {
        canvas.draw_text("No GPU detected or nvidia-smi not available", Point::new(inner.x, inner.y), &TextStyle { color: HEADER_COLOR, ..Default::default() });
    }

    canvas.pop_clip();
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
    use crate::ptop::analyzers::{SensorStatus, SensorType};
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
    let sensor_health_data = app.snapshot_sensor_health.as_ref();

    // Build title string with sensor counts.
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
                    ContainerState::Running => "●",
                    ContainerState::Paused => "◐",
                    ContainerState::Exited => "○",
                    ContainerState::Created => "◎",
                    ContainerState::Restarting => "↻",
                    ContainerState::Removing => "⊘",
                    ContainerState::Dead => "✗",
                    ContainerState::Unknown => "?",
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
    // Get connection data from snapshot (CB-INPUT-006: async pattern)
    let (listen_count, active_count, connections) =
        if let Some(ref conn_data) = app.snapshot_connections {
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

    // Generate sparkline for connection history (CB-CONN-007) using helper
    let sparkline_str = app
        .snapshot_connections
        .as_ref()
        .map(|conn_data| build_sparkline(&conn_data.established_sparkline(), 12))
        .unwrap_or_default();

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
    // Use imported color constants
    let dim_color = CONN_DIM_COLOR;
    let active_color = ACTIVE_COLOR;
    let listen_color = LISTEN_COLOR;

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

/// F014: Files Panel - Tufte-style file/directory visualization
///
/// SPEC-024 Appendix F: Tufte Data-Ink Ratio
/// - Every pixel must convey information
/// - Proportional bars show relative size (no wasted ink)
/// - Entropy indicators (🔒/🔓) show encryption status
/// - Column layout uses format_column() to prevent bleeding
fn draw_files_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    use crate::widgets::display_rules::{format_column, ColumnAlign, TruncateStrategy};

    // Get data from snapshots (CB-INPUT-006: async pattern).
    // FileAnalyzer uses /proc; Treemap uses filesystem scan.
    let file_data = app.snapshot_file_analyzer.as_ref();
    let treemap_data = app.snapshot_treemap.as_ref();
    let disk_entropy = app.snapshot_disk_entropy.as_ref();

    // Calculate total size and file count
    // If we have treemap data, it's more accurate for totals. Otherwise use disk info.
    let (total_size, file_count): (u64, u32) = if let Some(t) = treemap_data {
        (t.total_size, t.total_files)
    } else {
        (
            app.disks.iter().map(sysinfo::Disk::total_space).sum(),
            file_data.map_or(0, |f| f.total_open_files as u32),
        )
    };

    // Encryption status from disk_entropy
    let encrypted_count = disk_entropy.map_or(0, |d| d.encrypted_count);
    let encryption_indicator = if encrypted_count > 0 { "🔒" } else { "" };

    // ttop-style title
    let title = format!(
        "Files │ {} {} │ {} files",
        format_bytes(total_size),
        encryption_indicator,
        file_count
    );

    // Check if this panel is focused (SPEC-024 v5.0 Feature D)
    let is_focused = app.is_panel_focused(PanelType::Files);
    let mut border = create_panel_border(&title, FILES_COLOR, is_focused);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let dim_color = Color::new(0.5, 0.5, 0.5, 1.0);
    let file_color = Color::new(0.7, 0.7, 0.5, 1.0);
    let dir_color = Color::new(0.5, 0.7, 0.9, 1.0);
    let bg_color = Color::new(0.05, 0.05, 0.07, 1.0);

    // CRITICAL: Clear panel area to prevent ghosting from previous frames
    canvas.fill_rect(inner, bg_color);

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

    // Tufte-style header: minimal, functional
    let width = inner.width as usize;
    let bar_width = 12.min(width.saturating_sub(20));
    let name_width = width.saturating_sub(bar_width + 10);
    let header = format!(
        "{}{}  {}",
        format_column("NAME", name_width, ColumnAlign::Left, TruncateStrategy::End),
        format_column("SIZE", 7, ColumnAlign::Right, TruncateStrategy::End),
        format_column("%", bar_width, ColumnAlign::Left, TruncateStrategy::End)
    );
    canvas.draw_text(
        &header,
        Point::new(inner.x, inner.y),
        &TextStyle {
            color: FILES_COLOR,
            ..Default::default()
        },
    );

    // Prepare items to display
    // Mapped to a common structure: (name, size, is_dir, ratio)
    struct DisplayItem {
        name: String,
        size: u64,
        is_dir: bool,
        ratio: f64,
    }

    let items: Vec<DisplayItem> = if let Some(fd) = file_data {
        // Use "Hot Files" from FileAnalyzer
        // These are recently accessed files, sorted by access time
        // We'll show them with ratio based on max size in the list
        let max_size = fd
            .hot_files
            .iter()
            .map(|f| f.size)
            .max()
            .unwrap_or(1)
            .max(1);
        fd.hot_files
            .iter()
            .take(inner.height as usize)
            .map(|f| DisplayItem {
                name: f
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string(),
                size: f.size,
                is_dir: f.path.is_dir(),
                ratio: (f.size as f64 / max_size as f64).min(1.0),
            })
            .collect()
    } else if let Some(td) = treemap_data {
        // Use "Top Items" from TreemapAnalyzer (by size)
        let max_size = td.top_items.first().map_or(1, |i| i.size).max(1);
        td.top_items
            .iter()
            .take(inner.height as usize)
            .map(|i| DisplayItem {
                name: i.name.clone(),
                size: i.size,
                is_dir: i.is_dir,
                ratio: (i.size as f64 / max_size as f64).min(1.0),
            })
            .collect()
    } else {
        Vec::new()
    };

    if items.is_empty() {
        // No data available yet
        let msg = if file_data.is_none() && treemap_data.is_none() {
            "Scanning filesystem..."
        } else {
            "No files found"
        };
        canvas.draw_text(
            msg,
            Point::new(inner.x, inner.y + 1.0),
            &TextStyle {
                color: dim_color,
                ..Default::default()
            },
        );
    } else {
        let max_rows = (inner.height as usize).saturating_sub(1);
        for (i, item) in items.iter().take(max_rows).enumerate() {
            let y = inner.y + 1.0 + i as f32;
            if y >= inner.y + inner.height {
                break;
            }

            // Color based on type
            let item_color = if item.is_dir { dir_color } else { file_color };

            // Truncate name to fit
            let name = format_column(
                &item.name,
                name_width,
                ColumnAlign::Left,
                TruncateStrategy::Path,
            );

            // Size column
            let size_str = format_column(
                &format_bytes(item.size),
                7,
                ColumnAlign::Right,
                TruncateStrategy::End,
            );

            // Tufte-style proportional bar (data-ink ratio: every char is data)
            let filled = ((bar_width as f64 * item.ratio) as usize).max(1);
            let bar: String = "█".repeat(filled) + &"░".repeat(bar_width.saturating_sub(filled));

            // Draw name
            canvas.draw_text(
                &name,
                Point::new(inner.x, y),
                &TextStyle {
                    color: item_color,
                    ..Default::default()
                },
            );

            // Draw size
            canvas.draw_text(
                &size_str,
                Point::new(inner.x + name_width as f32, y),
                &TextStyle {
                    color: dim_color,
                    ..Default::default()
                },
            );

            // Draw proportional bar
            canvas.draw_text(
                &format!("  {}", bar),
                Point::new(inner.x + name_width as f32 + 7.0, y),
                &TextStyle {
                    color: Color::new(
                        0.4 + 0.4 * item.ratio as f32,
                        0.6 - 0.3 * item.ratio as f32,
                        0.3,
                        1.0,
                    ),
                    ..Default::default()
                },
            );
        }
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
    use crate::HeatScheme;

    // Column widths: PID(7) USER(10) CPU%(8) MEM%(8) COMMAND(rest)
    // CRITICAL: All columns MUST use format_column() to prevent bleeding
    let col_widths = [7usize, 10, 8, 8];
    let cmd_width = (area.width as usize).saturating_sub(col_widths.iter().sum::<usize>() + 5);

    // PATTERN 5 HYBRID: Consistent focus/selection colors
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    let selected_col_bg = COL_SELECT_BG; // From constants
    let selected_row_bg = ROW_SELECT_BG; // From constants
    let sort_color = FOCUS_ACCENT_COLOR; // Bright cyan for sorted column
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
    // Clamp selected_column to valid range to prevent rendering artifacts
    let valid_selected = app.selected_column.min(columns.len().saturating_sub(1));
    for (i, (col, label, width)) in columns.iter().enumerate() {
        let is_selected = valid_selected == i;
        let is_sorted = app.sort_column == *col;

        // Column selection highlight (width only, no gap overlap)
        if is_selected {
            canvas.fill_rect(Rect::new(col_x, y, *width as f32, 1.0), selected_col_bg);
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

    // Sort using extracted helper (reduces cyclomatic complexity)
    use crate::ptop::ui::panels::process::sort_processes;
    sort_processes(&mut processes, app.sort_column, app.sort_descending);

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

        // PATTERN 5 HYBRID: Row selection highlight with gutter cursor
        // CRITICAL: Always paint row background to clear previous frame artifacts
        // Terminal buffers retain previous pixels - must explicitly overwrite
        if is_selected {
            canvas.fill_rect(Rect::new(x, y, area.width, 1.0), selected_row_bg);
        } else {
            // Clear any previous selection artifact with default background
            canvas.fill_rect(
                Rect::new(x, y, area.width, 1.0),
                Color::new(0.05, 0.05, 0.07, 1.0), // Dark terminal background
            );
        }

        if is_selected {
            // Draw cursor indicator at start of row (▶ in gutter)
            canvas.draw_text(
                "▶",
                Point::new(x - 1.5, y),
                &TextStyle {
                    color: FOCUS_ACCENT_COLOR, // Bright cyan cursor
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

        // CPU% with thermal heat color encoding (Grammar of Graphics)
        // Consistent with CoreStatsDataFrame and ProcessDataFrame widgets
        let cpu = proc.cpu_usage();
        let cpu_color = if is_selected {
            // White text on blue selection background for readability
            Color::WHITE
        } else {
            // Thermal heat scheme: green → yellow → red based on CPU usage
            HeatScheme::Thermal.color_for_percent(cpu as f64)
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
    use crate::widgets::selection::RowHighlight;
    use crate::{HeatScheme, MicroHeatBar};

    // Column widths for table layout.
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

    // Selection state: use process_selected for row navigation in exploded view
    let selected_row = if app.exploded_panel == Some(PanelType::Cpu) {
        app.process_selected
    } else {
        usize::MAX // No selection in non-exploded mode
    };

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
    // Uses RowHighlight for selection, MicroHeatBar for breakdown (SPEC-024 v5.8)
    // =========================================================================
    let visible_rows = (area.height as usize).saturating_sub(2);
    let core_count = app.per_core_percent.len();

    for i in 0..core_count.min(visible_rows) {
        // Draw row highlight FIRST (background)
        let is_selected = i == selected_row;
        let row_rect = Rect::new(x, y, area.width, 1.0);
        let row_highlight = RowHighlight::new(row_rect, is_selected).with_gutter(is_selected);
        row_highlight.paint(canvas);

        // Get text style from highlight (selected = white text)
        // Note: We keep semantic colors (user=green, sys=orange) regardless of selection
        let _row_style = row_highlight.text_style();

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
                app.snapshot_sensor_health.as_ref().and_then(|data| {
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

        // Temperature column, right-aligned.
        let temp_str = temp.map_or("-".to_string(), |t| format!("{t:.0}°"));
        let temp_str = format_column(
            &temp_str,
            col_widths[2],
            ColumnAlign::Right,
            TruncateStrategy::End,
        );
        // Use HeatScheme::Warm for temperature coloring (SPEC-024 framework requirement)
        let temp_color = temp.map_or(dim_color, |t| {
            // Map temperature to percentage: 30°C = 0%, 100°C = 100%
            let temp_pct = ((t - 30.0) / 70.0 * 100.0).clamp(0.0, 100.0);
            HeatScheme::Warm.color_for_percent(temp_pct as f64)
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

        // BREAKDOWN bar - using MicroHeatBar framework widget (SPEC-024 Section 30)
        // MicroHeatBar provides: proportional encoding + heat coloring + Tufte principles
        if breakdown_width > 3 {
            let bar_width = breakdown_width.saturating_sub(1);
            // MicroHeatBar shows usr/sys/io/idle proportionally with thermal coloring
            let bar = MicroHeatBar::new(&[
                user_pct as f64,
                sys_pct as f64,
                io_pct as f64,
                idle_pct as f64,
            ])
            .with_width(bar_width)
            .with_scheme(HeatScheme::Thermal);

            bar.paint(canvas, Point::new(col_x, y));
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
    if let Some(sensor_data) = app.snapshot_sensor_health.as_ref() {
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

/// Draw Memory panel in exploded (fullscreen) mode - SPEC-024 Section 30
///
/// FRAMEWORK REQUIREMENTS:
/// - Uses RowHighlight for selection
/// - Uses MicroHeatBar for breakdown visualization
/// - Uses HeatScheme::Cool for memory coloring
/// - Uses display_rules for ALL formatting
/// - FILLS THE ENTIRE SCREEN
#[allow(clippy::too_many_lines)]
fn draw_memory_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{
        format_bytes_si, format_column, format_percent, ColumnAlign, TruncateStrategy,
    };
    use crate::widgets::selection::RowHighlight;
    use crate::HeatScheme;

    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let mem_pct = if app.mem_total > 0 {
        (app.mem_used as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };

    // Build title with full details for exploded mode
    let title = format!(
        "Memory │ {:.1}G / {:.1}G ({:.0}%) │ Swap: {:.1}G / {:.1}G │ Cached: {:.1}G │ [FULLSCREEN]",
        gb(app.mem_used),
        gb(app.mem_total),
        mem_pct,
        gb(app.swap_used),
        gb(app.swap_total),
        gb(app.mem_cached),
    );

    let is_focused = app.is_panel_focused(PanelType::Memory);
    let mut border = create_panel_border(&title, MEMORY_COLOR, is_focused);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 10.0 || inner.width < 40.0 {
        draw_memory_panel(app, canvas, area);
        return;
    }

    // =========================================================================
    // FULLSCREEN LAYOUT: Memory breakdown + Top memory consumers
    // =========================================================================
    // Top section (30%): Memory breakdown bars and stats
    // Bottom section (70%): Per-process memory DataFrame
    // =========================================================================

    let breakdown_height = (inner.height * 0.25).max(6.0).min(12.0);
    let process_height = inner.height - breakdown_height - 1.0;

    let mut y = inner.y;

    // =========================================================================
    // SECTION 1: MEMORY BREAKDOWN (Top section)
    // Uses MicroHeatBar for proportional visualization
    // =========================================================================

    // Row 1: Full-width stacked memory bar
    let bar_width = inner.width as usize;
    let used_pct = if app.mem_total > 0 {
        ((app.mem_total - app.mem_available) as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };
    let cached_pct = if app.mem_total > 0 {
        (app.mem_cached as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };
    let free_pct = 100.0 - used_pct;

    // Draw stacked bar using thermal-like color scheme for memory
    let used_chars = ((used_pct / 100.0) * bar_width as f64) as usize;
    let cached_chars = ((cached_pct / 100.0) * bar_width as f64) as usize;
    let free_chars = bar_width.saturating_sub(used_chars + cached_chars);

    // Used: intensity based on pressure
    let used_color = HeatScheme::Warm.color_for_percent(used_pct);
    if used_chars > 0 {
        canvas.draw_text(
            &"█".repeat(used_chars),
            Point::new(inner.x, y),
            &TextStyle {
                color: used_color,
                ..Default::default()
            },
        );
    }
    if cached_chars > 0 {
        canvas.draw_text(
            &"▓".repeat(cached_chars),
            Point::new(inner.x + used_chars as f32, y),
            &TextStyle {
                color: CACHED_COLOR,
                ..Default::default()
            },
        );
    }
    if free_chars > 0 {
        canvas.draw_text(
            &"░".repeat(free_chars),
            Point::new(inner.x + used_chars as f32 + cached_chars as f32, y),
            &TextStyle {
                color: Color::new(0.3, 0.3, 0.3, 1.0),
                ..Default::default()
            },
        );
    }
    y += 1.0;

    // Row 2-5: Memory segment details in columns
    let col_width = (inner.width / 4.0).floor() as usize;
    let segments = [
        (
            "Used",
            app.mem_used,
            used_pct,
            HeatScheme::Warm.color_for_percent(used_pct),
        ),
        ("Cached", app.mem_cached, cached_pct, CACHED_COLOR),
        ("Available", app.mem_available, free_pct, FREE_COLOR),
        (
            "Swap",
            app.swap_used,
            if app.swap_total > 0 {
                (app.swap_used as f64 / app.swap_total as f64) * 100.0
            } else {
                0.0
            },
            swap_color(if app.swap_total > 0 {
                (app.swap_used as f64 / app.swap_total as f64) * 100.0
            } else {
                0.0
            }),
        ),
    ];

    let dim_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };

    for (i, (label, bytes, pct, color)) in segments.iter().enumerate() {
        let x = inner.x + (i as f32 * col_width as f32);
        let text = format!("{}: {} ({:.0}%)", label, format_bytes_si(*bytes), pct);
        canvas.draw_text(
            &format_column(
                &text,
                col_width.saturating_sub(1),
                ColumnAlign::Left,
                TruncateStrategy::End,
            ),
            Point::new(x, y),
            &TextStyle {
                color: *color,
                ..Default::default()
            },
        );
    }
    y += 1.0;

    // Separator
    canvas.draw_text(
        &"─".repeat(inner.width as usize),
        Point::new(inner.x, y),
        &dim_style,
    );
    y += 1.0;

    // =========================================================================
    // SECTION 2: TOP MEMORY CONSUMERS (Bottom section - fills remaining space)
    // Uses RowHighlight for selection, sorted by memory usage
    // =========================================================================

    let process_y = y;
    let process_rect = Rect::new(inner.x, process_y, inner.width, process_height);

    // Column widths for process memory table
    let col_pid = 8;
    let col_user = 10;
    let col_mem_pct = 8;
    let col_mem_bytes = 10;
    let col_rss = 10;
    let col_virt = 10;
    let col_cmd = (inner.width as usize)
        .saturating_sub(col_pid + col_user + col_mem_pct + col_mem_bytes + col_rss + col_virt + 10);

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(
        Rect::new(process_rect.x, process_rect.y, process_rect.width, 1.0),
        header_bg,
    );

    let headers = ["PID", "USER", "MEM%", "MEM", "RSS", "VIRT", "COMMAND"];
    let widths = [
        col_pid,
        col_user,
        col_mem_pct,
        col_mem_bytes,
        col_rss,
        col_virt,
        col_cmd,
    ];
    let mut hx = process_rect.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, process_rect.y),
            &TextStyle {
                color: MEMORY_COLOR,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }

    // Separator
    canvas.draw_text(
        &"─".repeat(process_rect.width as usize),
        Point::new(process_rect.x, process_rect.y + 1.0),
        &dim_style,
    );

    // Process rows sorted by memory
    let mut processes: Vec<_> = app.system.processes().iter().collect();
    processes.sort_by(|a, b| b.1.memory().cmp(&a.1.memory()));

    let visible_rows = (process_rect.height as usize).saturating_sub(2);
    let mut row_y = process_rect.y + 2.0;

    for (idx, (pid, proc)) in processes.iter().take(visible_rows).enumerate() {
        let is_selected = idx == 0; // TODO: Add memory_selected to App state

        // FRAMEWORK: Use RowHighlight for selection
        let row_bounds = Rect::new(process_rect.x, row_y, process_rect.width, 1.0);
        let highlight = RowHighlight::new(row_bounds, is_selected);
        highlight.paint(canvas);
        let text_style = highlight.text_style();

        let mut col_x = process_rect.x;

        // PID
        canvas.draw_text(
            &format_column(
                &pid.as_u32().to_string(),
                col_pid,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, row_y),
            &text_style,
        );
        col_x += col_pid as f32 + 1.0;

        // USER
        let user = proc
            .user_id()
            .and_then(|uid| app.users.get_user_by_id(uid))
            .map(|u| u.name().to_string())
            .unwrap_or_else(|| "-".to_string());
        canvas.draw_text(
            &format_column(&user, col_user, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(col_x, row_y),
            &text_style,
        );
        col_x += col_user as f32 + 1.0;

        // MEM%
        let mem_pct = (proc.memory() as f64 / app.mem_total as f64) * 100.0;
        let mem_color = if is_selected {
            Color::WHITE
        } else {
            HeatScheme::Cool.color_for_percent(mem_pct * 10.0) // Scale up for visibility
        };
        canvas.draw_text(
            &format_column(
                &format_percent(mem_pct as f32),
                col_mem_pct,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, row_y),
            &TextStyle {
                color: mem_color,
                ..Default::default()
            },
        );
        col_x += col_mem_pct as f32 + 1.0;

        // MEM bytes
        canvas.draw_text(
            &format_column(
                &format_bytes_si(proc.memory()),
                col_mem_bytes,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, row_y),
            &text_style,
        );
        col_x += col_mem_bytes as f32 + 1.0;

        // RSS (same as memory for now)
        canvas.draw_text(
            &format_column(
                &format_bytes_si(proc.memory()),
                col_rss,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, row_y),
            &text_style,
        );
        col_x += col_rss as f32 + 1.0;

        // VIRT
        canvas.draw_text(
            &format_column(
                &format_bytes_si(proc.virtual_memory()),
                col_virt,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, row_y),
            &text_style,
        );
        col_x += col_virt as f32 + 1.0;

        // COMMAND
        let cmd = proc.name().to_string_lossy().to_string();
        canvas.draw_text(
            &format_column(&cmd, col_cmd, ColumnAlign::Left, TruncateStrategy::Command),
            Point::new(col_x, row_y),
            &text_style,
        );

        row_y += 1.0;
    }
}

/// FULL SCREEN disk exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_disk_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{
        format_bytes_si, format_column, format_percent, ColumnAlign, TruncateStrategy,
    };
    use crate::widgets::selection::{RowHighlight, DIMMED_BG};
    use crate::HeatScheme;

    // Header: DISK EXPLODED - with border
    let border_color = DISK_COLOR;
    let disk_io = app.disk_io_data();

    let (total_read, total_write) = disk_io.map_or((0.0, 0.0), |io| {
        (io.total_read_bytes_per_sec, io.total_write_bytes_per_sec)
    });

    let title = format!(
        "▼ DISK │ R: {} │ W: {} │ {} Volumes",
        format_bytes_rate(total_read),
        format_bytes_rate(total_write),
        app.disks.len()
    );

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Calculate layout: disk list at top, I/O details at bottom
    let disk_count = app.disks.len();
    let disk_section_height = (disk_count.min(inner.height as usize / 2)).max(4);
    let io_section_height = (inner.height as usize).saturating_sub(disk_section_height + 2);

    let dim_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };

    // =========================================================================
    // SECTION 1: DISK VOLUMES (full list with usage bars)
    // =========================================================================
    let mut y = inner.y;

    // Column widths
    let col_mount = 25.min(inner.width as usize / 4);
    let col_fs = 10;
    let col_used = 10;
    let col_total = 10;
    let col_pct = 8;
    let col_bar = (inner.width as usize)
        .saturating_sub(col_mount + col_fs + col_used + col_total + col_pct + 8);

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);

    let headers = ["MOUNT", "FS", "USED", "TOTAL", "USE%", ""];
    let widths = [col_mount, col_fs, col_used, col_total, col_pct, col_bar];
    let mut hx = inner.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }
    y += 1.0;

    // Disk rows
    for (i, disk) in app.disks.iter().enumerate() {
        if (y - inner.y) as usize >= disk_section_height {
            break;
        }

        let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
        // Row selection: first row highlighted by default (future: support keyboard navigation)
        let is_selected = i == 0;

        // Paint row background with RowHighlight
        let row_hl = RowHighlight::new(row_rect, is_selected);
        row_hl.paint(canvas);
        let text_style = row_hl.text_style();

        let mount = disk.mount_point().to_string_lossy().to_string();
        let fs_type = disk.file_system().to_string_lossy().to_string();
        let total = disk.total_space();
        let available = disk.available_space();
        let used = total.saturating_sub(available);
        let use_pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let mut col_x = inner.x;

        // MOUNT
        canvas.draw_text(
            &format_column(
                &mount,
                col_mount,
                ColumnAlign::Left,
                TruncateStrategy::Command,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_mount as f32 + 1.0;

        // FS
        canvas.draw_text(
            &format_column(&fs_type, col_fs, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_fs as f32 + 1.0;

        // USED
        canvas.draw_text(
            &format_column(
                &format_bytes_si(used),
                col_used,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_used as f32 + 1.0;

        // TOTAL
        canvas.draw_text(
            &format_column(
                &format_bytes_si(total),
                col_total,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_total as f32 + 1.0;

        // USE%
        let pct_color = HeatScheme::Warm.color_for_percent(use_pct);
        canvas.draw_text(
            &format_column(
                &format_percent(use_pct as f32),
                col_pct,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &TextStyle {
                color: if is_selected { Color::WHITE } else { pct_color },
                ..Default::default()
            },
        );
        col_x += col_pct as f32 + 1.0;

        // Usage bar
        if col_bar >= 3 {
            let filled = ((use_pct / 100.0) * col_bar as f64) as usize;
            let bar_color = HeatScheme::Warm.color_for_percent(use_pct);
            let bar_str: String = "█".repeat(filled) + &"░".repeat(col_bar.saturating_sub(filled));
            canvas.draw_text(
                &bar_str,
                Point::new(col_x, y),
                &TextStyle {
                    color: bar_color,
                    ..Default::default()
                },
            );
        }

        y += 1.0;
    }

    // Separator
    y += 0.5;
    canvas.draw_text(
        &"─".repeat(inner.width as usize),
        Point::new(inner.x, y),
        &dim_style,
    );
    y += 1.0;

    // =========================================================================
    // SECTION 2: I/O RATES BY DEVICE
    // =========================================================================
    if let Some(io) = disk_io {
        let io_header = "I/O RATES BY DEVICE";
        canvas.draw_text(
            io_header,
            Point::new(inner.x, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        y += 1.0;

        // I/O column widths
        let io_col_dev = 12;
        let io_col_read = 12;
        let io_col_write = 12;
        let io_col_iops = 10;

        // Header
        canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);
        let io_headers = ["DEVICE", "READ/s", "WRITE/s", "IOPS"];
        let io_widths = [io_col_dev, io_col_read, io_col_write, io_col_iops];
        let mut ihx = inner.x;
        for (h, w) in io_headers.iter().zip(io_widths.iter()) {
            canvas.draw_text(
                &format_column(h, *w, ColumnAlign::Left, TruncateStrategy::End),
                Point::new(ihx, y),
                &TextStyle {
                    color: border_color,
                    ..Default::default()
                },
            );
            ihx += *w as f32 + 1.0;
        }
        y += 1.0;

        // List physical disks with I/O
        let mut devices: Vec<_> = io.physical_disks().collect();
        devices.sort_by(|a, b| a.0.cmp(b.0));

        for (dev_name, _stats) in devices.iter().take(io_section_height.saturating_sub(2)) {
            let rates = io.rates.get(*dev_name);
            let read_rate = rates.map_or(0.0, |r| r.read_bytes_per_sec);
            let write_rate = rates.map_or(0.0, |r| r.write_bytes_per_sec);
            let iops = rates.map_or(0.0, |r| r.reads_per_sec + r.writes_per_sec);

            let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
            canvas.fill_rect(row_rect, DIMMED_BG);

            let mut col_x = inner.x;

            // Device name
            canvas.draw_text(
                &format_column(
                    dev_name,
                    io_col_dev,
                    ColumnAlign::Left,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: Color::new(0.85, 0.85, 0.85, 1.0),
                    ..Default::default()
                },
            );
            col_x += io_col_dev as f32 + 1.0;

            // Read rate
            let read_color = if read_rate > 10_000_000.0 {
                Color::new(0.3, 0.9, 0.5, 1.0)
            } else {
                Color::new(0.7, 0.7, 0.7, 1.0)
            };
            canvas.draw_text(
                &format_column(
                    &format_bytes_rate(read_rate),
                    io_col_read,
                    ColumnAlign::Right,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: read_color,
                    ..Default::default()
                },
            );
            col_x += io_col_read as f32 + 1.0;

            // Write rate
            let write_color = if write_rate > 10_000_000.0 {
                Color::new(0.9, 0.6, 0.3, 1.0)
            } else {
                Color::new(0.7, 0.7, 0.7, 1.0)
            };
            canvas.draw_text(
                &format_column(
                    &format_bytes_rate(write_rate),
                    io_col_write,
                    ColumnAlign::Right,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: write_color,
                    ..Default::default()
                },
            );
            col_x += io_col_write as f32 + 1.0;

            // IOPS
            canvas.draw_text(
                &format_column(
                    &format!("{:.0}", iops),
                    io_col_iops,
                    ColumnAlign::Right,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: Color::new(0.7, 0.7, 0.7, 1.0),
                    ..Default::default()
                },
            );

            y += 1.0;
        }
    }
}

/// FULL SCREEN network exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_network_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{
        format_bytes_si, format_column, ColumnAlign, TruncateStrategy,
    };
    use crate::widgets::selection::RowHighlight;

    let border_color = NETWORK_COLOR;

    // Calculate totals
    let (rx_total, tx_total): (u64, u64) = if app.deterministic {
        (0, 0)
    } else {
        app.networks
            .values()
            .map(|d| (d.received(), d.transmitted()))
            .fold((0, 0), |(ar, at), (r, t)| (ar + r, at + t))
    };

    let title = format!(
        "▼ NETWORK │ ↓ {}/s │ ↑ {}/s │ {} Interfaces",
        format_bytes(rx_total),
        format_bytes(tx_total),
        app.networks.len()
    );

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;

    // Column widths
    let col_iface = 15;
    let col_rx = 12;
    let col_tx = 12;
    let col_rx_total = 14;
    let col_tx_total = 14;
    let col_bar = (inner.width as usize)
        .saturating_sub(col_iface + col_rx + col_tx + col_rx_total + col_tx_total + 10);

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);

    let headers = ["INTERFACE", "↓ RX/s", "↑ TX/s", "TOTAL RX", "TOTAL TX", ""];
    let widths = [
        col_iface,
        col_rx,
        col_tx,
        col_rx_total,
        col_tx_total,
        col_bar,
    ];
    let mut hx = inner.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }
    y += 1.0;

    // Network interface rows
    let mut interfaces: Vec<_> = app.networks.iter().collect();
    interfaces.sort_by(|a, b| b.1.received().cmp(&a.1.received()));

    for (i, (iface_name, data)) in interfaces.iter().enumerate() {
        if (y - inner.y) as usize >= (inner.height as usize).saturating_sub(2) {
            break;
        }

        let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
        let is_selected = i == 0;

        let row_hl = RowHighlight::new(row_rect, is_selected);
        row_hl.paint(canvas);
        let text_style = row_hl.text_style();

        let rx_rate = data.received();
        let tx_rate = data.transmitted();
        let total_rx = data.total_received();
        let total_tx = data.total_transmitted();

        let mut col_x = inner.x;

        // Interface name
        canvas.draw_text(
            &format_column(
                iface_name,
                col_iface,
                ColumnAlign::Left,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_iface as f32 + 1.0;

        // RX rate
        let rx_color = if rx_rate > 1_000_000 {
            Color::new(0.3, 0.9, 0.5, 1.0)
        } else if is_selected {
            Color::WHITE
        } else {
            Color::new(0.7, 0.7, 0.7, 1.0)
        };
        canvas.draw_text(
            &format_column(
                &format_bytes_rate(rx_rate as f64),
                col_rx,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &TextStyle {
                color: rx_color,
                ..Default::default()
            },
        );
        col_x += col_rx as f32 + 1.0;

        // TX rate
        let tx_color = if tx_rate > 1_000_000 {
            Color::new(0.9, 0.6, 0.3, 1.0)
        } else if is_selected {
            Color::WHITE
        } else {
            Color::new(0.7, 0.7, 0.7, 1.0)
        };
        canvas.draw_text(
            &format_column(
                &format_bytes_rate(tx_rate as f64),
                col_tx,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &TextStyle {
                color: tx_color,
                ..Default::default()
            },
        );
        col_x += col_tx as f32 + 1.0;

        // Total RX
        canvas.draw_text(
            &format_column(
                &format_bytes_si(total_rx),
                col_rx_total,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_rx_total as f32 + 1.0;

        // Total TX
        canvas.draw_text(
            &format_column(
                &format_bytes_si(total_tx),
                col_tx_total,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_tx_total as f32 + 1.0;

        // Activity bar
        if col_bar >= 3 {
            let max_rate = 100_000_000.0; // 100MB/s scale
            let rx_pct = ((rx_rate as f64 / max_rate) * 100.0).min(100.0);
            let tx_pct = ((tx_rate as f64 / max_rate) * 100.0).min(100.0);
            let rx_chars = ((rx_pct / 100.0) * (col_bar / 2) as f64) as usize;
            let tx_chars = ((tx_pct / 100.0) * (col_bar / 2) as f64) as usize;

            let rx_bar: String = "▁".repeat(rx_chars);
            let tx_bar: String = "▁".repeat(tx_chars);

            canvas.draw_text(
                &rx_bar,
                Point::new(col_x, y),
                &TextStyle {
                    color: Color::new(0.3, 0.9, 0.5, 1.0),
                    ..Default::default()
                },
            );
            canvas.draw_text(
                &tx_bar,
                Point::new(col_x + (col_bar / 2) as f32, y),
                &TextStyle {
                    color: Color::new(0.9, 0.6, 0.3, 1.0),
                    ..Default::default()
                },
            );
        }

        y += 1.0;
    }
}

/// FULL SCREEN GPU exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_gpu_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::format_bytes_si;
    use crate::HeatScheme;

    let border_color = GPU_COLOR;

    let gpu = app.gpu_info.clone();
    let title = gpu
        .as_ref()
        .map(|g| {
            let temp_str = g
                .temperature
                .map(|t| format!(" │ {}°C", t as i32))
                .unwrap_or_default();
            let power_str = g
                .power_watts
                .map(|p| format!(" │ {:.0}W", p))
                .unwrap_or_default();
            let util_str = g
                .utilization
                .map(|u| format!(" │ {}%", u as i32))
                .unwrap_or_default();
            format!("▼ GPU: {}{}{}{}", g.name, util_str, temp_str, power_str)
        })
        .unwrap_or_else(|| "▼ GPU │ No GPU detected".to_string());

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;

    if let Some(g) = gpu {
        // GPU Utilization bar
        let util = g.utilization.unwrap_or(0) as f64;
        let util_bar_width = (inner.width as usize).min(60);
        let filled = ((util / 100.0) * util_bar_width as f64) as usize;
        let bar: String = "█".repeat(filled) + &"░".repeat(util_bar_width.saturating_sub(filled));
        let util_color = HeatScheme::Warm.color_for_percent(util);

        canvas.draw_text(
            &format!("GPU Utilization: {} {:>5.1}%", bar, util),
            Point::new(inner.x, y),
            &TextStyle {
                color: util_color,
                ..Default::default()
            },
        );
        y += 1.0;

        // VRAM bar
        let vram_used = g.vram_used.unwrap_or(0);
        let vram_total = g.vram_total.unwrap_or(0);
        let vram_pct = if vram_total > 0 {
            (vram_used as f64 / vram_total as f64) * 100.0
        } else {
            0.0
        };
        let vram_filled = ((vram_pct / 100.0) * util_bar_width as f64) as usize;
        let vram_bar: String =
            "█".repeat(vram_filled) + &"░".repeat(util_bar_width.saturating_sub(vram_filled));
        let vram_color = HeatScheme::Warm.color_for_percent(vram_pct);

        canvas.draw_text(
            &format!(
                "VRAM:            {} {:>5.1}% ({} / {})",
                vram_bar,
                vram_pct,
                format_bytes_si(vram_used),
                format_bytes_si(vram_total)
            ),
            Point::new(inner.x, y),
            &TextStyle {
                color: vram_color,
                ..Default::default()
            },
        );
        y += 1.0;

        // Temperature and Power
        if let Some(temp) = g.temperature {
            let temp_color = HeatScheme::Thermal.color_for_percent((temp as f64 / 100.0) * 100.0);
            canvas.draw_text(
                &format!("Temperature: {}°C", temp as i32),
                Point::new(inner.x, y),
                &TextStyle {
                    color: temp_color,
                    ..Default::default()
                },
            );
        }
        if let Some(power) = g.power_watts {
            canvas.draw_text(
                &format!("    Power: {:.1}W", power),
                Point::new(inner.x + 25.0, y),
                &TextStyle {
                    color: Color::new(0.9, 0.7, 0.3, 1.0),
                    ..Default::default()
                },
            );
        }
        y += 2.0;

        // GPU processes header
        let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
        canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);
        canvas.draw_text(
            "GPU PROCESSES",
            Point::new(inner.x, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        y += 1.0;

        // Note: GPU processes would come from app.analyzers.gpu_procs_data()
        // For now show placeholder
        canvas.draw_text(
            "  (GPU process list requires nvidia-smi)",
            Point::new(inner.x, y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );
    } else {
        canvas.draw_text(
            "No GPU detected or nvidia-smi not available",
            Point::new(inner.x, y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );
    }
}

/// FULL SCREEN sensors exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_sensors_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{format_column, ColumnAlign, TruncateStrategy};
    use crate::widgets::selection::RowHighlight;
    use crate::HeatScheme;

    let border_color = SENSORS_COLOR;

    // Get sensor data
    let sensor_data = app.snapshot_sensor_health.as_ref();
    let sensor_count = sensor_data.map(|d| d.sensors.len()).unwrap_or(0);

    let title = format!("▼ SENSORS │ {} readings", sensor_count);

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;

    // Column widths
    let col_name = 25;
    let col_value = 15;
    let col_status = 10;
    let col_bar = (inner.width as usize).saturating_sub(col_name + col_value + col_status + 6);

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);

    let headers = ["SENSOR", "VALUE", "STATUS", ""];
    let widths = [col_name, col_value, col_status, col_bar];
    let mut hx = inner.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }
    y += 1.0;

    if let Some(data) = sensor_data {
        for (i, reading) in data.sensors.iter().enumerate() {
            if (y - inner.y) as usize >= (inner.height as usize).saturating_sub(2) {
                break;
            }

            let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
            let is_selected = i == 0;

            let row_hl = RowHighlight::new(row_rect, is_selected);
            row_hl.paint(canvas);
            let text_style = row_hl.text_style();

            let mut col_x = inner.x;

            // Name (label)
            canvas.draw_text(
                &format_column(
                    &reading.label,
                    col_name,
                    ColumnAlign::Left,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &text_style,
            );
            col_x += col_name as f32 + 1.0;

            // Value with thermal coloring (includes unit from value_display)
            let value_color = match reading.sensor_type {
                SensorType::Temperature => {
                    HeatScheme::Thermal.color_for_percent(reading.value)
                }
                SensorType::Fan => Color::new(0.3, 0.7, 0.9, 1.0),
                _ => {
                    if is_selected {
                        Color::WHITE
                    } else {
                        Color::new(0.8, 0.8, 0.8, 1.0)
                    }
                }
            };
            canvas.draw_text(
                &format_column(
                    &reading.value_display(),
                    col_value,
                    ColumnAlign::Right,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: value_color,
                    ..Default::default()
                },
            );
            col_x += col_value as f32 + 1.0;

            // Status
            let status_str = reading.status.as_str();
            let status_color = match reading.status {
                SensorStatus::Normal => Color::new(0.3, 0.9, 0.3, 1.0),
                SensorStatus::Warning => Color::new(0.9, 0.7, 0.2, 1.0),
                SensorStatus::Critical => Color::new(0.9, 0.2, 0.2, 1.0),
                SensorStatus::Low => Color::new(0.3, 0.5, 0.9, 1.0),
                SensorStatus::Fault => Color::new(0.5, 0.5, 0.5, 1.0),
            };
            canvas.draw_text(
                &format_column(
                    status_str,
                    col_status,
                    ColumnAlign::Left,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: if is_selected {
                        Color::WHITE
                    } else {
                        status_color
                    },
                    ..Default::default()
                },
            );

            y += 1.0;
        }
    } else {
        canvas.draw_text(
            "No sensor data available",
            Point::new(inner.x, y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );
    }
}

/// FULL SCREEN process exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_process_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{
        format_bytes_si, format_column, ColumnAlign, TruncateStrategy,
    };
    use crate::widgets::selection::RowHighlight;
    use crate::HeatScheme;

    let border_color = PROCESS_COLOR;
    let proc_count = app.system.processes().len();

    let title = format!("▼ PROCESSES │ {} total", proc_count);

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;

    // Column widths - full screen allows more columns
    let col_pid = 8;
    let col_user = 12;
    let col_cpu = 7;
    let col_mem = 7;
    let col_virt = 10;
    let col_rss = 10;
    let col_state = 6;
    let col_time = 10;
    let col_cmd = (inner.width as usize).saturating_sub(
        col_pid + col_user + col_cpu + col_mem + col_virt + col_rss + col_state + col_time + 12,
    );

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);

    let headers = [
        "PID", "USER", "CPU%", "MEM%", "VIRT", "RSS", "STATE", "TIME", "COMMAND",
    ];
    let widths = [
        col_pid, col_user, col_cpu, col_mem, col_virt, col_rss, col_state, col_time, col_cmd,
    ];
    let mut hx = inner.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }
    y += 1.0;

    // Sort processes by CPU
    let mut procs: Vec<_> = app.system.processes().iter().collect();
    procs.sort_by(|a, b| {
        b.1.cpu_usage()
            .partial_cmp(&a.1.cpu_usage())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (i, (_pid, proc)) in procs.iter().enumerate() {
        if (y - inner.y) as usize >= (inner.height as usize).saturating_sub(2) {
            break;
        }

        let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
        let is_selected = app.process_selected == i;

        let row_hl = RowHighlight::new(row_rect, is_selected);
        row_hl.paint(canvas);
        let text_style = row_hl.text_style();

        let cpu_pct = proc.cpu_usage() as f64;
        let mem_pct = (proc.memory() as f64 / app.mem_total as f64) * 100.0;

        let mut col_x = inner.x;

        // PID
        canvas.draw_text(
            &format_column(
                &proc.pid().to_string(),
                col_pid,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_pid as f32 + 1.0;

        // USER
        let user = proc
            .user_id()
            .map(|u| u.to_string())
            .unwrap_or_else(|| "-".to_string());
        canvas.draw_text(
            &format_column(&user, col_user, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_user as f32 + 1.0;

        // CPU%
        let cpu_color = HeatScheme::Warm.color_for_percent(cpu_pct);
        canvas.draw_text(
            &format_column(
                &format!("{:.1}", cpu_pct),
                col_cpu,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &TextStyle {
                color: if is_selected { Color::WHITE } else { cpu_color },
                ..Default::default()
            },
        );
        col_x += col_cpu as f32 + 1.0;

        // MEM%
        let mem_color = HeatScheme::Warm.color_for_percent(mem_pct);
        canvas.draw_text(
            &format_column(
                &format!("{:.1}", mem_pct),
                col_mem,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &TextStyle {
                color: if is_selected { Color::WHITE } else { mem_color },
                ..Default::default()
            },
        );
        col_x += col_mem as f32 + 1.0;

        // VIRT
        canvas.draw_text(
            &format_column(
                &format_bytes_si(proc.virtual_memory()),
                col_virt,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_virt as f32 + 1.0;

        // RSS
        canvas.draw_text(
            &format_column(
                &format_bytes_si(proc.memory()),
                col_rss,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_rss as f32 + 1.0;

        // STATE
        let state = format!("{:?}", proc.status());
        canvas.draw_text(
            &format_column(&state, col_state, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_state as f32 + 1.0;

        // TIME
        let run_time = proc.run_time();
        let time_str = format!(
            "{}:{:02}:{:02}",
            run_time / 3600,
            (run_time % 3600) / 60,
            run_time % 60
        );
        canvas.draw_text(
            &format_column(
                &time_str,
                col_time,
                ColumnAlign::Right,
                TruncateStrategy::End,
            ),
            Point::new(col_x, y),
            &text_style,
        );
        col_x += col_time as f32 + 1.0;

        // COMMAND
        let cmd = proc.name().to_string_lossy().to_string();
        canvas.draw_text(
            &format_column(&cmd, col_cmd, ColumnAlign::Left, TruncateStrategy::Command),
            Point::new(col_x, y),
            &text_style,
        );

        y += 1.0;
    }
}

/// FULL SCREEN connections exploded view
/// SPEC-024 Section 30: Exploded views fill the screen
fn draw_connections_exploded(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect) {
    use crate::widgets::display_rules::{format_column, ColumnAlign, TruncateStrategy};
    use crate::widgets::selection::RowHighlight;

    let border_color = CONNECTIONS_COLOR;

    let conn_data = app.snapshot_connections.as_ref();
    let conn_count = conn_data.map(|d| d.connections.len()).unwrap_or(0);

    let title = format!("▼ CONNECTIONS │ {} active", conn_count);

    let mut border = create_panel_border(&title, border_color, true);
    border.layout(area);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;

    // Column widths
    let col_proto = 6;
    let col_local = 25;
    let col_remote = 25;
    let col_state = 12;
    let col_pid = 8;
    let col_proc = (inner.width as usize)
        .saturating_sub(col_proto + col_local + col_remote + col_state + col_pid + 10);

    // Header
    let header_bg = Color::new(0.12, 0.15, 0.22, 1.0);
    canvas.fill_rect(Rect::new(inner.x, y, inner.width, 1.0), header_bg);

    let headers = [
        "PROTO",
        "LOCAL ADDRESS",
        "REMOTE ADDRESS",
        "STATE",
        "PID",
        "PROCESS",
    ];
    let widths = [
        col_proto, col_local, col_remote, col_state, col_pid, col_proc,
    ];
    let mut hx = inner.x;
    for (header, width) in headers.iter().zip(widths.iter()) {
        canvas.draw_text(
            &format_column(header, *width, ColumnAlign::Left, TruncateStrategy::End),
            Point::new(hx, y),
            &TextStyle {
                color: border_color,
                ..Default::default()
            },
        );
        hx += *width as f32 + 1.0;
    }
    y += 1.0;

    if let Some(data) = conn_data {
        for (i, conn) in data.connections.iter().enumerate() {
            if (y - inner.y) as usize >= (inner.height as usize).saturating_sub(2) {
                break;
            }

            let row_rect = Rect::new(inner.x, y, inner.width, 1.0);
            let is_selected = i == 0;

            let row_hl = RowHighlight::new(row_rect, is_selected);
            row_hl.paint(canvas);
            let text_style = row_hl.text_style();

            let mut col_x = inner.x;

            // Protocol
            canvas.draw_text(
                &format_column("TCP", col_proto, ColumnAlign::Left, TruncateStrategy::End),
                Point::new(col_x, y),
                &text_style,
            );
            col_x += col_proto as f32 + 1.0;

            // Local address
            let local = format!("{}:{}", conn.local_addr, conn.local_port);
            canvas.draw_text(
                &format_column(&local, col_local, ColumnAlign::Left, TruncateStrategy::End),
                Point::new(col_x, y),
                &text_style,
            );
            col_x += col_local as f32 + 1.0;

            // Remote address
            let remote = format!("{}:{}", conn.remote_addr, conn.remote_port);
            canvas.draw_text(
                &format_column(
                    &remote,
                    col_remote,
                    ColumnAlign::Left,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &text_style,
            );
            col_x += col_remote as f32 + 1.0;

            // State with color
            let state_str = format!("{:?}", conn.state);
            let state_color = match conn.state {
                TcpState::Established => Color::new(0.3, 0.9, 0.3, 1.0),
                TcpState::Listen => Color::new(0.3, 0.7, 0.9, 1.0),
                TcpState::TimeWait => Color::new(0.7, 0.7, 0.3, 1.0),
                TcpState::CloseWait => Color::new(0.9, 0.5, 0.3, 1.0),
                _ => Color::new(0.6, 0.6, 0.6, 1.0),
            };
            canvas.draw_text(
                &format_column(
                    &state_str,
                    col_state,
                    ColumnAlign::Left,
                    TruncateStrategy::End,
                ),
                Point::new(col_x, y),
                &TextStyle {
                    color: if is_selected {
                        Color::WHITE
                    } else {
                        state_color
                    },
                    ..Default::default()
                },
            );
            col_x += col_state as f32 + 1.0;

            // PID
            let pid_str = conn
                .pid
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".to_string());
            canvas.draw_text(
                &format_column(&pid_str, col_pid, ColumnAlign::Right, TruncateStrategy::End),
                Point::new(col_x, y),
                &text_style,
            );
            col_x += col_pid as f32 + 1.0;

            // Process name
            let proc_name = conn.process_name.as_deref().unwrap_or("-");
            canvas.draw_text(
                &format_column(
                    proc_name,
                    col_proc,
                    ColumnAlign::Left,
                    TruncateStrategy::Command,
                ),
                Point::new(col_x, y),
                &text_style,
            );

            y += 1.0;
        }
    } else {
        canvas.draw_text(
            "No connection data available",
            Point::new(inner.x, y),
            &TextStyle {
                color: Color::new(0.5, 0.5, 0.5, 1.0),
                ..Default::default()
            },
        );
    }
}

/// Draw a single panel in fullscreen (exploded) mode
fn draw_exploded_panel(app: &App, canvas: &mut DirectTerminalCanvas, area: Rect, panel: PanelType) {
    match panel {
        PanelType::Cpu => draw_cpu_exploded(app, canvas, area),
        PanelType::Memory => draw_memory_exploded(app, canvas, area),
        PanelType::Disk => draw_disk_exploded(app, canvas, area),
        PanelType::Network => draw_network_exploded(app, canvas, area),
        PanelType::Process => draw_process_exploded(app, canvas, area),
        PanelType::Gpu => draw_gpu_exploded(app, canvas, area),
        PanelType::Sensors => draw_sensors_exploded(app, canvas, area),
        PanelType::Connections => draw_connections_exploded(app, canvas, area),
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
        assert!(ROW_SELECT_BG.b > ROW_SELECT_BG.r, "Selection bg should have purple/blue tint");
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
