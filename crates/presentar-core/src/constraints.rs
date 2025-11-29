//! Layout constraints for widgets.

use crate::geometry::Size;
use serde::{Deserialize, Serialize};

/// Layout constraints that specify minimum and maximum sizes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Constraints {
    /// Minimum width
    pub min_width: f32,
    /// Maximum width
    pub max_width: f32,
    /// Minimum height
    pub min_height: f32,
    /// Maximum height
    pub max_height: f32,
}

impl Constraints {
    /// Create new constraints.
    #[must_use]
    pub const fn new(min_width: f32, max_width: f32, min_height: f32, max_height: f32) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    /// Create tight constraints that allow only the exact size.
    #[must_use]
    pub fn tight(size: Size) -> Self {
        Self::new(size.width, size.width, size.height, size.height)
    }

    /// Create loose constraints that allow any size up to the given maximum.
    #[must_use]
    pub fn loose(size: Size) -> Self {
        Self::new(0.0, size.width, 0.0, size.height)
    }

    /// Create unbounded constraints.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(0.0, f32::INFINITY, 0.0, f32::INFINITY)
    }

    /// Constrain a size to fit within these constraints.
    #[must_use]
    pub fn constrain(&self, size: Size) -> Size {
        Size::new(
            size.width.clamp(self.min_width, self.max_width),
            size.height.clamp(self.min_height, self.max_height),
        )
    }

    /// Check if constraints specify an exact size.
    #[must_use]
    pub fn is_tight(&self) -> bool {
        self.min_width == self.max_width && self.min_height == self.max_height
    }

    /// Check if width is bounded (not infinite).
    #[must_use]
    pub fn has_bounded_width(&self) -> bool {
        self.max_width.is_finite()
    }

    /// Check if height is bounded (not infinite).
    #[must_use]
    pub fn has_bounded_height(&self) -> bool {
        self.max_height.is_finite()
    }

    /// Check if both dimensions are bounded.
    #[must_use]
    pub fn is_bounded(&self) -> bool {
        self.has_bounded_width() && self.has_bounded_height()
    }

    /// Get the biggest size that satisfies these constraints.
    #[must_use]
    pub fn biggest(&self) -> Size {
        Size::new(
            if self.max_width.is_finite() {
                self.max_width
            } else {
                self.min_width
            },
            if self.max_height.is_finite() {
                self.max_height
            } else {
                self.min_height
            },
        )
    }

    /// Get the smallest size that satisfies these constraints.
    #[must_use]
    pub fn smallest(&self) -> Size {
        Size::new(self.min_width, self.min_height)
    }

    /// Create constraints with a different minimum width.
    #[must_use]
    pub fn with_min_width(&self, min_width: f32) -> Self {
        Self::new(min_width, self.max_width, self.min_height, self.max_height)
    }

    /// Create constraints with a different maximum width.
    #[must_use]
    pub fn with_max_width(&self, max_width: f32) -> Self {
        Self::new(self.min_width, max_width, self.min_height, self.max_height)
    }

    /// Create constraints with a different minimum height.
    #[must_use]
    pub fn with_min_height(&self, min_height: f32) -> Self {
        Self::new(self.min_width, self.max_width, min_height, self.max_height)
    }

    /// Create constraints with a different maximum height.
    #[must_use]
    pub fn with_max_height(&self, max_height: f32) -> Self {
        Self::new(self.min_width, self.max_width, self.min_height, max_height)
    }

    /// Deflate constraints by padding.
    #[must_use]
    pub fn deflate(&self, horizontal: f32, vertical: f32) -> Self {
        Self::new(
            (self.min_width - horizontal).max(0.0),
            (self.max_width - horizontal).max(0.0),
            (self.min_height - vertical).max(0.0),
            (self.max_height - vertical).max(0.0),
        )
    }
}

impl Default for Constraints {
    fn default() -> Self {
        Self::unbounded()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_default() {
        let c = Constraints::default();
        assert_eq!(c.min_width, 0.0);
        assert_eq!(c.max_width, f32::INFINITY);
    }

    #[test]
    fn test_constraints_biggest() {
        let c = Constraints::new(10.0, 100.0, 20.0, 200.0);
        assert_eq!(c.biggest(), Size::new(100.0, 200.0));
    }

    #[test]
    fn test_constraints_smallest() {
        let c = Constraints::new(10.0, 100.0, 20.0, 200.0);
        assert_eq!(c.smallest(), Size::new(10.0, 20.0));
    }

    #[test]
    fn test_constraints_deflate() {
        let c = Constraints::new(100.0, 200.0, 100.0, 200.0);
        let deflated = c.deflate(20.0, 20.0);
        assert_eq!(deflated.min_width, 80.0);
        assert_eq!(deflated.max_width, 180.0);
    }

    #[test]
    fn test_constraints_with_methods() {
        let c = Constraints::new(0.0, 100.0, 0.0, 100.0);
        assert_eq!(c.with_min_width(10.0).min_width, 10.0);
        assert_eq!(c.with_max_width(200.0).max_width, 200.0);
        assert_eq!(c.with_min_height(10.0).min_height, 10.0);
        assert_eq!(c.with_max_height(200.0).max_height, 200.0);
    }

    #[test]
    fn test_constraints_tight() {
        let c = Constraints::tight(Size::new(100.0, 50.0));
        assert_eq!(c.min_width, 100.0);
        assert_eq!(c.max_width, 100.0);
        assert_eq!(c.min_height, 50.0);
        assert_eq!(c.max_height, 50.0);
        assert!(c.is_tight());
    }

    #[test]
    fn test_constraints_loose() {
        let c = Constraints::loose(Size::new(100.0, 50.0));
        assert_eq!(c.min_width, 0.0);
        assert_eq!(c.max_width, 100.0);
        assert_eq!(c.min_height, 0.0);
        assert_eq!(c.max_height, 50.0);
        assert!(!c.is_tight());
    }

    #[test]
    fn test_constraints_unbounded() {
        let c = Constraints::unbounded();
        assert_eq!(c.min_width, 0.0);
        assert!(c.max_width.is_infinite());
        assert!(!c.is_bounded());
    }

    #[test]
    fn test_constraints_constrain() {
        let c = Constraints::new(10.0, 100.0, 20.0, 80.0);
        assert_eq!(c.constrain(Size::new(50.0, 50.0)), Size::new(50.0, 50.0));
        assert_eq!(c.constrain(Size::new(5.0, 5.0)), Size::new(10.0, 20.0));
        assert_eq!(c.constrain(Size::new(200.0, 200.0)), Size::new(100.0, 80.0));
    }

    #[test]
    fn test_constraints_is_tight_false() {
        let c = Constraints::new(0.0, 100.0, 0.0, 100.0);
        assert!(!c.is_tight());
    }

    #[test]
    fn test_constraints_has_bounded_width() {
        let c = Constraints::new(0.0, 100.0, 0.0, f32::INFINITY);
        assert!(c.has_bounded_width());
        assert!(!c.has_bounded_height());
    }

    #[test]
    fn test_constraints_is_bounded() {
        let bounded = Constraints::new(0.0, 100.0, 0.0, 100.0);
        assert!(bounded.is_bounded());

        let unbounded = Constraints::unbounded();
        assert!(!unbounded.is_bounded());
    }

    #[test]
    fn test_constraints_biggest_unbounded() {
        let c = Constraints::unbounded();
        assert_eq!(c.biggest(), Size::new(0.0, 0.0));
    }

    #[test]
    fn test_constraints_deflate_to_zero() {
        let c = Constraints::new(10.0, 20.0, 10.0, 20.0);
        let deflated = c.deflate(50.0, 50.0);
        assert_eq!(deflated.min_width, 0.0);
        assert_eq!(deflated.max_width, 0.0);
    }
}
