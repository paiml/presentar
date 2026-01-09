//! System Dashboard Example - cbtop Style
//!
//! Comprehensive system monitoring dashboard combining CPU, Memory,
//! Network, and Disk metrics in a single view. Uses the new ttop-style widgets.
//!
//! Run with: cargo run -p presentar-terminal --example system_dashboard

use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};
use presentar_terminal::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use presentar_terminal::{
    BrailleGraph, ColorMode, CpuGrid, GraphMode, MemoryBar, MemorySegment, NetworkInterface,
    NetworkPanel, ProcessEntry, ProcessTable,
};

fn main() {
    println!("=== System Dashboard (cbtop style) ===\n");

    // Simulate system metrics
    let cpu_history = simulate_cpu(60);
    let mem_history = simulate_memory(60);
    let net_rx = simulate_network_rx(60);
    let net_tx = simulate_network_tx(60);

    // Create buffer (larger for more content)
    let mut buffer = CellBuffer::new(100, 40);
    let mut renderer = DiffRenderer::with_color_mode(ColorMode::TrueColor);

    {
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        // Background
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 100.0, 40.0),
            Color::new(0.02, 0.02, 0.05, 1.0),
        );

        // Header
        draw_header(&mut canvas);

        // CPU Panel with CpuGrid widget (top-left)
        draw_cpu_panel_with_grid(&mut canvas, &cpu_history, Rect::new(1.0, 2.0, 48.0, 10.0));

        // Memory Panel with MemoryBar widget (top-right)
        draw_memory_panel_with_bar(&mut canvas, &mem_history, Rect::new(51.0, 2.0, 48.0, 10.0));

        // Network Panel with NetworkPanel widget (middle-left)
        draw_network_panel_with_widget(
            &mut canvas,
            &net_rx,
            &net_tx,
            Rect::new(1.0, 13.0, 48.0, 7.0),
        );

        // Disk Panel (middle-right)
        draw_disk_panel(&mut canvas, Rect::new(51.0, 13.0, 48.0, 7.0));

        // Process Table widget (bottom)
        draw_process_table(&mut canvas, Rect::new(1.0, 21.0, 98.0, 16.0));

        // Footer with keybindings
        draw_footer(&mut canvas);
    }

    // Render
    let mut output = Vec::with_capacity(16384);
    let cells_written = renderer.flush(&mut buffer, &mut output).unwrap();

    println!("Buffer: {}x{}", buffer.width(), buffer.height());
    println!("Cells written: {}", cells_written);
    println!("Output bytes: {}\n", output.len());

    println!("Rendered output:");
    println!("{}", "─".repeat(102));
    std::io::Write::write_all(&mut std::io::stdout(), &output).unwrap();
    println!();
    println!("{}", "─".repeat(102));
}

fn draw_header(canvas: &mut DirectTerminalCanvas<'_>) {
    let title_style = TextStyle {
        color: Color::new(0.4, 0.8, 1.0, 1.0),
        weight: presentar_core::FontWeight::Bold,
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
    canvas.draw_text("uptime: 5d 12:34:56", Point::new(80.0, 0.0), &time_style);
}

fn draw_cpu_panel_with_grid(canvas: &mut DirectTerminalCanvas<'_>, history: &[f64], bounds: Rect) {
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

    // Braille graph for overall CPU history
    let mut graph = BrailleGraph::new(history.to_vec())
        .with_color(color)
        .with_range(0.0, 100.0)
        .with_mode(GraphMode::Braille);

    graph.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        3.0,
    ));
    graph.paint(canvas);

    // Per-core CPU grid using the new CpuGrid widget
    let core_usage: Vec<f64> = (0..16)
        .map(|i| {
            // Simulate per-core usage
            let base = 45.0 + 35.0 * ((i as f64 * 0.7) + history.len() as f64 / 10.0).sin();
            let noise = ((i * 7919) % 40) as f64;
            (base + noise).clamp(5.0, 98.0)
        })
        .collect();

    let mut cpu_grid = CpuGrid::new(core_usage).with_columns(8).compact();
    cpu_grid.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 5.0,
        bounds.width - 2.0,
        4.0,
    ));
    cpu_grid.paint(canvas);
}

fn draw_memory_panel_with_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    history: &[f64],
    bounds: Rect,
) {
    draw_panel_border(canvas, "Memory", bounds, Color::new(0.6, 0.3, 1.0, 1.0));

    let total_gb = 128.0;
    let used_gb = history.last().copied().unwrap_or(0.0);

    // Usage info
    let info_style = TextStyle {
        color: Color::new(0.8, 0.8, 0.8, 1.0),
        ..Default::default()
    };
    canvas.draw_text(
        &format!("{:.1}/{:.0} GB", used_gb, total_gb),
        Point::new(bounds.x + bounds.width - 15.0, bounds.y),
        &info_style,
    );

    // Graph
    let pct_history: Vec<f64> = history.iter().map(|&v| (v / total_gb) * 100.0).collect();
    let mut graph = BrailleGraph::new(pct_history)
        .with_color(Color::new(0.6, 0.3, 1.0, 1.0))
        .with_range(0.0, 100.0)
        .with_mode(GraphMode::Braille);

    graph.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        3.0,
    ));
    graph.paint(canvas);

    // Memory bar using the new MemoryBar widget
    let total_bytes = (total_gb * 1024.0 * 1024.0 * 1024.0) as u64;
    let mut memory_bar = MemoryBar::from_usage(
        (50.0 * 1024.0 * 1024.0 * 1024.0) as u64, // 50G used
        (30.0 * 1024.0 * 1024.0 * 1024.0) as u64, // 30G cached
        (2.0 * 1024.0 * 1024.0 * 1024.0) as u64,  // 2G swap used
        (16.0 * 1024.0 * 1024.0 * 1024.0) as u64, // 16G swap total
        total_bytes,
    )
    .with_bar_width(25);

    memory_bar.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 5.0,
        bounds.width - 2.0,
        4.0,
    ));
    memory_bar.paint(canvas);
}

fn draw_network_panel_with_widget(
    canvas: &mut DirectTerminalCanvas<'_>,
    rx: &[f64],
    tx: &[f64],
    bounds: Rect,
) {
    draw_panel_border(canvas, "Network", bounds, Color::new(0.3, 0.9, 0.5, 1.0));

    // Create network interfaces
    let mut eth0 = NetworkInterface::new("eth0");
    for (i, (&r, &t)) in rx.iter().zip(tx.iter()).enumerate() {
        eth0.update(r * 1024.0 * 1024.0, t * 1024.0 * 1024.0); // Convert to bytes
    }
    eth0.set_totals(
        1024 * 1024 * 1024 * 50, // 50GB received
        1024 * 1024 * 1024 * 10, // 10GB transmitted
    );

    let mut wlan0 = NetworkInterface::new("wlan0");
    for i in 0..30 {
        let r = 5.0 + 3.0 * (i as f64 / 5.0).sin();
        let t = 2.0 + 1.5 * (i as f64 / 4.0).cos();
        wlan0.update(r * 1024.0 * 1024.0, t * 1024.0 * 1024.0);
    }
    wlan0.set_totals(
        1024 * 1024 * 1024 * 5, // 5GB received
        1024 * 1024 * 512,      // 512MB transmitted
    );

    let mut network_panel = NetworkPanel::new().with_spark_width(15).compact();
    network_panel.set_interfaces(vec![eth0, wlan0]);

    network_panel.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        bounds.height - 2.0,
    ));
    network_panel.paint(canvas);
}

fn draw_disk_panel(canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    draw_panel_border(canvas, "Disk", bounds, Color::new(0.9, 0.7, 0.3, 1.0));

    let disks = [
        ("/", 256.0, 180.5),
        ("/home", 512.0, 320.8),
        ("/data", 2000.0, 1450.2),
        ("/nvme", 1000.0, 450.0),
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
            &format!("{:<8}", mount),
            Point::new(bounds.x + 1.0, y),
            &label_style,
        );

        let bar_style = TextStyle {
            color,
            ..Default::default()
        };
        let bar_width = 20;
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let mut bar = String::with_capacity(bar_width);
        for j in 0..bar_width {
            bar.push(if j < filled { '█' } else { '░' });
        }
        canvas.draw_text(&bar, Point::new(bounds.x + 10.0, y), &bar_style);
        canvas.draw_text(
            &format!("{:5.1}% ({:.0}/{:.0}G)", pct, used, total),
            Point::new(bounds.x + 32.0, y),
            &label_style,
        );
    }
}

fn draw_process_table(canvas: &mut DirectTerminalCanvas<'_>, bounds: Rect) {
    draw_panel_border(canvas, "Processes", bounds, Color::new(0.5, 0.7, 0.9, 1.0));

    // Create process table with sample data
    let processes = vec![
        ProcessEntry::new(1234, "noah", 25.3, 5.5, "firefox"),
        ProcessEntry::new(5678, "noah", 18.7, 12.3, "rustc"),
        ProcessEntry::new(9012, "noah", 15.2, 8.1, "code"),
        ProcessEntry::new(3456, "root", 12.8, 3.2, "dockerd"),
        ProcessEntry::new(7890, "noah", 8.5, 2.1, "chrome"),
        ProcessEntry::new(2345, "noah", 6.2, 1.8, "slack"),
        ProcessEntry::new(6789, "root", 4.1, 0.9, "sshd"),
        ProcessEntry::new(1357, "noah", 3.5, 1.2, "nvim"),
        ProcessEntry::new(2468, "root", 2.8, 0.5, "systemd"),
        ProcessEntry::new(3579, "noah", 2.1, 0.3, "zsh"),
        ProcessEntry::new(4680, "noah", 1.5, 0.2, "tmux"),
        ProcessEntry::new(5791, "root", 1.2, 0.1, "containerd"),
    ];

    let mut process_table = ProcessTable::new();
    process_table.set_processes(processes);

    process_table.layout(Rect::new(
        bounds.x + 1.0,
        bounds.y + 1.0,
        bounds.width - 2.0,
        bounds.height - 2.0,
    ));
    process_table.paint(canvas);
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

    let y = 38.0;
    canvas.draw_text("[q]", Point::new(2.0, y), &key_style);
    canvas.draw_text("quit", Point::new(5.0, y), &desc_style);
    canvas.draw_text("[h]", Point::new(12.0, y), &key_style);
    canvas.draw_text("help", Point::new(15.0, y), &desc_style);
    canvas.draw_text("[c]", Point::new(22.0, y), &key_style);
    canvas.draw_text("sort:cpu", Point::new(25.0, y), &desc_style);
    canvas.draw_text("[m]", Point::new(36.0, y), &key_style);
    canvas.draw_text("sort:mem", Point::new(39.0, y), &desc_style);
    canvas.draw_text("[p]", Point::new(50.0, y), &key_style);
    canvas.draw_text("sort:pid", Point::new(53.0, y), &desc_style);
    canvas.draw_text("[k]", Point::new(64.0, y), &key_style);
    canvas.draw_text("kill", Point::new(67.0, y), &desc_style);
    canvas.draw_text("[/]", Point::new(74.0, y), &key_style);
    canvas.draw_text("filter", Point::new(77.0, y), &desc_style);
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
            let base = 65.0 + 10.0 * (t * 2.0).sin();
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
