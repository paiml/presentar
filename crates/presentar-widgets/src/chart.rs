//! `Chart` widget for data visualization.

use presentar_core::{
    widget::{AccessibleRole, LayoutResult, TextStyle},
    Canvas, Color, Constraints, Point, Rect, Size, TypeId, Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Chart type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ChartType {
    /// Line chart
    #[default]
    Line,
    /// Bar chart
    Bar,
    /// Scatter plot
    Scatter,
    /// Area chart
    Area,
    /// Pie chart
    Pie,
    /// Histogram
    Histogram,
    /// Heatmap - displays matrix data with color encoding
    Heatmap,
    /// Box plot - displays statistical distributions
    BoxPlot,
}

/// A single data series for the chart.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DataSeries {
    /// Series name/label
    pub name: String,
    /// Data points (x, y)
    pub points: Vec<(f64, f64)>,
    /// Series color
    pub color: Color,
    /// Line width (for line/area charts)
    pub line_width: f32,
    /// Point size (for scatter/line charts)
    pub point_size: f32,
    /// Whether to show points
    pub show_points: bool,
    /// Whether to fill area under line
    pub fill: bool,
}

impl DataSeries {
    /// Create a new data series.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            points: Vec::new(),
            color: Color::new(0.2, 0.47, 0.96, 1.0),
            line_width: 2.0,
            point_size: 4.0,
            show_points: true,
            fill: false,
        }
    }

    /// Add a data point.
    #[must_use]
    pub fn point(mut self, x: f64, y: f64) -> Self {
        self.points.push((x, y));
        self
    }

    /// Add multiple data points.
    #[must_use]
    pub fn points(mut self, points: impl IntoIterator<Item = (f64, f64)>) -> Self {
        self.points.extend(points);
        self
    }

    /// Set series color.
    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set line width.
    #[must_use]
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = width.max(0.5);
        self
    }

    /// Set point size.
    #[must_use]
    pub fn point_size(mut self, size: f32) -> Self {
        self.point_size = size.max(1.0);
        self
    }

    /// Set whether to show points.
    #[must_use]
    pub const fn show_points(mut self, show: bool) -> Self {
        self.show_points = show;
        self
    }

    /// Set whether to fill area.
    #[must_use]
    pub const fn fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Get min/max X values.
    #[must_use]
    pub fn x_range(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let min = self
            .points
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::INFINITY, f64::min);
        let max = self
            .points
            .iter()
            .map(|(x, _)| *x)
            .fold(f64::NEG_INFINITY, f64::max);
        Some((min, max))
    }

    /// Get min/max Y values.
    #[must_use]
    pub fn y_range(&self) -> Option<(f64, f64)> {
        if self.points.is_empty() {
            return None;
        }
        let min = self
            .points
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min);
        let max = self
            .points
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::NEG_INFINITY, f64::max);
        Some((min, max))
    }
}

/// Axis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Axis {
    /// Axis label
    pub label: Option<String>,
    /// Minimum value (auto if None)
    pub min: Option<f64>,
    /// Maximum value (auto if None)
    pub max: Option<f64>,
    /// Number of grid lines
    pub grid_lines: usize,
    /// Show grid
    pub show_grid: bool,
    /// Axis color
    pub color: Color,
    /// Grid color
    pub grid_color: Color,
}

impl Default for Axis {
    fn default() -> Self {
        Self {
            label: None,
            min: None,
            max: None,
            grid_lines: 5,
            show_grid: true,
            color: Color::new(0.3, 0.3, 0.3, 1.0),
            grid_color: Color::new(0.9, 0.9, 0.9, 1.0),
        }
    }
}

impl Axis {
    /// Create a new axis.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set axis label.
    #[must_use]
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set minimum value.
    #[must_use]
    pub const fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Set maximum value.
    #[must_use]
    pub const fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Set range.
    #[must_use]
    pub const fn range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Set number of grid lines.
    #[must_use]
    pub fn grid_lines(mut self, count: usize) -> Self {
        self.grid_lines = count.max(2);
        self
    }

    /// Set whether to show grid.
    #[must_use]
    pub const fn show_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Set axis color.
    #[must_use]
    pub const fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set grid color.
    #[must_use]
    pub const fn grid_color(mut self, color: Color) -> Self {
        self.grid_color = color;
        self
    }
}

/// Legend position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum LegendPosition {
    /// No legend
    None,
    /// Top right (default)
    #[default]
    TopRight,
    /// Top left
    TopLeft,
    /// Bottom right
    BottomRight,
    /// Bottom left
    BottomLeft,
}

/// `Chart` widget for data visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chart {
    /// Chart type
    kind: ChartType,
    /// Data series
    series: Vec<DataSeries>,
    /// Chart title
    title: Option<String>,
    /// X axis configuration
    x_axis: Axis,
    /// Y axis configuration
    y_axis: Axis,
    /// Legend position
    legend: LegendPosition,
    /// Background color
    background: Color,
    /// Padding around chart area
    padding: f32,
    /// Width
    width: Option<f32>,
    /// Height
    height: Option<f32>,
    /// Accessible name
    accessible_name_value: Option<String>,
    /// Test ID
    test_id_value: Option<String>,
    /// Cached bounds
    #[serde(skip)]
    bounds: Rect,
}

impl Default for Chart {
    fn default() -> Self {
        Self {
            kind: ChartType::Line,
            series: Vec::new(),
            title: None,
            x_axis: Axis::default(),
            y_axis: Axis::default(),
            legend: LegendPosition::TopRight,
            background: Color::WHITE,
            padding: 40.0,
            width: None,
            height: None,
            accessible_name_value: None,
            test_id_value: None,
            bounds: Rect::default(),
        }
    }
}

impl Chart {
    /// Create a new chart.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a line chart.
    #[must_use]
    pub fn line() -> Self {
        Self::new().chart_type(ChartType::Line)
    }

    /// Create a bar chart.
    #[must_use]
    pub fn bar() -> Self {
        Self::new().chart_type(ChartType::Bar)
    }

    /// Create a scatter chart.
    #[must_use]
    pub fn scatter() -> Self {
        Self::new().chart_type(ChartType::Scatter)
    }

    /// Create an area chart.
    #[must_use]
    pub fn area() -> Self {
        Self::new().chart_type(ChartType::Area)
    }

    /// Create a pie chart.
    #[must_use]
    pub fn pie() -> Self {
        Self::new().chart_type(ChartType::Pie)
    }

    /// Create a heatmap chart.
    #[must_use]
    pub fn heatmap() -> Self {
        Self::new().chart_type(ChartType::Heatmap)
    }

    /// Create a box plot chart.
    #[must_use]
    pub fn boxplot() -> Self {
        Self::new().chart_type(ChartType::BoxPlot)
    }

    /// Set chart type.
    #[must_use]
    pub const fn chart_type(mut self, chart_type: ChartType) -> Self {
        self.kind = chart_type;
        self
    }

    /// Add a data series.
    #[must_use]
    pub fn series(mut self, series: DataSeries) -> Self {
        self.series.push(series);
        self
    }

    /// Add multiple data series.
    #[must_use]
    pub fn add_series(mut self, series: impl IntoIterator<Item = DataSeries>) -> Self {
        self.series.extend(series);
        self
    }

    /// Set chart title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set X axis.
    #[must_use]
    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.x_axis = axis;
        self
    }

    /// Set Y axis.
    #[must_use]
    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.y_axis = axis;
        self
    }

    /// Set legend position.
    #[must_use]
    pub const fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
        self
    }

    /// Set background color.
    #[must_use]
    pub const fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Set padding.
    #[must_use]
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding.max(0.0);
        self
    }

    /// Set width.
    #[must_use]
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width.max(100.0));
        self
    }

    /// Set height.
    #[must_use]
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height.max(100.0));
        self
    }

    /// Set accessible name.
    #[must_use]
    pub fn accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name_value = Some(name.into());
        self
    }

    /// Set test ID.
    #[must_use]
    pub fn test_id(mut self, id: impl Into<String>) -> Self {
        self.test_id_value = Some(id.into());
        self
    }

    /// Get chart type.
    #[must_use]
    pub const fn get_chart_type(&self) -> ChartType {
        self.kind
    }

    /// Get data series.
    #[must_use]
    pub fn get_series(&self) -> &[DataSeries] {
        &self.series
    }

    /// Get series count.
    #[must_use]
    pub fn series_count(&self) -> usize {
        self.series.len()
    }

    /// Check if chart has data.
    #[must_use]
    pub fn has_data(&self) -> bool {
        self.series.iter().any(|s| !s.points.is_empty())
    }

    /// Get title.
    #[must_use]
    pub fn get_title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Compute data bounds across all series.
    #[must_use]
    pub fn data_bounds(&self) -> Option<(f64, f64, f64, f64)> {
        if !self.has_data() {
            return None;
        }

        let mut x_min = f64::INFINITY;
        let mut x_max = f64::NEG_INFINITY;
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;

        for series in &self.series {
            if let Some((min, max)) = series.x_range() {
                x_min = x_min.min(min);
                x_max = x_max.max(max);
            }
            if let Some((min, max)) = series.y_range() {
                y_min = y_min.min(min);
                y_max = y_max.max(max);
            }
        }

        // Apply axis overrides
        if let Some(min) = self.x_axis.min {
            x_min = min;
        }
        if let Some(max) = self.x_axis.max {
            x_max = max;
        }
        if let Some(min) = self.y_axis.min {
            y_min = min;
        }
        if let Some(max) = self.y_axis.max {
            y_max = max;
        }

        Some((x_min, x_max, y_min, y_max))
    }

    /// Get plot area (excluding padding and labels).
    fn plot_area(&self) -> Rect {
        let title_height = if self.title.is_some() { 30.0 } else { 0.0 };
        Rect::new(
            self.bounds.x + self.padding,
            self.bounds.y + self.padding + title_height,
            self.padding.mul_add(-2.0, self.bounds.width),
            self.padding.mul_add(-2.0, self.bounds.height) - title_height,
        )
    }

    /// Map data point to screen coordinates.
    fn map_point(&self, x: f64, y: f64, bounds: &(f64, f64, f64, f64), plot: &Rect) -> Point {
        let (x_min, x_max, y_min, y_max) = *bounds;
        let x_range = (x_max - x_min).max(1e-10);
        let y_range = (y_max - y_min).max(1e-10);

        let px = (((x - x_min) / x_range) as f32).mul_add(plot.width, plot.x);
        let py = (((y - y_min) / y_range) as f32).mul_add(-plot.height, plot.y + plot.height);

        Point::new(px, py)
    }

    /// Paint grid lines.
    fn paint_grid(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        let (x_min, x_max, y_min, y_max) = *bounds;

        // Vertical grid lines
        if self.x_axis.show_grid {
            for i in 0..=self.x_axis.grid_lines {
                let t = i as f32 / self.x_axis.grid_lines as f32;
                let x = t.mul_add(plot.width, plot.x);
                canvas.draw_line(
                    Point::new(x, plot.y),
                    Point::new(x, plot.y + plot.height),
                    self.x_axis.grid_color,
                    1.0,
                );
            }
        }

        // Horizontal grid lines
        if self.y_axis.show_grid {
            for i in 0..=self.y_axis.grid_lines {
                let t = i as f32 / self.y_axis.grid_lines as f32;
                let y = t.mul_add(plot.height, plot.y);
                canvas.draw_line(
                    Point::new(plot.x, y),
                    Point::new(plot.x + plot.width, y),
                    self.y_axis.grid_color,
                    1.0,
                );
            }
        }

        // Axis labels
        let text_style = TextStyle {
            size: 10.0,
            color: self.x_axis.color,
            ..TextStyle::default()
        };

        // X axis labels
        for i in 0..=self.x_axis.grid_lines {
            let t = i as f64 / self.x_axis.grid_lines as f64;
            let value = t.mul_add(x_max - x_min, x_min);
            let x = (t as f32).mul_add(plot.width, plot.x);
            canvas.draw_text(
                &format!("{value:.1}"),
                Point::new(x, plot.y + plot.height + 15.0),
                &text_style,
            );
        }

        // Y axis labels
        for i in 0..=self.y_axis.grid_lines {
            let t = i as f64 / self.y_axis.grid_lines as f64;
            let value = t.mul_add(-(y_max - y_min), y_max);
            let y = (t as f32).mul_add(plot.height, plot.y);
            canvas.draw_text(
                &format!("{value:.1}"),
                Point::new(plot.x - 35.0, y + 4.0),
                &text_style,
            );
        }
    }

    /// Paint line/area chart.
    fn paint_line(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        for series in &self.series {
            if series.points.len() < 2 {
                continue;
            }

            // Collect points for the path
            let path_points: Vec<Point> = series
                .points
                .iter()
                .map(|&(x, y)| self.map_point(x, y, bounds, plot))
                .collect();

            // Draw line using proper path
            canvas.draw_path(&path_points, series.color, series.line_width);

            // For area charts, fill the area under the line
            if series.fill {
                let mut fill_points = path_points.clone();
                // Add bottom corners
                if let (Some(first), Some(last)) = (path_points.first(), path_points.last()) {
                    fill_points.push(Point::new(last.x, plot.y + plot.height));
                    fill_points.push(Point::new(first.x, plot.y + plot.height));
                }
                let mut fill_color = series.color;
                fill_color.a = 0.3; // Semi-transparent fill
                canvas.fill_polygon(&fill_points, fill_color);
            }

            // Draw points as circles
            if series.show_points {
                for &(x, y) in &series.points {
                    let pt = self.map_point(x, y, bounds, plot);
                    canvas.fill_circle(pt, series.point_size / 2.0, series.color);
                }
            }
        }
    }

    /// Paint bar chart.
    fn paint_bar(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        let (_, _, y_min, y_max) = *bounds;
        let y_range = (y_max - y_min).max(1e-10);

        let series_count = self.series.len();
        if series_count == 0 {
            return;
        }

        // Calculate bar width based on number of points
        let max_points = self
            .series
            .iter()
            .map(|s| s.points.len())
            .max()
            .unwrap_or(1);
        let group_width = plot.width / max_points as f32;
        let bar_width = (group_width * 0.8) / series_count as f32;
        let bar_gap = group_width * 0.1;

        for (si, series) in self.series.iter().enumerate() {
            for (i, &(_, y)) in series.points.iter().enumerate() {
                let bar_height = ((y - y_min) / y_range) as f32 * plot.height;
                let x = (si as f32)
                    .mul_add(bar_width, (i as f32).mul_add(group_width, plot.x + bar_gap));
                let rect = Rect::new(
                    x,
                    plot.y + plot.height - bar_height,
                    bar_width - 2.0,
                    bar_height,
                );
                canvas.fill_rect(rect, series.color);
            }
        }
    }

    /// Paint scatter chart.
    fn paint_scatter(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        for series in &self.series {
            for &(x, y) in &series.points {
                let pt = self.map_point(x, y, bounds, plot);
                canvas.fill_circle(pt, series.point_size / 2.0, series.color);
            }
        }
    }

    /// Paint pie chart.
    fn paint_pie(&self, canvas: &mut dyn Canvas, plot: &Rect) {
        // Sum all Y values across series
        let total: f64 = self
            .series
            .iter()
            .flat_map(|s| s.points.iter().map(|(_, y)| *y))
            .sum();

        if total <= 0.0 {
            return;
        }

        let cx = plot.x + plot.width / 2.0;
        let cy = plot.y + plot.height / 2.0;
        let radius = plot.width.min(plot.height) / 2.0 * 0.8;
        let center = Point::new(cx, cy);

        // Draw pie segments as arcs
        let mut start_angle: f32 = -std::f32::consts::FRAC_PI_2; // Start from top
        for series in &self.series {
            for &(_, y) in &series.points {
                let fraction = (y / total) as f32;
                let sweep = fraction * std::f32::consts::TAU;
                let end_angle = start_angle + sweep;

                canvas.fill_arc(center, radius, start_angle, end_angle, series.color);

                start_angle = end_angle;
            }
        }
    }

    /// Paint heatmap chart - displays matrix data with color encoding.
    fn paint_heatmap(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        let (_, _, y_min, y_max) = *bounds;
        let y_range = (y_max - y_min).max(1e-10);

        // For heatmap, we treat each series as a row and each point as a cell
        let row_count = self.series.len();
        if row_count == 0 {
            return;
        }

        let col_count = self
            .series
            .iter()
            .map(|s| s.points.len())
            .max()
            .unwrap_or(1);

        let cell_width = plot.width / col_count as f32;
        let cell_height = plot.height / row_count as f32;

        for (row, series) in self.series.iter().enumerate() {
            for (col, &(_, value)) in series.points.iter().enumerate() {
                // Map value to color intensity (blue to red)
                let t = ((value - y_min) / y_range) as f32;
                let color = Color::new(t, 0.2, 1.0 - t, 1.0);

                let rect = Rect::new(
                    (col as f32).mul_add(cell_width, plot.x),
                    (row as f32).mul_add(cell_height, plot.y),
                    cell_width - 1.0,
                    cell_height - 1.0,
                );
                canvas.fill_rect(rect, color);
            }
        }
    }

    /// Paint box plot - displays statistical distributions.
    fn paint_boxplot(&self, canvas: &mut dyn Canvas, plot: &Rect, bounds: &(f64, f64, f64, f64)) {
        let (_, _, y_min, y_max) = *bounds;
        let y_range = (y_max - y_min).max(1e-10);

        let series_count = self.series.len();
        if series_count == 0 {
            return;
        }

        let box_width = (plot.width / series_count as f32) * 0.6;
        let gap = (plot.width / series_count as f32) * 0.2;

        for (i, series) in self.series.iter().enumerate() {
            if series.points.len() < 5 {
                continue; // Need at least 5 points for box plot (min, q1, median, q3, max)
            }

            // Sort points by y value for quartile calculation
            let mut values: Vec<f64> = series.points.iter().map(|(_, y)| *y).collect();
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let min_val = values[0];
            let q1 = values[values.len() / 4];
            let median = values[values.len() / 2];
            let q3 = values[3 * values.len() / 4];
            let max_val = values[values.len() - 1];

            let x_center = (i as f32).mul_add(plot.width / series_count as f32, plot.x + gap);

            // Map y values to screen coordinates
            let map_y = |v: f64| -> f32 {
                let t = (v - y_min) / y_range;
                (1.0 - t as f32).mul_add(plot.height, plot.y)
            };

            let y_min_px = map_y(min_val);
            let y_q1 = map_y(q1);
            let y_median = map_y(median);
            let y_q3 = map_y(q3);
            let y_max_px = map_y(max_val);

            // Draw whiskers (vertical lines from min to q1 and q3 to max)
            canvas.draw_line(
                Point::new(x_center + box_width / 2.0, y_min_px),
                Point::new(x_center + box_width / 2.0, y_q1),
                series.color,
                1.0,
            );
            canvas.draw_line(
                Point::new(x_center + box_width / 2.0, y_q3),
                Point::new(x_center + box_width / 2.0, y_max_px),
                series.color,
                1.0,
            );

            // Draw box (from q1 to q3)
            let box_rect = Rect::new(x_center, y_q3, box_width, y_q1 - y_q3);
            canvas.fill_rect(box_rect, series.color);
            canvas.stroke_rect(box_rect, Color::new(0.0, 0.0, 0.0, 1.0), 1.0);

            // Draw median line
            canvas.draw_line(
                Point::new(x_center, y_median),
                Point::new(x_center + box_width, y_median),
                Color::new(0.0, 0.0, 0.0, 1.0),
                2.0,
            );

            // Draw caps (horizontal lines at min and max)
            let cap_width = box_width * 0.3;
            canvas.draw_line(
                Point::new(x_center + box_width / 2.0 - cap_width / 2.0, y_min_px),
                Point::new(x_center + box_width / 2.0 + cap_width / 2.0, y_min_px),
                series.color,
                1.0,
            );
            canvas.draw_line(
                Point::new(x_center + box_width / 2.0 - cap_width / 2.0, y_max_px),
                Point::new(x_center + box_width / 2.0 + cap_width / 2.0, y_max_px),
                series.color,
                1.0,
            );
        }
    }

    /// Paint legend.
    fn paint_legend(&self, canvas: &mut dyn Canvas) {
        if self.legend == LegendPosition::None || self.series.is_empty() {
            return;
        }

        let entry_height = 20.0;
        let legend_width = 100.0;
        let legend_height = (self.series.len() as f32).mul_add(entry_height, 10.0);

        let (lx, ly) = match self.legend {
            LegendPosition::TopRight => (
                self.bounds.x + self.bounds.width - legend_width - 10.0,
                self.bounds.y + self.padding + 10.0,
            ),
            LegendPosition::TopLeft => (
                self.bounds.x + self.padding + 10.0,
                self.bounds.y + self.padding + 10.0,
            ),
            LegendPosition::BottomRight => (
                self.bounds.x + self.bounds.width - legend_width - 10.0,
                self.bounds.y + self.bounds.height - legend_height - 10.0,
            ),
            LegendPosition::BottomLeft => (
                self.bounds.x + self.padding + 10.0,
                self.bounds.y + self.bounds.height - legend_height - 10.0,
            ),
            LegendPosition::None => return,
        };

        // Legend background
        canvas.fill_rect(
            Rect::new(lx, ly, legend_width, legend_height),
            Color::new(1.0, 1.0, 1.0, 0.9),
        );
        canvas.stroke_rect(
            Rect::new(lx, ly, legend_width, legend_height),
            Color::new(0.8, 0.8, 0.8, 1.0),
            1.0,
        );

        // Legend entries
        let text_style = TextStyle {
            size: 12.0,
            color: Color::new(0.2, 0.2, 0.2, 1.0),
            ..TextStyle::default()
        };

        for (i, series) in self.series.iter().enumerate() {
            let ey = (i as f32).mul_add(entry_height, ly + 5.0);
            // Color box
            canvas.fill_rect(Rect::new(lx + 5.0, ey + 4.0, 12.0, 12.0), series.color);
            // Label
            canvas.draw_text(&series.name, Point::new(lx + 22.0, ey + 14.0), &text_style);
        }
    }
}

impl Widget for Chart {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let width = self.width.unwrap_or(400.0);
        let height = self.height.unwrap_or(300.0);
        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        // Background
        canvas.fill_rect(self.bounds, self.background);

        // Title
        if let Some(ref title) = self.title {
            let text_style = TextStyle {
                size: 16.0,
                color: Color::new(0.1, 0.1, 0.1, 1.0),
                ..TextStyle::default()
            };
            canvas.draw_text(
                title,
                Point::new(
                    (title.len() as f32).mul_add(-4.0, self.bounds.x + self.bounds.width / 2.0),
                    self.bounds.y + 25.0,
                ),
                &text_style,
            );
        }

        let plot = self.plot_area();

        // Get data bounds
        let Some(bounds) = self.data_bounds() else {
            return;
        };

        // Draw grid
        self.paint_grid(canvas, &plot, &bounds);

        // Draw chart based on type
        match self.kind {
            ChartType::Line | ChartType::Area => self.paint_line(canvas, &plot, &bounds),
            ChartType::Bar | ChartType::Histogram => self.paint_bar(canvas, &plot, &bounds),
            ChartType::Scatter => self.paint_scatter(canvas, &plot, &bounds),
            ChartType::Pie => self.paint_pie(canvas, &plot),
            ChartType::Heatmap => self.paint_heatmap(canvas, &plot, &bounds),
            ChartType::BoxPlot => self.paint_boxplot(canvas, &plot, &bounds),
        }

        // Draw legend
        self.paint_legend(canvas);
    }

    fn event(&mut self, _event: &presentar_core::Event) -> Option<Box<dyn Any + Send>> {
        // Charts are currently view-only
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }

    fn is_interactive(&self) -> bool {
        false
    }

    fn is_focusable(&self) -> bool {
        false
    }

    fn accessible_name(&self) -> Option<&str> {
        self.accessible_name_value
            .as_deref()
            .or(self.title.as_deref())
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Image // Charts are treated as images for accessibility
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id_value.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ===== ChartType Tests =====

    #[test]
    fn test_chart_type_default() {
        assert_eq!(ChartType::default(), ChartType::Line);
    }

    #[test]
    fn test_chart_type_variants() {
        let types = [
            ChartType::Line,
            ChartType::Bar,
            ChartType::Scatter,
            ChartType::Area,
            ChartType::Pie,
            ChartType::Histogram,
            ChartType::Heatmap,
            ChartType::BoxPlot,
        ];
        assert_eq!(types.len(), 8);
    }

    #[test]
    fn test_chart_heatmap() {
        let chart = Chart::new().chart_type(ChartType::Heatmap);
        assert_eq!(chart.get_chart_type(), ChartType::Heatmap);
    }

    #[test]
    fn test_chart_boxplot() {
        let chart = Chart::new().chart_type(ChartType::BoxPlot);
        assert_eq!(chart.get_chart_type(), ChartType::BoxPlot);
    }

    // ===== DataSeries Tests =====

    #[test]
    fn test_data_series_new() {
        let series = DataSeries::new("Sales");
        assert_eq!(series.name, "Sales");
        assert!(series.points.is_empty());
        assert!(series.show_points);
        assert!(!series.fill);
    }

    #[test]
    fn test_data_series_point() {
        let series = DataSeries::new("Data")
            .point(1.0, 10.0)
            .point(2.0, 20.0)
            .point(3.0, 15.0);
        assert_eq!(series.points.len(), 3);
        assert_eq!(series.points[0], (1.0, 10.0));
    }

    #[test]
    fn test_data_series_points() {
        let data = vec![(1.0, 5.0), (2.0, 10.0), (3.0, 7.0)];
        let series = DataSeries::new("Data").points(data);
        assert_eq!(series.points.len(), 3);
    }

    #[test]
    fn test_data_series_color() {
        let series = DataSeries::new("Data").color(Color::RED);
        assert_eq!(series.color, Color::RED);
    }

    #[test]
    fn test_data_series_line_width() {
        let series = DataSeries::new("Data").line_width(3.0);
        assert_eq!(series.line_width, 3.0);
    }

    #[test]
    fn test_data_series_line_width_min() {
        let series = DataSeries::new("Data").line_width(0.1);
        assert_eq!(series.line_width, 0.5);
    }

    #[test]
    fn test_data_series_point_size() {
        let series = DataSeries::new("Data").point_size(6.0);
        assert_eq!(series.point_size, 6.0);
    }

    #[test]
    fn test_data_series_point_size_min() {
        let series = DataSeries::new("Data").point_size(0.5);
        assert_eq!(series.point_size, 1.0);
    }

    #[test]
    fn test_data_series_show_points() {
        let series = DataSeries::new("Data").show_points(false);
        assert!(!series.show_points);
    }

    #[test]
    fn test_data_series_fill() {
        let series = DataSeries::new("Data").fill(true);
        assert!(series.fill);
    }

    #[test]
    fn test_data_series_x_range() {
        let series = DataSeries::new("Data")
            .point(1.0, 10.0)
            .point(5.0, 20.0)
            .point(3.0, 15.0);
        assert_eq!(series.x_range(), Some((1.0, 5.0)));
    }

    #[test]
    fn test_data_series_x_range_empty() {
        let series = DataSeries::new("Data");
        assert_eq!(series.x_range(), None);
    }

    #[test]
    fn test_data_series_y_range() {
        let series = DataSeries::new("Data")
            .point(1.0, 10.0)
            .point(2.0, 30.0)
            .point(3.0, 5.0);
        assert_eq!(series.y_range(), Some((5.0, 30.0)));
    }

    #[test]
    fn test_data_series_y_range_empty() {
        let series = DataSeries::new("Data");
        assert_eq!(series.y_range(), None);
    }

    // ===== Axis Tests =====

    #[test]
    fn test_axis_default() {
        let axis = Axis::default();
        assert!(axis.label.is_none());
        assert!(axis.min.is_none());
        assert!(axis.max.is_none());
        assert_eq!(axis.grid_lines, 5);
        assert!(axis.show_grid);
    }

    #[test]
    fn test_axis_label() {
        let axis = Axis::new().label("Time");
        assert_eq!(axis.label, Some("Time".to_string()));
    }

    #[test]
    fn test_axis_min_max() {
        let axis = Axis::new().min(0.0).max(100.0);
        assert_eq!(axis.min, Some(0.0));
        assert_eq!(axis.max, Some(100.0));
    }

    #[test]
    fn test_axis_range() {
        let axis = Axis::new().range(10.0, 50.0);
        assert_eq!(axis.min, Some(10.0));
        assert_eq!(axis.max, Some(50.0));
    }

    #[test]
    fn test_axis_grid_lines() {
        let axis = Axis::new().grid_lines(10);
        assert_eq!(axis.grid_lines, 10);
    }

    #[test]
    fn test_axis_grid_lines_min() {
        let axis = Axis::new().grid_lines(1);
        assert_eq!(axis.grid_lines, 2);
    }

    #[test]
    fn test_axis_show_grid() {
        let axis = Axis::new().show_grid(false);
        assert!(!axis.show_grid);
    }

    #[test]
    fn test_axis_colors() {
        let axis = Axis::new().color(Color::RED).grid_color(Color::BLUE);
        assert_eq!(axis.color, Color::RED);
        assert_eq!(axis.grid_color, Color::BLUE);
    }

    // ===== LegendPosition Tests =====

    #[test]
    fn test_legend_position_default() {
        assert_eq!(LegendPosition::default(), LegendPosition::TopRight);
    }

    // ===== Chart Construction Tests =====

    #[test]
    fn test_chart_new() {
        let chart = Chart::new();
        assert_eq!(chart.get_chart_type(), ChartType::Line);
        assert_eq!(chart.series_count(), 0);
        assert!(!chart.has_data());
    }

    #[test]
    fn test_chart_line() {
        let chart = Chart::line();
        assert_eq!(chart.get_chart_type(), ChartType::Line);
    }

    #[test]
    fn test_chart_bar() {
        let chart = Chart::bar();
        assert_eq!(chart.get_chart_type(), ChartType::Bar);
    }

    #[test]
    fn test_chart_scatter() {
        let chart = Chart::scatter();
        assert_eq!(chart.get_chart_type(), ChartType::Scatter);
    }

    #[test]
    fn test_chart_area() {
        let chart = Chart::area();
        assert_eq!(chart.get_chart_type(), ChartType::Area);
    }

    #[test]
    fn test_chart_pie() {
        let chart = Chart::pie();
        assert_eq!(chart.get_chart_type(), ChartType::Pie);
    }

    #[test]
    fn test_chart_builder() {
        let chart = Chart::new()
            .chart_type(ChartType::Bar)
            .series(DataSeries::new("Sales").point(1.0, 100.0))
            .series(DataSeries::new("Expenses").point(1.0, 80.0))
            .title("Revenue")
            .x_axis(Axis::new().label("Month"))
            .y_axis(Axis::new().label("Amount"))
            .legend(LegendPosition::BottomRight)
            .background(Color::WHITE)
            .padding(50.0)
            .width(600.0)
            .height(400.0)
            .accessible_name("Revenue chart")
            .test_id("revenue-chart");

        assert_eq!(chart.get_chart_type(), ChartType::Bar);
        assert_eq!(chart.series_count(), 2);
        assert!(chart.has_data());
        assert_eq!(chart.get_title(), Some("Revenue"));
        assert_eq!(Widget::accessible_name(&chart), Some("Revenue chart"));
        assert_eq!(Widget::test_id(&chart), Some("revenue-chart"));
    }

    #[test]
    fn test_chart_add_series() {
        let series_list = vec![DataSeries::new("A"), DataSeries::new("B")];
        let chart = Chart::new().add_series(series_list);
        assert_eq!(chart.series_count(), 2);
    }

    // ===== Data Bounds Tests =====

    #[test]
    fn test_chart_data_bounds() {
        let chart = Chart::new()
            .series(DataSeries::new("S1").point(0.0, 10.0).point(5.0, 20.0))
            .series(DataSeries::new("S2").point(1.0, 5.0).point(4.0, 25.0));

        let bounds = chart.data_bounds().unwrap();
        assert_eq!(bounds.0, 0.0); // x_min
        assert_eq!(bounds.1, 5.0); // x_max
        assert_eq!(bounds.2, 5.0); // y_min
        assert_eq!(bounds.3, 25.0); // y_max
    }

    #[test]
    fn test_chart_data_bounds_with_axis_override() {
        let chart = Chart::new()
            .series(DataSeries::new("S1").point(0.0, 10.0).point(5.0, 20.0))
            .x_axis(Axis::new().min(-5.0).max(10.0))
            .y_axis(Axis::new().min(0.0).max(50.0));

        let bounds = chart.data_bounds().unwrap();
        assert_eq!(bounds.0, -5.0); // x_min (overridden)
        assert_eq!(bounds.1, 10.0); // x_max (overridden)
        assert_eq!(bounds.2, 0.0); // y_min (overridden)
        assert_eq!(bounds.3, 50.0); // y_max (overridden)
    }

    #[test]
    fn test_chart_data_bounds_empty() {
        let chart = Chart::new();
        assert!(chart.data_bounds().is_none());
    }

    // ===== Dimension Tests =====

    #[test]
    fn test_chart_padding_min() {
        let chart = Chart::new().padding(-10.0);
        assert_eq!(chart.padding, 0.0);
    }

    #[test]
    fn test_chart_width_min() {
        let chart = Chart::new().width(50.0);
        assert_eq!(chart.width, Some(100.0));
    }

    #[test]
    fn test_chart_height_min() {
        let chart = Chart::new().height(50.0);
        assert_eq!(chart.height, Some(100.0));
    }

    // ===== Widget Trait Tests =====

    #[test]
    fn test_chart_type_id() {
        let chart = Chart::new();
        assert_eq!(Widget::type_id(&chart), TypeId::of::<Chart>());
    }

    #[test]
    fn test_chart_measure_default() {
        let chart = Chart::new();
        let size = chart.measure(Constraints::loose(Size::new(1000.0, 1000.0)));
        assert_eq!(size.width, 400.0);
        assert_eq!(size.height, 300.0);
    }

    #[test]
    fn test_chart_measure_custom() {
        let chart = Chart::new().width(600.0).height(400.0);
        let size = chart.measure(Constraints::loose(Size::new(1000.0, 1000.0)));
        assert_eq!(size.width, 600.0);
        assert_eq!(size.height, 400.0);
    }

    #[test]
    fn test_chart_layout() {
        let mut chart = Chart::new();
        let bounds = Rect::new(10.0, 20.0, 500.0, 300.0);
        let result = chart.layout(bounds);
        assert_eq!(result.size, Size::new(500.0, 300.0));
        assert_eq!(chart.bounds, bounds);
    }

    #[test]
    fn test_chart_children() {
        let chart = Chart::new();
        assert!(chart.children().is_empty());
    }

    #[test]
    fn test_chart_is_interactive() {
        let chart = Chart::new();
        assert!(!chart.is_interactive());
    }

    #[test]
    fn test_chart_is_focusable() {
        let chart = Chart::new();
        assert!(!chart.is_focusable());
    }

    #[test]
    fn test_chart_accessible_role() {
        let chart = Chart::new();
        assert_eq!(chart.accessible_role(), AccessibleRole::Image);
    }

    #[test]
    fn test_chart_accessible_name_from_title() {
        let chart = Chart::new().title("Sales Chart");
        assert_eq!(Widget::accessible_name(&chart), Some("Sales Chart"));
    }

    #[test]
    fn test_chart_accessible_name_explicit() {
        let chart = Chart::new()
            .title("Sales Chart")
            .accessible_name("Custom name");
        assert_eq!(Widget::accessible_name(&chart), Some("Custom name"));
    }

    #[test]
    fn test_chart_test_id() {
        let chart = Chart::new().test_id("my-chart");
        assert_eq!(Widget::test_id(&chart), Some("my-chart"));
    }

    // ===== Plot Area Tests =====

    #[test]
    fn test_chart_plot_area_no_title() {
        let mut chart = Chart::new().padding(40.0);
        chart.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let plot = chart.plot_area();
        assert_eq!(plot.x, 40.0);
        assert_eq!(plot.y, 40.0);
        assert_eq!(plot.width, 320.0);
        assert_eq!(plot.height, 220.0);
    }

    #[test]
    fn test_chart_plot_area_with_title() {
        let mut chart = Chart::new().padding(40.0).title("Test");
        chart.bounds = Rect::new(0.0, 0.0, 400.0, 300.0);
        let plot = chart.plot_area();
        assert_eq!(plot.y, 70.0); // 40 + 30 for title
    }

    // ===== Map Point Tests =====

    #[test]
    fn test_chart_map_point() {
        let chart = Chart::new();
        let bounds = (0.0, 10.0, 0.0, 100.0);
        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);

        let pt = chart.map_point(5.0, 50.0, &bounds, &plot);
        assert!((pt.x - 50.0).abs() < 0.1);
        assert!((pt.y - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_chart_map_point_origin() {
        let chart = Chart::new();
        let bounds = (0.0, 10.0, 0.0, 100.0);
        let plot = Rect::new(0.0, 0.0, 100.0, 100.0);

        let pt = chart.map_point(0.0, 0.0, &bounds, &plot);
        assert!((pt.x - 0.0).abs() < 0.1);
        assert!((pt.y - 100.0).abs() < 0.1); // Y is flipped
    }

    // ===== Has Data Tests =====

    #[test]
    fn test_chart_has_data_empty_series() {
        let chart = Chart::new().series(DataSeries::new("Empty"));
        assert!(!chart.has_data());
    }

    #[test]
    fn test_chart_has_data_with_points() {
        let chart = Chart::new().series(DataSeries::new("Data").point(1.0, 1.0));
        assert!(chart.has_data());
    }

    // =========================================================================
    // Additional Coverage Tests
    // =========================================================================

    #[test]
    fn test_data_series_eq() {
        let s1 = DataSeries::new("A").point(1.0, 2.0);
        let s2 = DataSeries::new("A").point(1.0, 2.0);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_chart_type_eq() {
        assert_eq!(ChartType::Line, ChartType::Line);
        assert_ne!(ChartType::Line, ChartType::Bar);
    }

    #[test]
    fn test_legend_position_all_variants() {
        let positions = [
            LegendPosition::None,
            LegendPosition::TopRight,
            LegendPosition::TopLeft,
            LegendPosition::BottomRight,
            LegendPosition::BottomLeft,
        ];
        assert_eq!(positions.len(), 5);
    }

    #[test]
    fn test_chart_children_mut() {
        let mut chart = Chart::new();
        assert!(chart.children_mut().is_empty());
    }

    #[test]
    fn test_chart_event_returns_none() {
        let mut chart = Chart::new();
        let result = chart.event(&presentar_core::Event::KeyDown { key: presentar_core::Key::Down });
        assert!(result.is_none());
    }

    #[test]
    fn test_axis_default_colors() {
        let axis = Axis::default();
        assert_eq!(axis.color.a, 1.0);
        assert_eq!(axis.grid_color.a, 1.0);
    }

    #[test]
    fn test_chart_get_series() {
        let chart = Chart::new()
            .series(DataSeries::new("A"))
            .series(DataSeries::new("B"));
        assert_eq!(chart.get_series().len(), 2);
        assert_eq!(chart.get_series()[0].name, "A");
    }

    #[test]
    fn test_chart_histogram() {
        let chart = Chart::new().chart_type(ChartType::Histogram);
        assert_eq!(chart.get_chart_type(), ChartType::Histogram);
    }

    #[test]
    fn test_chart_data_bounds_single_point() {
        let chart = Chart::new()
            .series(DataSeries::new("S").point(5.0, 10.0));
        let bounds = chart.data_bounds().unwrap();
        assert_eq!(bounds.0, 5.0); // x_min
        assert_eq!(bounds.1, 5.0); // x_max (same as min for single point)
    }

    #[test]
    fn test_chart_legend_none() {
        let chart = Chart::new().legend(LegendPosition::None);
        assert_eq!(chart.legend, LegendPosition::None);
    }

    #[test]
    fn test_chart_legend_top_left() {
        let chart = Chart::new().legend(LegendPosition::TopLeft);
        assert_eq!(chart.legend, LegendPosition::TopLeft);
    }

    #[test]
    fn test_chart_legend_bottom_left() {
        let chart = Chart::new().legend(LegendPosition::BottomLeft);
        assert_eq!(chart.legend, LegendPosition::BottomLeft);
    }

    #[test]
    fn test_chart_test_id_none() {
        let chart = Chart::new();
        assert!(Widget::test_id(&chart).is_none());
    }

    #[test]
    fn test_chart_accessible_name_none() {
        let chart = Chart::new();
        assert!(Widget::accessible_name(&chart).is_none());
    }

    #[test]
    fn test_data_series_default_values() {
        let series = DataSeries::new("Test");
        assert_eq!(series.line_width, 2.0);
        assert_eq!(series.point_size, 4.0);
    }
}
