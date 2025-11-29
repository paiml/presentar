//! Flexbox layout types.

use serde::{Deserialize, Serialize};

/// Direction for flex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlexDirection {
    /// Horizontal (left to right)
    #[default]
    Row,
    /// Horizontal (right to left)
    RowReverse,
    /// Vertical (top to bottom)
    Column,
    /// Vertical (bottom to top)
    ColumnReverse,
}

/// Main axis alignment for flex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlexJustify {
    /// Pack items at the start
    #[default]
    Start,
    /// Pack items at the end
    End,
    /// Center items
    Center,
    /// Distribute space evenly between items
    SpaceBetween,
    /// Distribute space evenly around items
    SpaceAround,
    /// Distribute space evenly, including edges
    SpaceEvenly,
}

/// Cross axis alignment for flex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FlexAlign {
    /// Align to the start
    Start,
    /// Align to the end
    End,
    /// Center items
    #[default]
    Center,
    /// Stretch to fill
    Stretch,
    /// Align to baseline
    Baseline,
}

/// Flex item properties.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct FlexItem {
    /// Flex grow factor
    pub grow: f32,
    /// Flex shrink factor
    pub shrink: f32,
    /// Flex basis (initial size)
    pub basis: Option<f32>,
    /// Self alignment override
    pub align_self: Option<FlexAlign>,
}

impl FlexItem {
    /// Create a new flex item with default values.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the grow factor.
    #[must_use]
    pub const fn grow(mut self, grow: f32) -> Self {
        self.grow = grow;
        self
    }

    /// Set the shrink factor.
    #[must_use]
    pub const fn shrink(mut self, shrink: f32) -> Self {
        self.shrink = shrink;
        self
    }

    /// Set the basis.
    #[must_use]
    pub const fn basis(mut self, basis: f32) -> Self {
        self.basis = Some(basis);
        self
    }

    /// Set self alignment.
    #[must_use]
    pub const fn align_self(mut self, align: FlexAlign) -> Self {
        self.align_self = Some(align);
        self
    }
}

/// Distribute available space among flex items.
#[must_use]
#[allow(dead_code)]
pub(crate) fn distribute_flex(items: &[FlexItem], sizes: &[f32], available: f32) -> Vec<f32> {
    if items.is_empty() {
        return Vec::new();
    }

    let total_size: f32 = sizes.iter().sum();
    let remaining = available - total_size;

    if remaining.abs() < 0.001 {
        return sizes.to_vec();
    }

    if remaining > 0.0 {
        // Grow items
        let total_grow: f32 = items.iter().map(|i| i.grow).sum();
        if total_grow > 0.0 {
            return sizes
                .iter()
                .zip(items.iter())
                .map(|(&size, item)| size + (remaining * item.grow / total_grow))
                .collect();
        }
    } else {
        // Shrink items
        let total_shrink: f32 = items.iter().map(|i| i.shrink).sum();
        if total_shrink > 0.0 {
            return sizes
                .iter()
                .zip(items.iter())
                .map(|(&size, item)| (size + (remaining * item.shrink / total_shrink)).max(0.0))
                .collect();
        }
    }

    sizes.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flex_direction_default() {
        assert_eq!(FlexDirection::default(), FlexDirection::Row);
    }

    #[test]
    fn test_flex_justify_default() {
        assert_eq!(FlexJustify::default(), FlexJustify::Start);
    }

    #[test]
    fn test_flex_align_default() {
        assert_eq!(FlexAlign::default(), FlexAlign::Center);
    }

    #[test]
    fn test_flex_item_builder() {
        let item = FlexItem::new()
            .grow(1.0)
            .shrink(0.0)
            .basis(100.0)
            .align_self(FlexAlign::Start);

        assert_eq!(item.grow, 1.0);
        assert_eq!(item.shrink, 0.0);
        assert_eq!(item.basis, Some(100.0));
        assert_eq!(item.align_self, Some(FlexAlign::Start));
    }

    #[test]
    fn test_distribute_flex_empty() {
        let result = distribute_flex(&[], &[], 100.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_distribute_flex_exact_fit() {
        let items = vec![FlexItem::new(), FlexItem::new()];
        let sizes = vec![50.0, 50.0];
        let result = distribute_flex(&items, &sizes, 100.0);
        assert_eq!(result, vec![50.0, 50.0]);
    }

    #[test]
    fn test_distribute_flex_grow() {
        let items = vec![FlexItem::new().grow(1.0), FlexItem::new().grow(1.0)];
        let sizes = vec![25.0, 25.0];
        let result = distribute_flex(&items, &sizes, 100.0);
        assert_eq!(result, vec![50.0, 50.0]);
    }

    #[test]
    fn test_distribute_flex_grow_uneven() {
        let items = vec![FlexItem::new().grow(1.0), FlexItem::new().grow(3.0)];
        let sizes = vec![0.0, 0.0];
        let result = distribute_flex(&items, &sizes, 100.0);
        assert_eq!(result, vec![25.0, 75.0]);
    }

    #[test]
    fn test_distribute_flex_shrink() {
        let items = vec![FlexItem::new().shrink(1.0), FlexItem::new().shrink(1.0)];
        let sizes = vec![75.0, 75.0];
        let result = distribute_flex(&items, &sizes, 100.0);
        assert_eq!(result, vec![50.0, 50.0]);
    }
}
