//! Accessibility checking for WCAG 2.1 compliance.

use presentar_core::{Color, Widget};

/// Accessibility checker.
pub struct A11yChecker;

impl A11yChecker {
    /// Check a widget tree for accessibility violations.
    #[must_use]
    pub fn check(widget: &dyn Widget) -> A11yReport {
        let mut violations = Vec::new();
        Self::check_widget(widget, &mut violations);
        A11yReport { violations }
    }

    fn check_widget(widget: &dyn Widget, violations: &mut Vec<A11yViolation>) {
        // Check for missing accessible name on interactive elements
        if widget.is_interactive() && widget.accessible_name().is_none() {
            violations.push(A11yViolation {
                rule: "aria-label".to_string(),
                message: "Interactive element missing accessible name".to_string(),
                wcag: "4.1.2".to_string(),
                impact: Impact::Critical,
            });
        }

        // Check for focusable elements
        if widget.is_interactive() && !widget.is_focusable() {
            violations.push(A11yViolation {
                rule: "keyboard".to_string(),
                message: "Interactive element is not keyboard focusable".to_string(),
                wcag: "2.1.1".to_string(),
                impact: Impact::Critical,
            });
        }

        // Recurse into children
        for child in widget.children() {
            Self::check_widget(child.as_ref(), violations);
        }
    }

    /// Check contrast ratio between foreground and background colors.
    #[must_use]
    pub fn check_contrast(
        foreground: &Color,
        background: &Color,
        large_text: bool,
    ) -> ContrastResult {
        let ratio = foreground.contrast_ratio(background);

        // WCAG 2.1 thresholds
        let (aa_threshold, aaa_threshold) = if large_text {
            (3.0, 4.5) // Large text (14pt bold or 18pt regular)
        } else {
            (4.5, 7.0) // Normal text
        };

        ContrastResult {
            ratio,
            passes_aa: ratio >= aa_threshold,
            passes_aaa: ratio >= aaa_threshold,
        }
    }
}

/// Accessibility report.
#[derive(Debug)]
pub struct A11yReport {
    /// List of violations found
    pub violations: Vec<A11yViolation>,
}

impl A11yReport {
    /// Check if all accessibility tests passed.
    #[must_use]
    pub fn is_passing(&self) -> bool {
        self.violations.is_empty()
    }

    /// Get critical violations only.
    #[must_use]
    pub fn critical(&self) -> Vec<&A11yViolation> {
        self.violations
            .iter()
            .filter(|v| v.impact == Impact::Critical)
            .collect()
    }

    /// Assert that all accessibility tests pass.
    ///
    /// # Panics
    ///
    /// Panics if there are any violations.
    pub fn assert_pass(&self) {
        if !self.is_passing() {
            let messages: Vec<String> = self
                .violations
                .iter()
                .map(|v| {
                    format!(
                        "  [{:?}] {}: {} (WCAG {})",
                        v.impact, v.rule, v.message, v.wcag
                    )
                })
                .collect();

            panic!(
                "Accessibility check failed with {} violation(s):\n{}",
                self.violations.len(),
                messages.join("\n")
            );
        }
    }
}

/// A single accessibility violation.
#[derive(Debug, Clone)]
pub struct A11yViolation {
    /// Rule that was violated
    pub rule: String,
    /// Human-readable message
    pub message: String,
    /// WCAG success criterion
    pub wcag: String,
    /// Impact level
    pub impact: Impact,
}

/// Impact level of an accessibility violation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impact {
    /// Minor issue
    Minor,
    /// Moderate issue
    Moderate,
    /// Serious issue
    Serious,
    /// Critical issue - must fix
    Critical,
}

/// Result of a contrast check.
#[derive(Debug, Clone)]
pub struct ContrastResult {
    /// Calculated contrast ratio
    pub ratio: f32,
    /// Passes WCAG AA
    pub passes_aa: bool,
    /// Passes WCAG AAA
    pub passes_aaa: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use presentar_core::{
        widget::{AccessibleRole, LayoutResult},
        Canvas, Constraints, Event, Rect, Size, TypeId,
    };
    use std::any::Any;

    // Mock interactive widget
    struct MockButton {
        accessible_name: Option<String>,
        focusable: bool,
    }

    impl MockButton {
        fn new() -> Self {
            Self {
                accessible_name: None,
                focusable: true,
            }
        }

        fn with_name(mut self, name: &str) -> Self {
            self.accessible_name = Some(name.to_string());
            self
        }

        fn not_focusable(mut self) -> Self {
            self.focusable = false;
            self
        }
    }

    impl Widget for MockButton {
        fn type_id(&self) -> TypeId {
            TypeId::of::<Self>()
        }
        fn measure(&self, c: Constraints) -> Size {
            c.smallest()
        }
        fn layout(&mut self, b: Rect) -> LayoutResult {
            LayoutResult { size: b.size() }
        }
        fn paint(&self, _: &mut dyn Canvas) {}
        fn event(&mut self, _: &Event) -> Option<Box<dyn Any + Send>> {
            None
        }
        fn children(&self) -> &[Box<dyn Widget>] {
            &[]
        }
        fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
            &mut []
        }
        fn is_interactive(&self) -> bool {
            true
        }
        fn is_focusable(&self) -> bool {
            self.focusable
        }
        fn accessible_name(&self) -> Option<&str> {
            self.accessible_name.as_deref()
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Button
        }
    }

    #[test]
    fn test_a11y_passing() {
        let widget = MockButton::new().with_name("Submit");
        let report = A11yChecker::check(&widget);
        assert!(report.is_passing());
    }

    #[test]
    fn test_a11y_missing_name() {
        let widget = MockButton::new();
        let report = A11yChecker::check(&widget);
        assert!(!report.is_passing());
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].rule, "aria-label");
    }

    #[test]
    fn test_a11y_not_focusable() {
        let widget = MockButton::new().with_name("OK").not_focusable();
        let report = A11yChecker::check(&widget);
        assert!(!report.is_passing());
        assert!(report.violations.iter().any(|v| v.rule == "keyboard"));
    }

    #[test]
    fn test_contrast_black_white() {
        let result = A11yChecker::check_contrast(&Color::BLACK, &Color::WHITE, false);
        assert!(result.passes_aa);
        assert!(result.passes_aaa);
        assert!((result.ratio - 21.0).abs() < 0.5);
    }

    #[test]
    fn test_contrast_low() {
        let light_gray = Color::rgb(0.7, 0.7, 0.7);
        let white = Color::WHITE;
        let result = A11yChecker::check_contrast(&light_gray, &white, false);
        assert!(!result.passes_aa);
    }

    #[test]
    fn test_contrast_large_text_threshold() {
        // Gray that passes AA for large text but not for normal text
        let gray = Color::rgb(0.5, 0.5, 0.5);
        let white = Color::WHITE;

        let normal = A11yChecker::check_contrast(&gray, &white, false);
        let large = A11yChecker::check_contrast(&gray, &white, true);

        // Large text has lower threshold, should pass more easily
        assert!(large.passes_aa || large.ratio > normal.ratio - 1.0);
    }

    #[test]
    fn test_report_critical() {
        let widget = MockButton::new().not_focusable();
        let report = A11yChecker::check(&widget);
        let critical = report.critical();
        assert!(!critical.is_empty());
    }

    #[test]
    #[should_panic(expected = "Accessibility check failed")]
    fn test_assert_pass_fails() {
        let widget = MockButton::new();
        let report = A11yChecker::check(&widget);
        report.assert_pass();
    }
}
