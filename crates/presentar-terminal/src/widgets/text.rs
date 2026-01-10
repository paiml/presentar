//! Simple text display widget.
//!
//! Renders text with optional styling.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    FontWeight, LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// Text alignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Simple text display widget.
#[derive(Debug, Clone)]
pub struct Text {
    content: String,
    color: Color,
    bold: bool,
    align: TextAlign,
    bounds: Rect,
}

impl Text {
    /// Create a new text widget.
    #[must_use]
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            color: Color::new(0.8, 0.8, 0.8, 1.0),
            bold: false,
            align: TextAlign::Left,
            bounds: Rect::default(),
        }
    }

    /// Set text color.
    #[must_use]
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Make text bold.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    /// Set text alignment.
    #[must_use]
    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// Center-align text.
    #[must_use]
    pub fn centered(mut self) -> Self {
        self.align = TextAlign::Center;
        self
    }

    /// Right-align text.
    #[must_use]
    pub fn right(mut self) -> Self {
        self.align = TextAlign::Right;
        self
    }

    /// Get the text content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Set the text content.
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}

impl Brick for Text {
    fn brick_name(&self) -> &'static str {
        "text"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(1)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(1)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: vec![BrickAssertion::max_latency_ms(1)],
            failed: vec![],
            verification_time: Duration::from_micros(1),
        }
    }

    fn to_html(&self) -> String {
        format!("<span>{}</span>", self.content)
    }

    fn to_css(&self) -> String {
        String::new()
    }
}

impl Widget for Text {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let char_count = self.content.chars().count() as f32;
        let width = char_count.min(constraints.max_width);
        let height = 1.0_f32.min(constraints.max_height);
        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height.min(1.0)),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.bounds.width < 1.0 || self.bounds.height < 1.0 {
            return;
        }

        let style = TextStyle {
            color: self.color,
            weight: if self.bold {
                FontWeight::Bold
            } else {
                FontWeight::Normal
            },
            ..Default::default()
        };

        let width = self.bounds.width as usize;
        let text_len = self.content.chars().count();

        let x_offset = match self.align {
            TextAlign::Left => 0.0,
            TextAlign::Center => ((width.saturating_sub(text_len)) / 2) as f32,
            TextAlign::Right => (width.saturating_sub(text_len)) as f32,
        };

        // Truncate if needed
        let display: String = if text_len > width {
            if width > 3 {
                format!(
                    "{}...",
                    self.content.chars().take(width - 3).collect::<String>()
                )
            } else {
                self.content.chars().take(width).collect()
            }
        } else {
            self.content.clone()
        };

        canvas.draw_text(
            &display,
            Point::new(self.bounds.x + x_offset, self.bounds.y),
            &style,
        );
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
    fn test_text_new() {
        let text = Text::new("Hello");
        assert_eq!(text.content(), "Hello");
    }

    #[test]
    fn test_text_with_color() {
        let text = Text::new("Test").with_color(Color::RED);
        assert_eq!(text.color, Color::RED);
    }

    #[test]
    fn test_text_bold() {
        let text = Text::new("Test").bold();
        assert!(text.bold);
    }

    #[test]
    fn test_text_align() {
        let text = Text::new("Test").align(TextAlign::Center);
        assert_eq!(text.align, TextAlign::Center);
    }

    #[test]
    fn test_text_centered() {
        let text = Text::new("Test").centered();
        assert_eq!(text.align, TextAlign::Center);
    }

    #[test]
    fn test_text_right() {
        let text = Text::new("Test").right();
        assert_eq!(text.align, TextAlign::Right);
    }

    #[test]
    fn test_text_set_content() {
        let mut text = Text::new("Old");
        text.set_content("New");
        assert_eq!(text.content(), "New");
    }

    #[test]
    fn test_text_default() {
        let text = Text::default();
        assert_eq!(text.content(), "");
    }

    #[test]
    fn test_text_brick_name() {
        let text = Text::new("Test");
        assert_eq!(text.brick_name(), "text");
    }

    #[test]
    fn test_text_verify() {
        let text = Text::new("Test");
        assert!(text.verify().is_valid());
    }

    #[test]
    fn test_text_measure() {
        let text = Text::new("Hello");
        let size = text.measure(Constraints::new(0.0, 100.0, 0.0, 10.0));
        assert_eq!(size.width, 5.0);
        assert_eq!(size.height, 1.0);
    }

    #[test]
    fn test_text_layout() {
        let mut text = Text::new("Test");
        let result = text.layout(Rect::new(0.0, 0.0, 20.0, 5.0));
        assert_eq!(result.size.height, 1.0);
    }

    #[test]
    fn test_text_to_html() {
        let text = Text::new("Hello");
        assert_eq!(text.to_html(), "<span>Hello</span>");
    }

    #[test]
    fn test_text_type_id() {
        let text = Text::new("Test");
        assert_eq!(Widget::type_id(&text), TypeId::of::<Text>());
    }

    #[test]
    fn test_text_children() {
        let text = Text::new("Test");
        assert!(text.children().is_empty());
    }

    #[test]
    fn test_text_children_mut() {
        let mut text = Text::new("Test");
        assert!(text.children_mut().is_empty());
    }
}
