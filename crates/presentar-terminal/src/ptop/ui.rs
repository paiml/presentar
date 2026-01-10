//! UI layout and rendering for ptop.
//!
//! Pixel-perfect ttop clone using presentar-terminal widgets.

use crate::direct::{CellBuffer, DirectTerminalCanvas};
use crate::{
    Border, BorderStyle, BrailleGraph, GraphMode, NetworkInterface, NetworkPanel, ProcessEntry,
    ProcessState, ProcessTable, Treemap, TreemapNode,
};
use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};

use super::app::App;

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
        format!("{:.0}B", bytes_per_sec)
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

    // Count visible top panels (like ttop)
    let top_panel_count = count_top_panels(app);
    let has_process = app.panels.process;

    // Layout: 45% top panels, 55% bottom row (like ttop)
    let top_height = if top_panel_count > 0 && has_process {
        (h * 0.45).max(8.0)
    } else if top_panel_count > 0 {
        h
    } else {
        0.0
    };
    let bottom_y = top_height;
    let bottom_height = h - bottom_y;

    // Draw top panels in grid layout
    if top_panel_count > 0 {
        draw_top_panels(app, &mut canvas, Rect::new(0.0, 0.0, w, top_height));
    }

    // Draw bottom row: 40% Processes | 30% Connections | 30% Treemap (like ttop)
    if has_process && bottom_height > 3.0 {
        let proc_w = w * 0.4;
        let conn_w = w * 0.3;
        let tree_w = w - proc_w - conn_w;

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

        if app.panels.treemap {
            draw_treemap_panel(
                app,
                &mut canvas,
                Rect::new(proc_w + conn_w, bottom_y, tree_w, bottom_height),
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

#[allow(clippy::type_complexity)]
fn draw_top_panels(app: &App, canvas: &mut DirectTerminalCanvas<'_>, area: Rect) {
    let mut panels: Vec<fn(&App, &mut DirectTerminalCanvas<'_>, Rect)> = Vec::new();

    // Core panels (P0)
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
    // Hardware panels (P1)
    if app.panels.gpu {
        panels.push(draw_gpu_panel);
    }
    if app.panels.sensors {
        panels.push(draw_sensors_panel);
    }
    if app.panels.psi {
        panels.push(draw_psi_panel);
    }
    if app.panels.connections {
        panels.push(draw_connections_panel);
    }
    // Optional panels (P2)
    if app.panels.battery {
        panels.push(draw_battery_panel);
    }
    if app.panels.sensors_compact {
        panels.push(draw_sensors_compact_panel);
    }
    if app.panels.system {
        panels.push(draw_system_panel);
    }
    // Advanced panels (P3)
    if app.panels.treemap {
        panels.push(draw_treemap_panel);
    }
    if app.panels.files {
        panels.push(draw_files_panel);
    }

    if panels.is_empty() {
        return;
    }

    // Grid layout: 2 columns, rows as needed (like ttop)
    let cols = panels.len().min(2);
    let rows = panels.len().div_ceil(cols);
    let cell_w = area.width / cols as f32;
    let cell_h = area.height / rows as f32;

    for (i, draw_fn) in panels.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = area.x + col as f32 * cell_w;
        let y = area.y + row as f32 * cell_h;
        draw_fn(app, canvas, Rect::new(x, y, cell_w, cell_h));
    }
}

#[allow(clippy::too_many_lines)]
fn draw_cpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    use sysinfo::{Cpu, System};

    let cpu_pct = app.cpu_history.last().copied().unwrap_or(0.0) * 100.0;
    let core_count = app.per_core_percent.len();
    let uptime = app.uptime();
    let load = System::load_average();

    // Get CPU frequency (max from all cores)
    let max_freq_mhz = app
        .system
        .cpus()
        .iter()
        .map(Cpu::frequency)
        .max()
        .unwrap_or(0);
    let is_boosting = max_freq_mhz > 3000; // Heuristic: >3GHz = boosting

    // ttop-style title: " CPU 45% │ 8 cores │ 3.5GHz⚡ │ up 2d 3h │ LAV 2.15 "
    let boost_icon = if is_boosting { "⚡" } else { "" };
    let title = format!(
        " CPU {cpu_pct:.0}% │ {core_count} cores │ {:.1}GHz{boost_icon} │ up {} │ LAV {:.2} ",
        max_freq_mhz as f64 / 1000.0,
        format_uptime(uptime),
        load.one
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(CPU_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 10.0 {
        return;
    }

    // Reserve 2 rows for load gauge + top consumers at bottom (like ttop)
    let reserved_bottom = 2.0_f32;
    let core_area_height = (inner.height - reserved_bottom).max(1.0);

    // ttop layout: per-core meters on LEFT, graph on RIGHT
    let meter_bar_width = 12.0_f32;
    let cores_per_col = core_area_height as usize;
    let num_meter_cols = if cores_per_col > 0 {
        core_count.div_ceil(cores_per_col)
    } else {
        1
    };
    let meters_width = (num_meter_cols as f32 * meter_bar_width).min(inner.width / 2.0);

    // Draw per-core meters on left side
    for (i, &percent) in app.per_core_percent.iter().enumerate() {
        if cores_per_col == 0 {
            break;
        }
        let col = i / cores_per_col;
        let row = i % cores_per_col;

        let cell_x = inner.x + col as f32 * meter_bar_width;
        let cell_y = inner.y + row as f32;

        if cell_x + meter_bar_width > inner.x + meters_width || cell_y >= inner.y + core_area_height
        {
            continue;
        }

        let color = percent_color(percent);
        let bar_len = 6;
        let filled = ((percent / 100.0) * bar_len as f64) as usize;
        let bar: String =
            "█".repeat(filled.min(bar_len)) + &"░".repeat(bar_len - filled.min(bar_len));

        let label = format!("{i:>2} {bar} {percent:>3.0}");
        canvas.draw_text(
            &label,
            Point::new(cell_x, cell_y),
            &TextStyle {
                color,
                ..Default::default()
            },
        );
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

        // Format: "Load ██████████ 2.15↑ 1.85↓ 1.50 │ Freq 3.5GHz ⚡"
        let load_str = format!(
            "Load {bar} {:.2}{trend_1_5} {:.2}{trend_5_15} {:.2}",
            load.one, load.five, load.fifteen
        );

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
    let consumers_y = inner.y + core_area_height + 1.0;
    if consumers_y < inner.y + inner.height && inner.width > 20.0 {
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

#[allow(clippy::too_many_lines)]
fn draw_memory_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let mem_pct = if app.mem_total > 0 {
        (app.mem_used as f64 / app.mem_total as f64) * 100.0
    } else {
        0.0
    };

    // Check for ZRAM
    let zram_stats = read_zram_stats();
    let zram_info = zram_stats
        .as_ref()
        .filter(|z| z.is_active())
        .map(|z| format!(" │ ZRAM:{:.1}x", z.ratio()))
        .unwrap_or_default();

    // ttop-style title: " Memory │ 16.5G / 32.0G (52%) │ ZRAM:2.5x "
    let title = format!(
        " Memory │ {:.1}G / {:.1}G ({:.0}%){} ",
        gb(app.mem_used),
        gb(app.mem_total),
        mem_pct,
        zram_info
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(MEMORY_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 || inner.width < 10.0 {
        return;
    }

    let mut y = inner.y;

    // Line 1: Stacked memory bar (ttop style: Used|Cached|Free)
    if app.mem_total > 0 {
        let bar_width = inner.width as usize;
        let used_actual_pct = if app.mem_total > 0 {
            ((app.mem_total - app.mem_available) as f64 / app.mem_total as f64) * 100.0
        } else {
            0.0
        };
        let cached_pct = (app.mem_cached as f64 / app.mem_total as f64) * 100.0;

        let used_chars = ((used_actual_pct / 100.0) * bar_width as f64) as usize;
        let cached_chars = ((cached_pct / 100.0) * bar_width as f64) as usize;
        let free_chars = bar_width.saturating_sub(used_chars + cached_chars);

        // Build stacked bar: Used (colored by percent) | Cached (cyan) | Free (dim)
        let used_color = percent_color(used_actual_pct);
        let cached_color = Color {
            r: 0.3,
            g: 0.8,
            b: 0.9,
            a: 1.0,
        }; // Cyan
        let free_color = Color {
            r: 0.3,
            g: 0.3,
            b: 0.3,
            a: 1.0,
        }; // Dark gray

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
                    color: cached_color,
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

        y += 1.0;
    }

    // Remaining lines: Memory breakdown rows (ttop style)
    if y < inner.y + inner.height && app.mem_total > 0 {
        let used_pct = (app.mem_used as f64 / app.mem_total as f64) * 100.0;
        let cached_pct = (app.mem_cached as f64 / app.mem_total as f64) * 100.0;
        let free_pct = (app.mem_available as f64 / app.mem_total as f64) * 100.0;
        let swap_pct = if app.swap_total > 0 {
            (app.swap_used as f64 / app.swap_total as f64) * 100.0
        } else {
            0.0
        };

        let mut rows: Vec<(&str, f64, f64, Color)> = vec![
            ("Used", gb(app.mem_used), used_pct, percent_color(used_pct)),
            (
                "Swap",
                gb(app.swap_used),
                swap_pct,
                if swap_pct > 50.0 {
                    Color {
                        r: 1.0,
                        g: 0.3,
                        b: 0.3,
                        a: 1.0,
                    }
                } else if swap_pct > 10.0 {
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
                },
            ),
            (
                "Cached",
                gb(app.mem_cached),
                cached_pct,
                Color {
                    r: 0.3,
                    g: 0.8,
                    b: 0.9,
                    a: 1.0,
                },
            ),
            (
                "Free",
                gb(app.mem_available),
                free_pct,
                Color {
                    r: 0.4,
                    g: 0.4,
                    b: 0.9,
                    a: 1.0,
                },
            ),
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

        for (label, value, pct, color) in &rows {
            if y >= inner.y + inner.height {
                break;
            }

            // Special handling for ZRAM row (ttop style: "ZRAM 2.5G→1.0G 2.5x lz4")
            if *label == "ZRAM" {
                if let Some((orig_gb, compr_gb, ratio, algo)) = &zram_row_data {
                    // Format size strings
                    let orig_str = if *orig_gb >= 1024.0 {
                        format!("{:.1}T", orig_gb / 1024.0)
                    } else {
                        format!("{:.1}G", orig_gb)
                    };
                    let compr_str = if *compr_gb >= 1024.0 {
                        format!("{:.1}T", compr_gb / 1024.0)
                    } else {
                        format!("{:.1}G", compr_gb)
                    };

                    // ZRAM row: "  ZRAM  2.5G→1.0G 2.5x lz4"
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

                    // Draw each part with its color
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
                    let ratio_x =
                        inner.x + 7.0 + orig_str.len() as f32 + 1.0 + compr_str.len() as f32 + 1.0;
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
            let bar: String =
                "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));

            let text = format!("{label:>6} {value:>5.1}G {bar} {pct:>5.1}%");
            canvas.draw_text(
                &text,
                Point::new(inner.x, y),
                &TextStyle {
                    color: *color,
                    ..Default::default()
                },
            );
            y += 1.0;
        }
    }
}

fn draw_disk_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Calculate total disk usage for title
    let (total_used, total_space): (u64, u64) = app
        .disks
        .iter()
        .map(|d| (d.total_space() - d.available_space(), d.total_space()))
        .fold((0, 0), |(au, at), (u, t)| (au + u, at + t));
    let total_pct = if total_space > 0 {
        (total_used as f64 / total_space as f64) * 100.0
    } else {
        0.0
    };

    // Get I/O rates
    let read_rate = app.disk_io_rates.read_bytes_per_sec;
    let write_rate = app.disk_io_rates.write_bytes_per_sec;

    // ttop-style title with I/O rates: " Disk │ R: 1.2M │ W: 345K │ 52G / 100G "
    let title = if read_rate > 0.0 || write_rate > 0.0 {
        format!(
            " Disk │ R: {}/s │ W: {}/s │ {:.0}G / {:.0}G ",
            format_bytes_rate(read_rate),
            format_bytes_rate(write_rate),
            total_used as f64 / 1024.0 / 1024.0 / 1024.0,
            total_space as f64 / 1024.0 / 1024.0 / 1024.0,
        )
    } else {
        // Fallback when no I/O data
        format!(
            " Disk │ {:.0}G / {:.0}G ({:.0}%) ",
            total_used as f64 / 1024.0 / 1024.0 / 1024.0,
            total_space as f64 / 1024.0 / 1024.0 / 1024.0,
            total_pct
        )
    };

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(DISK_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
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

        let bar_width = 12.min((inner.width as usize).saturating_sub(24));
        let filled = ((pct / 100.0) * bar_width as f64) as usize;
        let bar: String =
            "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width - filled.min(bar_width));

        // ttop format: mount | size | bar | percent
        let text = format!("{mount_short:<8} {total_gb:>5.0}G {bar} {pct:>5.1}%");
        canvas.draw_text(
            &text,
            Point::new(inner.x, y),
            &TextStyle {
                color: percent_color(pct),
                ..Default::default()
            },
        );
    }
}

fn draw_network_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // Calculate total network rates for title
    let (rx_total, tx_total): (u64, u64) = app
        .networks
        .iter()
        .map(|(_name, d)| (d.received(), d.transmitted()))
        .fold((0, 0), |(ar, at), (r, t)| (ar + r, at + t));

    // Find primary interface (highest traffic, excluding loopback)
    let primary_iface = app
        .networks
        .iter()
        .filter(|(name, _)| !name.starts_with("lo"))
        .max_by_key(|(_, data)| data.received() + data.transmitted())
        .map(|(name, _)| name.as_str())
        .unwrap_or("none");

    // ttop-style title: " Network (eth0) │ ↓ 1.2M/s │ ↑ 345K/s "
    let title = format!(
        " Network ({}) │ ↓ {}/s │ ↑ {}/s ",
        primary_iface,
        format_bytes(rx_total),
        format_bytes(tx_total)
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(NETWORK_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    let mut interfaces: Vec<NetworkInterface> = Vec::new();
    for (name, data) in &app.networks {
        let mut iface = NetworkInterface::new(name);
        iface.update(data.received() as f64, data.transmitted() as f64);
        iface.set_totals(data.total_received(), data.total_transmitted());
        interfaces.push(iface);
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
    let sort_name = match app.sort_column {
        super::app::ProcessSortColumn::Cpu => "CPU",
        super::app::ProcessSortColumn::Mem => "MEM",
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

    // ttop-style title: " Processes (143) │ Sort: CPU ▼ │ Filter: "chrome" "
    let title = format!(
        " Processes ({}) │ Sort: {} {}{} ",
        app.process_count(),
        sort_name,
        arrow,
        filter_str
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(PROCESS_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 {
        return;
    }

    // Get sorted processes
    let procs = app.sorted_processes();
    let total_mem = app.mem_total as f64;

    // Convert to ProcessEntry with state
    let entries: Vec<ProcessEntry> = procs
        .iter()
        .take(100)
        .map(|p| {
            let mem_pct = if total_mem > 0.0 {
                (p.memory() as f64 / total_mem) * 100.0
            } else {
                0.0
            };
            let user = p
                .user_id()
                .map_or_else(|| "-".to_string(), |u| u.to_string());
            let user_short: String = user.chars().take(8).collect();
            let cmd: String = p.name().to_string_lossy().chars().take(40).collect();

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

            ProcessEntry::new(
                p.pid().as_u32(),
                &user_short,
                p.cpu_usage(),
                mem_pct as f32,
                &cmd,
            )
            .with_state(state)
        })
        .collect();

    let mut table = ProcessTable::new().compact();
    table.set_processes(entries);
    table.select(app.process_selected);
    table.layout(inner);
    table.paint(canvas);
}

fn draw_help_overlay(canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let popup_w = 55.0;
    let popup_h = 18.0;
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

    let help_lines = [
        ("q, Esc, Ctrl+C", "Quit"),
        ("h, ?", "Toggle help"),
        ("j/k, ↑/↓", "Navigate processes"),
        ("PgUp/PgDn", "Page up/down"),
        ("g/G", "Go to top/bottom"),
        ("c", "Sort by CPU"),
        ("m", "Sort by Memory"),
        ("p", "Sort by PID"),
        ("s, Tab", "Cycle sort column"),
        ("r", "Reverse sort"),
        ("/, f", "Filter processes"),
        ("Delete", "Clear filter"),
        ("1-5", "Toggle panels"),
        ("0", "Reset panels"),
    ];

    for (i, (key, desc)) in help_lines.iter().enumerate() {
        let y = py + 1.0 + i as f32;
        canvas.draw_text(&format!("{key:>14}"), Point::new(px + 2.0, y), &key_style);
        canvas.draw_text(desc, Point::new(px + 18.0, y), &text_style);
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
#[derive(Debug, Default)]
struct GpuInfo {
    /// GPU name/model
    name: String,
    /// GPU utilization (0-100)
    utilization: Option<u8>,
    /// Temperature in Celsius
    temperature: Option<u32>,
    /// Power consumption in Watts
    power_watts: Option<f32>,
    /// VRAM used in bytes
    vram_used: Option<u64>,
    /// VRAM total in bytes
    vram_total: Option<u64>,
}

/// Read GPU info from nvidia-smi (NVIDIA) or sysfs (AMD/Intel)
fn read_gpu_info() -> Option<GpuInfo> {
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
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| "AMD GPU".to_string());

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
fn draw_gpu_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let gpu = read_gpu_info();

    let title = gpu
        .as_ref()
        .map(|g| {
            let temp_str = g
                .temperature
                .map(|t| format!(" │ {t}°C"))
                .unwrap_or_default();
            let power_str = g
                .power_watts
                .map(|p| format!(" │ {p:.0}W"))
                .unwrap_or_default();
            format!(" {} {}{} ", g.name, temp_str, power_str)
        })
        .unwrap_or_else(|| " GPU │ No GPU detected ".to_string());

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(GPU_COLOR);
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

            let text = format!("GPU  {bar} {:>3}%", util);
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
            }
        }
    } else {
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
}

/// Battery information from /sys/class/power_supply
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

/// Read battery info from /sys/class/power_supply (Linux only)
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
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "Unknown".to_string());

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
fn draw_battery_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let battery = read_battery_info();

    let title = battery
        .as_ref()
        .map(|b| {
            let time_str = b
                .time_remaining_mins
                .map(|m| {
                    if m >= 60 {
                        format!(" │ {}h{}m", m / 60, m % 60)
                    } else {
                        format!(" │ {}m", m)
                    }
                })
                .unwrap_or_default();
            format!(" Battery │ {}% │ {}{} ", b.capacity, b.status, time_str)
        })
        .unwrap_or_else(|| " Battery │ No battery ".to_string());

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(BATTERY_COLOR);
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
    use sysinfo::{Component, Components};

    let components = Components::new_with_refreshed_list();
    let max_temp = components
        .iter()
        .filter_map(Component::temperature)
        .fold(0.0_f32, f32::max);

    let title = format!(" Sensors │ Max: {max_temp:.0}°C ");

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(SENSORS_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let mut y = inner.y;
    for component in components.iter().take(inner.height as usize) {
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
    }

    if components.is_empty() {
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

    // Suppress unused warning
    let _ = app;
}

/// F010: PSI Panel - shows CPU/Memory/IO pressure (Linux only)
fn draw_psi_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let title = " Pressure │ — ";

    let mut border = Border::new()
        .with_title(title)
        .with_style(BorderStyle::Rounded)
        .with_color(PSI_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Try to read PSI from /proc/pressure (Linux only)
    let psi_data = read_psi_data();

    if let Some((cpu, mem, io)) = psi_data {
        let mut y = inner.y;

        // CPU pressure
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
}

/// Read PSI data from /proc/pressure (Linux only)
fn read_psi_data() -> Option<(f64, f64, f64)> {
    #[cfg(target_os = "linux")]
    {
        use std::fs;

        let parse_psi = |path: &str| -> Option<f64> {
            let content = fs::read_to_string(path).ok()?;
            // Parse "some avg10=X.XX avg60=..." format
            for line in content.lines() {
                if line.starts_with("some") {
                    for part in line.split_whitespace() {
                        if let Some(value) = part.strip_prefix("avg10=") {
                            return value.parse().ok();
                        }
                    }
                }
            }
            None
        };

        let cpu = parse_psi("/proc/pressure/cpu")?;
        let mem = parse_psi("/proc/pressure/memory")?;
        let io = parse_psi("/proc/pressure/io")?;
        Some((cpu, mem, io))
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
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

/// Parse /proc/net/tcp to get active connections
#[cfg(target_os = "linux")]
fn read_tcp_connections() -> Vec<TcpConnection> {
    use std::fs;

    let mut connections = Vec::new();

    // Read TCP connections
    if let Ok(content) = fs::read_to_string("/proc/net/tcp") {
        for line in content.lines().skip(1) {
            if let Some(conn) = parse_tcp_line(line, "tcp") {
                connections.push(conn);
            }
        }
    }

    // Read TCP6 connections
    if let Ok(content) = fs::read_to_string("/proc/net/tcp6") {
        for line in content.lines().skip(1) {
            if let Some(conn) = parse_tcp_line(line, "tcp6") {
                connections.push(conn);
            }
        }
    }

    connections
}

#[cfg(not(target_os = "linux"))]
fn read_tcp_connections() -> Vec<TcpConnection> {
    Vec::new()
}

#[derive(Debug)]
struct TcpConnection {
    #[allow(dead_code)]
    local_addr: String,
    local_port: u16,
    remote_addr: String,
    remote_port: u16,
    state: &'static str,
    #[allow(dead_code)]
    proto: &'static str,
}

/// Parse a line from /proc/net/tcp
fn parse_tcp_line(line: &str, proto: &'static str) -> Option<TcpConnection> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 4 {
        return None;
    }

    // Format: sl local_address rem_address st ...
    let local = parts[1];
    let remote = parts[2];
    let state_hex = parts[3];

    let (local_addr, local_port) = parse_hex_addr(local)?;
    let (remote_addr, remote_port) = parse_hex_addr(remote)?;

    let state = match u8::from_str_radix(state_hex, 16).ok()? {
        0x01 => "E", // ESTABLISHED
        0x02 => "S", // SYN_SENT
        0x03 => "R", // SYN_RECV
        0x04 => "F", // FIN_WAIT1
        0x05 => "F", // FIN_WAIT2
        0x06 => "W", // TIME_WAIT
        0x07 => "C", // CLOSE
        0x08 => "C", // CLOSE_WAIT
        0x09 => "L", // LAST_ACK
        0x0A => "L", // LISTEN
        0x0B => "C", // CLOSING
        _ => "?",
    };

    Some(TcpConnection {
        local_addr,
        local_port,
        remote_addr,
        remote_port,
        state,
        proto,
    })
}

/// Parse hex IP:port from /proc/net/tcp format
fn parse_hex_addr(hex: &str) -> Option<(String, u16)> {
    let parts: Vec<&str> = hex.split(':').collect();
    if parts.len() != 2 {
        return None;
    }

    let addr_hex = parts[0];
    let port = u16::from_str_radix(parts[1], 16).ok()?;

    // Handle IPv4 (8 chars) vs IPv6 (32 chars)
    let addr = if addr_hex.len() == 8 {
        // IPv4: parse as little-endian
        let ip = u32::from_str_radix(addr_hex, 16).ok()?;
        format!(
            "{}.{}.{}.{}",
            ip & 0xFF,
            (ip >> 8) & 0xFF,
            (ip >> 16) & 0xFF,
            (ip >> 24) & 0xFF
        )
    } else {
        // IPv6: simplified display
        "::".to_string()
    };

    Some((addr, port))
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
fn draw_connections_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let connections = read_tcp_connections();

    let listen_count = connections.iter().filter(|c| c.state == "L").count();
    let active_count = connections.iter().filter(|c| c.state == "E").count();

    let title = format!(
        " Connections │ {} active │ {} listen ",
        active_count, listen_count
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(CONNECTIONS_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Header
    let header = "SVC    LOCAL        REMOTE              ST";
    canvas.draw_text(
        header,
        Point::new(inner.x, inner.y),
        &TextStyle {
            color: CONNECTIONS_COLOR,
            ..Default::default()
        },
    );

    // Show connections (skip loopback, prioritize ESTABLISHED and LISTEN)
    let mut display_conns: Vec<_> = connections
        .iter()
        .filter(|c| c.remote_addr != "127.0.0.1" || c.state == "L")
        .collect();

    // Sort: LISTEN first, then ESTABLISHED, then others
    display_conns.sort_by(|a, b| {
        let order = |s: &str| match s {
            "L" => 0,
            "E" => 1,
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
        let remote = if conn.state == "L" {
            "*.*".to_string()
        } else {
            format!("{}:{}", conn.remote_addr, conn.remote_port)
        };

        let state_color = match conn.state {
            "E" => active_color,
            "L" => listen_color,
            _ => dim_color,
        };

        // Format: SVC    LOCAL        REMOTE              ST
        let line = format!("{:<6} {:<12} {:<19} {}", svc, local, remote, conn.state);
        canvas.draw_text(
            &line,
            Point::new(inner.x, y),
            &TextStyle {
                color: state_color,
                ..Default::default()
            },
        );
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

    let title = format!(" Sensors │ {max_temp:.0}°C ");

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(SENSORS_COLOR);
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
fn draw_system_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let title = " System ";

    let mut border = Border::new()
        .with_title(title)
        .with_style(BorderStyle::Rounded)
        .with_color(Color {
            r: 0.5,
            g: 0.7,
            b: 0.9,
            a: 1.0,
        });
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Show system info
    let mut y = inner.y;

    // Hostname
    if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
        let host = hostname.trim();
        canvas.draw_text(
            &format!("Host: {host}"),
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

    // Kernel version
    if let Ok(kernel) = std::fs::read_to_string("/proc/version") {
        let version: String = kernel
            .split_whitespace()
            .take(3)
            .collect::<Vec<_>>()
            .join(" ");
        if y < inner.y + inner.height {
            canvas.draw_text(
                &version,
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
    }

    // Container detection (Docker/Podman)
    let in_container = std::path::Path::new("/.dockerenv").exists()
        || std::fs::read_to_string("/proc/1/cgroup")
            .map(|s| s.contains("docker") || s.contains("containerd"))
            .unwrap_or(false);

    if y < inner.y + inner.height {
        let container_text = if in_container {
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
        " Treemap │ {} disk{} │ {:.0}G / {:.0}G ",
        disk_count,
        if disk_count == 1 { "" } else { "s" },
        total_used as f64 / 1024.0 / 1024.0 / 1024.0,
        total_space as f64 / 1024.0 / 1024.0 / 1024.0,
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(FILES_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 4.0 {
        return;
    }

    // Build treemap nodes from disk data
    let mut disk_nodes: Vec<TreemapNode> = Vec::new();

    for disk in app.disks.iter() {
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

/// F014: Files Panel - file activity and statistics
fn draw_files_panel(_app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let title = " Files │ 0 total │ 0 hot │ 0 dup │ 0 wasted ";

    let mut border = Border::new()
        .with_title(title)
        .with_style(BorderStyle::Rounded)
        .with_color(FILES_COLOR);
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    // Header row with sparklines (placeholder)
    let header = "Activity  Entropy   Duplicates  Recent";
    canvas.draw_text(
        header,
        Point::new(inner.x, inner.y),
        &TextStyle {
            color: FILES_COLOR,
            ..Default::default()
        },
    );

    // Sparkline placeholders
    if inner.height > 1.0 {
        let sparklines = "▁▂▃▄▅▆▇█  ▃▃▃▃▃▃▃▃  ▁▁▂▁▁▁▁▁  ▁▂▃▂▁▂▃▄";
        canvas.draw_text(
            sparklines,
            Point::new(inner.x, inner.y + 1.0),
            &TextStyle {
                color: Color {
                    r: 0.6,
                    g: 0.7,
                    b: 0.5,
                    a: 1.0,
                },
                ..Default::default()
            },
        );
    }

    // File list placeholder
    if inner.height > 2.0 {
        canvas.draw_text(
            "File monitoring requires inotify/fanotify",
            Point::new(inner.x, inner.y + 2.0),
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
