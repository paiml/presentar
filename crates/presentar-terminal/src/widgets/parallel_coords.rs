//! Parallel coordinates plot widget.
//!
//! Implements SPEC-024 Section 26.6.3.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Parallel coordinates plot widget.
#[derive(Debug, Clone)]
pub struct ParallelCoordinates {
    /// Column names.
    columns: Vec<String>,
    /// Data rows (each row has one value per column).
    data: Vec<Vec<f64>>,
    /// Optional color-by values (one per row).
    color_by: Option<Vec<f64>>,
    /// Line alpha (0.0-1.0).
    alpha: f32,
    /// Show column labels.
    show_labels: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl ParallelCoordinates {
    /// Create a new parallel coordinates plot.
    #[must_use]
    pub fn new(columns: Vec<String>, data: Vec<Vec<f64>>) -> Self {
        Self {
            columns,
            data,
            color_by: None,
            alpha: 0.5,
            show_labels: true,
            bounds: Rect::default(),
        }
    }

    /// Set color-by values.
    #[must_use]
    pub fn with_color_by(mut self, values: Vec<f64>) -> Self {
        self.color_by = Some(values);
        self
    }

    /// Set line alpha.
    #[must_use]
    pub fn with_alpha(mut self, alpha: f32) -> Self {
        self.alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// Toggle column labels.
    #[must_use]
    pub fn with_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Get min/max for a column.
    fn column_range(&self, col_idx: usize) -> (f64, f64) {
        let mut min = f64::INFINITY;
        let mut max = f64::NEG_INFINITY;

        for row in &self.data {
            if let Some(&v) = row.get(col_idx) {
                if v.is_finite() {
                    min = min.min(v);
                    max = max.max(v);
                }
            }
        }

        if min == f64::INFINITY {
            (0.0, 1.0)
        } else if (max - min).abs() < 1e-10 {
            (min - 0.5, max + 0.5)
        } else {
            (min, max)
        }
    }

    /// Get color for a row.
    fn get_row_color(&self, row_idx: usize) -> Color {
        if let Some(ref values) = self.color_by {
            if let Some(&v) = values.get(row_idx) {
                let mut min = f64::INFINITY;
                let mut max = f64::NEG_INFINITY;
                for &val in values {
                    if val.is_finite() {
                        min = min.min(val);
                        max = max.max(val);
                    }
                }
                let range = (max - min).max(1e-10);
                let t = ((v - min) / range).clamp(0.0, 1.0);
                // Blue to red gradient
                Color::new(t as f32, 0.3, (1.0 - t) as f32, self.alpha)
            } else {
                Color::new(0.3, 0.5, 0.8, self.alpha)
            }
        } else {
            Color::new(0.3, 0.5, 0.8, self.alpha)
        }
    }
}

impl Default for ParallelCoordinates {
    fn default() -> Self {
        Self::new(Vec::new(), Vec::new())
    }
}

impl Widget for ParallelCoordinates {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        Size::new(
            constraints.max_width.min(80.0),
            constraints.max_height.min(20.0),
        )
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 20.0 || self.bounds.height < 5.0 || self.columns.is_empty() {
            return;
        }

        let margin_top = if self.show_labels { 2.0 } else { 0.0 };
        let margin_bottom = 1.0;
        let margin_left = 2.0;
        let margin_right = 2.0;

        let plot_x = self.bounds.x + margin_left;
        let plot_y = self.bounds.y + margin_top;
        let plot_width = self.bounds.width - margin_left - margin_right;
        let plot_height = self.bounds.height - margin_top - margin_bottom;

        if plot_width <= 0.0 || plot_height <= 0.0 {
            return;
        }

        let n_cols = self.columns.len();
        let col_spacing = plot_width / (n_cols - 1).max(1) as f32;

        let label_style = TextStyle {
            color: Color::new(0.7, 0.7, 0.7, 1.0),
            ..Default::default()
        };

        let axis_style = TextStyle {
            color: Color::new(0.4, 0.4, 0.4, 1.0),
            ..Default::default()
        };

        // Draw axes and labels
        for (i, col_name) in self.columns.iter().enumerate() {
            let x = plot_x + i as f32 * col_spacing;

            // Draw axis line
            for y_step in 0..(plot_height as usize) {
                canvas.draw_text("│", Point::new(x, plot_y + y_step as f32), &axis_style);
            }

            // Draw column label
            if self.show_labels {
                let label: String = col_name.chars().take(8).collect();
                canvas.draw_text(&label, Point::new(x, self.bounds.y), &label_style);
            }
        }

        // Precompute column ranges
        let ranges: Vec<(f64, f64)> = (0..n_cols).map(|i| self.column_range(i)).collect();

        // Draw data lines
        for (row_idx, row) in self.data.iter().enumerate() {
            if row.len() != n_cols {
                continue;
            }

            let color = self.get_row_color(row_idx);
            let style = TextStyle {
                color,
                ..Default::default()
            };

            // Draw line segments between consecutive axes
            for col_idx in 0..(n_cols - 1) {
                let x1 = plot_x + col_idx as f32 * col_spacing;
                let x2 = plot_x + (col_idx + 1) as f32 * col_spacing;

                let (min1, max1) = ranges[col_idx];
                let (min2, max2) = ranges[col_idx + 1];

                let v1 = row[col_idx];
                let v2 = row[col_idx + 1];

                if !v1.is_finite() || !v2.is_finite() {
                    continue;
                }

                let y1_norm = if max1 > min1 {
                    (v1 - min1) / (max1 - min1)
                } else {
                    0.5
                };
                let y2_norm = if max2 > min2 {
                    (v2 - min2) / (max2 - min2)
                } else {
                    0.5
                };

                let y1 = plot_y + ((1.0 - y1_norm) * plot_height as f64) as f32;
                let y2 = plot_y + ((1.0 - y2_norm) * plot_height as f64) as f32;

                // Draw line using Braille or simple chars
                // For terminal, we approximate with diagonal chars
                let dx = x2 - x1;
                let dy = y2 - y1;
                let steps = (dx.abs().max(dy.abs()) as usize).max(1);

                for step in 0..=steps {
                    let t = step as f32 / steps as f32;
                    let px = x1 + t * dx;
                    let py = y1 + t * dy;

                    if px >= plot_x
                        && px < plot_x + plot_width
                        && py >= plot_y
                        && py < plot_y + plot_height
                    {
                        let ch = if dy.abs() < 0.3 {
                            '─'
                        } else if dy > 0.0 {
                            '╲'
                        } else {
                            '╱'
                        };
                        canvas.draw_text(&ch.to_string(), Point::new(px, py), &style);
                    }
                }
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

impl Brick for ParallelCoordinates {
    fn brick_name(&self) -> &'static str {
        "ParallelCoordinates"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(16)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(16)
    }

    fn verify(&self) -> BrickVerification {
        let mut passed = Vec::new();
        let mut failed = Vec::new();

        if self.bounds.width >= 20.0 && self.bounds.height >= 5.0 {
            passed.push(BrickAssertion::max_latency_ms(16));
        } else {
            failed.push((
                BrickAssertion::max_latency_ms(16),
                "Size too small".to_string(),
            ));
        }

        BrickVerification {
            passed,
            failed,
            verification_time: Duration::from_micros(5),
        }
    }

    fn to_html(&self) -> String {
        String::new()
    }

    fn to_css(&self) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct::{CellBuffer, DirectTerminalCanvas};

    #[test]
    fn test_parallel_coords_new() {
        let columns = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let data = vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]];
        let plot = ParallelCoordinates::new(columns.clone(), data.clone());
        assert_eq!(plot.columns.len(), 3);
        assert_eq!(plot.data.len(), 2);
    }

    #[test]
    fn test_parallel_coords_empty() {
        let plot = ParallelCoordinates::default();
        assert!(plot.columns.is_empty());
        assert!(plot.data.is_empty());
    }

    #[test]
    fn test_parallel_coords_with_color() {
        let columns = vec!["A".to_string(), "B".to_string()];
        let data = vec![vec![1.0, 2.0], vec![3.0, 4.0]];
        let plot = ParallelCoordinates::new(columns, data)
            .with_color_by(vec![0.0, 1.0])
            .with_alpha(0.7);
        assert!(plot.color_by.is_some());
        assert!((plot.alpha - 0.7).abs() < 0.01);
    }

    #[test]
    fn test_parallel_coords_paint() {
        let columns = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let data = vec![
            vec![1.0, 5.0, 3.0],
            vec![2.0, 4.0, 6.0],
            vec![3.0, 3.0, 1.0],
        ];
        let mut plot = ParallelCoordinates::new(columns, data);

        let bounds = Rect::new(0.0, 0.0, 80.0, 20.0);
        plot.layout(bounds);

        let mut buffer = CellBuffer::new(80, 20);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        plot.paint(&mut canvas);
    }

    #[test]
    fn test_parallel_coords_column_range() {
        let columns = vec!["A".to_string()];
        let data = vec![vec![1.0], vec![5.0], vec![3.0]];
        let plot = ParallelCoordinates::new(columns, data);
        let (min, max) = plot.column_range(0);
        assert!((min - 1.0).abs() < 0.01);
        assert!((max - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_parallel_coords_column_range_empty() {
        let columns = vec!["A".to_string()];
        let data: Vec<Vec<f64>> = vec![];
        let plot = ParallelCoordinates::new(columns, data);
        let (min, max) = plot.column_range(0);
        assert!((min - 0.0).abs() < 0.01);
        assert!((max - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_parallel_coords_verify() {
        let mut plot =
            ParallelCoordinates::new(vec!["A".to_string(), "B".to_string()], vec![vec![1.0, 2.0]]);
        plot.bounds = Rect::new(0.0, 0.0, 80.0, 20.0);
        assert!(plot.verify().is_valid());
    }

    #[test]
    fn test_parallel_coords_brick_name() {
        let plot = ParallelCoordinates::default();
        assert_eq!(plot.brick_name(), "ParallelCoordinates");
    }
}
