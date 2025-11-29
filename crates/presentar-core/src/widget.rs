//! Widget trait and related types.

use crate::constraints::Constraints;
use crate::event::Event;
use crate::geometry::{Rect, Size};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Unique identifier for a widget instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WidgetId(pub u64);

impl WidgetId {
    /// Create a new widget ID.
    #[must_use]
    pub const fn new(id: u64) -> Self {
        Self(id)
    }
}

/// Type identifier for widget types (used for diffing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId(std::any::TypeId);

impl TypeId {
    /// Get the type ID for a type.
    #[must_use]
    pub fn of<T: 'static>() -> Self {
        Self(std::any::TypeId::of::<T>())
    }
}

/// Result of laying out a widget.
#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutResult {
    /// Computed size after layout
    pub size: Size,
}

/// Core widget trait that all UI elements implement.
///
/// Widgets follow a measure-layout-paint cycle:
/// 1. `measure`: Compute intrinsic size given constraints
/// 2. `layout`: Position self and children within allocated bounds
/// 3. `paint`: Generate draw commands
pub trait Widget: Send + Sync {
    /// Get the type identifier for this widget type.
    fn type_id(&self) -> TypeId;

    /// Compute intrinsic size constraints.
    ///
    /// Called during the measure phase to determine the widget's preferred size.
    fn measure(&self, constraints: Constraints) -> Size;

    /// Position children within allocated bounds.
    ///
    /// Called during the layout phase after sizes are known.
    fn layout(&mut self, bounds: Rect) -> LayoutResult;

    /// Generate draw commands for rendering.
    ///
    /// Called during the paint phase to produce GPU draw commands.
    fn paint(&self, canvas: &mut dyn Canvas);

    /// Handle input events.
    ///
    /// Returns a message if the event triggered a state change.
    fn event(&mut self, event: &Event) -> Option<Box<dyn Any + Send>>;

    /// Get child widgets for tree traversal.
    fn children(&self) -> &[Box<dyn Widget>];

    /// Get mutable child widgets.
    fn children_mut(&mut self) -> &mut [Box<dyn Widget>];

    /// Check if this widget is interactive (can receive focus/events).
    fn is_interactive(&self) -> bool {
        false
    }

    /// Check if this widget can receive keyboard focus.
    fn is_focusable(&self) -> bool {
        false
    }

    /// Get the accessible name for screen readers.
    fn accessible_name(&self) -> Option<&str> {
        None
    }

    /// Get the accessible role.
    fn accessible_role(&self) -> AccessibleRole {
        AccessibleRole::Generic
    }

    /// Get the test ID for this widget (if any).
    fn test_id(&self) -> Option<&str> {
        None
    }
}

/// Canvas trait for paint operations.
///
/// This is a minimal abstraction over the rendering backend.
pub trait Canvas {
    /// Draw a filled rectangle.
    fn fill_rect(&mut self, rect: Rect, color: crate::Color);

    /// Draw a stroked rectangle.
    fn stroke_rect(&mut self, rect: Rect, color: crate::Color, width: f32);

    /// Draw text.
    fn draw_text(&mut self, text: &str, position: crate::Point, style: &TextStyle);

    /// Push a clip region.
    fn push_clip(&mut self, rect: Rect);

    /// Pop the clip region.
    fn pop_clip(&mut self);

    /// Push a transform.
    fn push_transform(&mut self, transform: Transform2D);

    /// Pop the transform.
    fn pop_transform(&mut self);
}

/// Text style for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextStyle {
    /// Font size in pixels
    pub size: f32,
    /// Text color
    pub color: crate::Color,
    /// Font weight
    pub weight: FontWeight,
    /// Font style
    pub style: FontStyle,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            size: 16.0,
            color: crate::Color::BLACK,
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        }
    }
}

/// Font weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontWeight {
    /// Thin (100)
    Thin,
    /// Light (300)
    Light,
    /// Normal (400)
    Normal,
    /// Medium (500)
    Medium,
    /// Semibold (600)
    Semibold,
    /// Bold (700)
    Bold,
    /// Black (900)
    Black,
}

/// Font style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FontStyle {
    /// Normal style
    Normal,
    /// Italic style
    Italic,
}

/// 2D affine transform.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Transform2D {
    /// Matrix elements [a, b, c, d, e, f] for:
    /// | a c e |
    /// | b d f |
    /// | 0 0 1 |
    pub matrix: [f32; 6],
}

impl Transform2D {
    /// Identity transform.
    pub const IDENTITY: Self = Self {
        matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// Create a translation transform.
    #[must_use]
    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            matrix: [1.0, 0.0, 0.0, 1.0, x, y],
        }
    }

    /// Create a scale transform.
    #[must_use]
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            matrix: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Create a rotation transform (angle in radians).
    #[must_use]
    pub fn rotate(angle: f32) -> Self {
        let (sin, cos) = angle.sin_cos();
        Self {
            matrix: [cos, sin, -sin, cos, 0.0, 0.0],
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Accessible role for screen readers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AccessibleRole {
    /// Generic element
    #[default]
    Generic,
    /// Button
    Button,
    /// Checkbox
    Checkbox,
    /// Text input
    TextInput,
    /// Link
    Link,
    /// Heading
    Heading,
    /// Image
    Image,
    /// List
    List,
    /// List item
    ListItem,
    /// Table
    Table,
    /// Table row
    TableRow,
    /// Table cell
    TableCell,
    /// Menu
    Menu,
    /// Menu item
    MenuItem,
    /// Slider
    Slider,
    /// Progress bar
    ProgressBar,
    /// Tab
    Tab,
    /// Tab panel
    TabPanel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_widget_id() {
        let id = WidgetId::new(42);
        assert_eq!(id.0, 42);
    }

    #[test]
    fn test_type_id() {
        let id1 = TypeId::of::<u32>();
        let id2 = TypeId::of::<u32>();
        let id3 = TypeId::of::<String>();

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_transform2d_identity() {
        let t = Transform2D::IDENTITY;
        assert_eq!(t.matrix, [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_transform2d_translate() {
        let t = Transform2D::translate(10.0, 20.0);
        assert_eq!(t.matrix[4], 10.0);
        assert_eq!(t.matrix[5], 20.0);
    }

    #[test]
    fn test_transform2d_scale() {
        let t = Transform2D::scale(2.0, 3.0);
        assert_eq!(t.matrix[0], 2.0);
        assert_eq!(t.matrix[3], 3.0);
    }

    #[test]
    fn test_text_style_default() {
        let style = TextStyle::default();
        assert_eq!(style.size, 16.0);
        assert_eq!(style.weight, FontWeight::Normal);
    }

    #[test]
    fn test_accessible_role_default() {
        assert_eq!(AccessibleRole::default(), AccessibleRole::Generic);
    }
}
