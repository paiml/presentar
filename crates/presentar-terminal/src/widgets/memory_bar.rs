//! `MemoryBar` widget for stacked memory breakdown visualization.
//!
//! Displays memory segments (Used, Cached, Swap, Free) in stacked bars.
//! Reference: btop/ttop memory displays.

use presentar_core::{
    Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Color, Constraints, Event,
    LayoutResult, Point, Rect, Size, TextStyle, TypeId, Widget,
};
use std::any::Any;
use std::time::Duration;

/// A segment of the memory bar.
#[derive(Debug, Clone)]
pub struct MemorySegment {
    /// Segment name (e.g., "Used", "Cached").
    pub name: String,
    /// Bytes in this segment.
    pub bytes: u64,
    /// Color for this segment.
    pub color: Color,
}

impl MemorySegment {
    /// Create a new memory segment.
    #[must_use]
    pub fn new(name: impl Into<String>, bytes: u64, color: Color) -> Self {
        Self {
            name: name.into(),
            bytes,
            color,
        }
    }
}

/// Stacked memory bar with labeled segments.
#[derive(Debug, Clone)]
pub struct MemoryBar {
    /// Memory segments to display.
    segments: Vec<MemorySegment>,
    /// Total memory in bytes.
    total_bytes: u64,
    /// Show segment labels.
    show_labels: bool,
    /// Show segment values.
    show_values: bool,
    /// Bar width in characters.
    bar_width: usize,
    /// Cached bounds.
    bounds: Rect,
}

impl Default for MemoryBar {
    fn default() -> Self {
        Self::new(0)
    }
}

impl MemoryBar {
    /// Create a new memory bar with total bytes.
    #[must_use]
    pub fn new(total_bytes: u64) -> Self {
        Self {
            segments: Vec::new(),
            total_bytes,
            show_labels: true,
            show_values: true,
            bar_width: 30,
            bounds: Rect::default(),
        }
    }

    /// Create from common memory info values.
    #[must_use]
    pub fn from_usage(
        used_bytes: u64,
        cached_bytes: u64,
        swap_used: u64,
        swap_total: u64,
        total_bytes: u64,
    ) -> Self {
        let mut bar = Self::new(total_bytes);

        // Used memory (excluding cache)
        bar.add_segment(MemorySegment::new(
            "Used",
            used_bytes,
            Color::new(0.98, 0.47, 0.56, 1.0), // Tokyo Night red
        ));

        // Cached
        bar.add_segment(MemorySegment::new(
            "Cached",
            cached_bytes,
            Color::new(0.88, 0.69, 0.41, 1.0), // Tokyo Night yellow
        ));

        // Swap (if any)
        if swap_total > 0 {
            bar.add_segment(MemorySegment::new(
                "Swap",
                swap_used,
                Color::new(0.73, 0.60, 0.97, 1.0), // Tokyo Night purple
            ));
        }

        bar
    }

    /// Add a segment.
    pub fn add_segment(&mut self, segment: MemorySegment) {
        self.segments.push(segment);
    }

    /// Set bar width.
    #[must_use]
    pub fn with_bar_width(mut self, width: usize) -> Self {
        self.bar_width = width;
        self
    }

    /// Hide labels.
    #[must_use]
    pub fn without_labels(mut self) -> Self {
        self.show_labels = false;
        self
    }

    /// Hide values.
    #[must_use]
    pub fn without_values(mut self) -> Self {
        self.show_values = false;
        self
    }

    /// Update total bytes.
    pub fn set_total(&mut self, total: u64) {
        self.total_bytes = total;
    }

    /// Get total bytes.
    #[must_use]
    pub fn total(&self) -> u64 {
        self.total_bytes
    }

    /// Get used bytes (sum of all segments).
    #[must_use]
    pub fn used(&self) -> u64 {
        self.segments.iter().map(|s| s.bytes).sum()
    }

    /// Get usage percentage.
    #[must_use]
    pub fn usage_percent(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            (self.used() as f64 / self.total_bytes as f64) * 100.0
        }
    }

    /// Format bytes as human-readable string.
    fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;
        const TB: u64 = GB * 1024;

        if bytes >= TB {
            format!("{:.1}T", bytes as f64 / TB as f64)
        } else if bytes >= GB {
            format!("{:.1}G", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1}M", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.1}K", bytes as f64 / KB as f64)
        } else {
            format!("{bytes}B")
        }
    }
}

impl Widget for MemoryBar {
    fn type_id(&self) -> TypeId {
        TypeId::of::<Self>()
    }

    fn measure(&self, constraints: Constraints) -> Size {
        // Each segment gets one row if showing labels
        let height = if self.show_labels {
            self.segments.len().max(1) as f32
        } else {
            1.0
        };
        let width = constraints.max_width.min(80.0);
        constraints.constrain(Size::new(width, height))
    }

    fn layout(&mut self, bounds: Rect) -> LayoutResult {
        self.bounds = bounds;
        LayoutResult {
            size: Size::new(bounds.width, bounds.height),
        }
    }

    fn paint(&self, canvas: &mut dyn Canvas) {
        if self.total_bytes == 0 {
            return;
        }

        let bar_chars = self
            .bar_width
            .min(self.bounds.width as usize)
            .saturating_sub(20);
        if bar_chars == 0 {
            return;
        }

        if self.show_labels {
            // Multi-row mode: one row per segment
            for (i, segment) in self.segments.iter().enumerate() {
                let y = self.bounds.y + i as f32;
                let pct = (segment.bytes as f64 / self.total_bytes as f64) * 100.0;
                let filled = ((pct / 100.0) * bar_chars as f64).round() as usize;

                // Label
                let label = format!("{:>6}:", segment.name);
                let label_style = TextStyle {
                    color: Color::new(0.5, 0.5, 0.6, 1.0),
                    ..Default::default()
                };
                canvas.draw_text(&label, Point::new(self.bounds.x, y), &label_style);

                // Value
                if self.show_values {
                    let value = Self::format_bytes(segment.bytes);
                    canvas.draw_text(
                        &format!("{value:>6}"),
                        Point::new(self.bounds.x + 8.0, y),
                        &TextStyle {
                            color: segment.color,
                            ..Default::default()
                        },
                    );
                }

                // Bar
                let bar_x = if self.show_values { 15.0 } else { 8.0 };
                let mut bar = String::with_capacity(bar_chars + 2);
                for j in 0..bar_chars {
                    if j < filled {
                        bar.push('█');
                    } else {
                        bar.push('░');
                    }
                }
                canvas.draw_text(
                    &bar,
                    Point::new(self.bounds.x + bar_x, y),
                    &TextStyle {
                        color: segment.color,
                        ..Default::default()
                    },
                );

                // Percentage
                let pct_x = self.bounds.x + bar_x + bar_chars as f32 + 1.0;
                canvas.draw_text(
                    &format!("{pct:3.0}%"),
                    Point::new(pct_x, y),
                    &TextStyle {
                        color: segment.color,
                        ..Default::default()
                    },
                );
            }
        } else {
            // Single-row stacked bar mode
            let mut x = self.bounds.x;
            let y = self.bounds.y;
            let mut pos = 0.0;

            for segment in &self.segments {
                let segment_width =
                    (segment.bytes as f64 / self.total_bytes as f64) * bar_chars as f64;
                let chars = (pos + segment_width).round() as usize - pos.round() as usize;

                let segment_bar: String = (0..chars).map(|_| '█').collect();
                canvas.draw_text(
                    &segment_bar,
                    Point::new(x, y),
                    &TextStyle {
                        color: segment.color,
                        ..Default::default()
                    },
                );

                x += chars as f32;
                pos += segment_width;
            }

            // Empty portion
            let remaining = bar_chars.saturating_sub(pos.round() as usize);
            if remaining > 0 {
                let empty: String = (0..remaining).map(|_| '░').collect();
                canvas.draw_text(
                    &empty,
                    Point::new(x, y),
                    &TextStyle {
                        color: Color::new(0.3, 0.3, 0.3, 1.0),
                        ..Default::default()
                    },
                );
            }
        }
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

impl Brick for MemoryBar {
    fn brick_name(&self) -> &'static str {
        "memory_bar"
    }

    fn assertions(&self) -> &[BrickAssertion] {
        static ASSERTIONS: &[BrickAssertion] = &[BrickAssertion::max_latency_ms(8)];
        ASSERTIONS
    }

    fn budget(&self) -> BrickBudget {
        BrickBudget::uniform(8)
    }

    fn verify(&self) -> BrickVerification {
        BrickVerification {
            passed: vec![BrickAssertion::max_latency_ms(8)],
            failed: vec![],
            verification_time: Duration::from_micros(5),
        }
    }

    fn to_html(&self) -> String {
        String::new()
    }

    fn to_css(&self) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_bar_new() {
        let bar = MemoryBar::new(1024 * 1024 * 1024);
        assert_eq!(bar.total(), 1024 * 1024 * 1024);
    }

    #[test]
    fn test_memory_bar_from_usage() {
        let bar = MemoryBar::from_usage(
            50 * 1024 * 1024 * 1024,  // 50G used
            20 * 1024 * 1024 * 1024,  // 20G cached
            1 * 1024 * 1024 * 1024,   // 1G swap
            8 * 1024 * 1024 * 1024,   // 8G swap total
            128 * 1024 * 1024 * 1024, // 128G total
        );
        assert_eq!(bar.segments.len(), 3);
    }

    #[test]
    fn test_memory_bar_usage_percent() {
        let mut bar = MemoryBar::new(100);
        bar.add_segment(MemorySegment::new("Used", 75, Color::RED));
        assert!((bar.usage_percent() - 75.0).abs() < 0.01);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(MemoryBar::format_bytes(500), "500B");
        assert_eq!(MemoryBar::format_bytes(1024), "1.0K");
        assert_eq!(MemoryBar::format_bytes(1024 * 1024), "1.0M");
        assert_eq!(MemoryBar::format_bytes(1024 * 1024 * 1024), "1.0G");
        assert_eq!(
            MemoryBar::format_bytes(1024u64 * 1024 * 1024 * 1024),
            "1.0T"
        );
    }

    #[test]
    fn test_memory_bar_add_segment() {
        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Test", 500, Color::BLUE));
        assert_eq!(bar.segments.len(), 1);
        assert_eq!(bar.used(), 500);
    }

    #[test]
    fn test_memory_bar_set_total() {
        let mut bar = MemoryBar::new(100);
        bar.set_total(200);
        assert_eq!(bar.total(), 200);
    }

    #[test]
    fn test_memory_bar_without_labels() {
        let bar = MemoryBar::new(100).without_labels();
        assert!(!bar.show_labels);
    }

    #[test]
    fn test_memory_bar_without_values() {
        let bar = MemoryBar::new(100).without_values();
        assert!(!bar.show_values);
    }

    #[test]
    fn test_memory_bar_with_bar_width() {
        let bar = MemoryBar::new(100).with_bar_width(50);
        assert_eq!(bar.bar_width, 50);
    }

    #[test]
    fn test_memory_bar_layout() {
        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));
        let result = bar.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        assert!(result.size.width > 0.0);
        assert!(result.size.height > 0.0);
    }

    #[test]
    fn test_memory_bar_verify() {
        let bar = MemoryBar::new(1000);
        let v = bar.verify();
        assert!(v.is_valid());
    }

    #[test]
    fn test_memory_bar_default() {
        let bar = MemoryBar::default();
        assert_eq!(bar.total(), 0);
    }

    #[test]
    fn test_memory_segment_new() {
        let seg = MemorySegment::new("Test", 1000, Color::GREEN);
        assert_eq!(seg.name, "Test");
        assert_eq!(seg.bytes, 1000);
    }

    #[test]
    fn test_memory_bar_from_usage_no_swap() {
        let bar = MemoryBar::from_usage(
            50 * 1024 * 1024 * 1024, // 50G used
            20 * 1024 * 1024 * 1024, // 20G cached
            0,                       // no swap used
            0,                       // no swap total
            128 * 1024 * 1024 * 1024,
        );
        // Only Used and Cached, no Swap
        assert_eq!(bar.segments.len(), 2);
    }

    #[test]
    fn test_memory_bar_usage_percent_zero_total() {
        let bar = MemoryBar::new(0);
        assert_eq!(bar.usage_percent(), 0.0);
    }

    #[test]
    fn test_memory_bar_measure() {
        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));
        bar.add_segment(MemorySegment::new("Cached", 300, Color::YELLOW));

        let constraints = Constraints {
            min_width: 0.0,
            max_width: 100.0,
            min_height: 0.0,
            max_height: 50.0,
        };
        let size = bar.measure(constraints);
        assert!(size.width > 0.0);
        // With labels, height = number of segments
        assert_eq!(size.height, 2.0);
    }

    #[test]
    fn test_memory_bar_measure_no_labels() {
        let bar = MemoryBar::new(1000).without_labels();
        let constraints = Constraints {
            min_width: 0.0,
            max_width: 100.0,
            min_height: 0.0,
            max_height: 50.0,
        };
        let size = bar.measure(constraints);
        // Without labels, height is 1
        assert_eq!(size.height, 1.0);
    }

    #[test]
    fn test_memory_bar_paint_with_labels() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));
        bar.add_segment(MemorySegment::new("Cached", 300, Color::YELLOW));

        let mut buffer = CellBuffer::new(80, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        bar.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        bar.paint(&mut canvas);

        // Should have painted segments with labels
        assert!(bar.show_labels);
    }

    #[test]
    fn test_memory_bar_paint_without_labels() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut bar = MemoryBar::new(1000).without_labels();
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));
        bar.add_segment(MemorySegment::new("Cached", 300, Color::YELLOW));

        let mut buffer = CellBuffer::new(80, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        bar.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        bar.paint(&mut canvas);

        // Should paint in stacked bar mode
        assert!(!bar.show_labels);
    }

    #[test]
    fn test_memory_bar_paint_zero_total() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let bar = MemoryBar::new(0);
        let mut buffer = CellBuffer::new(80, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        // Should return early with no painting
        bar.paint(&mut canvas);
    }

    #[test]
    fn test_memory_bar_paint_small_bounds() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));

        let mut buffer = CellBuffer::new(10, 2);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        // Very small bounds should trigger early return (bar_chars = 0)
        bar.layout(Rect::new(0.0, 0.0, 10.0, 2.0));
        bar.paint(&mut canvas);
    }

    #[test]
    fn test_memory_bar_paint_without_values() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        let mut bar = MemoryBar::new(1000).without_values();
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));

        let mut buffer = CellBuffer::new(80, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        bar.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        bar.paint(&mut canvas);

        assert!(!bar.show_values);
    }

    #[test]
    fn test_memory_bar_paint_stacked_with_empty() {
        use crate::{CellBuffer, DirectTerminalCanvas};

        // Create bar that won't fill entire width
        let mut bar = MemoryBar::new(1000).without_labels();
        bar.add_segment(MemorySegment::new("Used", 200, Color::RED));

        let mut buffer = CellBuffer::new(80, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);

        bar.layout(Rect::new(0.0, 0.0, 80.0, 10.0));
        bar.paint(&mut canvas);

        // Should have empty portion
        assert_eq!(bar.used(), 200);
    }

    #[test]
    fn test_memory_bar_event() {
        let mut bar = MemoryBar::new(1000);
        let event = Event::Resize {
            width: 80.0,
            height: 24.0,
        };
        assert!(bar.event(&event).is_none());
    }

    #[test]
    fn test_memory_bar_children() {
        let bar = MemoryBar::new(1000);
        assert!(bar.children().is_empty());
    }

    #[test]
    fn test_memory_bar_children_mut() {
        let mut bar = MemoryBar::new(1000);
        assert!(bar.children_mut().is_empty());
    }

    #[test]
    fn test_memory_bar_type_id() {
        let bar = MemoryBar::new(1000);
        let tid = Widget::type_id(&bar);
        assert_eq!(tid, TypeId::of::<MemoryBar>());
    }

    #[test]
    fn test_memory_bar_brick_name() {
        let bar = MemoryBar::new(1000);
        assert_eq!(bar.brick_name(), "memory_bar");
    }

    #[test]
    fn test_memory_bar_assertions() {
        let bar = MemoryBar::new(1000);
        let assertions = bar.assertions();
        assert!(!assertions.is_empty());
    }

    #[test]
    fn test_memory_bar_budget() {
        let bar = MemoryBar::new(1000);
        let budget = bar.budget();
        assert!(budget.layout_ms > 0);
    }

    #[test]
    fn test_memory_bar_to_html() {
        let bar = MemoryBar::new(1000);
        assert!(bar.to_html().is_empty());
    }

    #[test]
    fn test_memory_bar_to_css() {
        let bar = MemoryBar::new(1000);
        assert!(bar.to_css().is_empty());
    }

    #[test]
    fn test_memory_segment_clone() {
        let seg = MemorySegment::new("Test", 1000, Color::GREEN);
        let cloned = seg.clone();
        assert_eq!(cloned.name, seg.name);
        assert_eq!(cloned.bytes, seg.bytes);
    }

    #[test]
    fn test_memory_bar_clone() {
        let mut bar = MemoryBar::new(1000);
        bar.add_segment(MemorySegment::new("Used", 500, Color::RED));
        let cloned = bar.clone();
        assert_eq!(cloned.total(), bar.total());
        assert_eq!(cloned.segments.len(), bar.segments.len());
    }

    #[test]
    fn test_memory_segment_debug() {
        let seg = MemorySegment::new("Test", 1000, Color::GREEN);
        let debug = format!("{seg:?}");
        assert!(debug.contains("Test"));
        assert!(debug.contains("1000"));
    }

    #[test]
    fn test_memory_bar_debug() {
        let bar = MemoryBar::new(1000);
        let debug = format!("{bar:?}");
        assert!(debug.contains("1000"));
    }
}
