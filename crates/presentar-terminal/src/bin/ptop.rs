//! ptop: System monitor using presentar-terminal widget composition
//!
//! Run: cargo run -p presentar-terminal --features ptop --bin ptop
//!
//! v1: Legacy mode (2800 lines, 83 `draw_text` calls)
//! v2: Widget composition (250 lines, 0 `draw_text` calls) - use --v2 flag

#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::unnecessary_debug_formatting)]

use std::io::{self, Write};
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::{
    cursor, execute,
    terminal::{self, ClearType},
};

use presentar_core::{Rect, Widget};
use presentar_terminal::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use presentar_terminal::ptop::{config::PtopConfig, ui, App, InputHandler, PtopView};
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

    /// Use v2 widget composition mode (default: v1 legacy mode)
    #[arg(long)]
    v2: bool,

    /// Path to custom config file (YAML)
    #[arg(short, long, value_name = "PATH")]
    config: Option<std::path::PathBuf>,

    /// Dump default configuration to stdout and exit
    #[arg(long)]
    dump_config: bool,
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

    // Create app BEFORE raw mode (so Ctrl+C works during init)
    let mut app = App::with_config(cli.deterministic, config);

    // Render-once mode: output single frame to stdout and exit
    if cli.render_once {
        // Must call collect_metrics() to collect system data before rendering
        // Note: CPU usage requires two samples, so we collect twice with a delay
        if !cli.deterministic {
            app.collect_metrics(); // First sample (baseline)
            std::thread::sleep(Duration::from_millis(100));
            app.collect_metrics(); // Second sample (calculates delta)
        }
        return render_once(&app, cli.width, cli.height);
    }

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

    let result = run_app(&mut stdout, &mut app, cli.refresh, color_mode, cli.v2);

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

fn run_app(
    stdout: &mut io::Stdout,
    app: &mut App,
    refresh_ms: u64,
    color_mode: ColorMode,
    use_v2: bool,
) -> io::Result<()> {
    let mut renderer = DiffRenderer::with_color_mode(color_mode);
    let collect_interval = Duration::from_millis(refresh_ms);

    // CB-INPUT-001: Spawn dedicated input thread for sub-50ms latency
    let input_handler = InputHandler::spawn();

    let mut last_collect = Instant::now();
    let mut frame_times: Vec<Duration> = Vec::with_capacity(60);

    // Initial collection
    app.collect_metrics();

    while app.running {
        let frame_start = Instant::now();

        // Get terminal size
        let (width, height) = terminal::size()?;

        // Collect metrics periodically
        if last_collect.elapsed() >= collect_interval {
            app.collect_metrics();
            last_collect = Instant::now();
        }

        // Draw to buffer
        let mut buffer = CellBuffer::new(width, height);

        if use_v2 {
            // v2: Widget composition mode (0 draw_text calls)
            let mut view = PtopView::from_app(app);
            let bounds = Rect::new(0.0, 0.0, f32::from(width), f32::from(height));
            view.layout(bounds);
            let mut canvas = DirectTerminalCanvas::new(&mut buffer);
            view.paint(&mut canvas);
        } else {
            // v1: Legacy mode (83 draw_text calls)
            ui::draw(app, &mut buffer);
        }

        // Render to terminal
        execute!(stdout, cursor::MoveTo(0, 0))?;
        let mut output = Vec::with_capacity(32768);
        renderer.flush(&mut buffer, &mut output)?;
        stdout.write_all(&output)?;
        stdout.flush()?;

        // Track frame time
        let frame_time = frame_start.elapsed();
        frame_times.push(frame_time);
        if frame_times.len() > 60 {
            frame_times.remove(0);
        }
        app.update_frame_stats(&frame_times);

        // CB-INPUT-001: Process all pending input events (non-blocking)
        // This ensures no keystrokes are lost even during slow render cycles
        for timestamped_key in input_handler.drain() {
            if app.handle_key(timestamped_key.event.code, timestamped_key.event.modifiers) {
                // Quit requested
                return Ok(());
            }
        }

        // Sleep briefly to avoid busy-waiting (render loop runs at ~60fps max)
        let elapsed = frame_start.elapsed();
        if elapsed < Duration::from_millis(16) {
            std::thread::sleep(Duration::from_millis(16) - elapsed);
        }
    }

    // InputHandler dropped here, thread exits cleanly (F-INPUT-004)
    Ok(())
}
