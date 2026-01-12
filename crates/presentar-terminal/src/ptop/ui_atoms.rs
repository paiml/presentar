//! Atomic widget helpers for ptop UI.
//!
//! This module provides convenience functions that bridge the SDK-level Atoms
//! (FlexCell, SemanticLabel, LabeledBar, etc.) into ptop's rendering context.
//!
//! ## Design Philosophy (SPEC-024 Appendix I)
//!
//! These helpers eliminate the "Boilerplate Tax" by:
//! 1. Encapsulating TextStyle creation
//! 2. Providing semantic color selection via SemanticStatus
//! 3. Composing Atoms into common UI patterns
//!
//! ## Usage
//!
//! Replace this boilerplate:
//! ```ignore
//! let color = if pct > 90.0 { RED } else if pct > 75.0 { YELLOW } else { GREEN };
//! let bar = "█".repeat(filled) + &"░".repeat(empty);
//! canvas.draw_text(&format!("{label} {bar} {pct}%"), point, &TextStyle { color, ..Default::default() });
//! ```
//!
//! With this:
//! ```ignore
//! draw_labeled_bar(canvas, inner.x, y, inner.width, label, pct / 100.0, None);
//! ```

use crate::direct::DirectTerminalCanvas;
use crate::widgets::{
    FlexAlignment, FlexCell, GutterCursor, LabeledBar, Overflow, ProportionalBar, SemanticLabel,
    SemanticStatus,
};
use presentar_core::{Canvas, Color, Point, Rect, TextStyle, Widget};

// =============================================================================
// SEMANTIC COLORS (derived from SemanticStatus)
// =============================================================================

/// Get color for a usage percentage (high = bad, like CPU/Memory usage).
/// Uses SemanticStatus::from_usage() internally.
#[inline]
pub fn usage_color(pct: f64) -> Color {
    SemanticStatus::from_usage(pct).color()
}

/// Get color for a "goodness" percentage (high = good, like battery charge).
/// Uses SemanticStatus::from_percentage() internally.
#[inline]
pub fn goodness_color(pct: f64) -> Color {
    SemanticStatus::from_percentage(pct).color()
}

/// Get color for a temperature value in Celsius.
#[inline]
pub fn temperature_color(temp_c: f64) -> Color {
    SemanticStatus::from_temperature(temp_c).color()
}

// =============================================================================
// QUICK TEXT RENDERING (replaces direct draw_text + TextStyle boilerplate)
// =============================================================================

/// Draw text with semantic coloring based on usage percentage.
/// Replaces the common pattern of computing color from percentage.
pub fn draw_usage_text(
    canvas: &mut DirectTerminalCanvas<'_>,
    text: &str,
    x: f32,
    y: f32,
    usage_pct: f64,
) {
    canvas.draw_text(
        text,
        Point::new(x, y),
        &TextStyle {
            color: usage_color(usage_pct),
            ..Default::default()
        },
    );
}

/// Draw text with a specific color (convenience wrapper).
#[inline]
pub fn draw_colored_text(
    canvas: &mut DirectTerminalCanvas<'_>,
    text: &str,
    x: f32,
    y: f32,
    color: Color,
) {
    canvas.draw_text(
        text,
        Point::new(x, y),
        &TextStyle {
            color,
            ..Default::default()
        },
    );
}

/// Draw a semantic label with automatic status coloring.
/// Returns the width consumed.
pub fn draw_semantic_label(
    canvas: &mut DirectTerminalCanvas<'_>,
    text: &str,
    x: f32,
    y: f32,
    status: SemanticStatus,
    max_width: Option<usize>,
) -> f32 {
    let mut label = SemanticLabel::new(text).with_status(status);
    if let Some(w) = max_width {
        label = label.with_max_width(w);
    }
    let width = text.chars().count().min(max_width.unwrap_or(usize::MAX)) as f32;
    label.layout(Rect::new(x, y, width, 1.0));
    label.paint(canvas);
    width
}

// =============================================================================
// LABELED BAR RENDERING (replaces manual bar + label + percentage)
// =============================================================================

/// Draw a labeled usage bar with automatic semantic coloring.
///
/// This replaces the common pattern:
/// ```ignore
/// let filled = (pct * bar_width as f64) as usize;
/// let bar = "█".repeat(filled) + &"░".repeat(bar_width - filled);
/// let text = format!("{label} {value} {bar} {pct}%");
/// canvas.draw_text(&text, point, &TextStyle { color, .. });
/// ```
///
/// With:
/// ```ignore
/// draw_labeled_bar(canvas, x, y, width, "Used", used_gb, Some(total_gb), pct);
/// ```
pub fn draw_labeled_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    value: f64, // Normalized 0.0-1.0
    color_override: Option<Color>,
) {
    let mut bar = LabeledBar::new(label, value);
    if let Some(c) = color_override {
        bar = bar.with_bar_color(c);
    }
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

/// Draw a memory-style bar (label, used/total, percentage).
#[allow(clippy::too_many_arguments)]
pub fn draw_memory_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    used_bytes: u64,
    total_bytes: u64,
    color_override: Option<Color>,
) {
    let mut bar = LabeledBar::memory(label, used_bytes, total_bytes);
    if let Some(c) = color_override {
        bar = bar.with_bar_color(c);
    }
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

/// Draw a percentage bar with semantic coloring.
pub fn draw_percentage_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    pct: f64,
) {
    let mut bar = LabeledBar::percentage(label, pct);
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

/// Draw a temperature bar with gradient coloring.
pub fn draw_temperature_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    label: &str,
    temp_c: f64,
    max_temp: f64,
) {
    let mut bar = LabeledBar::temperature(label, temp_c, max_temp);
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

// =============================================================================
// FLEX CELL RENDERING (bounded text with overflow handling)
// =============================================================================

/// Draw text with strict bounds enforcement (never bleeds).
/// Uses FlexCell internally with Ellipsis overflow.
pub fn draw_bounded_text(
    canvas: &mut DirectTerminalCanvas<'_>,
    text: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
    alignment: FlexAlignment,
) {
    let mut cell = FlexCell::new(text)
        .with_color(color)
        .with_overflow(Overflow::Ellipsis)
        .with_alignment(alignment);
    cell.layout(Rect::new(x, y, max_width as f32, 1.0));
    cell.paint(canvas);
}

/// Draw a path with middle truncation (preserves start and end).
pub fn draw_path_text(
    canvas: &mut DirectTerminalCanvas<'_>,
    path: &str,
    x: f32,
    y: f32,
    max_width: usize,
    color: Color,
) {
    let mut cell = FlexCell::new(path)
        .with_color(color)
        .with_overflow(Overflow::EllipsisMiddle);
    cell.layout(Rect::new(x, y, max_width as f32, 1.0));
    cell.paint(canvas);
}

// =============================================================================
// PROPORTIONAL BAR (inline bar without label)
// =============================================================================

/// Draw a simple proportional bar (no label, just the bar).
pub fn draw_proportional_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    value: f64,
    color: Color,
) {
    let mut bar = ProportionalBar::new().with_segment(value, color);
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

/// Draw a multi-segment proportional bar.
pub fn draw_stacked_bar(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    width: f32,
    segments: &[(f64, Color)],
) {
    let mut bar = ProportionalBar::new();
    for (value, color) in segments {
        bar = bar.with_segment(*value, *color);
    }
    bar.layout(Rect::new(x, y, width, 1.0));
    bar.paint(canvas);
}

// =============================================================================
// SELECTION CURSOR (gutter indicator)
// =============================================================================

/// Draw a selection cursor at the specified row.
pub fn draw_row_cursor(
    canvas: &mut DirectTerminalCanvas<'_>,
    x: f32,
    y: f32,
    visible_rows: usize,
    selected_row: Option<usize>,
) {
    let mut cursor = GutterCursor::new().with_visible_rows(visible_rows);
    if let Some(row) = selected_row {
        cursor = cursor.with_selected(row);
    }
    cursor.layout(Rect::new(x, y, 1.0, visible_rows as f32));
    cursor.paint(canvas);
}

// =============================================================================
// SEVERITY TO STATUS (maps severity float to SemanticStatus)
// =============================================================================

/// Convert a 0.0-1.0 severity to SemanticStatus.
/// 0.0-0.2 = Normal, 0.2-0.4 = Good, 0.4-0.6 = Warning, 0.6-0.8 = High, 0.8-1.0 = Critical
pub fn severity_to_status(severity: f64) -> SemanticStatus {
    if severity >= 0.8 {
        SemanticStatus::Critical
    } else if severity >= 0.6 {
        SemanticStatus::High
    } else if severity >= 0.4 {
        SemanticStatus::Warning
    } else if severity >= 0.2 {
        SemanticStatus::Good
    } else {
        SemanticStatus::Normal
    }
}

/// Get color from severity (convenience function).
#[inline]
pub fn severity_color(severity: f64) -> Color {
    severity_to_status(severity).color()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direct::CellBuffer;

    #[test]
    fn test_usage_color_gradient() {
        // Low usage = green
        let low = usage_color(10.0);
        assert!(low.g > low.r, "Low usage should be greenish");

        // High usage = red
        let high = usage_color(95.0);
        assert!(high.r > high.g, "High usage should be reddish");

        // Mid usage checks
        let mid = usage_color(50.0);
        assert!(mid.r > 0.5 && mid.g > 0.5, "Mid usage should be yellowish");
    }

    #[test]
    fn test_goodness_color_gradient() {
        // High goodness = green
        let good = goodness_color(90.0);
        assert!(good.g > good.r, "High goodness should be greenish");

        // Low goodness = red
        let bad = goodness_color(10.0);
        assert!(bad.r > bad.g, "Low goodness should be reddish");
    }

    #[test]
    fn test_temperature_color_gradient() {
        let cool = temperature_color(40.0);
        assert!(cool.g > cool.r, "Cool temp should be greenish");

        let hot = temperature_color(95.0);
        assert!(hot.r > hot.g, "Hot temp should be reddish");

        // Warm temp
        let warm = temperature_color(70.0);
        assert!(
            warm.r > 0.5 && warm.g > 0.5,
            "Warm temp should be yellowish"
        );
    }

    #[test]
    fn test_severity_mapping() {
        assert_eq!(severity_to_status(0.1), SemanticStatus::Normal);
        assert_eq!(severity_to_status(0.3), SemanticStatus::Good);
        assert_eq!(severity_to_status(0.5), SemanticStatus::Warning);
        assert_eq!(severity_to_status(0.7), SemanticStatus::High);
        assert_eq!(severity_to_status(0.9), SemanticStatus::Critical);
    }

    #[test]
    fn test_severity_color() {
        let low = severity_color(0.1);
        let high = severity_color(0.9);
        assert!(low.g > low.r, "Low severity should be greenish");
        assert!(high.r > high.g, "High severity should be reddish");
    }

    #[test]
    fn test_draw_usage_text() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_usage_text(&mut canvas, "Test", 0.0, 0.0, 50.0);
        // Verify text was drawn
        let cell = buffer.get(0, 0).unwrap();
        assert_eq!(cell.symbol, "T");
    }

    #[test]
    fn test_draw_colored_text() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_colored_text(&mut canvas, "Hello", 0.0, 0.0, Color::RED);
        let cell = buffer.get(0, 0).unwrap();
        assert_eq!(cell.symbol, "H");
    }

    #[test]
    fn test_draw_semantic_label() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        let width = draw_semantic_label(&mut canvas, "OK", 0.0, 0.0, SemanticStatus::Normal, None);
        assert_eq!(width, 2.0);
    }

    #[test]
    fn test_draw_semantic_label_with_max_width() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        let width = draw_semantic_label(
            &mut canvas,
            "Very Long Text",
            0.0,
            0.0,
            SemanticStatus::Warning,
            Some(5),
        );
        assert_eq!(width, 5.0);
    }

    #[test]
    fn test_draw_labeled_bar() {
        let mut buffer = CellBuffer::new(30, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_labeled_bar(&mut canvas, 0.0, 0.0, 30.0, "Test", 0.5, None);
        // Should render without panic
    }

    #[test]
    fn test_draw_labeled_bar_with_color() {
        let mut buffer = CellBuffer::new(30, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_labeled_bar(&mut canvas, 0.0, 0.0, 30.0, "CPU", 0.8, Some(Color::BLUE));
        // Should render without panic
    }

    #[test]
    fn test_draw_memory_bar() {
        let mut buffer = CellBuffer::new(40, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_memory_bar(
            &mut canvas,
            0.0,
            0.0,
            40.0,
            "Mem",
            4 * 1024 * 1024 * 1024, // 4GB used
            8 * 1024 * 1024 * 1024, // 8GB total
            None,
        );
    }

    #[test]
    fn test_draw_memory_bar_with_color() {
        let mut buffer = CellBuffer::new(40, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_memory_bar(
            &mut canvas,
            0.0,
            0.0,
            40.0,
            "Swap",
            1024 * 1024 * 1024, // 1GB
            4 * 1024 * 1024 * 1024,
            Some(Color::new(0.5, 0.3, 0.8, 1.0)),
        );
    }

    #[test]
    fn test_draw_percentage_bar() {
        let mut buffer = CellBuffer::new(25, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_percentage_bar(&mut canvas, 0.0, 0.0, 25.0, "CPU", 75.0);
    }

    #[test]
    fn test_draw_temperature_bar() {
        let mut buffer = CellBuffer::new(25, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_temperature_bar(&mut canvas, 0.0, 0.0, 25.0, "Core", 65.0, 100.0);
    }

    #[test]
    fn test_draw_bounded_text() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_bounded_text(
            &mut canvas,
            "Long text that exceeds bounds",
            0.0,
            0.0,
            10,
            Color::WHITE,
            FlexAlignment::Left,
        );
        // Should truncate to fit
    }

    #[test]
    fn test_draw_bounded_text_center() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_bounded_text(
            &mut canvas,
            "Hi",
            0.0,
            0.0,
            10,
            Color::WHITE,
            FlexAlignment::Center,
        );
    }

    #[test]
    fn test_draw_bounded_text_right() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_bounded_text(
            &mut canvas,
            "Hi",
            0.0,
            0.0,
            10,
            Color::WHITE,
            FlexAlignment::Right,
        );
    }

    #[test]
    fn test_draw_path_text() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_path_text(
            &mut canvas,
            "/home/user/very/long/path/to/file.txt",
            0.0,
            0.0,
            15,
            Color::new(0.0, 1.0, 1.0, 1.0), // Cyan
        );
    }

    #[test]
    fn test_draw_proportional_bar() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_proportional_bar(&mut canvas, 0.0, 0.0, 20.0, 0.6, Color::GREEN);
    }

    #[test]
    fn test_draw_stacked_bar() {
        let mut buffer = CellBuffer::new(20, 1);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_stacked_bar(
            &mut canvas,
            0.0,
            0.0,
            20.0,
            &[(0.3, Color::RED), (0.4, Color::YELLOW), (0.2, Color::GREEN)],
        );
    }

    #[test]
    fn test_draw_row_cursor_no_selection() {
        let mut buffer = CellBuffer::new(1, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_row_cursor(&mut canvas, 0.0, 0.0, 10, None);
    }

    #[test]
    fn test_draw_row_cursor_with_selection() {
        let mut buffer = CellBuffer::new(1, 10);
        let mut canvas = DirectTerminalCanvas::new(&mut buffer);
        draw_row_cursor(&mut canvas, 0.0, 0.0, 10, Some(3));
    }
}
