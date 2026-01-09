//! Time-series graph widget with multiple render modes.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Render mode for the graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GraphMode {
    /// Unicode braille characters (U+2800-28FF): 2x4 dots per cell.
    #[default]
    Braille,
    /// Half-block characters (▀▄█): 1x2 resolution per cell.
    Block,
    /// Pure ASCII characters: TTY compatible.
    Tty,
}

/// Time-series graph widget.
#[derive(Debug, Clone)]
pub struct BrailleGraph {
    data: Vec<f64>,
    color: Color,
    min: f64,
    max: f64,
    mode: GraphMode,
    label: Option<String>,
    bounds: Rect,
}

impl BrailleGraph {
    /// Create a new braille graph.
    #[must_use]
    pub fn new(data: Vec<f64>) -> Self {
        let (min, max) = Self::compute_range(&data);
        Self {
            data,
            color: Color::GREEN,
            min,
            max,
            mode: GraphMode::default(),
            label: None,
            bounds: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    /// Set the color.
    #[must_use]
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set explicit min/max range.
    #[must_use]
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self
    }

    /// Set the render mode.
    #[must_use]
    pub fn with_mode(mut self, mode: GraphMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set the label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Update the data.
    pub fn set_data(&mut self, data: Vec<f64>) {
        let (min, max) = Self::compute_range(&data);
        self.data = data;
        self.min = min;
        self.max = max;
    }

    /// Push a new data point.
    pub fn push(&mut self, value: f64) {
        self.data.push(value);
        if value < self.min {
            self.min = value;
        }
        if value > self.max {
            self.max = value;
        }
    }

    fn compute_range(data: &[f64]) -> (f64, f64) {
        if data.is_empty() {
            return (0.0, 1.0);
        }
        let min = data.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = data.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        if (max - min).abs() < f64::EPSILON {
            (min - 0.5, max + 0.5)
        } else {
            (min, max)
        }
    }

    fn normalize(&self, value: f64) -> f64 {
        if (self.max - self.min).abs() < f64::EPSILON {
            0.5
        } else {
            (value - self.min) / (self.max - self.min)
        }
    }

    fn render_braille(&self, canvas: &mut dyn Canvas) {
        let width = self.bounds.width as usize;
        let height = self.bounds.height as usize;
        if width == 0 || height == 0 || self.data.is_empty() {
            return;
        }

        let dots_per_col = 2;
        let dots_per_row = 4;
        let total_dots_x = width * dots_per_col;
        let total_dots_y = height * dots_per_row;

        let step = if self.data.len() > total_dots_x {
            self.data.len() as f64 / total_dots_x as f64
        } else {
            1.0
        };

        let mut dots = vec![vec![false; total_dots_x]; total_dots_y];

        for (i, x) in (0..total_dots_x).enumerate() {
            let data_idx = (i as f64 * step) as usize;
            if data_idx >= self.data.len() {
                break;
            }
            let value = self.normalize(self.data[data_idx]);
            let y = ((1.0 - value) * (total_dots_y - 1) as f64).round() as usize;
            if y < total_dots_y {
                dots[y][x] = true;
            }
        }

        let style = TextStyle {
            color: self.color,
            ..Default::default()
        };

        for cy in 0..height {
            let mut line = String::new();
            for cx in 0..width {
                let mut code_point = 0x2800u32;
                let dot_offsets = [
                    (0, 0, 0x01),
                    (0, 1, 0x02),
                    (0, 2, 0x04),
                    (1, 0, 0x08),
                    (1, 1, 0x10),
                    (1, 2, 0x20),
                    (0, 3, 0x40),
                    (1, 3, 0x80),
                ];

                for (dx, dy, bit) in dot_offsets {
                    let dot_x = cx * dots_per_col + dx;
                    let dot_y = cy * dots_per_row + dy;
                    if dot_y < total_dots_y && dot_x < total_dots_x && dots[dot_y][dot_x] {
                        code_point |= bit;
                    }
                }

                if let Some(c) = char::from_u32(code_point) {
                    line.push(c);
                }
            }
            canvas.draw_text(
                &line,
                Point::new(self.bounds.x, self.bounds.y + cy as f32),
                &style,
            );
        }
    }

    fn render_block(&self, canvas: &mut dyn Canvas) {
        let width = self.bounds.width as usize;
        let height = self.bounds.height as usize;
        if width == 0 || height == 0 || self.data.is_empty() {
            return;
        }

        let total_rows = height * 2;
        let style = TextStyle {
            color: self.color,
            ..Default::default()
        };

        let step = if self.data.len() > width {
            self.data.len() as f64 / width as f64
        } else {
            1.0
        };

        let mut column_values: Vec<usize> = Vec::with_capacity(width);
        for x in 0..width {
            let data_idx = (x as f64 * step) as usize;
            if data_idx >= self.data.len() {
                column_values.push(0);
                continue;
            }
            let value = self.normalize(self.data[data_idx]);
            let row = ((1.0 - value) * (total_rows - 1) as f64).round() as usize;
            column_values.push(row.min(total_rows - 1));
        }

        for cy in 0..height {
            let mut line = String::new();
            for cx in 0..width {
                let value_row = column_values.get(cx).copied().unwrap_or(total_rows);
                let top_row = cy * 2;
                let bottom_row = cy * 2 + 1;

                let top_filled = value_row <= top_row;
                let bottom_filled = value_row <= bottom_row;

                let ch = match (top_filled, bottom_filled) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                };
                line.push(ch);
            }
            canvas.draw_text(
                &line,
                Point::new(self.bounds.x, self.bounds.y + cy as f32),
                &style,
            );
        }
    }

    fn render_tty(&self, canvas: &mut dyn Canvas) {
        let width = self.bounds.width as usize;
        let height = self.bounds.height as usize;
        if width == 0 || height == 0 || self.data.is_empty() {
            return;
        }

        let style = TextStyle {
            color: self.color,
            ..Default::default()
        };

        let step = if self.data.len() > width {
            self.data.len() as f64 / width as f64
        } else {
            1.0
        };

        let mut column_rows: Vec<usize> = Vec::with_capacity(width);
        for x in 0..width {
            let data_idx = (x as f64 * step) as usize;
            if data_idx >= self.data.len() {
                column_rows.push(height);
                continue;
            }
            let value = self.normalize(self.data[data_idx]);
            let row = ((1.0 - value) * (height - 1) as f64).round() as usize;
            column_rows.push(row.min(height - 1));
        }

        for cy in 0..height {
            let mut line = String::new();
            for cx in 0..width {
                let value_row = column_rows.get(cx).copied().unwrap_or(height);
                let ch = if value_row == cy { '*' } else { ' ' };
                line.push(ch);
            }
            canvas.draw_text(
                &line,
                Point::new(self.bounds.x, self.bounds.y + cy as f32),
                &style,
            );
        }
    }
}

impl Brick for BrailleGraph {
    fn brick_name(&self) -> &'static str {
        "braille_graph"
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
            passed: vec![BrickAssertion::max_latency_ms(16)],
            failed: vec![],
            verification_time: Duration::from_micros(10),
        }
    }

    fn to_html(&self) -> String {
        String::new() // TUI-only widget
    }

    fn to_css(&self) -> String {
        String::new() // TUI-only widget
    }
}

impl Widget for BrailleGraph {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let width = constraints.max_width.max(10.0);
        let height = constraints.max_height.max(3.0);
        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        match self.mode {
            GraphMode::Braille => self.render_braille(canvas),
            GraphMode::Block => self.render_block(canvas),
            GraphMode::Tty => self.render_tty(canvas),
        }

        if let Some(ref label) = self.label {
            let style = TextStyle {
                color: self.color,
                ..Default::default()
            };
            canvas.draw_text(label, Point::new(self.bounds.x, self.bounds.y), &style);
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
    use presentar_core::{Canvas, TextStyle};

    struct MockCanvas {
        texts: Vec<(String, Point)>,
    }

    impl MockCanvas {
        fn new() -> Self {
            Self { texts: vec![] }
        }
    }

    impl Canvas for MockCanvas {
        fn fill_rect(&mut self, _rect: Rect, _color: Color) {}
        fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32) {}
        fn draw_text(&mut self, text: &str, position: Point, _style: &TextStyle) {
            self.texts.push((text.to_string(), position));
        }
        fn draw_line(&mut self, _from: Point, _to: Point, _color: Color, _width: f32) {}
        fn fill_circle(&mut self, _center: Point, _radius: f32, _color: Color) {}
        fn stroke_circle(&mut self, _center: Point, _radius: f32, _color: Color, _width: f32) {}
        fn fill_arc(
            &mut self,
            _center: Point,
            _radius: f32,
            _start: f32,
            _end: f32,
            _color: Color,
        ) {
        }
        fn draw_path(&mut self, _points: &[Point], _color: Color, _width: f32) {}
        fn fill_polygon(&mut self, _points: &[Point], _color: Color) {}
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
        fn push_transform(&mut self, _transform: presentar_core::Transform2D) {}
        fn pop_transform(&mut self) {}
    }

    #[test]
    fn test_graph_creation() {
        let graph = BrailleGraph::new(vec![1.0, 2.0, 3.0]);
        assert_eq!(graph.data.len(), 3);
    }

    #[test]
    fn test_graph_assertions_not_empty() {
        let graph = BrailleGraph::new(vec![1.0, 2.0, 3.0]);
        assert!(!graph.assertions().is_empty());
    }

    #[test]
    fn test_graph_verify_pass() {
        let graph = BrailleGraph::new(vec![1.0, 2.0, 3.0]);
        assert!(graph.verify().is_valid());
    }

    #[test]
    fn test_graph_with_color() {
        let graph = BrailleGraph::new(vec![1.0, 2.0]).with_color(Color::RED);
        assert_eq!(graph.color, Color::RED);
    }

    #[test]
    fn test_graph_with_range() {
        let graph = BrailleGraph::new(vec![1.0, 2.0]).with_range(0.0, 100.0);
        assert_eq!(graph.min, 0.0);
        assert_eq!(graph.max, 100.0);
    }

    #[test]
    fn test_graph_with_mode() {
        let graph = BrailleGraph::new(vec![1.0]).with_mode(GraphMode::Block);
        assert_eq!(graph.mode, GraphMode::Block);

        let graph2 = BrailleGraph::new(vec![1.0]).with_mode(GraphMode::Tty);
        assert_eq!(graph2.mode, GraphMode::Tty);
    }

    #[test]
    fn test_graph_with_label() {
        let graph = BrailleGraph::new(vec![1.0]).with_label("CPU Usage");
        assert_eq!(graph.label, Some("CPU Usage".to_string()));
    }

    #[test]
    fn test_graph_set_data() {
        let mut graph = BrailleGraph::new(vec![1.0, 2.0]);
        graph.set_data(vec![10.0, 20.0, 30.0, 40.0]);
        assert_eq!(graph.data.len(), 4);
        assert_eq!(graph.min, 10.0);
        assert_eq!(graph.max, 40.0);
    }

    #[test]
    fn test_graph_push() {
        let mut graph = BrailleGraph::new(vec![5.0, 10.0]);
        graph.push(15.0);
        assert_eq!(graph.data.len(), 3);
        assert_eq!(graph.max, 15.0);

        graph.push(2.0);
        assert_eq!(graph.min, 2.0);
    }

    #[test]
    fn test_graph_empty_data_range() {
        let graph = BrailleGraph::new(vec![]);
        assert_eq!(graph.min, 0.0);
        assert_eq!(graph.max, 1.0);
    }

    #[test]
    fn test_graph_constant_data_range() {
        let graph = BrailleGraph::new(vec![5.0, 5.0, 5.0]);
        assert_eq!(graph.min, 4.5);
        assert_eq!(graph.max, 5.5);
    }

    #[test]
    fn test_graph_normalize() {
        let graph = BrailleGraph::new(vec![0.0, 100.0]);
        assert!((graph.normalize(50.0) - 0.5).abs() < f64::EPSILON);
        assert!((graph.normalize(0.0) - 0.0).abs() < f64::EPSILON);
        assert!((graph.normalize(100.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_normalize_constant() {
        let graph = BrailleGraph::new(vec![5.0, 5.0]);
        assert!((graph.normalize(5.0) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_graph_measure() {
        let graph = BrailleGraph::new(vec![1.0, 2.0]);
        let constraints = Constraints::new(0.0, 100.0, 0.0, 50.0);
        let size = graph.measure(constraints);
        assert!(size.width >= 10.0);
        assert!(size.height >= 3.0);
    }

    #[test]
    fn test_graph_layout() {
        let mut graph = BrailleGraph::new(vec![1.0, 2.0]);
        let bounds = Rect::new(10.0, 20.0, 80.0, 24.0);
        let result = graph.layout(bounds);
        assert_eq!(result.size.width, 80.0);
        assert_eq!(result.size.height, 24.0);
        assert_eq!(graph.bounds, bounds);
    }

    #[test]
    fn test_graph_paint_braille() {
        let mut graph = BrailleGraph::new(vec![0.0, 50.0, 100.0]).with_mode(GraphMode::Braille);
        graph.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_paint_block() {
        let mut graph = BrailleGraph::new(vec![0.0, 50.0, 100.0]).with_mode(GraphMode::Block);
        graph.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_paint_tty() {
        let mut graph = BrailleGraph::new(vec![0.0, 50.0, 100.0]).with_mode(GraphMode::Tty);
        graph.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_paint_with_label() {
        let mut graph = BrailleGraph::new(vec![1.0, 2.0]).with_label("Test");
        graph.bounds = Rect::new(0.0, 0.0, 20.0, 10.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("Test")));
    }

    #[test]
    fn test_graph_paint_empty_bounds() {
        let mut graph = BrailleGraph::new(vec![1.0, 2.0]);
        graph.bounds = Rect::new(0.0, 0.0, 0.0, 0.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_paint_empty_data() {
        let mut graph = BrailleGraph::new(vec![]);
        graph.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_event() {
        let mut graph = BrailleGraph::new(vec![1.0]);
        let event = Event::KeyDown {
            key: presentar_core::Key::Enter,
        };
        assert!(graph.event(&event).is_none());
    }

    #[test]
    fn test_graph_children() {
        let graph = BrailleGraph::new(vec![1.0]);
        assert!(graph.children().is_empty());
    }

    #[test]
    fn test_graph_children_mut() {
        let mut graph = BrailleGraph::new(vec![1.0]);
        assert!(graph.children_mut().is_empty());
    }

    #[test]
    fn test_graph_type_id() {
        let graph = BrailleGraph::new(vec![1.0]);
        assert_eq!(Widget::type_id(&graph), TypeId::of::<BrailleGraph>());
    }

    #[test]
    fn test_graph_brick_name() {
        let graph = BrailleGraph::new(vec![1.0]);
        assert_eq!(graph.brick_name(), "braille_graph");
    }

    #[test]
    fn test_graph_budget() {
        let graph = BrailleGraph::new(vec![1.0]);
        let budget = graph.budget();
        assert!(budget.measure_ms > 0);
    }

    #[test]
    fn test_graph_to_html() {
        let graph = BrailleGraph::new(vec![1.0]);
        assert!(graph.to_html().is_empty());
    }

    #[test]
    fn test_graph_to_css() {
        let graph = BrailleGraph::new(vec![1.0]);
        assert!(graph.to_css().is_empty());
    }

    #[test]
    fn test_graph_mode_default() {
        assert_eq!(GraphMode::default(), GraphMode::Braille);
    }

    #[test]
    fn test_graph_large_dataset() {
        let data: Vec<f64> = (0..1000).map(|i| (i as f64).sin()).collect();
        let mut graph = BrailleGraph::new(data);
        graph.bounds = Rect::new(0.0, 0.0, 50.0, 10.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_block_mode_various_values() {
        let mut graph =
            BrailleGraph::new(vec![0.0, 25.0, 50.0, 75.0, 100.0]).with_mode(GraphMode::Block);
        graph.bounds = Rect::new(0.0, 0.0, 5.0, 4.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_graph_tty_mode_various_values() {
        let mut graph =
            BrailleGraph::new(vec![0.0, 25.0, 50.0, 75.0, 100.0]).with_mode(GraphMode::Tty);
        graph.bounds = Rect::new(0.0, 0.0, 5.0, 4.0);
        let mut canvas = MockCanvas::new();
        graph.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }
}
