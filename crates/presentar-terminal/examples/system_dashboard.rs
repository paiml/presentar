//! System Dashboard Example - cbtop Style
//!
//! Comprehensive system monitoring dashboard combining CPU, Memory,
//! Network, and Disk metrics in a single view. Similar to btop/htop.
//!
//! Run with: cargo run -p presentar-terminal --example system_dashboard

use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};
use presentar_terminal::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use presentar_terminal::{BrailleGraph, ColorMode, GraphMode};

fn main() {
    println!("=== System Dashboard (cbtop style) ===\n");

    // Simulate system metrics
    let cpu_history = simulate_cpu(60);
    let mem_history = simulate_memory(60);
    let net_rx = simulate_network_rx(60);
    let net_tx = simulate_network_tx(60);

    // Create buffer
    let mut buffer = CellBuffer::new(80, 24);
    let mut renderer = DiffRenderer::with_color_mode(ColorMode::TrueColor);

    {
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        // Background
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 80.0, 24.0),
            Color::new(0.02, 0.02, 0.05, 1.0),
        );

        // Header
        draw_header(&mut canvas);

        // CPU Panel (top-left quadrant)
        draw_cpu_panel(&mut canvas, &cpu_history, Rect::new(1.0, 2.0, 38.0, 9.0));

        // Memory Panel (top-right quadrant)
        draw_memory_panel(&mut canvas, &mem_history, Rect::new(41.0, 2.0, 38.0, 9.0));

        // Network Panel (bottom-left)
        draw_network_panel(
            &mut canvas,
            &net_rx,
            &net_tx,
            Rect::new(1.0, 12.0, 38.0, 6.0),
        );

        // Disk Panel (bottom-right)
        draw_disk_panel(&mut canvas, Rect::new(41.0, 12.0, 38.0, 6.0));

        // Process list (bottom)
        draw_process_summary(&mut canvas, 1.0, 19.0);

        // Footer with keybindings
        draw_footer(&mut canvas);
    }

    // Render
    let mut output = Vec::with_capacity(8192);
    let cells_written = renderer.flush(&mut buffer, &mut output).unwrap();

    println!("Buffer: {}x{}", buffer.width(), buffer.height());
    println!("Cells written: {}", cells_written);
    println!("Output bytes: {}\n", output.len());

    println!("Rendered output:");
    println!("{}", "─".repeat(82));
    std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();
    println!();
    println!("{}", "─".repeat(82));
}

fn draw_header(canvas: &mut DirectTerminalCanvas<'_>) {
    let title_style = TextStyle {
        color: Color::new(0.4, 0.8, 1.0, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        "cbtop - ComputeBlock System Monitor",
        Point::new(2.0, 0.0),
        &title_style,
    );

    let time_style = TextStyle {
        color: Color::new(0.6, 0.6, 0.6, 1.0),
        ..Default::default()
    };
    canvas.draw_text("uptime: 5d 12:34:56", Point::new(60.0, 0.0), &time_style);
}

fn draw_cpu_panel(canvas: &mut DirectTerminalCanvas<'_>, history: &[f64], bounds: Rect) {
    // Panel border
    draw_panel_border(canvas, "CPU", bounds, Color::new(0.3, 0.6, 1.0, 1.0));

    let current = history.last().copied().unwrap_or(0.0);
    let color = cpu_usage_color(current);

    // Usage percentage
    let pct_style = TextStyle {
        color,
        ..Default::default()
    };
    canvas.draw_text(
        &format!("{:5.1}%", current),
        Point::new(bounds.x + bounds.width - 8.0, bounds.y),
        &pct_style,
    );

    // Graph
    let mut graph = BrailleGraph::new(history.to_vec())
        .with_color(color)
        .with_range(0.0, 100.0)
        .with_mode(GraphMode::Braille);

    graph.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        4.0,
    ));
    graph.paint(canvas);

    // Per-core bars
    let cores = [65.2, 42.8, 78.1, 35.6, 89.3, 51.2, 28.9, 67.4];
    draw_mini_meters(
        canvas,
        &cores,
        bounds.x + 1.0,
        bounds.y + 6.0,
        bounds.width - 2.0,
    );
}

fn draw_memory_panel(canvas: &mut DirectTerminalCanvas<'_>, history: &[f64], bounds: Rect) {
    draw_panel_border(canvas, "Memory", bounds, Color::new(0.6, 0.3, 1.0, 1.0));

    let total = 32.0;
    let used = history.last().copied().unwrap_or(0.0);
    let _pct = (used / total) * 100.0;

    // Usage info
    let info_style = TextStyle {
        color: Color::new(0.8, 0.8, 0.8, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        &format!("{:.1}/{:.1} GB", used, total),
        Point::new(bounds.x + bounds.width - 13.0, bounds.y),
        &info_style,
    );

    // Graph
    let pct_history: Vec<f64> = history.iter().map(|&v| (v / total) * 100.0).collect();
    let mut graph = BrailleGraph::new(pct_history)
        .with_color(Color::new(0.6, 0.3, 1.0, 1.0))
        .with_range(0.0, 100.0)
        .with_mode(GraphMode::Braille);

    graph.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        4.0,
    ));
    graph.paint(canvas);

    // Memory breakdown bar
    draw_memory_bar(canvas, bounds.x + 1.0, bounds.y + 6.0, bounds.width - 2.0);

    // Swap info
    let swap_style = TextStyle {
        color: Color::new(0.9, 0.6, 0.3, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        "Swap: 1.2/8.0 GB (15%)",
        Point::new(bounds.x + 1.0, bounds.y + 7.0),
        &swap_style,
    );
}

fn draw_network_panel(canvas: &mut DirectTerminalCanvas<'_>, rx: &[f64], tx: &[f64], bounds: Rect) {
    draw_panel_border(canvas, "Network", bounds, Color::new(0.3, 0.9, 0.5, 1.0));

    // RX/TX current
    let rx_current = rx.last().copied().unwrap_or(0.0);
    let tx_current = tx.last().copied().unwrap_or(0.0);

    let rx_style = TextStyle {
        color: Color::new(0.3, 0.9, 0.5, 1.0),
        ..Default::default()
    };
    let tx_style = TextStyle {
        color: Color::new(0.9, 0.5, 0.3, 1.0),
        ..Default::default()
    };

    canvas.draw_text(
        &format!("↓{:6.1}MB/s", rx_current),
        Point::new(bounds.x + 10.0, bounds.y),
        &rx_style,
    );
    canvas.draw_text(
        &format!("↑{:6.1}MB/s", tx_current),
        Point::new(bounds.x + 24.0, bounds.y),
        &tx_style,
    );

    // Combined graph
    let mut graph = BrailleGraph::new(rx.to_vec())
        .with_color(Color::new(0.3, 0.9, 0.5, 1.0))
        .with_range(0.0, 150.0)
        .with_mode(GraphMode::Block);

    graph.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        bounds.height - 2.0,
    ));
    graph.paint(canvas);
}

fn draw_disk_panel(canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    draw_panel_border(canvas, "Disk", bounds, Color::new(0.9, 0.7, 0.3, 1.0));

    let disks = [
        ("/", 256.0, 180.5),
        ("/home", 512.0, 320.8),
        ("/data", 2000.0, 1450.2),
    ];

    let label_style = TextStyle {
        color: Color::new(0.7, 0.7, 0.7, 1.0),
        ..Default::default()
    };

    for (i, (mount, total, used)) in disks.iter().enumerate() {
        let y = bounds.y + 1.0 + i as f32;
        let pct = (used / total) * 100.0;
        let color = if pct > 90.0 {
            Color::new(1.0, 0.3, 0.3, 1.0)
        } else if pct > 75.0 {
            Color::new(1.0, 0.7, 0.2, 1.0)
        } else {
            Color::new(0.9, 0.7, 0.3, 1.0)
        };

        canvas.draw_text(
            &format!("{:<6}", mount),
            Point::new(bounds.x + 1.0, y),
            &label_style,
        );

        let bar_style = TextStyle {
            color,
            ..Default::default()
        };
        let bar_width = 15;
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let mut bar = String::with_capacity(bar_width);
        for j in 0..bar_width {
            bar.push(if j < filled { '█' } else { '░' });
        }
        canvas.draw_text(&bar, Point::new(bounds.x + 8.0, y), &bar_style);
        canvas.draw_text(
            &format!("{:5.1}%", pct),
            Point::new(bounds.x + 25.0, y),
            &label_style,
        );
    }
}

fn draw_process_summary(canvas: &mut DirectTerminalCanvas<'_>, x: f32, y: f32) {
    let header_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        "Top Processes: firefox(12.3%) code(8.5%) docker(6.2%) chrome(5.8%) slack(3.1%)",
        Point::new(x, y),
        &header_style,
    );
    canvas.draw_text(
        "Tasks: 342 total, 3 running, 339 sleeping | Load: 2.45 1.89 1.52",
        Point::new(x, y + 1.0),
        &header_style,
    );
}

fn draw_footer(canvas: &mut DirectTerminalCanvas<'_>) {
    let key_style = TextStyle {
        color: Color::new(0.3, 0.7, 0.9, 1.0),
        ..Default::default()
    };
    let desc_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };

    let y = 22.0;
    canvas.draw_text("[q]", Point::new(2.0, y), &key_style);
    canvas.draw_text("quit", Point::new(5.0, y), &desc_style);
    canvas.draw_text("[h]", Point::new(12.0, y), &key_style);
    canvas.draw_text("help", Point::new(15.0, y), &desc_style);
    canvas.draw_text("[1-4]", Point::new(22.0, y), &key_style);
    canvas.draw_text("panels", Point::new(27.0, y), &desc_style);
    canvas.draw_text("[p]", Point::new(36.0, y), &key_style);
    canvas.draw_text("processes", Point::new(39.0, y), &desc_style);
    canvas.draw_text("[s]", Point::new(51.0, y), &key_style);
    canvas.draw_text("sort", Point::new(54.0, y), &desc_style);
}

fn draw_panel_border(
    canvas: &mut DirectTerminalCanvas<'_>,
    title: &str,
    bounds: Rect,
    color: Color,
) {
    let border_style = TextStyle {
        color,
        ..Default::default()
    };

    // Top border with title
    let title_line = format!("─{}─", title);
    canvas.draw_text(&title_line, Point::new(bounds.x, bounds.y), &border_style);

    // Fill remaining top border
    let remaining = (bounds.width as usize).saturating_sub(title.len() + 2);
    if remaining > 0 {
        canvas.draw_text(
            &"─".repeat(remaining),
            Point::new(bounds.x + title.len() as f32 + 2.0, bounds.y),
            &border_style,
        );
    }
}

fn draw_mini_meters(
    canvas: &mut DirectTerminalCanvas<'_>,
    values: &[f64],
    x: f32,
    y: f32,
    width: f32,
) {
    let meter_width = (width / values.len() as f32).floor() as usize - 1;

    for (i, &val) in values.iter().enumerate() {
        let mx = x + (i as f32 * (meter_width as f32 + 1.0));
        let color = cpu_usage_color(val);
        let style = TextStyle {
            color,
            ..Default::default()
        };

        let filled = ((val / 100.0) * meter_width as f64).round() as usize;
        let mut bar = String::with_capacity(meter_width);
        for j in 0..meter_width {
            bar.push(if j < filled { '▮' } else { '▯' });
        }
        canvas.draw_text(&bar, Point::new(mx, y), &style);
    }

    // Core labels
    let label_style = TextStyle {
        color: Color::new(0.5, 0.5, 0.5, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        "0  1  2  3  4  5  6  7",
        Point::new(x, y + 1.0),
        &label_style,
    );
}

fn draw_memory_bar(canvas: &mut DirectTerminalCanvas<'_>, x: f32, y: f32, width: f32) {
    // Stacked memory bar: Used | Cached | Buffers | Free
    let used_pct = 0.45;
    let cached_pct = 0.25;
    let buffers_pct = 0.10;

    let bar_width = width as usize;
    let used_cells = (used_pct * bar_width as f64).round() as usize;
    let cached_cells = (cached_pct * bar_width as f64).round() as usize;
    let buffers_cells = (buffers_pct * bar_width as f64).round() as usize;

    let mut bar = String::with_capacity(bar_width);
    for i in 0..bar_width {
        if i < used_cells {
            bar.push('█');
        } else if i < used_cells + cached_cells {
            bar.push('▓');
        } else if i < used_cells + cached_cells + buffers_cells {
            bar.push('▒');
        } else {
            bar.push('░');
        }
    }

    let bar_style = TextStyle {
        color: Color::new(0.6, 0.3, 1.0, 1.0),
        ..Default::default()
    };
    canvas.draw_text(&bar, Point::new(x, y), &bar_style);
}

fn cpu_usage_color(usage: f64) -> Color {
    if usage > 90.0 {
        Color::new(1.0, 0.3, 0.3, 1.0)
    } else if usage > 70.0 {
        Color::new(1.0, 0.7, 0.2, 1.0)
    } else if usage > 50.0 {
        Color::new(1.0, 1.0, 0.3, 1.0)
    } else {
        Color::new(0.3, 0.8, 1.0, 1.0)
    }
}

fn simulate_cpu(count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let t = i as f64 / count as f64;
            let base = 45.0 + 25.0 * (t * 4.0).sin();
            let noise = ((i * 7919) % 30) as f64;
            (base + noise).clamp(5.0, 95.0)
        })
        .collect()
}

fn simulate_memory(count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let t = i as f64 / count as f64;
            let base = 18.0 + 2.0 * (t * 2.0).sin();
            let noise = ((i * 6971) % 10) as f64 / 10.0;
            base + noise
        })
        .collect()
}

fn simulate_network_rx(count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let base = 40.0 + 30.0 * (i as f64 / 8.0).sin();
            let spike = if i % 12 == 0 { 50.0 } else { 0.0 };
            let noise = ((i * 7919) % 20) as f64;
            (base + spike + noise).max(0.0)
        })
        .collect()
}

fn simulate_network_tx(count: usize) -> Vec<f64> {
    (0..count)
        .map(|i| {
            let base = 15.0 + 10.0 * (i as f64 / 6.0).cos();
            let noise = ((i * 6971) % 15) as f64;
            (base + noise).max(0.0)
        })
        .collect()
}
