//! UI layout and rendering for ptop.
//!
//! Mirrors ttop's ui.rs - defines panel layout and dispatches to widgets.

use crate::direct::{CellBuffer, DirectTerminalCanvas};
use crate::{
    Border, BorderStyle, BrailleGraph, CpuGrid, GraphMode, MemoryBar, NetworkInterface,
    NetworkPanel, ProcessEntry, ProcessTable,
};
use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};

use super::app::App;

/// Main draw function - called each frame
pub fn draw(app: &App, buffer: &mut CellBuffer) {
    let w = buffer.width() as f32;
    let h = buffer.height() as f32;

    let mut canvas = DirectTerminalCanvas::new(buffer);

    // Clear with background
    // Main border
    let mut main_border = Border::new()
        .with_title("ptop - Presentar System Monitor")
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.4, 0.4, 0.4, 1.0));
    main_border.layout(Rect::new(0.0, 0.0, w, h));
    main_border.paint(&mut canvas);

    // Uptime in title bar
    let uptime = app.uptime();
    let days = uptime / 86400;
    let hours = (uptime % 86400) / 3600;
    let mins = (uptime % 3600) / 60;
    let secs = uptime % 60;
    let uptime_str = format!("up: {}d {:02}:{:02}:{:02}", days, hours, mins, secs);
    let time_style = TextStyle {
        color: Color::new(0.6, 0.6, 0.6, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        &uptime_str,
        Point::new(w - uptime_str.len() as f32 - 2.0, 0.0),
        &time_style,
    );

    // Count visible top panels
    let mut top_count = 0;
    if app.panels.cpu {
        top_count += 1;
    }
    if app.panels.memory {
        top_count += 1;
    }
    if app.panels.disk {
        top_count += 1;
    }
    if app.panels.network {
        top_count += 1;
    }

    // Layout: 45% top panels, 55% process table (if both visible)
    let has_process = app.panels.process;
    let top_height = if top_count > 0 && has_process {
        (h * 0.45).max(8.0)
    } else if top_count > 0 {
        h - 2.0
    } else {
        0.0
    };
    let process_y = 1.0 + top_height;
    let process_height = h - process_y - 2.0;

    // Draw top panels in 2x2 grid
    if top_count > 0 {
        draw_top_panels(app, &mut canvas, Rect::new(1.0, 1.0, w - 2.0, top_height));
    }

    // Draw process panel
    if has_process && process_height > 3.0 {
        draw_process_panel(
            app,
            &mut canvas,
            Rect::new(1.0, process_y, w - 2.0, process_height),
        );
    }

    // Footer
    draw_footer(app, &mut canvas, Rect::new(1.0, h - 1.0, w - 2.0, 1.0));

    // Help overlay
    if app.show_help {
        draw_help_overlay(&mut canvas, w, h);
    }

    // Filter input overlay
    if app.show_filter_input {
        draw_filter_overlay(app, &mut canvas, w, h);
    }
}

fn draw_top_panels(app: &App, canvas: &mut DirectTerminalCanvas<'_>, area: Rect) {
    // Count panels
    let mut panels: Vec<(&str, fn(&App, &mut DirectTerminalCanvas<'_>, Rect))> = Vec::new();
    if app.panels.cpu {
        panels.push(("CPU", draw_cpu_panel));
    }
    if app.panels.memory {
        panels.push(("Memory", draw_memory_panel));
    }
    if app.panels.disk {
        panels.push(("Disk", draw_disk_panel));
    }
    if app.panels.network {
        panels.push(("Network", draw_network_panel));
    }

    if panels.is_empty() {
        return;
    }

    // Grid layout: 2 columns, rows as needed
    let cols = 2.min(panels.len());
    let rows = panels.len().div_ceil(cols);
    let cell_w = area.width / cols as f32;
    let cell_h = area.height / rows as f32;

    for (i, (_name, draw_fn)) in panels.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;
        let x = area.x + col as f32 * cell_w;
        let y = area.y + row as f32 * cell_h;
        draw_fn(app, canvas, Rect::new(x, y, cell_w, cell_h));
    }
}

fn draw_cpu_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    // CPU usage percentage
    let cpu_pct = app.cpu_history.last().copied().unwrap_or(0.0) * 100.0;

    let title = format!(" CPU {:.1}% ", cpu_pct);
    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.3, 0.6, 1.0, 1.0));
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 5.0 {
        return;
    }

    // Braille graph (top half)
    let history: Vec<f64> = app
        .cpu_history
        .as_slice()
        .iter()
        .map(|&v| v * 100.0)
        .collect();
    if !history.is_empty() {
        let graph_h = (inner.height / 2.0).max(2.0);
        let mut graph = BrailleGraph::new(history)
            .with_color(cpu_color(cpu_pct))
            .with_range(0.0, 100.0)
            .with_mode(GraphMode::Braille);
        graph.layout(Rect::new(inner.x, inner.y, inner.width, graph_h));
        graph.paint(canvas);
    }

    // CPU grid (bottom half)
    if !app.per_core_percent.is_empty() {
        let grid_y = inner.y + (inner.height / 2.0).max(2.0);
        let grid_h = inner.height - (inner.height / 2.0).max(2.0);
        if grid_h > 0.0 {
            let cols = 8.min(app.per_core_percent.len());
            let mut grid = CpuGrid::new(app.per_core_percent.clone())
                .with_columns(cols)
                .compact();
            grid.layout(Rect::new(inner.x, grid_y, inner.width, grid_h));
            grid.paint(canvas);
        }
    }
}

fn draw_memory_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let gb = |b: u64| b as f64 / 1024.0 / 1024.0 / 1024.0;
    let title = format!(
        " Memory {:.1}/{:.0} GB ",
        gb(app.mem_used),
        gb(app.mem_total)
    );

    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.8, 0.3, 0.8, 1.0));
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 || inner.width < 5.0 {
        return;
    }

    // Braille graph (top half)
    let history: Vec<f64> = app
        .mem_history
        .as_slice()
        .iter()
        .map(|&v| v * 100.0)
        .collect();
    if !history.is_empty() {
        let graph_h = (inner.height / 2.0).max(2.0);
        let mut graph = BrailleGraph::new(history)
            .with_color(Color::new(0.8, 0.3, 0.8, 1.0))
            .with_range(0.0, 100.0)
            .with_mode(GraphMode::Braille);
        graph.layout(Rect::new(inner.x, inner.y, inner.width, graph_h));
        graph.paint(canvas);
    }

    // Memory bar (bottom half)
    let bar_y = inner.y + (inner.height / 2.0).max(2.0);
    let bar_h = inner.height - (inner.height / 2.0).max(2.0);
    if bar_h > 0.0 && app.mem_total > 0 {
        let bar_width = (inner.width as usize).saturating_sub(4).max(10);
        let mut memory_bar = MemoryBar::from_usage(
            app.mem_used,
            app.mem_cached,
            app.swap_used,
            app.mem_available,
            app.mem_total,
        )
        .with_bar_width(bar_width);
        memory_bar.layout(Rect::new(inner.x, bar_y, inner.width, bar_h));
        memory_bar.paint(canvas);
    }
}

fn draw_disk_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let mut border = Border::new()
        .with_title(" Disk ")
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.9, 0.7, 0.3, 1.0));
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 1.0 {
        return;
    }

    let style = TextStyle {
        color: Color::new(0.8, 0.8, 0.8, 1.0),
        ..Default::default()
    };

    let max_disks = (inner.height as usize).max(1);
    for (i, disk) in app.disks.iter().take(max_disks).enumerate() {
        let y = inner.y + i as f32;
        if y >= inner.y + inner.height {
            break;
        }

        let mount = disk.mount_point().to_string_lossy();
        let mount_short: String = mount.chars().take(8).collect();

        let total = disk.total_space();
        let used = total - disk.available_space();
        let pct = if total > 0 {
            (used as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        canvas.draw_text(
            &format!("{:<8}", mount_short),
            Point::new(inner.x, y),
            &style,
        );

        // Bar
        let bar_width = 12.min((inner.width as usize).saturating_sub(18));
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let bar: String = (0..bar_width)
            .map(|j| if j < filled { '█' } else { '░' })
            .collect();
        let bar_color = if pct > 80.0 {
            Color::new(1.0, 0.3, 0.3, 1.0)
        } else if pct > 60.0 {
            Color::new(0.9, 0.7, 0.3, 1.0)
        } else {
            Color::new(0.3, 0.8, 0.5, 1.0)
        };
        canvas.draw_text(
            &bar,
            Point::new(inner.x + 9.0, y),
            &TextStyle {
                color: bar_color,
                ..Default::default()
            },
        );

        canvas.draw_text(
            &format!("{:5.1}%", pct),
            Point::new(inner.x + 9.0 + bar_width as f32 + 1.0, y),
            &style,
        );
    }
}

fn draw_network_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let mut border = Border::new()
        .with_title(" Network ")
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.3, 0.8, 0.5, 1.0));
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    let mut interfaces: Vec<NetworkInterface> = Vec::new();
    for (name, data) in app.networks.iter() {
        let mut iface = NetworkInterface::new(name);
        iface.update(data.received() as f64, data.transmitted() as f64);
        iface.set_totals(data.total_received(), data.total_transmitted());
        interfaces.push(iface);
    }

    interfaces.truncate(4);

    if !interfaces.is_empty() && inner.height > 0.0 {
        let spark_w = (inner.width as usize / 4).max(5);
        let mut panel = NetworkPanel::new().with_spark_width(spark_w).compact();
        panel.set_interfaces(interfaces);
        panel.layout(inner);
        panel.paint(canvas);
    }
}

fn draw_process_panel(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let title = format!(" Processes ({}) ", app.process_count());
    let mut border = Border::new()
        .with_title(&title)
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.5, 0.7, 0.9, 1.0));
    border.layout(bounds);
    border.paint(canvas);
    let inner = border.inner_rect();

    if inner.height < 2.0 {
        return;
    }

    // Get sorted processes
    let procs = app.sorted_processes();
    let total_mem = app.mem_total as f64;

    // Convert to ProcessEntry
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
                .map(|u| u.to_string())
                .unwrap_or_else(|| "-".to_string());
            let user_short: String = user.chars().take(8).collect();
            let cmd: String = p.name().to_string_lossy().chars().take(30).collect();

            ProcessEntry::new(
                p.pid().as_u32(),
                &user_short,
                p.cpu_usage(),
                mem_pct as f32,
                &cmd,
            )
        })
        .collect();

    let mut table = ProcessTable::new();
    table.set_processes(entries);
    table.select(app.process_selected);
    table.layout(inner);
    table.paint(canvas);
}

fn draw_footer(app: &App, canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    let key_style = TextStyle {
        color: Color::new(0.3, 0.7, 0.9, 1.0),
        ..Default::default()
    };
    let desc_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };

    let keys = [
        ("[q]", "quit"),
        ("[h]", "help"),
        ("[c]", "cpu"),
        ("[m]", "mem"),
        ("[p]", "pid"),
        ("[/]", "filter"),
    ];

    let mut x = bounds.x;
    for (k, d) in keys {
        canvas.draw_text(k, Point::new(x, bounds.y), &key_style);
        x += k.len() as f32;
        canvas.draw_text(d, Point::new(x, bounds.y), &desc_style);
        x += d.len() as f32 + 1.0;
    }

    // Sort indicator
    let sort_name = match app.sort_column {
        super::app::ProcessSortColumn::Cpu => "cpu",
        super::app::ProcessSortColumn::Mem => "mem",
        super::app::ProcessSortColumn::Pid => "pid",
        super::app::ProcessSortColumn::User => "user",
        super::app::ProcessSortColumn::Command => "cmd",
    };
    let arrow = if app.sort_descending { "▼" } else { "▲" };
    let sort_str = format!(" sort:{}{}", sort_name, arrow);
    canvas.draw_text(
        &sort_str,
        Point::new(bounds.x + bounds.width - sort_str.len() as f32, bounds.y),
        &key_style,
    );

    // FPS if enabled
    if app.show_fps {
        let fps_str = format!(" {}μs", app.avg_frame_time_us);
        canvas.draw_text(
            &fps_str,
            Point::new(
                bounds.x + bounds.width - sort_str.len() as f32 - fps_str.len() as f32 - 1.0,
                bounds.y,
            ),
            &desc_style,
        );
    }
}

fn draw_help_overlay(canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let popup_w = 50.0;
    let popup_h = 16.0;
    let px = (w - popup_w) / 2.0;
    let py = (h - popup_h) / 2.0;

    // Clear background
    let bg_style = TextStyle {
        color: Color::new(0.15, 0.15, 0.15, 1.0),
        ..Default::default()
    };
    for y in 0..popup_h as u16 {
        let spaces: String = (0..popup_w as usize).map(|_| ' ').collect();
        canvas.draw_text(&spaces, Point::new(px, py + y as f32), &bg_style);
    }

    let mut border = Border::new()
        .with_title(" Help ")
        .with_style(BorderStyle::Double)
        .with_color(Color::new(0.9, 0.8, 0.3, 1.0));
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
        ("1-5", "Toggle panels"),
    ];

    for (i, (key, desc)) in help_lines.iter().enumerate() {
        let y = py + 1.0 + i as f32;
        canvas.draw_text(&format!("{:>14}", key), Point::new(px + 2.0, y), &key_style);
        canvas.draw_text(*desc, Point::new(px + 18.0, y), &text_style);
    }
}

fn draw_filter_overlay(app: &App, canvas: &mut DirectTerminalCanvas<'_>, w: f32, h: f32) {
    let popup_w = 40.0;
    let popup_h = 3.0;
    let px = (w - popup_w) / 2.0;
    let py = (h - popup_h) / 2.0;

    let mut border = Border::new()
        .with_title(" Filter ")
        .with_style(BorderStyle::Rounded)
        .with_color(Color::new(0.3, 0.8, 0.9, 1.0));
    border.layout(Rect::new(px, py, popup_w, popup_h));
    border.paint(canvas);

    let text_style = TextStyle {
        color: Color::new(1.0, 1.0, 1.0, 1.0),
        ..Default::default()
    };

    let filter_display = format!("{}_", app.filter);
    canvas.draw_text(&filter_display, Point::new(px + 2.0, py + 1.0), &text_style);
}

fn cpu_color(usage: f64) -> Color {
    if usage > 90.0 {
        Color::new(1.0, 0.3, 0.3, 1.0) // Red
    } else if usage > 70.0 {
        Color::new(1.0, 0.6, 0.3, 1.0) // Orange
    } else if usage > 50.0 {
        Color::new(1.0, 1.0, 0.3, 1.0) // Yellow
    } else {
        Color::new(0.3, 0.8, 1.0, 1.0) // Cyan
    }
}
