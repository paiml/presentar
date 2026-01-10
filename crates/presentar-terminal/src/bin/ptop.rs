//! ptop: Pixel-perfect ttop clone using presentar-terminal
//!
//! Run: cargo run -p presentar-terminal --features ptop --bin ptop

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
use presentar_terminal::ptop::{ui, App};
use presentar_terminal::ColorMode;

/// Presentar System Monitor - pixel-perfect ttop clone
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
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    // Create app BEFORE raw mode (so Ctrl+C works during init)
    let mut app = App::new(cli.deterministic);

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

    let result = run_app(&mut stdout, &mut app, cli.refresh, color_mode);

    // Cleanup
    execute!(stdout, cursor::Show, terminal::LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    result
}

fn run_app(
    stdout: &mut io::Stdout,
    app: &mut App,
    refresh_ms: u64,
    color_mode: ColorMode,
) -> io::Result<()> {
    let mut renderer = DiffRenderer::with_color_mode(color_mode);
    let tick_rate = Duration::from_millis(50);
    let collect_interval = Duration::from_millis(refresh_ms);

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
        ui::draw(app, &mut buffer);

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

        // Handle input
        if event::poll(tick_rate)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press && app.handle_key(key.code, key.modifiers) {
                    break;
                }
            }
        }
    }

    Ok(())
}
