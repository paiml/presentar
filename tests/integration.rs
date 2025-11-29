//! Integration tests for Presentar framework.

use presentar::{
    widgets::{Button, Column, Row, Text},
    Color, Constraints, Rect, RecordingCanvas, Size, Widget,
};

#[test]
fn test_simple_widget_tree() {
    // Build a simple widget tree
    let ui = Column::new(vec![
        Box::new(Text::new("Hello")),
        Box::new(Text::new("World")),
    ]);

    // Measure the tree
    let constraints = Constraints::new(0.0, 800.0, 0.0, 600.0);
    let size = ui.measure(&constraints);

    // Verify size is within constraints
    assert!(size.width <= 800.0);
    assert!(size.height <= 600.0);
}

#[test]
fn test_widget_painting() {
    // Create a button
    let button = Button::new("Click me");

    // Measure and layout
    let constraints = Constraints::new(0.0, 200.0, 0.0, 50.0);
    let size = button.measure(&constraints);

    let mut button = button;
    button.layout(size);

    // Paint to recording canvas
    let mut canvas = RecordingCanvas::new();
    button.paint(&mut canvas);

    // Verify draw commands were emitted
    assert!(canvas.command_count() > 0);
}

#[test]
fn test_color_contrast_wcag() {
    // WCAG AA requires 4.5:1 contrast for normal text
    use presentar_test::A11yChecker;

    let result = A11yChecker::check_contrast(&Color::BLACK, &Color::WHITE, false);

    assert!(result.passes_aa);
    assert!(result.ratio >= 4.5);
}

#[test]
fn test_nested_layout() {
    // Create nested layout
    let ui = Row::new(vec![
        Box::new(Column::new(vec![
            Box::new(Text::new("A")),
            Box::new(Text::new("B")),
        ])),
        Box::new(Column::new(vec![
            Box::new(Text::new("C")),
            Box::new(Text::new("D")),
        ])),
    ]);

    let constraints = Constraints::new(0.0, 400.0, 0.0, 200.0);
    let size = ui.measure(&constraints);

    // Size should be reasonable
    assert!(size.width > 0.0);
    assert!(size.height > 0.0);
}

#[test]
fn test_recording_canvas_commands() {
    let mut canvas = RecordingCanvas::new();

    // Draw some primitives
    canvas.fill_rect(Rect::new(0.0, 0.0, 100.0, 100.0), Color::RED);
    canvas.fill_rect(Rect::new(50.0, 50.0, 100.0, 100.0), Color::BLUE);

    // Verify commands recorded
    assert_eq!(canvas.command_count(), 2);
}
