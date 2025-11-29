//! Container widget for layout grouping.

use presentar_core::{
    widget::LayoutResult, Canvas, Color, Constraints, CornerRadius, Event, Rect, Size, TypeId,
    Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Container widget for grouping and styling children.
#[derive(Serialize, Deserialize)]
pub struct Container {
    /// Background color
    pub background: Option<Color>,
    /// Corner radius for rounded corners
    pub corner_radius: CornerRadius,
    /// Padding (all sides)
    pub padding: f32,
    /// Minimum width constraint
    pub min_width: Option<f32>,
    /// Minimum height constraint
    pub min_height: Option<f32>,
    /// Maximum width constraint
    pub max_width: Option<f32>,
    /// Maximum height constraint
    pub max_height: Option<f32>,
    /// Children widgets
    #[serde(skip)]
    children: Vec<Box<dyn Widget>>,
    /// Test ID for this widget
    test_id_value: Option<String>,
    /// Cached bounds after layout
    #[serde(skip)]
    bounds: Rect,
}

impl Default for Container {
    fn default() -> Self {
        Self {
            background: None,
            corner_radius: CornerRadius::ZERO,
            padding: 0.0,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            children: Vec::new(),
            test_id_value: None,
            bounds: Rect::default(),
        }
    }
}

impl Container {
    /// Create a new empty container.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the background color.
    #[must_use]
    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Set the corner radius.
    #[must_use]
    pub fn corner_radius(mut self, radius: CornerRadius) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Set uniform padding on all sides.
    #[must_use]
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set minimum width.
    #[must_use]
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = Some(width);
        self
    }

    /// Set minimum height.
    #[must_use]
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Set maximum width.
    #[must_use]
    pub fn max_width(mut self, width: f32) -> Self {
        self.max_width = Some(width);
        self
    }

    /// Set maximum height.
    #[must_use]
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Add a child widget.
    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    /// Set the test ID.
    #[must_use]
    pub fn with_test_id(mut self, id: impl Into<String>) -> Self {
        self.test_id_value = Some(id.into());
        self
    }
}

impl Widget for Container {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        let padding2 = self.padding * 2.0;

        // Measure children
        let child_constraints = Constraints::new(
            0.0,
            (constraints.max_width - padding2).max(0.0),
            0.0,
            (constraints.max_height - padding2).max(0.0),
        );

        let mut child_size = Size::ZERO;
        for child in &self.children {
            let size = child.measure(child_constraints);
            child_size.width = child_size.width.max(size.width);
            child_size.height = child_size.height.max(size.height);
        }

        // Add padding and apply constraints
        let mut size = Size::new(child_size.width + padding2, child_size.height + padding2);

        // Apply min/max constraints
        if let Some(min_w) = self.min_width {
            size.width = size.width.max(min_w);
        }
        if let Some(min_h) = self.min_height {
            size.height = size.height.max(min_h);
        }
        if let Some(max_w) = self.max_width {
            size.width = size.width.min(max_w);
        }
        if let Some(max_h) = self.max_height {
            size.height = size.height.min(max_h);
        }

        constraints.constrain(size)
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;

        // Layout children within padded bounds
        let child_bounds = bounds.inset(self.padding);
        for child in &mut self.children {
            child.layout(child_bounds);
        }

        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        // Draw background
        if let Some(color) = self.background {
            canvas.fill_rect(self.bounds, color);
        }

        // Paint children
        for child in &self.children {
            child.paint(canvas);
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        // Propagate to children
        for child in &mut self.children {
            if let Some(msg) = child.event(event) {
                return Some(msg);
            }
        }
        None
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }

    fn test_id(&self) -> Option<&str> {
        self.test_id_value.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_default() {
        let c = Container::new();
        assert!(c.background.is_none());
        assert_eq!(c.padding, 0.0);
        assert!(c.children.is_empty());
    }

    #[test]
    fn test_container_builder() {
        let c = Container::new()
            .background(Color::WHITE)
            .padding(10.0)
            .min_width(100.0)
            .with_test_id("my-container");

        assert_eq!(c.background, Some(Color::WHITE));
        assert_eq!(c.padding, 10.0);
        assert_eq!(c.min_width, Some(100.0));
        assert_eq!(Widget::test_id(&c), Some("my-container"));
    }

    #[test]
    fn test_container_measure_empty() {
        let c = Container::new().padding(10.0);
        let size = c.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::new(20.0, 20.0)); // padding * 2
    }

    #[test]
    fn test_container_measure_with_min_size() {
        let c = Container::new().min_width(50.0).min_height(50.0);
        let size = c.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::new(50.0, 50.0));
    }

    #[test]
    fn test_container_measure_with_max_size() {
        let c = Container::new()
            .max_width(30.0)
            .max_height(30.0)
            .min_width(100.0);
        let size = c.measure(Constraints::loose(Size::new(200.0, 200.0)));
        assert_eq!(size.width, 30.0); // max wins over min
    }

    #[test]
    fn test_container_corner_radius() {
        let c = Container::new().corner_radius(CornerRadius::uniform(8.0));
        assert_eq!(c.corner_radius, CornerRadius::uniform(8.0));
    }

    #[test]
    fn test_container_type_id() {
        let c = Container::new();
        assert_eq!(Widget::type_id(&c), TypeId::of::<Container>());
    }

    #[test]
    fn test_container_layout_sets_bounds() {
        let mut c = Container::new().padding(10.0);
        let result = c.layout(Rect::new(0.0, 0.0, 100.0, 80.0));
        assert_eq!(result.size, Size::new(100.0, 80.0));
        assert_eq!(c.bounds, Rect::new(0.0, 0.0, 100.0, 80.0));
    }

    #[test]
    fn test_container_children_empty() {
        let c = Container::new();
        assert!(c.children().is_empty());
    }

    #[test]
    fn test_container_event_no_children_returns_none() {
        let mut c = Container::new();
        c.layout(Rect::new(0.0, 0.0, 100.0, 100.0));
        let result = c.event(&Event::MouseEnter);
        assert!(result.is_none());
    }

    // Paint tests
    use presentar_core::draw::DrawCommand;
    use presentar_core::RecordingCanvas;

    #[test]
    fn test_container_paint_no_background() {
        let mut c = Container::new();
        c.layout(Rect::new(0.0, 0.0, 100.0, 100.0));
        let mut canvas = RecordingCanvas::new();
        c.paint(&mut canvas);
        assert_eq!(canvas.command_count(), 0);
    }

    #[test]
    fn test_container_paint_with_background() {
        let mut c = Container::new().background(Color::RED);
        c.layout(Rect::new(0.0, 0.0, 100.0, 50.0));
        let mut canvas = RecordingCanvas::new();
        c.paint(&mut canvas);
        assert_eq!(canvas.command_count(), 1);
        match &canvas.commands()[0] {
            DrawCommand::Rect { bounds, style, .. } => {
                assert_eq!(bounds.width, 100.0);
                assert_eq!(bounds.height, 50.0);
                assert_eq!(style.fill, Some(Color::RED));
            }
            _ => panic!("Expected Rect"),
        }
    }
}
