//! Row widget for horizontal layout.

use presentar_core::{
    widget::LayoutResult, Canvas, Constraints, Event, Rect, Size, TypeId, Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Horizontal alignment options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MainAxisAlignment {
    /// Pack children at the start
    #[default]
    Start,
    /// Pack children at the end
    End,
    /// Center children
    Center,
    /// Distribute space evenly between children
    SpaceBetween,
    /// Distribute space evenly around children
    SpaceAround,
    /// Distribute space evenly, including edges
    SpaceEvenly,
}

/// Vertical alignment options for row children.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CrossAxisAlignment {
    /// Align to the start (top)
    Start,
    /// Align to the end (bottom)
    End,
    /// Center vertically
    #[default]
    Center,
    /// Stretch to fill
    Stretch,
}

/// Row widget for horizontal layout of children.
#[derive(Serialize, Deserialize)]
pub struct Row {
    /// Main axis (horizontal) alignment
    main_axis_alignment: MainAxisAlignment,
    /// Cross axis (vertical) alignment
    cross_axis_alignment: CrossAxisAlignment,
    /// Gap between children
    gap: f32,
    /// Children widgets
    #[serde(skip)]
    children: Vec<Box<dyn Widget>>,
    /// Test ID
    test_id_value: Option<String>,
    /// Cached bounds
    #[serde(skip)]
    bounds: Rect,
    /// Cached child positions
    #[serde(skip)]
    child_bounds: Vec<Rect>,
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Row {
    /// Create a new empty row.
    #[must_use]
    pub fn new() -> Self {
        Self {
            main_axis_alignment: MainAxisAlignment::Start,
            cross_axis_alignment: CrossAxisAlignment::Center,
            gap: 0.0,
            children: Vec::new(),
            test_id_value: None,
            bounds: Rect::default(),
            child_bounds: Vec::new(),
        }
    }

    /// Set main axis alignment.
    #[must_use]
    pub fn main_axis_alignment(mut self, alignment: MainAxisAlignment) -> Self {
        self.main_axis_alignment = alignment;
        self
    }

    /// Set cross axis alignment.
    #[must_use]
    pub fn cross_axis_alignment(mut self, alignment: CrossAxisAlignment) -> Self {
        self.cross_axis_alignment = alignment;
        self
    }

    /// Set gap between children.
    #[must_use]
    pub fn gap(mut self, gap: f32) -> Self {
        self.gap = gap;
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
}

impl Widget for Row {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        if self.children.is_empty() {
            return Size::ZERO;
        }

        let mut total_width = 0.0f32;
        let mut max_height = 0.0f32;

        // Measure all children
        for (i, child) in self.children.iter().enumerate() {
            let child_constraints = Constraints::new(
                0.0,
                (constraints.max_width - total_width).max(0.0),
                0.0,
                constraints.max_height,
            );

            let child_size = child.measure(child_constraints);
            total_width += child_size.width;
            max_height = max_height.max(child_size.height);

            if i < self.children.len() - 1 {
                total_width += self.gap;
            }
        }

        constraints.constrain(Size::new(total_width, max_height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        self.child_bounds.clear();

        if self.children.is_empty() {
            return LayoutResult { size: Size::ZERO };
        }

        // First pass: measure children
        let mut child_sizes: Vec<Size> = Vec::with_capacity(self.children.len());
        let mut total_width = 0.0f32;

        for child in &self.children {
            let child_constraints = Constraints::loose(bounds.size());
            let size = child.measure(child_constraints);
            total_width += size.width;
            child_sizes.push(size);
        }

        let gaps_width = self.gap * (self.children.len() - 1).max(0) as f32;
        let content_width = total_width + gaps_width;
        let remaining_space = (bounds.width - content_width).max(0.0);

        // Calculate starting position based on alignment
        let (mut x, extra_gap) = match self.main_axis_alignment {
            MainAxisAlignment::Start => (bounds.x, 0.0),
            MainAxisAlignment::End => (bounds.x + remaining_space, 0.0),
            MainAxisAlignment::Center => (bounds.x + remaining_space / 2.0, 0.0),
            MainAxisAlignment::SpaceBetween => {
                if self.children.len() > 1 {
                    (bounds.x, remaining_space / (self.children.len() - 1) as f32)
                } else {
                    (bounds.x, 0.0)
                }
            }
            MainAxisAlignment::SpaceAround => {
                let gap = remaining_space / self.children.len() as f32;
                (bounds.x + gap / 2.0, gap)
            }
            MainAxisAlignment::SpaceEvenly => {
                let gap = remaining_space / (self.children.len() + 1) as f32;
                (bounds.x + gap, gap)
            }
        };

        // Second pass: position children
        let num_children = self.children.len();
        for (i, (child, size)) in self.children.iter_mut().zip(child_sizes.iter()).enumerate() {
            let y = match self.cross_axis_alignment {
                CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => bounds.y,
                CrossAxisAlignment::End => bounds.y + bounds.height - size.height,
                CrossAxisAlignment::Center => bounds.y + (bounds.height - size.height) / 2.0,
            };

            let height = if self.cross_axis_alignment == CrossAxisAlignment::Stretch {
                bounds.height
            } else {
                size.height
            };

            let child_bounds = Rect::new(x, y, size.width, height);
            child.layout(child_bounds);
            self.child_bounds.push(child_bounds);

            x += size.width + self.gap + extra_gap;
            if i < num_children - 1 && self.main_axis_alignment == MainAxisAlignment::SpaceBetween {
                x += extra_gap - self.gap; // SpaceBetween replaces gap with extra_gap
            }
        }

        LayoutResult {
            size: bounds.size(),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        for child in &self.children {
            child.paint(canvas);
        }
    }

    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>> {
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
    use presentar_core::Widget;

    #[test]
    fn test_row_empty() {
        let row = Row::new();
        let size = row.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::ZERO);
    }

    #[test]
    fn test_row_builder() {
        let row = Row::new()
            .gap(10.0)
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_test_id("my-row");

        assert_eq!(row.gap, 10.0);
        assert_eq!(row.main_axis_alignment, MainAxisAlignment::Center);
        assert_eq!(row.cross_axis_alignment, CrossAxisAlignment::Start);
        assert_eq!(Widget::test_id(&row), Some("my-row"));
    }

    #[test]
    fn test_row_default() {
        let row = Row::default();
        assert_eq!(row.main_axis_alignment, MainAxisAlignment::Start);
        assert_eq!(row.cross_axis_alignment, CrossAxisAlignment::Center);
        assert_eq!(row.gap, 0.0);
    }
}
