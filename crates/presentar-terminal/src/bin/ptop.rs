//! ptop: System monitor using presentar-terminal widget composition
//!
//! Run: cargo run -p presentar-terminal --features ptop --bin ptop

#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::unnecessary_debug_formatting)]

use std::io::{self, Write};
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::{
    cursor,
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{self, ClearType},
};

use presentar_terminal::direct::{CellBuffer, DiffRenderer};
use presentar_terminal::ptop::{config::PtopConfig, ui, App, PanelType};
use presentar_terminal::ColorMode;

/// Presentar System Monitor - widget composition demo
#[derive(Parser)]
#[command(name = "ptop", version, about, long_about = None)]
struct Cli {
    /// Refresh interval in milliseconds
    #[arg(short, long, default_value = "1000")]
    refresh: u64,

    /// Enable deterministic mode for testing (disables timestamps/dynamic data)
    #[arg(long)]
    deterministic: bool,

    /// Disable colors (use plain text)
    #[arg(long)]
    no_color: bool,

    /// Render once to stdout and exit (for comparison/testing)
    #[arg(long)]
    render_once: bool,

    /// Terminal width for render-once mode
    #[arg(long, default_value = "120")]
    width: u16,

    /// Terminal height for render-once mode
    #[arg(long, default_value = "40")]
    height: u16,

    /// Path to custom config file (YAML)
    #[arg(short, long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,

    /// Dump default configuration to stdout and exit
    #[arg(long)]
    dump_config: bool,

    /// QA timing mode: output timing diagnostics to stderr
    #[arg(long)]
    qa_timing: bool,

    /// Explode a specific panel for QA (cpu, memory, disk, network, process, gpu, sensors, connections, psi, files, battery, containers)
    #[arg(long, value_name = "PANEL")]
    explode: Option<String>,
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Handle --dump-config: print default config and exit
    if cli.dump_config {
        println!("{}", PtopConfig::default_yaml());
        return Ok(());
    }

    // Load config from custom path if specified
    let config = if let Some(ref path) = cli.config {
        PtopConfig::load_from_file(path).unwrap_or_else(|| {
            eprintln!("[ptop] Warning: Could not load config from {path:?}, using defaults");
            PtopConfig::default()
        })
    } else {
        PtopConfig::load()
    };

    // Render-once mode: fast path with lightweight init (skip heavy process scan)
    if cli.render_once {
        let mut app = App::with_config_lightweight(cli.deterministic, config);
        // Must call collect_metrics() to collect system data before rendering
        // Note: CPU usage requires two samples, so we collect twice with a delay
        if !cli.deterministic {
            app.collect_metrics(); // First sample + process data
            std::thread::sleep(Duration::from_millis(100));
            app.collect_metrics(); // Second sample (calculates CPU delta)
        }
        // Set exploded panel if specified via --explode flag
        if let Some(ref panel_name) = cli.explode {
            app.exploded_panel = parse_panel_type(panel_name);
        }
        return render_once(&app, cli.width, cli.height);
    }

    // Create app BEFORE raw mode (so Ctrl+C works during init)
    // Full initialization for interactive mode (includes process scan)
    let app = App::with_config(cli.deterministic, config);

    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        terminal::Clear(ClearType::All)
    )?;

    let color_mode = if cli.no_color {
        ColorMode::Mono
    } else {
        ColorMode::TrueColor
    };

    let result = run_app(
        &mut stdout,
        app,
        cli.refresh,
        color_mode,
        cli.qa_timing,
    );

    // Cleanup
    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

/// Render a single frame to stdout (for comparison/testing)
fn render_once(app: &App, width: u16, height: u16) -> io::Result<()> {
    let mut buffer = CellBuffer::new(width, height);
    ui::draw(app, &mut buffer);

    let mut stdout = io::stdout();

    // Output each row as plain text (no ANSI sequences)
    for y in 0..height {
        for x in 0..width {
            if let Some(cell) = buffer.get(x, y) {
                // Get first char of symbol (handles multi-byte)
                let ch = cell.symbol.chars().next().unwrap_or(' ');
                write!(stdout, "{ch}")?;
            } else {
                write!(stdout, " ")?;
            }
        }
        writeln!(stdout)?;
    }

    stdout.flush()?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::items_after_statements)]
fn run_app(
    stdout: &mut io::Stdout,
    mut app: App,
    refresh_ms: u64,
    color_mode: ColorMode,
    qa_timing: bool,
) -> io::Result<()> {
    use presentar_terminal::ptop::app::MetricsCollector;
    use presentar_terminal::AsyncCollector;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{mpsc, Arc};

    let mut renderer = DiffRenderer::with_color_mode(color_mode);

    // CB-INPUT-006: Non-blocking architecture (PROPER IMPLEMENTATION)
    // - Background thread owns MetricsCollector (all heavy I/O objects)
    // - Main thread owns App (UI state only)
    // - Channel sends MetricsSnapshot from collector to main thread
    // - NO MUTEX CONTENTION - main thread never blocks
    let collect_interval = Duration::from_millis(refresh_ms);

    // QA timing: track collect duration (atomic for thread-safe read)
    let collect_time_us = Arc::new(AtomicU64::new(0));
    let collect_time_bg = Arc::clone(&collect_time_us);
    let bg_running = Arc::new(AtomicBool::new(true));
    let bg_running_thread = Arc::clone(&bg_running);

    // Channel for MetricsSnapshot transport (no mutex needed!)
    let (tx, rx) = mpsc::channel::<presentar_terminal::ptop::app::MetricsSnapshot>();

    // Background collector thread - owns all heavy I/O objects
    let deterministic = app.deterministic;
    let _collector = std::thread::spawn(move || {
        let mut collector = MetricsCollector::new(deterministic);

        while bg_running_thread.load(Ordering::Relaxed) {
            let collect_start = Instant::now();

            // Collect metrics (can take seconds - safe because NO lock held)
            let snapshot = collector.collect();

            let collect_dur = collect_start.elapsed();
            collect_time_bg.store(collect_dur.as_micros() as u64, Ordering::Relaxed);

            // Send snapshot to main thread (non-blocking for sender)
            if tx.send(snapshot).is_err() {
                break; // Receiver dropped, exit
            }

            // Wait for next collection interval
            std::thread::sleep(collect_interval);
        }
    });

    let mut last_render = Instant::now();
    let render_interval = Duration::from_millis(16); // ~60fps max
    let mut frame_times: Vec<Duration> = Vec::with_capacity(60);
    let mut was_exploded = false;

    // QA timing stats (no lock times - we don't use locks anymore!)
    let mut input_times: Vec<u64> = Vec::with_capacity(100);
    let mut render_times: Vec<u64> = Vec::with_capacity(100);
    let mut qa_report_interval = Instant::now();

    loop {
        // PRIORITY 1: Process ALL pending input events (non-blocking)
        let input_start = Instant::now();
        while event::poll(Duration::from_millis(1))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Direct access to app - no lock!
                    if app.handle_key(key.code, key.modifiers) {
                        app.running = false;
                        bg_running.store(false, Ordering::Relaxed);
                        return Ok(());
                    }
                }
            }
        }
        let input_dur = input_start.elapsed();
        if qa_timing {
            input_times.push(input_dur.as_micros() as u64);
        }

        // PRIORITY 2: Apply any pending snapshots (non-blocking receive)
        while let Ok(snapshot) = rx.try_recv() {
            app.apply_snapshot(snapshot);
        }

        // Rate-limit rendering to ~60fps
        if last_render.elapsed() < render_interval {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        let frame_start = Instant::now();
        let (width, height) = terminal::size()?;

        // Render frame - direct access to app, no lock!
        let render_start = Instant::now();
        let mut buffer = CellBuffer::new(width, height);
        ui::draw(&app, &mut buffer);
        let render_dur = render_start.elapsed();
        if qa_timing {
            render_times.push(render_dur.as_micros() as u64);
        }

        let is_exploded = app.exploded_panel.is_some();
        if !app.running {
            bg_running.store(false, Ordering::Relaxed);
            break;
        }

        // Detect exploded mode change for full refresh
        let mode_changed = is_exploded != was_exploded;
        was_exploded = is_exploded;

        // Render to terminal
        execute!(stdout, cursor::MoveTo(0, 0))?;
        let mut output = Vec::with_capacity(32768);

        if mode_changed {
            renderer.render_full(&mut buffer, &mut output)?;
        } else {
            renderer.flush(&mut buffer, &mut output)?;
        }

        stdout.write_all(&output)?;
        stdout.flush()?;

        last_render = Instant::now();

        // Track frame time
        let frame_time = frame_start.elapsed();
        frame_times.push(frame_time);
        if frame_times.len() > 60 {
            frame_times.remove(0);
        }

        // Update frame stats in app (direct access)
        app.update_frame_stats(&frame_times);

        // QA timing: periodic report to stderr
        if qa_timing && qa_report_interval.elapsed() >= Duration::from_secs(2) {
            qa_report_interval = Instant::now();
            let avg = |v: &[u64]| {
                if v.is_empty() {
                    0
                } else {
                    v.iter().sum::<u64>() / v.len() as u64
                }
            };
            let max = |v: &[u64]| v.iter().max().copied().unwrap_or(0);
            eprintln!(
                "[QA] input: avg={}us max={}us | render: avg={}us max={}us | collect: {}us (NO LOCK)",
                avg(&input_times), max(&input_times),
                avg(&render_times), max(&render_times),
                collect_time_us.load(Ordering::Relaxed)
            );
            input_times.clear();
            render_times.clear();
        }
    }

    Ok(())
}

/// Parse panel type from string for --explode flag
fn parse_panel_type(name: &str) -> Option<PanelType> {
    match name.to_lowercase().as_str() {
        "cpu" => Some(PanelType::Cpu),
        "memory" | "mem" => Some(PanelType::Memory),
        "disk" => Some(PanelType::Disk),
        "network" | "net" => Some(PanelType::Network),
        "process" | "proc" | "processes" => Some(PanelType::Process),
        "gpu" => Some(PanelType::Gpu),
        "sensors" | "sensor" => Some(PanelType::Sensors),
        "connections" | "conn" => Some(PanelType::Connections),
        "psi" | "pressure" => Some(PanelType::Psi),
        "files" | "file" => Some(PanelType::Files),
        "battery" | "bat" => Some(PanelType::Battery),
        "containers" | "container" | "docker" => Some(PanelType::Containers),
        _ => {
            eprintln!("[ptop] Unknown panel: {name}. Valid: cpu, memory, disk, network, process, gpu, sensors, connections, psi, files, battery, containers");
            None
        }
    }
}
