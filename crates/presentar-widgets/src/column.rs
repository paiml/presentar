//! Column widget for vertical layout.

use presentar_core::{
    widget::LayoutResult, Canvas, Constraints, Event, Rect, Size, TypeId, Widget,
};
use serde::{Deserialize, Serialize};
use std::any::Any;

use crate::row::{CrossAxisAlignment, MainAxisAlignment};

/// Column widget for vertical layout of children.
#[derive(Serialize, Deserialize)]
pub struct Column {
    /// Main axis (vertical) alignment
    main_axis_alignment: MainAxisAlignment,
    /// Cross axis (horizontal) alignment
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

impl Default for Column {
    fn default() -> Self {
        Self::new()
    }
}

impl Column {
    /// Create a new empty column.
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

impl Widget for Column {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        if self.children.is_empty() {
            return Size::ZERO;
        }

        let mut max_width = 0.0f32;
        let mut total_height = 0.0f32;

        // Measure all children
        for (i, child) in self.children.iter().enumerate() {
            let child_constraints = Constraints::new(
                0.0,
                constraints.max_width,
                0.0,
                (constraints.max_height - total_height).max(0.0),
            );

            let child_size = child.measure(child_constraints);
            max_width = max_width.max(child_size.width);
            total_height += child_size.height;

            if i < self.children.len() - 1 {
                total_height += self.gap;
            }
        }

        constraints.constrain(Size::new(max_width, total_height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        self.child_bounds.clear();

        if self.children.is_empty() {
            return LayoutResult { size: Size::ZERO };
        }

        // First pass: measure children
        let mut child_sizes: Vec<Size> = Vec::with_capacity(self.children.len());
        let mut total_height = 0.0f32;

        for child in &self.children {
            let child_constraints = Constraints::loose(bounds.size());
            let size = child.measure(child_constraints);
            total_height += size.height;
            child_sizes.push(size);
        }

        let gaps_height = self.gap * (self.children.len() - 1).max(0) as f32;
        let content_height = total_height + gaps_height;
        let remaining_space = (bounds.height - content_height).max(0.0);

        // Calculate starting position based on alignment
        let (mut y, extra_gap) = match self.main_axis_alignment {
            MainAxisAlignment::Start => (bounds.y, 0.0),
            MainAxisAlignment::End => (bounds.y + remaining_space, 0.0),
            MainAxisAlignment::Center => (bounds.y + remaining_space / 2.0, 0.0),
            MainAxisAlignment::SpaceBetween => {
                if self.children.len() > 1 {
                    (bounds.y, remaining_space / (self.children.len() - 1) as f32)
                } else {
                    (bounds.y, 0.0)
                }
            }
            MainAxisAlignment::SpaceAround => {
                let gap = remaining_space / self.children.len() as f32;
                (bounds.y + gap / 2.0, gap)
            }
            MainAxisAlignment::SpaceEvenly => {
                let gap = remaining_space / (self.children.len() + 1) as f32;
                (bounds.y + gap, gap)
            }
        };

        // Second pass: position children
        let num_children = self.children.len();
        for (i, (child, size)) in self.children.iter_mut().zip(child_sizes.iter()).enumerate() {
            let x = match self.cross_axis_alignment {
                CrossAxisAlignment::Start | CrossAxisAlignment::Stretch => bounds.x,
                CrossAxisAlignment::End => bounds.x + bounds.width - size.width,
                CrossAxisAlignment::Center => bounds.x + (bounds.width - size.width) / 2.0,
            };

            let width = if self.cross_axis_alignment == CrossAxisAlignment::Stretch {
                bounds.width
            } else {
                size.width
            };

            let child_bounds = Rect::new(x, y, width, size.height);
            child.layout(child_bounds);
            self.child_bounds.push(child_bounds);

            y += size.height + self.gap + extra_gap;
            if i < num_children - 1 && self.main_axis_alignment == MainAxisAlignment::SpaceBetween {
                y += extra_gap - self.gap;
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
    fn test_column_empty() {
        let col = Column::new();
        let size = col.measure(Constraints::loose(Size::new(100.0, 100.0)));
        assert_eq!(size, Size::ZERO);
    }

    #[test]
    fn test_column_builder() {
        let col = Column::new()
            .gap(10.0)
            .main_axis_alignment(MainAxisAlignment::Center)
            .cross_axis_alignment(CrossAxisAlignment::Start)
            .with_test_id("my-column");

        assert_eq!(col.gap, 10.0);
        assert_eq!(col.main_axis_alignment, MainAxisAlignment::Center);
        assert_eq!(col.cross_axis_alignment, CrossAxisAlignment::Start);
        assert_eq!(Widget::test_id(&col), Some("my-column"));
    }

    #[test]
    fn test_column_default() {
        let col = Column::default();
        assert_eq!(col.main_axis_alignment, MainAxisAlignment::Start);
        assert_eq!(col.cross_axis_alignment, CrossAxisAlignment::Center);
        assert_eq!(col.gap, 0.0);
    }
}
