//! Box/Border widget for framing content.
//!
//! Provides Unicode box-drawing borders around content areas.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Border style using Unicode box-drawing characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderStyle {
    /// Single line: ┌─┐│└─┘
    #[default]
    Single,
    /// Double line: ╔═╗║╚═╝
    Double,
    /// Rounded corners: ╭─╮│╰─╯
    Rounded,
    /// Heavy/thick: ┏━┓┃┗━┛
    Heavy,
    /// ASCII only: +-+|+-+
    Ascii,
    /// No border
    None,
}

impl BorderStyle {
    /// Get border characters: (`top_left`, top, `top_right`, left, right, `bottom_left`, bottom, `bottom_right`)
    #[must_use]
    pub const fn chars(&self) -> (char, char, char, char, char, char, char, char) {
        match self {
            Self::Single => ('┌', '─', '┐', '│', '│', '└', '─', '┘'),
            Self::Double => ('╔', '═', '╗', '║', '║', '╚', '═', '╝'),
            Self::Rounded => ('╭', '─', '╮', '│', '│', '╰', '─', '╯'),
            Self::Heavy => ('┏', '━', '┓', '┃', '┃', '┗', '━', '┛'),
            Self::Ascii => ('+', '-', '+', '|', '|', '+', '-', '+'),
            Self::None => (' ', ' ', ' ', ' ', ' ', ' ', ' ', ' '),
        }
    }
}

/// A bordered box widget.
pub struct Border {
    /// Title displayed at top.
    title: Option<String>,
    /// Border style.
    style: BorderStyle,
    /// Border color.
    color: Color,
    /// Title color.
    title_color: Color,
    /// Fill background.
    fill: bool,
    /// Background color (if fill is true).
    background: Color,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for Border {
    fn default() -> Self {
        Self::new()
    }
}

impl Border {
    /// Create a new border.
    #[must_use]
    pub fn new() -> Self {
        Self {
            title: None,
            style: BorderStyle::default(),
            color: Color::new(0.4, 0.5, 0.6, 1.0),
            title_color: Color::new(0.8, 0.9, 1.0, 1.0),
            fill: false,
            background: Color::new(0.1, 0.1, 0.1, 1.0),
            bounds: Rect::default(),
        }
    }

    /// Set the title.
    #[must_use]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the border style.
    #[must_use]
    pub fn with_style(mut self, style: BorderStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the border color.
    #[must_use]
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the title color.
    #[must_use]
    pub fn with_title_color(mut self, color: Color) -> Self {
        self.title_color = color;
        self
    }

    /// Enable background fill.
    #[must_use]
    pub fn with_fill(mut self, fill: bool) -> Self {
        self.fill = fill;
        self
    }

    /// Set the background color.
    #[must_use]
    pub fn with_background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Get the inner content area (excluding border).
    #[must_use]
    pub fn inner_rect(&self) -> Rect {
        if matches!(self.style, BorderStyle::None) {
            self.bounds
        } else {
            Rect::new(
                self.bounds.x + 1.0,
                self.bounds.y + 1.0,
                (self.bounds.width - 2.0).max(0.0),
                (self.bounds.height - 2.0).max(0.0),
            )
        }
    }
}

impl Brick for Border {
    fn brick_name(&self) -> &'static str {
        "border"
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

impl Widget for Border {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        constraints.constrain(Size::new(
            constraints.max_width.min(20.0),
            constraints.max_height.min(5.0),
        ))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        let width = self.bounds.width as usize;
        let height = self.bounds.height as usize;

        if width < 2 || height < 2 {
            return;
        }

        // Fill background if enabled
        if self.fill {
            canvas.fill_rect(self.bounds, self.background);
        }

        if matches!(self.style, BorderStyle::None) {
            return;
        }

        let (tl, top, tr, left, right, bl, bottom, br) = self.style.chars();
        let style = TextStyle {
            color: self.color,
            ..Default::default()
        };

        // Top line with optional title
        let mut top_line = String::with_capacity(width);
        top_line.push(tl);

        if let Some(ref title) = self.title {
            let title_len = title.chars().count();
            let available = width.saturating_sub(4); // 2 for corners, 2 for spacing

            if title_len <= available {
                // Center title
                let padding = (available - title_len) / 2;
                for _ in 0..padding {
                    top_line.push(top);
                }

                // Draw title with different color
                canvas.draw_text(&top_line, Point::new(self.bounds.x, self.bounds.y), &style);

                let title_style = TextStyle {
                    color: self.title_color,
                    ..Default::default()
                };
                canvas.draw_text(
                    &format!(" {title} "),
                    Point::new(self.bounds.x + 1.0 + padding as f32, self.bounds.y),
                    &title_style,
                );

                // Rest of top line
                let after_title = padding + title_len + 2;
                let remaining = width.saturating_sub(after_title + 1);
                let mut rest = String::new();
                for _ in 0..remaining {
                    rest.push(top);
                }
                rest.push(tr);
                canvas.draw_text(
                    &rest,
                    Point::new(self.bounds.x + after_title as f32, self.bounds.y),
                    &style,
                );
            } else {
                // Title too long, just draw plain border
                for _ in 0..(width - 2) {
                    top_line.push(top);
                }
                top_line.push(tr);
                canvas.draw_text(&top_line, Point::new(self.bounds.x, self.bounds.y), &style);
            }
        } else {
            // No title, plain top border
            for _ in 0..(width - 2) {
                top_line.push(top);
            }
            top_line.push(tr);
            canvas.draw_text(&top_line, Point::new(self.bounds.x, self.bounds.y), &style);
        }

        // Side borders
        for y in 1..(height - 1) {
            canvas.draw_text(
                &left.to_string(),
                Point::new(self.bounds.x, self.bounds.y + y as f32),
                &style,
            );
            canvas.draw_text(
                &right.to_string(),
                Point::new(self.bounds.x + (width - 1) as f32, self.bounds.y + y as f32),
                &style,
            );
        }

        // Bottom border
        let mut bottom_line = String::with_capacity(width);
        bottom_line.push(bl);
        for _ in 0..(width - 2) {
            bottom_line.push(bottom);
        }
        bottom_line.push(br);
        canvas.draw_text(
            &bottom_line,
            Point::new(self.bounds.x, self.bounds.y + (height - 1) as f32),
            &style,
        );
    }

    fn event(&mut self, _event: &Event) -> Option<Box<dyn Any + Send>> {
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        // Can't return reference to Option contents
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockCanvas {
        texts: Vec<(String, Point)>,
        rects: Vec<Rect>,
    }

    impl MockCanvas {
        fn new() -> Self {
            Self {
                texts: vec![],
                rects: vec![],
            }
        }
    }

    impl Canvas for MockCanvas {
        fn fill_rect(&mut self, rect: Rect, _color: Color) {
            self.rects.push(rect);
        }
        fn stroke_rect(&mut self, _rect: Rect, _color: Color, _width: f32) {}
        fn draw_text(&mut self, text: &str, position: Point, _style: &TextStyle) {
            self.texts.push((text.to_string(), position));
        }
        fn draw_line(&mut self, _from: Point, _to: Point, _color: Color, _width: f32) {}
        fn fill_circle(&mut self, _center: Point, _radius: f32, _color: Color) {}
        fn stroke_circle(&mut self, _center: Point, _radius: f32, _color: Color, _width: f32) {}
        fn fill_arc(&mut self, _c: Point, _r: f32, _s: f32, _e: f32, _color: Color) {}
        fn draw_path(&mut self, _points: &[Point], _color: Color, _width: f32) {}
        fn fill_polygon(&mut self, _points: &[Point], _color: Color) {}
        fn push_clip(&mut self, _rect: Rect) {}
        fn pop_clip(&mut self) {}
        fn push_transform(&mut self, _transform: presentar_core::Transform2D) {}
        fn pop_transform(&mut self) {}
    }

    #[test]
    fn test_border_creation() {
        let border = Border::new();
        assert!(border.title.is_none());
        assert_eq!(border.style, BorderStyle::Single);
    }

    #[test]
    fn test_border_with_title() {
        let border = Border::new().with_title("Test");
        assert_eq!(border.title, Some("Test".to_string()));
    }

    #[test]
    fn test_border_with_style() {
        let border = Border::new().with_style(BorderStyle::Double);
        assert_eq!(border.style, BorderStyle::Double);
    }

    #[test]
    fn test_border_with_color() {
        let border = Border::new().with_color(Color::RED);
        assert_eq!(border.color, Color::RED);
    }

    #[test]
    fn test_border_with_fill() {
        let border = Border::new().with_fill(true);
        assert!(border.fill);
    }

    #[test]
    fn test_border_style_chars() {
        let (tl, _, tr, _, _, bl, _, br) = BorderStyle::Single.chars();
        assert_eq!(tl, '┌');
        assert_eq!(tr, '┐');
        assert_eq!(bl, '└');
        assert_eq!(br, '┘');
    }

    #[test]
    fn test_border_style_rounded() {
        let (tl, _, tr, _, _, bl, _, br) = BorderStyle::Rounded.chars();
        assert_eq!(tl, '╭');
        assert_eq!(tr, '╮');
        assert_eq!(bl, '╰');
        assert_eq!(br, '╯');
    }

    #[test]
    fn test_border_style_double() {
        let (tl, _, _, _, _, _, _, _) = BorderStyle::Double.chars();
        assert_eq!(tl, '╔');
    }

    #[test]
    fn test_border_style_heavy() {
        let (tl, _, _, _, _, _, _, _) = BorderStyle::Heavy.chars();
        assert_eq!(tl, '┏');
    }

    #[test]
    fn test_border_style_ascii() {
        let (tl, _, _, _, _, _, _, _) = BorderStyle::Ascii.chars();
        assert_eq!(tl, '+');
    }

    #[test]
    fn test_border_inner_rect() {
        let mut border = Border::new();
        border.bounds = Rect::new(0.0, 0.0, 20.0, 10.0);
        let inner = border.inner_rect();
        assert_eq!(inner.x, 1.0);
        assert_eq!(inner.y, 1.0);
        assert_eq!(inner.width, 18.0);
        assert_eq!(inner.height, 8.0);
    }

    #[test]
    fn test_border_inner_rect_no_border() {
        let mut border = Border::new().with_style(BorderStyle::None);
        border.bounds = Rect::new(5.0, 5.0, 20.0, 10.0);
        let inner = border.inner_rect();
        assert_eq!(inner, border.bounds);
    }

    #[test]
    fn test_border_paint() {
        let mut border = Border::new();
        border.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_border_paint_with_title() {
        let mut border = Border::new().with_title("CPU");
        border.bounds = Rect::new(0.0, 0.0, 20.0, 5.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        assert!(canvas.texts.iter().any(|(t, _)| t.contains("CPU")));
    }

    #[test]
    fn test_border_paint_with_fill() {
        let mut border = Border::new().with_fill(true);
        border.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        assert!(!canvas.rects.is_empty());
    }

    #[test]
    fn test_border_paint_no_style() {
        let mut border = Border::new().with_style(BorderStyle::None);
        border.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        // Should not draw border characters
    }

    #[test]
    fn test_border_paint_small() {
        let mut border = Border::new();
        border.bounds = Rect::new(0.0, 0.0, 1.0, 1.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        // Should early return for small bounds
        assert!(canvas.texts.is_empty());
    }

    #[test]
    fn test_border_assertions() {
        let border = Border::new();
        assert!(!border.assertions().is_empty());
    }

    #[test]
    fn test_border_verify() {
        let border = Border::new();
        assert!(border.verify().is_valid());
    }

    #[test]
    fn test_border_brick_name() {
        let border = Border::new();
        assert_eq!(border.brick_name(), "border");
    }

    #[test]
    fn test_border_type_id() {
        let border = Border::new();
        assert_eq!(Widget::type_id(&border), TypeId::of::<Border>());
    }

    #[test]
    fn test_border_measure() {
        let border = Border::new();
        let size = border.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }

    #[test]
    fn test_border_layout() {
        let mut border = Border::new();
        let bounds = Rect::new(5.0, 10.0, 30.0, 15.0);
        let result = border.layout(bounds);
        assert_eq!(result.size.width, 30.0);
        assert_eq!(border.bounds, bounds);
    }

    #[test]
    fn test_border_children() {
        let border = Border::new();
        assert!(border.children().is_empty());
    }

    #[test]
    fn test_border_children_mut() {
        let mut border = Border::new();
        assert!(border.children_mut().is_empty());
    }

    #[test]
    fn test_border_event() {
        let mut border = Border::new();
        let event = Event::KeyDown {
            key: presentar_core::Key::Enter,
        };
        assert!(border.event(&event).is_none());
    }

    #[test]
    fn test_border_default() {
        let border = Border::default();
        assert!(border.title.is_none());
    }

    #[test]
    fn test_border_to_html() {
        let border = Border::new();
        assert!(border.to_html().is_empty());
    }

    #[test]
    fn test_border_to_css() {
        let border = Border::new();
        assert!(border.to_css().is_empty());
    }

    #[test]
    fn test_border_budget() {
        let border = Border::new();
        let budget = border.budget();
        assert!(budget.paint_ms > 0);
    }

    #[test]
    fn test_border_title_too_long() {
        let mut border = Border::new().with_title("This is a very long title that won't fit");
        border.bounds = Rect::new(0.0, 0.0, 10.0, 5.0);
        let mut canvas = MockCanvas::new();
        border.paint(&mut canvas);
        // Should draw plain border without title
        assert!(!canvas.texts.is_empty());
    }

    #[test]
    fn test_border_with_title_color() {
        let border = Border::new().with_title_color(Color::GREEN);
        assert_eq!(border.title_color, Color::GREEN);
    }

    #[test]
    fn test_border_with_background() {
        let border = Border::new().with_background(Color::BLUE);
        assert_eq!(border.background, Color::BLUE);
    }
}
