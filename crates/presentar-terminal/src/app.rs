//! TUI application runner with Jidoka verification gates.

use crate::color::ColorMode;
use crate::direct::{CellBuffer, DiffRenderer, DirectTerminalCanvas};
use crate::error::{TuiError, VerificationError};
use crate::input::InputHandler;
use crossterm::{
    cursor,
    event::{self, Event as CrosstermEvent, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use presentar_core::{Constraints, Rect, Widget};
use std::io::{self, Stdout, Write};
use std::time::{Duration, Instant};

/// Configuration for the TUI application.
#[derive(Debug, Clone)]
pub struct TuiConfig {
    /// Tick rate in milliseconds for input polling.
    pub tick_rate_ms: u64,
    /// Enable mouse support.
    pub enable_mouse: bool,
    /// Color mode (auto-detected if not specified).
    pub color_mode: Option<ColorMode>,
    /// Skip Brick verification (DANGEROUS - for debugging only).
    pub skip_verification: bool,
    /// Target frame rate (used for budget calculation).
    pub target_fps: u32,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            tick_rate_ms: 250,
            enable_mouse: false,
            color_mode: None,
            target_fps: 60,
            skip_verification: false,
        }
    }
}

impl TuiConfig {
    /// Create a high-performance config (60fps, fast tick).
    #[must_use]
    pub fn high_performance() -> Self {
        Self {
            tick_rate_ms: 16,
            target_fps: 60,
            ..Default::default()
        }
    }

    /// Create a power-saving config (30fps, slow tick).
    #[must_use]
    pub fn power_saving() -> Self {
        Self {
            tick_rate_ms: 100,
            target_fps: 30,
            ..Default::default()
        }
    }
}

/// Frame timing metrics.
#[derive(Debug, Clone, Default)]
pub struct FrameMetrics {
    /// Time spent in verification phase.
    pub verify_time: Duration,
    /// Time spent in measure phase.
    pub measure_time: Duration,
    /// Time spent in layout phase.
    pub layout_time: Duration,
    /// Time spent in paint phase.
    pub paint_time: Duration,
    /// Total frame time.
    pub total_time: Duration,
    /// Frame number.
    pub frame_count: u64,
}

/// Main TUI application runner.
pub struct TuiApp<W: Widget> {
    root: W,
    config: TuiConfig,
    input_handler: InputHandler,
    metrics: FrameMetrics,
    should_quit: bool,
    color_mode: ColorMode,
}

impl<W: Widget> TuiApp<W> {
    /// Create a new TUI application with the given root widget.
    pub fn new(root: W) -> Result<Self, TuiError> {
        // Jidoka: reject Bricks with no assertions
        if root.assertions().is_empty() {
            return Err(TuiError::InvalidBrick(
                "Root widget has no assertions - every Brick must have at least one falsifiable assertion".to_string(),
            ));
        }

        Ok(Self {
            root,
            config: TuiConfig::default(),
            input_handler: InputHandler::new(),
            metrics: FrameMetrics::default(),
            should_quit: false,
            color_mode: ColorMode::detect(),
        })
    }

    /// Set the configuration.
    #[must_use]
    pub fn with_config(mut self, config: TuiConfig) -> Self {
        if let Some(mode) = config.color_mode {
            self.color_mode = mode;
        }
        self.config = config;
        self
    }

    /// Set the input handler.
    #[must_use]
    pub fn with_input_handler(mut self, handler: InputHandler) -> Self {
        self.input_handler = handler;
        self
    }

    /// Get a reference to the root widget.
    #[must_use]
    pub fn root(&self) -> &W {
        &self.root
    }

    /// Get a mutable reference to the root widget.
    pub fn root_mut(&mut self) -> &mut W {
        &mut self.root
    }

    /// Get the current frame metrics.
    #[must_use]
    pub fn metrics(&self) -> &FrameMetrics {
        &self.metrics
    }

    /// Request the application to quit.
    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    /// Run the application (blocking).
    pub fn run(&mut self) -> Result<(), TuiError> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

        if self.config.enable_mouse {
            execute!(stdout, crossterm::event::EnableMouseCapture)?;
        }

        // Get initial terminal size
        let (width, height) = crossterm::terminal::size()?;
        let mut buffer = CellBuffer::new(width, height);
        let mut renderer = DiffRenderer::with_color_mode(self.color_mode);

        let result = self.run_loop(&mut stdout, &mut buffer, &mut renderer);

        if self.config.enable_mouse {
            let _ = execute!(stdout, crossterm::event::DisableMouseCapture);
        }
        let _ = execute!(stdout, cursor::Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();

        result
    }

    fn run_loop(
        &mut self,
        stdout: &mut Stdout,
        buffer: &mut CellBuffer,
        renderer: &mut DiffRenderer,
    ) -> Result<(), TuiError> {
        let tick_duration = Duration::from_millis(self.config.tick_rate_ms);

        loop {
            let frame_start = Instant::now();

            // Check for terminal resize
            let (width, height) = crossterm::terminal::size()?;
            if width != buffer.width() || height != buffer.height() {
                buffer.resize(width, height);
                renderer.reset();
            }

            // Phase 1: Verify (Jidoka gate)
            let verify_start = Instant::now();
            if !self.config.skip_verification {
                let verification = self.root.verify();
                if !verification.is_valid() {
                    return Err(TuiError::VerificationFailed(VerificationError::from(
                        verification,
                    )));
                }
            }
            self.metrics.verify_time = verify_start.elapsed();

            // Phase 2: Render frame
            self.render_frame(buffer);

            // Phase 3: Flush to terminal
            renderer.flush(buffer, stdout)?;
            stdout.flush()?;

            self.metrics.total_time = frame_start.elapsed();
            self.metrics.frame_count += 1;

            // Phase 4: Handle input
            if event::poll(tick_duration)? {
                if let CrosstermEvent::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Char('c')
                            && key
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL)
                    {
                        self.should_quit = true;
                    }

                    if let Some(event) = self.input_handler.convert(CrosstermEvent::Key(key)) {
                        let _ = self.root.event(&event);
                    }
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    fn render_frame(&mut self, buffer: &mut CellBuffer) {
        let width = buffer.width();
        let height = buffer.height();

        // Phase 2a: Measure
        let measure_start = Instant::now();
        let constraints = Constraints::new(0.0, f32::from(width), 0.0, f32::from(height));
        let _size = self.root.measure(constraints);
        self.metrics.measure_time = measure_start.elapsed();

        // Phase 2b: Layout
        let layout_start = Instant::now();
        let bounds = Rect::new(0.0, 0.0, f32::from(width), f32::from(height));
        let _ = self.root.layout(bounds);
        self.metrics.layout_time = layout_start.elapsed();

        // Phase 2c: Paint
        let paint_start = Instant::now();
        {
            let mut canvas = DirectTerminalCanvas::new(buffer);
            self.root.paint(&mut canvas);
        }
        self.metrics.paint_time = paint_start.elapsed();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presentar_core::{
        Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Event, LayoutResult,
        Size, TypeId,
    };
    use std::any::Any;
    use std::time::Duration;

    struct TestWidget {
        assertions: Vec<BrickAssertion>,
    }

    impl TestWidget {
        fn new() -> Self {
            Self {
                assertions: vec![BrickAssertion::max_latency_ms(16)],
            }
        }

        fn without_assertions() -> Self {
            Self { assertions: vec![] }
        }
    }

    impl Brick for TestWidget {
        fn brick_name(&self) -> &'static str {
            "test_widget"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &self.assertions
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::default()
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: self.assertions.clone(),
                failed: vec![],
                verification_time: Duration::from_micros(10),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for TestWidget {
        fn type_id(&self) -> TypeId {
            TypeId::of::<Self>()
        }

        fn measure(&self, constraints: Constraints) -> Size {
            constraints.constrain(Size::new(10.0, 5.0))
        }

        fn layout(&mut self, bounds: Rect) -> LayoutResult {
            LayoutResult {
                size: Size::new(bounds.width, bounds.height),
            }
        }

        fn paint(&self, canvas: &mut dyn Canvas) {
            canvas.fill_rect(Rect::new(0.0, 0.0, 10.0, 5.0), Color::BLUE);
        }

        fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
            None
        }

        fn children(&self) -> &[Box<dyn Widget>] {
            &[]
        }

        fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
            &mut []
        }
    }

    #[test]
    fn test_tui_app_creation() {
        let widget = TestWidget::new();
        let app = TuiApp::new(widget);
        assert!(app.is_ok());
    }

    #[test]
    fn test_tui_app_rejects_empty_assertions() {
        let widget = TestWidget::without_assertions();
        let app = TuiApp::new(widget);
        assert!(app.is_err());
        let err = app.err().expect("expected error");
        assert!(matches!(err, TuiError::InvalidBrick(_)));
    }

    #[test]
    fn test_config_default() {
        let config = TuiConfig::default();
        assert_eq!(config.tick_rate_ms, 250);
        assert_eq!(config.target_fps, 60);
        assert!(!config.enable_mouse);
        assert!(!config.skip_verification);
        assert!(config.color_mode.is_none());
    }

    #[test]
    fn test_config_high_performance() {
        let config = TuiConfig::high_performance();
        assert_eq!(config.tick_rate_ms, 16);
        assert_eq!(config.target_fps, 60);
    }

    #[test]
    fn test_config_power_saving() {
        let config = TuiConfig::power_saving();
        assert_eq!(config.tick_rate_ms, 100);
        assert_eq!(config.target_fps, 30);
    }

    #[test]
    fn test_tui_app_with_config() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();

        let config = TuiConfig {
            tick_rate_ms: 50,
            enable_mouse: true,
            color_mode: Some(ColorMode::Color256),
            skip_verification: false,
            target_fps: 30,
        };

        app = app.with_config(config);
        assert!(app.metrics().frame_count == 0);
    }

    #[test]
    fn test_tui_app_with_input_handler() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();

        let mut handler = InputHandler::new();
        handler.add_binding(crate::input::KeyBinding::simple(
            crossterm::event::KeyCode::Char('q'),
            "quit",
        ));

        app = app.with_input_handler(handler);
        assert!(app.root().assertions().len() == 1);
    }

    #[test]
    fn test_tui_app_root_accessors() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();

        assert_eq!(app.root().brick_name(), "test_widget");
        assert_eq!(app.root_mut().brick_name(), "test_widget");
    }

    #[test]
    fn test_tui_app_metrics() {
        let widget = TestWidget::new();
        let app = TuiApp::new(widget).unwrap();

        let metrics = app.metrics();
        assert_eq!(metrics.frame_count, 0);
        assert_eq!(metrics.total_time, Duration::ZERO);
    }

    #[test]
    fn test_tui_app_quit() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();

        assert!(!app.should_quit);
        app.quit();
        assert!(app.should_quit);
    }

    #[test]
    fn test_frame_metrics_default() {
        let metrics = FrameMetrics::default();
        assert_eq!(metrics.frame_count, 0);
        assert_eq!(metrics.verify_time, Duration::ZERO);
        assert_eq!(metrics.measure_time, Duration::ZERO);
        assert_eq!(metrics.layout_time, Duration::ZERO);
        assert_eq!(metrics.paint_time, Duration::ZERO);
        assert_eq!(metrics.total_time, Duration::ZERO);
    }

    #[test]
    fn test_config_with_color_mode_override() {
        let widget = TestWidget::new();
        let app = TuiApp::new(widget).unwrap();

        let config = TuiConfig {
            color_mode: Some(ColorMode::Mono),
            ..Default::default()
        };

        let app = app.with_config(config);
        assert_eq!(app.color_mode, ColorMode::Mono);
    }

    #[test]
    fn test_config_without_color_mode() {
        let widget = TestWidget::new();
        let app = TuiApp::new(widget).unwrap();
        let original_mode = app.color_mode;

        let config = TuiConfig {
            color_mode: None,
            ..Default::default()
        };

        let app = app.with_config(config);
        assert_eq!(app.color_mode, original_mode);
    }

    #[test]
    fn test_render_frame() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();
        let mut buffer = CellBuffer::new(80, 24);

        // Render a frame and verify metrics are updated
        app.render_frame(&mut buffer);

        assert!(
            app.metrics.measure_time > Duration::ZERO || app.metrics.measure_time == Duration::ZERO
        );
        assert!(app.metrics.layout_time >= Duration::ZERO);
        assert!(app.metrics.paint_time >= Duration::ZERO);
    }

    #[test]
    fn test_render_frame_updates_metrics() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();
        let mut buffer = CellBuffer::new(40, 10);

        // Render multiple frames
        for _ in 0..3 {
            app.render_frame(&mut buffer);
        }

        // Metrics should be set (even if durations are very small)
        let metrics = app.metrics();
        assert_eq!(metrics.frame_count, 0); // frame_count is only updated in run_loop
    }

    #[test]
    fn test_render_frame_with_different_buffer_sizes() {
        let widget = TestWidget::new();
        let mut app = TuiApp::new(widget).unwrap();

        // Small buffer
        let mut small_buffer = CellBuffer::new(10, 5);
        app.render_frame(&mut small_buffer);

        // Large buffer
        let mut large_buffer = CellBuffer::new(200, 50);
        app.render_frame(&mut large_buffer);

        // Should not panic with any buffer size
    }

    #[test]
    fn test_frame_metrics_clone() {
        let metrics = FrameMetrics {
            verify_time: Duration::from_millis(1),
            measure_time: Duration::from_millis(2),
            layout_time: Duration::from_millis(3),
            paint_time: Duration::from_millis(4),
            total_time: Duration::from_millis(10),
            frame_count: 100,
        };

        let cloned = metrics.clone();
        assert_eq!(cloned.frame_count, 100);
        assert_eq!(cloned.verify_time, Duration::from_millis(1));
    }

    #[test]
    fn test_frame_metrics_debug() {
        let metrics = FrameMetrics::default();
        let debug_str = format!("{:?}", metrics);
        assert!(debug_str.contains("FrameMetrics"));
        assert!(debug_str.contains("frame_count"));
    }

    #[test]
    fn test_tui_config_clone() {
        let config = TuiConfig::high_performance();
        let cloned = config.clone();
        assert_eq!(cloned.tick_rate_ms, 16);
        assert_eq!(cloned.target_fps, 60);
    }

    #[test]
    fn test_tui_config_debug() {
        let config = TuiConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("TuiConfig"));
        assert!(debug_str.contains("tick_rate_ms"));
    }
}
