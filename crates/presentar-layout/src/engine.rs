//! Layout engine implementation.

use presentar_core::{Constraints, Rect, Size, Widget};
use std::collections::HashMap;

use crate::cache::LayoutCache;

/// Layout tree containing computed positions.
#[derive(Debug, Default)]
pub struct LayoutTree {
    /// Computed sizes for each widget
    pub sizes: HashMap<u64, Size>,
    /// Computed positions for each widget
    pub positions: HashMap<u64, Rect>,
}

/// Layout engine with memoization.
#[derive(Debug, Default)]
pub struct LayoutEngine {
    cache: LayoutCache,
    next_id: u64,
}

impl LayoutEngine {
    /// Create a new layout engine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Compute layout for the widget tree.
    pub fn compute(&mut self, root: &dyn Widget, viewport: Size) -> LayoutTree {
        self.cache.clear();
        self.next_id = 0;

        let constraints = Constraints::tight(viewport);

        // Phase 1: Measure (bottom-up)
        let mut sizes = HashMap::new();
        self.measure_tree(root, constraints, &mut sizes);

        // Phase 2: Layout (top-down)
        let mut positions = HashMap::new();
        let bounds = Rect::from_size(viewport);
        self.position_tree(root, bounds, &mut positions);

        LayoutTree { sizes, positions }
    }

    fn measure_tree(
        &mut self,
        widget: &dyn Widget,
        constraints: Constraints,
        sizes: &mut HashMap<u64, Size>,
    ) -> Size {
        let id = self.next_id;
        self.next_id += 1;

        // Measure children first
        for child in widget.children() {
            self.measure_tree(child.as_ref(), constraints, sizes);
        }

        // Then measure self
        let size = widget.measure(constraints);
        sizes.insert(id, size);
        size
    }

    fn position_tree(
        &mut self,
        widget: &dyn Widget,
        bounds: Rect,
        positions: &mut HashMap<u64, Rect>,
    ) {
        let id = self.next_id;
        self.next_id += 1;

        positions.insert(id, bounds);

        // Position children (simplified - actual implementation would use layout results)
        for child in widget.children() {
            self.position_tree(child.as_ref(), bounds, positions);
        }
    }

    /// Clear the layout cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_engine_new() {
        let engine = LayoutEngine::new();
        assert_eq!(engine.next_id, 0);
    }

    #[test]
    fn test_layout_tree_default() {
        let tree = LayoutTree::default();
        assert!(tree.sizes.is_empty());
        assert!(tree.positions.is_empty());
    }
}
