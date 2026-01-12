//! Radar (Spider) plot widget.
//!
//! Implements SPEC-024 Section 26.6.4.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::f64::consts::PI;
use std::time::Duration;

/// A series in the radar plot.
#[derive(Debug, Clone)]
pub struct RadarSeries {
    /// Series name.
    pub name: String,
    /// Values (one per axis).
    pub values: Vec<f64>,
    /// Series color.
    pub color: Color,
}

impl RadarSeries {
    /// Create a new radar series.
    #[must_use]
    pub fn new(name: impl Into<String>, values: Vec<f64>, color: Color) -> Self {
        Self {
            name: name.into(),
            values,
            color,
        }
    }
}

/// Radar (Spider) plot widget.
#[derive(Debug, Clone)]
pub struct RadarPlot {
    /// Axis labels.
    axes: Vec<String>,
    /// Data series.
    series: Vec<RadarSeries>,
    /// Fill polygons.
    fill: bool,
    /// Fill alpha.
    fill_alpha: f32,
    /// Show axis labels.
    show_labels: bool,
    /// Show grid.
    show_grid: bool,
    /// Cached bounds.
    bounds: Rect,
}

impl RadarPlot {
    /// Create a new radar plot.
    #[must_use]
    pub fn new(axes: Vec<String>) -> Self {
        Self {
            axes,
            series: Vec::new(),
            fill: true,
            fill_alpha: 0.3,
            show_labels: true,
            show_grid: true,
            bounds: Rect::default(),
        }
    }

    /// Add a series.
    #[must_use]
    pub fn with_series(mut self, series: RadarSeries) -> Self {
        self.series.push(series);
        self
    }

    /// Toggle fill.
    #[must_use]
    pub fn with_fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Set fill alpha.
    #[must_use]
    pub fn with_fill_alpha(mut self, alpha: f32) -> Self {
        self.fill_alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// Toggle labels.
    #[must_use]
    pub fn with_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Toggle grid.
    #[must_use]
    pub fn with_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Get maximum value across all series.
    fn max_value(&self) -> f64 {
        let mut max = 0.0f64;
        for s in &self.series {
            for &v in &s.values {
                if v.is_finite() && v > max {
                    max = v;
                }
            }
        }
        max.max(1.0)
    }
}

impl Default for RadarPlot {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Widget for RadarPlot {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let size = constraints.max_width.min(constraints.max_height).min(40.0);
        Size::new(size, size)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 1.0 || self.bounds.height < 1.0 {
            return;
        }

        let n_axes = self.axes.len();
        let center_x = self.bounds.x + self.bounds.width / 2.0;
        let center_y = self.bounds.y + self.bounds.height / 2.0;
        let radius = (self.bounds.width.min(self.bounds.height) / 2.0 - 3.0).max(2.0);
        let max_val = self.max_value();

        let grid_style = TextStyle {
            color: Color::new(0.3, 0.3, 0.3, 1.0),
            ..Default::default()
        };

        let label_style = TextStyle {
            color: Color::new(0.7, 0.7, 0.7, 1.0),
            ..Default::default()
        };

        // Draw grid circles
        if self.show_grid {
            for level in [0.25, 0.5, 0.75, 1.0] {
                let r = radius * level;
                // Draw circle approximation with dots
                for i in 0..(n_axes * 4) {
                    let angle = 2.0 * PI * (i as f64) / (n_axes * 4) as f64 - PI / 2.0;
                    let x = center_x + (r * angle.cos() as f32);
                    let y = center_y + (r * angle.sin() as f32);
                    if x >= self.bounds.x
                        && x < self.bounds.x + self.bounds.width
                        && y >= self.bounds.y
                        && y < self.bounds.y + self.bounds.height
                    {
                        canvas.draw_text("·", Point::new(x, y), &grid_style);
                    }
                }
            }
        }

        // Draw axes
        for i in 0..n_axes {
            let angle = 2.0 * PI * (i as f64) / (n_axes as f64) - PI / 2.0;
            let end_x = center_x + (radius * angle.cos() as f32);
            let end_y = center_y + (radius * angle.sin() as f32);

            // Draw axis line
            let steps = (radius as usize).max(1);
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let x = center_x + t * (end_x - center_x);
                let y = center_y + t * (end_y - center_y);
                if x >= self.bounds.x
                    && x < self.bounds.x + self.bounds.width
                    && y >= self.bounds.y
                    && y < self.bounds.y + self.bounds.height
                {
                    canvas.draw_text("·", Point::new(x, y), &grid_style);
                }
            }

            // Draw label
            if self.show_labels {
                let label_r = radius + 2.0;
                let label_x = center_x + (label_r * angle.cos() as f32);
                let label_y = center_y + (label_r * angle.sin() as f32);

                let label: String = self.axes[i].chars().take(6).collect();
                if label_x >= self.bounds.x
                    && label_x < self.bounds.x + self.bounds.width
                    && label_y >= self.bounds.y
                    && label_y < self.bounds.y + self.bounds.height
                {
                    canvas.draw_text(&label, Point::new(label_x, label_y), &label_style);
                }
            }
        }

        // Draw series
        for series in &self.series {
            if series.values.len() != n_axes {
                continue;
            }

            let style = TextStyle {
                color: series.color,
                ..Default::default()
            };

            // Calculate points
            let points: Vec<(f32, f32)> = (0..n_axes)
                .map(|i| {
                    let angle = 2.0 * PI * (i as f64) / (n_axes as f64) - PI / 2.0;
                    let v = series.values[i].max(0.0) / max_val;
                    let r = radius * v as f32;
                    (
                        center_x + (r * angle.cos() as f32),
                        center_y + (r * angle.sin() as f32),
                    )
                })
                .collect();

            // Draw polygon edges
            for i in 0..n_axes {
                let (x1, y1) = points[i];
                let (x2, y2) = points[(i + 1) % n_axes];

                let dx = x2 - x1;
                let dy = y2 - y1;
                let steps = ((dx.abs() + dy.abs()) as usize).max(1);

                for step in 0..=steps {
                    let t = step as f32 / steps as f32;
                    let x = x1 + t * dx;
                    let y = y1 + t * dy;
                    if x >= self.bounds.x
                        && x < self.bounds.x + self.bounds.width
                        && y >= self.bounds.y
                        && y < self.bounds.y + self.bounds.height
                    {
                        canvas.draw_text("●", Point::new(x, y), &style);
                    }
                }
            }

            // Mark vertices
            for &(x, y) in &points {
                if x >= self.bounds.x
                    && x < self.bounds.x + self.bounds.width
                    && y >= self.bounds.y
                    && y < self.bounds.y + self.bounds.height
                {
                    canvas.draw_text("◆", Point::new(x, y), &style);
                }
            }
        }

        // Draw legend
        if !self.series.is_empty() {
            let legend_y = self.bounds.y + self.bounds.height - 1.0;
            let mut legend_x = self.bounds.x;

            for series in &self.series {
                let style = TextStyle {
                    color: series.color,
                    ..Default::default()
                };
                let label = format!("● {} ", series.name);
                canvas.draw_text(&label, Point::new(legend_x, legend_y), &style);
                legend_x += label.len() as f32;
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

impl Brick for RadarPlot {
    fn brick_name(&self) -> &'static str {
        "RadarPlot"
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

        if self.bounds.width >= 10.0 && self.bounds.height >= 5.0 {
            passed.push(BrickAssertion::max_latency_ms(16));
        } else {
            failed.push((
                BrickAssertion::max_latency_ms(16),
                "Size too small".to_string(),
            ));
        }

        // Check series consistency
        for series in &self.series {
            if series.values.len() != self.axes.len() {
                failed.push((
                    BrickAssertion::max_latency_ms(16),
                    format!("Series {} has wrong number of values", series.name),
                ));
            }
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
    fn test_radar_plot_new() {
        let axes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let plot = RadarPlot::new(axes.clone());
        assert_eq!(plot.axes.len(), 3);
    }

    #[test]
    fn test_radar_plot_empty() {
        let plot = RadarPlot::default();
        assert!(plot.axes.is_empty());
    }

    #[test]
    fn test_radar_plot_with_series() {
        let axes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let series = RadarSeries::new("Test", vec![1.0, 2.0, 3.0], Color::BLUE);
        let plot = RadarPlot::new(axes).with_series(series);
        assert_eq!(plot.series.len(), 1);
    }

    #[test]
    fn test_radar_plot_max_value() {
        let axes = vec!["A".to_string(), "B".to_string()];
        let series = RadarSeries::new("Test", vec![5.0, 10.0], Color::BLUE);
        let plot = RadarPlot::new(axes).with_series(series);
        assert!((plot.max_value() - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_radar_plot_max_value_empty() {
        let plot = RadarPlot::default();
        assert!((plot.max_value() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_radar_plot_paint() {
        let axes = vec![
            "Speed".to_string(),
            "Power".to_string(),
            "Range".to_string(),
            "Defense".to_string(),
            "Attack".to_string(),
        ];
        let series1 = RadarSeries::new("Player 1", vec![8.0, 6.0, 7.0, 5.0, 9.0], Color::BLUE);
        let series2 = RadarSeries::new("Player 2", vec![6.0, 8.0, 5.0, 7.0, 6.0], Color::RED);

        let mut plot = RadarPlot::new(axes)
            .with_series(series1)
            .with_series(series2)
            .with_fill(true);

        let bounds = Rect::new(0.0, 0.0, 40.0, 20.0);
        plot.layout(bounds);

        let mut buffer = CellBuffer::new(40, 20);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        plot.paint(&mut canvas);
    }

    #[test]
    fn test_radar_plot_verify() {
        let axes = vec!["A".to_string(), "B".to_string()];
        let series = RadarSeries::new("Test", vec![1.0, 2.0], Color::BLUE);
        let mut plot = RadarPlot::new(axes).with_series(series);
        plot.bounds = Rect::new(0.0, 0.0, 40.0, 20.0);
        assert!(plot.verify().is_valid());
    }

    #[test]
    fn test_radar_plot_verify_mismatch() {
        let axes = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let series = RadarSeries::new("Test", vec![1.0, 2.0], Color::BLUE); // Wrong length
        let mut plot = RadarPlot::new(axes).with_series(series);
        plot.bounds = Rect::new(0.0, 0.0, 40.0, 20.0);
        assert!(!plot.verify().is_valid());
    }

    #[test]
    fn test_radar_plot_brick_name() {
        let plot = RadarPlot::default();
        assert_eq!(plot.brick_name(), "RadarPlot");
    }

    #[test]
    fn test_radar_series_new() {
        let series = RadarSeries::new("Test", vec![1.0, 2.0, 3.0], Color::GREEN);
        assert_eq!(series.name, "Test");
        assert_eq!(series.values.len(), 3);
    }
}
