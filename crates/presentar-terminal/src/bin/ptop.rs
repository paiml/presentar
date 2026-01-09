use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use presentar_core::Rect;
use presentar_terminal::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use presentar_terminal::ptop::{app::App, ui};
use std::{error::Error, io, time::Duration};

fn main() -> Result<(), Box<dyn Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    // Run app
    let res = run_app();

    // Restore terminal
    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen, DisableMouseCapture)?;

    if let Err(e) = res {
        println!("Error: {e}");
    }

    Ok(())
}

fn run_app() -> Result<(), Box<dyn Error>> {
    let mut app = App::new();
    let (cols, rows) = crossterm::terminal::size()?;
    let mut buffer = CellBuffer::new(cols, rows);
    let mut renderer = DiffRenderer::new();

    // Initial draw
    let mut stdout = io::stdout();

    loop {
        // Handle resizing
        let (new_cols, new_rows) = crossterm::terminal::size()?;
        if new_cols != buffer.width() || new_rows != buffer.height() {
            buffer = CellBuffer::new(new_cols, new_rows);
            renderer.clear(); // Force full redraw
        }

        // Draw
        {
            let mut canvas = DirectTerminalCanvas::new(&mut buffer);
            // Clear background
            canvas.fill_rect(
                Rect::new(0.0, 0.0, new_cols as f32, new_rows as f32),
                presentar_core::Color::new(0.02, 0.02, 0.05, 1.0),
            );

            ui::draw(
                &mut app,
                &mut canvas,
                Rect::new(0.0, 0.0, new_cols as f32, new_rows as f32),
            );
        }

        // Render to stdout
        let mut output =
            Vec::with_capacity(buffer.width() as usize * buffer.height() as usize * 10);
        renderer.flush(&mut buffer, &mut output)?;
        use std::io::Write;
        stdout.write_all(&output)?;
        stdout.flush()?;

        // Input handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char(c) = key.code {
                    app.on_key(c);
                }
            }
        }

        // Update state
        app.on_tick();

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
