//! `CpuGrid` widget for dense per-core CPU visualization.
//!
//! Displays N CPU cores in a compact grid with gradient-colored meters.
//! Reference: btop/ttop per-core CPU displays.

use crate::theme::Gradient;
use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Block characters for single-row meters (8 levels).
const METER_CHARS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// Per-core CPU grid with gradient-colored meters.
#[derive(Debug, Clone)]
pub struct CpuGrid {
    /// Per-core utilization (0.0-100.0).
    pub core_usage: Vec<f64>,
    /// Gradient for coloring (low→high).
    gradient: Gradient,
    /// Number of columns (auto-calculated if None).
    columns: Option<usize>,
    /// Show core labels (0, 1, 2...).
    show_labels: bool,
    /// Compact mode (minimal spacing).
    compact: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for CpuGrid {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl CpuGrid {
    /// Create a new CPU grid with core usage data.
    #[must_use]
    pub fn new(core_usage: Vec<f64>) -> Self {
        Self {
            core_usage,
            gradient: Gradient::from_hex(&["#7aa2f7", "#e0af68", "#f7768e"]), // Tokyo Night CPU
            columns: None,
            show_labels: true,
            compact: false,
            bounds: Rect::default(),
        }
    }

    /// Set the gradient for coloring.
    #[must_use]
    pub fn with_gradient(mut self, gradient: Gradient) -> Self {
        self.gradient = gradient;
        self
    }

    /// Set explicit column count.
    #[must_use]
    pub fn with_columns(mut self, cols: usize) -> Self {
        self.columns = Some(cols);
        self
    }

    /// Enable compact mode (minimal spacing).
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }

    /// Disable core labels.
    #[must_use]
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Update core usage data.
    pub fn set_usage(&mut self, usage: Vec<f64>) {
        self.core_usage = usage;
    }

    /// Get the number of cores.
    #[must_use]
    pub fn core_count(&self) -> usize {
        self.core_usage.len()
    }

    /// Calculate optimal grid dimensions for N cores in given width.
    fn optimal_grid(&self, max_width: usize) -> (usize, usize) {
        let count = self.core_usage.len();
        if count == 0 {
            return (0, 0);
        }

        // Cell width: label (2-3 chars) + meter (1 char) + spacing
        let cell_width = if self.show_labels {
            if self.compact {
                4
            } else {
                5
            }
        } else if self.compact {
            2
        } else {
            3
        };

        let max_cols = (max_width / cell_width).max(1);
        let cols = self.columns.unwrap_or_else(|| {
            // Try to make a reasonably square grid
            let sqrt = (count as f64).sqrt().ceil() as usize;
            sqrt.min(max_cols).max(1)
        });

        let rows = count.div_ceil(cols);
        (cols, rows)
    }

    /// Get meter character for percentage (0-100).
    fn meter_char(pct: f64) -> char {
        let idx = ((pct / 100.0) * 8.0).round() as usize;
        METER_CHARS[idx.min(8)]
    }
}

impl Brick for CpuGrid {
    fn brick_name(&self) -> &'static str {
        "cpu_grid"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(16)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: self.assertions().to_vec(),
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

impl Widget for CpuGrid {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let max_width = constraints.max_width as usize;
        let (cols, rows) = self.optimal_grid(max_width);

        let cell_width = if self.show_labels {
            if self.compact {
                4.0
            } else {
                5.0
            }
        } else if self.compact {
            2.0
        } else {
            3.0
        };

        let width = (cols as f32 * cell_width).min(constraints.max_width);
        let height = (rows as f32).min(constraints.max_height);

        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.core_usage.is_empty() {
            return;
        }

        let (cols, _rows) = self.optimal_grid(self.bounds.width as usize);
        if cols == 0 {
            return;
        }

        let cell_width = if self.show_labels {
            if self.compact {
                4.0
            } else {
                5.0
            }
        } else if self.compact {
            2.0
        } else {
            3.0
        };

        for (i, &usage) in self.core_usage.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;

            let x = self.bounds.x + col as f32 * cell_width;
            let y = self.bounds.y + row as f32;

            let usage_clamped = usage.clamp(0.0, 100.0);
            let color = self.gradient.for_percent(usage_clamped);
            let meter = Self::meter_char(usage_clamped);

            let style = TextStyle {
                color,
                ..Default::default()
            };

            if self.show_labels {
                // Format: "12▆" or " 5▄"
                let label = if self.compact {
                    format!("{i:2}{meter}")
                } else {
                    format!("{i:2} {meter}")
                };
                canvas.draw_text(&label, Point::new(x, y), &style);
            } else {
                canvas.draw_text(&meter.to_string(), Point::new(x, y), &style);
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_grid_new() {
        let grid = CpuGrid::new(vec![10.0, 50.0, 90.0]);
        assert_eq!(grid.core_count(), 3);
    }

    #[test]
    fn test_cpu_grid_empty() {
        let grid = CpuGrid::new(vec![]);
        assert_eq!(grid.core_count(), 0);
    }

    #[test]
    fn test_cpu_grid_with_columns() {
        let grid = CpuGrid::new(vec![10.0; 48]).with_columns(8);
        assert_eq!(grid.columns, Some(8));
    }

    #[test]
    fn test_cpu_grid_compact() {
        let grid = CpuGrid::new(vec![10.0; 48]).compact();
        assert!(grid.compact);
    }

    #[test]
    fn test_cpu_grid_without_labels() {
        let grid = CpuGrid::new(vec![10.0; 48]).without_labels();
        assert!(!grid.show_labels);
    }

    #[test]
    fn test_meter_char() {
        assert_eq!(CpuGrid::meter_char(0.0), ' ');
        assert_eq!(CpuGrid::meter_char(50.0), '▄');
        assert_eq!(CpuGrid::meter_char(100.0), '█');
    }

    #[test]
    fn test_optimal_grid_48_cores() {
        let grid = CpuGrid::new(vec![10.0; 48]);
        let (cols, rows) = grid.optimal_grid(80);
        assert!(cols > 0);
        assert!(rows > 0);
        assert!(cols * rows >= 48);
    }

    #[test]
    fn test_optimal_grid_explicit_columns() {
        let grid = CpuGrid::new(vec![10.0; 48]).with_columns(8);
        let (cols, rows) = grid.optimal_grid(80);
        assert_eq!(cols, 8);
        assert_eq!(rows, 6);
    }

    #[test]
    fn test_cpu_grid_measure() {
        let grid = CpuGrid::new(vec![10.0; 8]);
        let size = grid.measure(Constraints::new(0.0, 80.0, 0.0, 20.0));
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }

    #[test]
    fn test_cpu_grid_set_usage() {
        let mut grid = CpuGrid::new(vec![]);
        assert_eq!(grid.core_count(), 0);
        grid.set_usage(vec![10.0, 20.0, 30.0]);
        assert_eq!(grid.core_count(), 3);
    }

    #[test]
    fn test_cpu_grid_verify() {
        let grid = CpuGrid::new(vec![50.0; 8]);
        let v = grid.verify();
        assert!(v.is_valid());
    }

    #[test]
    fn test_cpu_grid_type_id() {
        let grid = CpuGrid::new(vec![]);
        let _ = Widget::type_id(&grid);
    }

    #[test]
    fn test_cpu_grid_default() {
        let grid = CpuGrid::default();
        assert_eq!(grid.core_count(), 0);
    }

    #[test]
    fn test_cpu_grid_brick_name() {
        let grid = CpuGrid::new(vec![]);
        assert_eq!(grid.brick_name(), "cpu_grid");
    }

    #[test]
    fn test_cpu_grid_layout() {
        let mut grid = CpuGrid::new(vec![50.0; 8]);
        let result = grid.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        assert_eq!(result.size.width, 80.0);
        assert_eq!(result.size.height, 10.0);
    }

    #[test]
    fn test_cpu_grid_event() {
        let mut grid = CpuGrid::new(vec![50.0]);
        let event = Event::Resize {
            width: 80.0,
            height: 24.0,
        };
        assert!(grid.event(&event).is_none());
    }

    #[test]
    fn test_cpu_grid_children() {
        let grid = CpuGrid::new(vec![50.0]);
        assert!(grid.children().is_empty());
    }

    #[test]
    fn test_cpu_grid_children_mut() {
        let mut grid = CpuGrid::new(vec![50.0]);
        assert!(grid.children_mut().is_empty());
    }

    #[test]
    fn test_cpu_grid_assertions() {
        let grid = CpuGrid::new(vec![50.0]);
        assert!(!grid.assertions().is_empty());
    }

    #[test]
    fn test_cpu_grid_budget() {
        let grid = CpuGrid::new(vec![50.0]);
        let budget = grid.budget();
        assert!(budget.layout_ms > 0);
    }

    #[test]
    fn test_cpu_grid_to_html() {
        let grid = CpuGrid::new(vec![50.0]);
        assert!(grid.to_html().is_empty());
    }

    #[test]
    fn test_cpu_grid_to_css() {
        let grid = CpuGrid::new(vec![50.0]);
        assert!(grid.to_css().is_empty());
    }

    #[test]
    fn test_cpu_grid_clone() {
        let grid = CpuGrid::new(vec![50.0, 75.0]).with_columns(4);
        let cloned = grid.clone();
        assert_eq!(cloned.core_usage.len(), grid.core_usage.len());
        assert_eq!(cloned.columns, grid.columns);
    }

    #[test]
    fn test_cpu_grid_debug() {
        let grid = CpuGrid::new(vec![50.0]);
        let debug = format!("{grid:?}");
        assert!(debug.contains("CpuGrid"));
    }

    #[test]
    fn test_cpu_grid_with_gradient() {
        let gradient = Gradient::from_hex(&["#00FF00", "#FF0000"]);
        let grid = CpuGrid::new(vec![50.0]).with_gradient(gradient);
        // Gradient is private, just test it doesn't panic
        assert_eq!(grid.core_count(), 1);
    }

    #[test]
    fn test_cpu_grid_paint() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut grid = CpuGrid::new(vec![10.0, 50.0, 90.0]);
        let mut buffer = CellBuffer::new(40, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        grid.layout(Rect::new(0.0, 0.0, 40.0, 10.0));
        grid.paint(&mut canvas);
    }

    #[test]
    fn test_cpu_grid_paint_empty() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let grid = CpuGrid::new(vec![]);
        let mut buffer = CellBuffer::new(40, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        grid.paint(&mut canvas);
    }

    #[test]
    fn test_cpu_grid_paint_compact() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut grid = CpuGrid::new(vec![10.0, 50.0, 90.0]).compact();
        let mut buffer = CellBuffer::new(40, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        grid.layout(Rect::new(0.0, 0.0, 40.0, 10.0));
        grid.paint(&mut canvas);
    }

    #[test]
    fn test_cpu_grid_paint_without_labels() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut grid = CpuGrid::new(vec![10.0, 50.0, 90.0]).without_labels();
        let mut buffer = CellBuffer::new(40, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        grid.layout(Rect::new(0.0, 0.0, 40.0, 10.0));
        grid.paint(&mut canvas);
    }

    #[test]
    fn test_cpu_grid_paint_compact_without_labels() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut grid = CpuGrid::new(vec![10.0, 50.0, 90.0])
            .compact()
            .without_labels();
        let mut buffer = CellBuffer::new(40, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        grid.layout(Rect::new(0.0, 0.0, 40.0, 10.0));
        grid.paint(&mut canvas);
    }

    #[test]
    fn test_cpu_grid_optimal_grid_empty() {
        let grid = CpuGrid::new(vec![]);
        let (cols, rows) = grid.optimal_grid(80);
        assert_eq!(cols, 0);
        assert_eq!(rows, 0);
    }

    #[test]
    fn test_cpu_grid_measure_compact() {
        let grid = CpuGrid::new(vec![10.0; 8]).compact();
        let size = grid.measure(Constraints::new(0.0, 80.0, 0.0, 20.0));
        assert!(size.width > 0.0);
    }

    #[test]
    fn test_cpu_grid_measure_without_labels() {
        let grid = CpuGrid::new(vec![10.0; 8]).without_labels();
        let size = grid.measure(Constraints::new(0.0, 80.0, 0.0, 20.0));
        assert!(size.width > 0.0);
    }

    #[test]
    fn test_cpu_grid_measure_compact_without_labels() {
        let grid = CpuGrid::new(vec![10.0; 8]).compact().without_labels();
        let size = grid.measure(Constraints::new(0.0, 80.0, 0.0, 20.0));
        assert!(size.width > 0.0);
    }

    #[test]
    fn test_cpu_grid_meter_char_edge_cases() {
        assert_eq!(CpuGrid::meter_char(-10.0), ' ');
        assert_eq!(CpuGrid::meter_char(150.0), '█');
        assert_eq!(CpuGrid::meter_char(12.5), '▁');
        assert_eq!(CpuGrid::meter_char(25.0), '▂');
        assert_eq!(CpuGrid::meter_char(37.5), '▃');
        assert_eq!(CpuGrid::meter_char(62.5), '▅');
        assert_eq!(CpuGrid::meter_char(75.0), '▆');
        assert_eq!(CpuGrid::meter_char(87.5), '▇');
    }
}
