//! Button widget for user interactions.

use presentar_core::{
    widget::{AccessibleRole, FontWeight, LayoutResult, TextStyle},
    Canvas, Color, Constraints, CornerRadius, Event, MouseButton, Point, Rect, Size, TypeId,
    Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Button widget with label and click handling.
#[derive(Clone, Serialize, Deserialize)]
pub struct Button {
    /// Button label
    label: String,
    /// Background color (normal state)
    background: Color,
    /// Background color (hover state)
    background_hover: Color,
    /// Background color (pressed state)
    background_pressed: Color,
    /// Text color
    text_color: Color,
    /// Corner radius
    corner_radius: CornerRadius,
    /// Padding
    padding: f32,
    /// Font size
    font_size: f32,
    /// Whether button is disabled
    disabled: bool,
    /// Test ID
    test_id_value: Option<String>,
    /// Accessible name (overrides label)
    accessible_name: Option<String>,
    /// Current hover state
    #[serde(skip)]
    hovered: bool,
    /// Current pressed state
    #[serde(skip)]
    pressed: bool,
    /// Cached bounds
    #[serde(skip)]
    bounds: Rect,
}

/// Message emitted when button is clicked.
#[derive(Debug, Clone)]
pub struct ButtonClicked;

impl Button {
    /// Create a new button with label.
    #[must_use]
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            background: Color::from_hex("#6366f1").unwrap_or(Color::BLACK),
            background_hover: Color::from_hex("#4f46e5").unwrap_or(Color::BLACK),
            background_pressed: Color::from_hex("#4338ca").unwrap_or(Color::BLACK),
            text_color: Color::WHITE,
            corner_radius: CornerRadius::uniform(4.0),
            padding: 12.0,
            font_size: 14.0,
            disabled: false,
            test_id_value: None,
            accessible_name: None,
            hovered: false,
            pressed: false,
            bounds: Rect::default(),
        }
    }

    /// Set background color.
    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    /// Set hover background color.
    #[must_use]
    pub fn background_hover(mut self, color: Color) -> Self {
        self.background_hover = color;
        self
    }

    /// Set pressed background color.
    #[must_use]
    pub fn background_pressed(mut self, color: Color) -> Self {
        self.background_pressed = color;
        self
    }

    /// Set text color.
    #[must_use]
    pub fn text_color(mut self, color: Color) -> Self {
        self.text_color = color;
        self
    }

    /// Set corner radius.
    #[must_use]
    pub fn corner_radius(mut self, radius: CornerRadius) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Set padding.
    #[must_use]
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set font size.
    #[must_use]
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set disabled state.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set test ID.
    #[must_use]
    pub fn with_test_id(mut self, id: impl Into<String>) -> Self {
        self.test_id_value = Some(id.into());
        self
    }

    /// Set accessible name.
    #[must_use]
    pub fn with_accessible_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }

    /// Get the current background color based on state.
    fn current_background(&self) -> Color {
        if self.disabled {
            // Desaturated version
            let gray = (self.background.r + self.background.g + self.background.b) / 3.0;
            Color::rgb(gray, gray, gray)
        } else if self.pressed {
            self.background_pressed
        } else if self.hovered {
            self.background_hover
        } else {
            self.background
        }
    }

    /// Estimate text size.
    fn estimate_text_size(&self) -> Size {
        let char_width = self.font_size * 0.6;
        let width = self.label.len() as f32 * char_width;
        let height = self.font_size * 1.2;
        Size::new(width, height)
    }
}

impl Widget for Button {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let text_size = self.estimate_text_size();
        let size = Size::new(
            text_size.width + self.padding * 2.0,
            text_size.height + self.padding * 2.0,
        );
        constraints.constrain(size)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        // Draw background
        canvas.fill_rect(self.bounds, self.current_background());

        // Draw text centered
        let text_size = self.estimate_text_size();
        let text_pos = Point::new(
            self.bounds.x + (self.bounds.width - text_size.width) / 2.0,
            self.bounds.y + (self.bounds.height - text_size.height) / 2.0,
        );

        let style = TextStyle {
            size: self.font_size,
            color: if self.disabled {
                Color::rgb(0.7, 0.7, 0.7)
            } else {
                self.text_color
            },
            weight: FontWeight::Medium,
            ..Default::default()
        };

        canvas.draw_text(&self.label, text_pos, &style);
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        if self.disabled {
            return None;
        }

        match event {
            Event::MouseEnter => {
                self.hovered = true;
                None
            }
            Event::MouseLeave => {
                self.hovered = false;
                self.pressed = false;
                None
            }
            Event::MouseDown {
                position,
                button: MouseButton::Left,
            } => {
                if self.bounds.contains_point(position) {
                    self.pressed = true;
                }
                None
            }
            Event::MouseUp {
                position,
                button: MouseButton::Left,
            } => {
                let was_pressed = self.pressed;
                self.pressed = false;

                if was_pressed && self.bounds.contains_point(position) {
                    Some(Box::new(ButtonClicked))
                } else {
                    None
                }
            }
            Event::KeyDown {
                key: presentar_core::Key::Enter | presentar_core::Key::Space,
            } => {
                self.pressed = true;
                None
            }
            Event::KeyUp {
                key: presentar_core::Key::Enter | presentar_core::Key::Space,
            } => {
                self.pressed = false;
                Some(Box::new(ButtonClicked))
            }
            _ => None,
        }
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &[]
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }

    fn is_interactive(&self) -> bool {
        !self.disabled
    }

    fn is_focusable(&self) -> bool {
        !self.disabled
    }

    fn accessible_name(&self) -> Option<&str> {
        self.accessible_name.as_deref().or(Some(&self.label))
    }

    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Button
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id_value.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use presentar_core::Widget;

    #[test]
    fn test_button_new() {
        let b = Button::new("Click me");
        assert_eq!(b.label, "Click me");
        assert!(!b.disabled);
    }

    #[test]
    fn test_button_builder() {
        let b = Button::new("Test")
            .padding(20.0)
            .font_size(18.0)
            .disabled(true)
            .with_test_id("my-button");

        assert_eq!(b.padding, 20.0);
        assert_eq!(b.font_size, 18.0);
        assert!(b.disabled);
        assert_eq!(Widget::test_id(&b), Some("my-button"));
    }

    #[test]
    fn test_button_accessible() {
        let b = Button::new("OK");
        assert_eq!(Widget::accessible_name(&b), Some("OK"));
        assert_eq!(Widget::accessible_role(&b), AccessibleRole::Button);
        assert!(Widget::is_focusable(&b));
    }

    #[test]
    fn test_button_disabled_not_focusable() {
        let b = Button::new("OK").disabled(true);
        assert!(!Widget::is_focusable(&b));
        assert!(!Widget::is_interactive(&b));
    }

    #[test]
    fn test_button_measure() {
        let b = Button::new("Test");
        let size = b.measure(Constraints::loose(Size::new(1000.0, 1000.0)));
        assert!(size.width > 0.0);
        assert!(size.height > 0.0);
    }
}
