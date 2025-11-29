//! Stack widget for z-axis overlapping children.

use presentar_core::{
    widget::LayoutResult, Canvas, Constraints, Event, Rect, Size, TypeId, Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// How to align children within the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StackAlignment {
    /// Align to top-left corner
    #[default]
    TopLeft,
    /// Align to top center
    TopCenter,
    /// Align to top-right corner
    TopRight,
    /// Align to center-left
    CenterLeft,
    /// Center both axes
    Center,
    /// Align to center-right
    CenterRight,
    /// Align to bottom-left corner
    BottomLeft,
    /// Align to bottom center
    BottomCenter,
    /// Align to bottom-right corner
    BottomRight,
}

impl StackAlignment {
    /// Get horizontal offset ratio (0.0 = left, 0.5 = center, 1.0 = right).
    #[must_use]
    pub fn horizontal_ratio(&self) -> f32 {
        match self {
            Self::TopLeft | Self::CenterLeft | Self::BottomLeft => 0.0,
            Self::TopCenter | Self::Center | Self::BottomCenter => 0.5,
            Self::TopRight | Self::CenterRight | Self::BottomRight => 1.0,
        }
    }

    /// Get vertical offset ratio (0.0 = top, 0.5 = center, 1.0 = bottom).
    #[must_use]
    pub fn vertical_ratio(&self) -> f32 {
        match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => 0.0,
            Self::CenterLeft | Self::Center | Self::CenterRight => 0.5,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => 1.0,
        }
    }
}

/// How to size the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StackFit {
    /// Size to the largest child
    #[default]
    Loose,
    /// Expand to fill available space
    Expand,
}

/// Stack widget for overlaying children.
///
/// Children are painted in order, with later children on top.
#[derive(Serialize, Deserialize)]
pub struct Stack {
    /// Alignment for non-positioned children
    alignment: StackAlignment,
    /// How to size the stack
    fit: StackFit,
    /// Children widgets
    #[serde(skip)]
    children: Vec<Box<dyn Widget>>,
    /// Test ID
    test_id_value: Option<String>,
    /// Cached bounds
    #[serde(skip)]
    bounds: Rect,
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl Stack {
    /// Create a new empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self {
            alignment: StackAlignment::TopLeft,
            fit: StackFit::Loose,
            children: Vec::new(),
            test_id_value: None,
            bounds: Rect::default(),
        }
    }

    /// Set alignment.
    #[must_use]
    pub fn alignment(mut self, alignment: StackAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Set fit mode.
    #[must_use]
    pub fn fit(mut self, fit: StackFit) -> Self {
        self.fit = fit;
        self
    }

    /// Add a child widget.
    pub fn child(mut self, widget: impl Widget + 'static) -> Self {
        self.children.push(Box::new(widget));
        self
    }

    /// Set test ID.
    #[must_use]
    pub fn with_test_id(mut self, id: impl Into<String>) -> Self {
        self.test_id_value = Some(id.into());
        self
    }

    /// Get alignment.
    #[must_use]
    pub fn get_alignment(&self) -> StackAlignment {
        self.alignment
    }

    /// Get fit mode.
    #[must_use]
    pub fn get_fit(&self) -> StackFit {
        self.fit
    }
}

impl Widget for Stack {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        if self.children.is_empty() {
            return match self.fit {
                StackFit::Loose => Size::ZERO,
                StackFit::Expand => Size::new(constraints.max_width, constraints.max_height),
            };
        }

        let mut max_width = 0.0f32;
        let mut max_height = 0.0f32;

        // Measure all children - size is the largest child
        for child in &self.children {
            let child_size = child.measure(constraints);
            max_width = max_width.max(child_size.width);
            max_height = max_height.max(child_size.height);
        }

        match self.fit {
            StackFit::Loose => constraints.constrain(Size::new(max_width, max_height)),
            StackFit::Expand => Size::new(constraints.max_width, constraints.max_height),
        }
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;

        // Layout all children with alignment
        for child in &mut self.children {
            let child_constraints = Constraints::loose(bounds.size());
            let child_size = child.measure(child_constraints);

            // Calculate position based on alignment
            let x =
                bounds.x + (bounds.width - child_size.width) * self.alignment.horizontal_ratio();
            let y =
                bounds.y + (bounds.height - child_size.height) * self.alignment.vertical_ratio();

            let child_bounds = Rect::new(x, y, child_size.width, child_size.height);
            child.layout(child_bounds);
        }

        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        // Paint children in order (first = bottom, last = top)
        for child in &self.children {
            child.paint(canvas);
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
        // Process events in reverse order (top-most first)
        for child in self.children.iter_mut().rev() {
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
    use presentar_core::Widget;

    // =========================================================================
    // StackAlignment Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_alignment_default() {
        assert_eq!(StackAlignment::default(), StackAlignment::TopLeft);
    }

    #[test]
    fn test_stack_alignment_horizontal_ratio() {
        // Left alignments
        assert_eq!(StackAlignment::TopLeft.horizontal_ratio(), 0.0);
        assert_eq!(StackAlignment::CenterLeft.horizontal_ratio(), 0.0);
        assert_eq!(StackAlignment::BottomLeft.horizontal_ratio(), 0.0);

        // Center alignments
        assert_eq!(StackAlignment::TopCenter.horizontal_ratio(), 0.5);
        assert_eq!(StackAlignment::Center.horizontal_ratio(), 0.5);
        assert_eq!(StackAlignment::BottomCenter.horizontal_ratio(), 0.5);

        // Right alignments
        assert_eq!(StackAlignment::TopRight.horizontal_ratio(), 1.0);
        assert_eq!(StackAlignment::CenterRight.horizontal_ratio(), 1.0);
        assert_eq!(StackAlignment::BottomRight.horizontal_ratio(), 1.0);
    }

    #[test]
    fn test_stack_alignment_vertical_ratio() {
        // Top alignments
        assert_eq!(StackAlignment::TopLeft.vertical_ratio(), 0.0);
        assert_eq!(StackAlignment::TopCenter.vertical_ratio(), 0.0);
        assert_eq!(StackAlignment::TopRight.vertical_ratio(), 0.0);

        // Center alignments
        assert_eq!(StackAlignment::CenterLeft.vertical_ratio(), 0.5);
        assert_eq!(StackAlignment::Center.vertical_ratio(), 0.5);
        assert_eq!(StackAlignment::CenterRight.vertical_ratio(), 0.5);

        // Bottom alignments
        assert_eq!(StackAlignment::BottomLeft.vertical_ratio(), 1.0);
        assert_eq!(StackAlignment::BottomCenter.vertical_ratio(), 1.0);
        assert_eq!(StackAlignment::BottomRight.vertical_ratio(), 1.0);
    }

    // =========================================================================
    // StackFit Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_fit_default() {
        assert_eq!(StackFit::default(), StackFit::Loose);
    }

    // =========================================================================
    // Stack Construction Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_new() {
        let stack = Stack::new();
        assert_eq!(stack.get_alignment(), StackAlignment::TopLeft);
        assert_eq!(stack.get_fit(), StackFit::Loose);
        assert!(stack.children().is_empty());
    }

    #[test]
    fn test_stack_default() {
        let stack = Stack::default();
        assert_eq!(stack.get_alignment(), StackAlignment::TopLeft);
        assert_eq!(stack.get_fit(), StackFit::Loose);
    }

    #[test]
    fn test_stack_builder() {
        let stack = Stack::new()
            .alignment(StackAlignment::Center)
            .fit(StackFit::Expand)
            .with_test_id("my-stack");

        assert_eq!(stack.get_alignment(), StackAlignment::Center);
        assert_eq!(stack.get_fit(), StackFit::Expand);
        assert_eq!(Widget::test_id(&stack), Some("my-stack"));
    }

    // =========================================================================
    // Stack Measure Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_empty_loose() {
        let stack = Stack::new().fit(StackFit::Loose);
        let size = stack.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::ZERO);
    }

    #[test]
    fn test_stack_empty_expand() {
        let stack = Stack::new().fit(StackFit::Expand);
        let size = stack.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::new(100.0, 100.0));
    }

    // =========================================================================
    // Stack Widget Trait Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_type_id() {
        let stack = Stack::new();
        let type_id = Widget::type_id(&stack);
        assert_eq!(type_id, TypeId::of::<Stack>());
    }

    #[test]
    fn test_stack_test_id_none() {
        let stack = Stack::new();
        assert_eq!(Widget::test_id(&stack), None);
    }

    #[test]
    fn test_stack_test_id_some() {
        let stack = Stack::new().with_test_id("test-stack");
        assert_eq!(Widget::test_id(&stack), Some("test-stack"));
    }

    // =========================================================================
    // StackAlignment Tests - TESTS FIRST
    // =========================================================================

    #[test]
    fn test_stack_alignment_horizontal_ratios() {
        assert_eq!(StackAlignment::TopLeft.horizontal_ratio(), 0.0);
        assert_eq!(StackAlignment::TopCenter.horizontal_ratio(), 0.5);
        assert_eq!(StackAlignment::TopRight.horizontal_ratio(), 1.0);
        assert_eq!(StackAlignment::Center.horizontal_ratio(), 0.5);
        assert_eq!(StackAlignment::BottomRight.horizontal_ratio(), 1.0);
    }

    #[test]
    fn test_stack_alignment_vertical_ratios() {
        assert_eq!(StackAlignment::TopLeft.vertical_ratio(), 0.0);
        assert_eq!(StackAlignment::CenterLeft.vertical_ratio(), 0.5);
        assert_eq!(StackAlignment::BottomLeft.vertical_ratio(), 1.0);
        assert_eq!(StackAlignment::Center.vertical_ratio(), 0.5);
        assert_eq!(StackAlignment::BottomRight.vertical_ratio(), 1.0);
    }

    #[test]
    fn test_stack_alignment_default_is_top_left() {
        let align = StackAlignment::default();
        assert_eq!(align, StackAlignment::TopLeft);
    }

    #[test]
    fn test_stack_fit_default_is_loose() {
        let fit = StackFit::default();
        assert_eq!(fit, StackFit::Loose);
    }

    #[test]
    fn test_stack_layout_sets_bounds() {
        let mut stack = Stack::new();
        let result = stack.layout(Rect::new(10.0, 20.0, 100.0, 80.0));
        assert_eq!(result.size, Size::new(100.0, 80.0));
        assert_eq!(stack.bounds, Rect::new(10.0, 20.0, 100.0, 80.0));
    }

    #[test]
    fn test_stack_children_empty() {
        let stack = Stack::new();
        assert!(stack.children().is_empty());
    }

    #[test]
    fn test_stack_event_no_children() {
        let mut stack = Stack::new();
        stack.layout(Rect::new(0.0, 0.0, 100.0, 100.0));
        let result = stack.event(&Event::MouseEnter);
        assert!(result.is_none());
    }
}
