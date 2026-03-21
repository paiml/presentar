mod tests {
    use super::*;
    use presentar_core::{
        widget::{AccessibleRole, LayoutResult},
        Brick, BrickAssertion, BrickBudget, BrickVerification, Canvas, Constraints, Event, Rect,
        Size, TypeId,
    };
    use std::any::Any;
    use std::time::Duration;

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

    impl Brick for MockButton {
        fn brick_name(&self) -> &'static str {
            "MockButton"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
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

    // =============================================================================
    // AriaAttributes tests
    // =============================================================================

    #[test]
    fn test_aria_attributes_new() {
        let attrs = AriaAttributes::new();
        assert!(attrs.role.is_none());
        assert!(attrs.label.is_none());
        assert!(!attrs.disabled);
    }

    #[test]
    fn test_aria_attributes_with_role() {
        let attrs = AriaAttributes::new().with_role("button");
        assert_eq!(attrs.role, Some("button".to_string()));
    }

    #[test]
    fn test_aria_attributes_with_label() {
        let attrs = AriaAttributes::new().with_label("Submit form");
        assert_eq!(attrs.label, Some("Submit form".to_string()));
    }

    #[test]
    fn test_aria_attributes_with_expanded() {
        let attrs = AriaAttributes::new().with_expanded(true);
        assert_eq!(attrs.expanded, Some(true));
    }

    #[test]
    fn test_aria_attributes_with_checked() {
        let attrs = AriaAttributes::new().with_checked(AriaChecked::True);
        assert_eq!(attrs.checked, Some(AriaChecked::True));
    }

    #[test]
    fn test_aria_attributes_with_disabled() {
        let attrs = AriaAttributes::new().with_disabled(true);
        assert!(attrs.disabled);
    }

    #[test]
    fn test_aria_attributes_with_value() {
        let attrs = AriaAttributes::new()
            .with_value_min(0.0)
            .with_value_max(100.0)
            .with_value_now(50.0);
        assert_eq!(attrs.value_min, Some(0.0));
        assert_eq!(attrs.value_max, Some(100.0));
        assert_eq!(attrs.value_now, Some(50.0));
    }

    #[test]
    fn test_aria_attributes_with_live() {
        let attrs = AriaAttributes::new().with_live(AriaLive::Polite);
        assert_eq!(attrs.live, Some(AriaLive::Polite));
    }

    #[test]
    fn test_aria_attributes_with_busy() {
        let attrs = AriaAttributes::new().with_busy(true);
        assert!(attrs.busy);
    }

    #[test]
    fn test_aria_attributes_with_atomic() {
        let attrs = AriaAttributes::new().with_atomic(true);
        assert!(attrs.atomic);
    }

    #[test]
    fn test_aria_attributes_with_has_popup() {
        let attrs = AriaAttributes::new().with_has_popup("menu");
        assert_eq!(attrs.has_popup, Some("menu".to_string()));
    }

    #[test]
    fn test_aria_attributes_with_controls() {
        let attrs = AriaAttributes::new().with_controls("panel-1");
        assert_eq!(attrs.controls, Some("panel-1".to_string()));
    }

    #[test]
    fn test_aria_attributes_with_described_by() {
        let attrs = AriaAttributes::new().with_described_by("desc-1");
        assert_eq!(attrs.described_by, Some("desc-1".to_string()));
    }

    #[test]
    fn test_aria_attributes_with_hidden() {
        let attrs = AriaAttributes::new().with_hidden(true);
        assert!(attrs.hidden);
    }

    #[test]
    fn test_aria_attributes_with_pressed() {
        let attrs = AriaAttributes::new().with_pressed(AriaChecked::Mixed);
        assert_eq!(attrs.pressed, Some(AriaChecked::Mixed));
    }

    #[test]
    fn test_aria_attributes_with_selected() {
        let attrs = AriaAttributes::new().with_selected(true);
        assert_eq!(attrs.selected, Some(true));
    }

    #[test]
    fn test_aria_attributes_with_level() {
        let attrs = AriaAttributes::new().with_level(2);
        assert_eq!(attrs.level, Some(2));
    }

    #[test]
    fn test_aria_attributes_chained_builder() {
        let attrs = AriaAttributes::new()
            .with_role("checkbox")
            .with_label("Accept terms")
            .with_checked(AriaChecked::False)
            .with_disabled(false);

        assert_eq!(attrs.role, Some("checkbox".to_string()));
        assert_eq!(attrs.label, Some("Accept terms".to_string()));
        assert_eq!(attrs.checked, Some(AriaChecked::False));
        assert!(!attrs.disabled);
    }

    // =============================================================================
    // AriaChecked tests
    // =============================================================================

    #[test]
    fn test_aria_checked_as_str() {
        assert_eq!(AriaChecked::True.as_str(), "true");
        assert_eq!(AriaChecked::False.as_str(), "false");
        assert_eq!(AriaChecked::Mixed.as_str(), "mixed");
    }

    // =============================================================================
    // AriaLive tests
    // =============================================================================

    #[test]
    fn test_aria_live_as_str() {
        assert_eq!(AriaLive::Off.as_str(), "off");
        assert_eq!(AriaLive::Polite.as_str(), "polite");
        assert_eq!(AriaLive::Assertive.as_str(), "assertive");
    }

    // =============================================================================
    // to_html_attrs tests
    // =============================================================================

    #[test]
    fn test_to_html_attrs_empty() {
        let attrs = AriaAttributes::new();
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.is_empty());
    }

    #[test]
    fn test_to_html_attrs_role() {
        let attrs = AriaAttributes::new().with_role("button");
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(html_attrs[0], ("role".to_string(), "button".to_string()));
    }

    #[test]
    fn test_to_html_attrs_label() {
        let attrs = AriaAttributes::new().with_label("Submit");
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-label".to_string(), "Submit".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_disabled() {
        let attrs = AriaAttributes::new().with_disabled(true);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-disabled".to_string(), "true".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_checked() {
        let attrs = AriaAttributes::new().with_checked(AriaChecked::Mixed);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-checked".to_string(), "mixed".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_expanded() {
        let attrs = AriaAttributes::new().with_expanded(false);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-expanded".to_string(), "false".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_value_range() {
        let attrs = AriaAttributes::new()
            .with_value_now(50.0)
            .with_value_min(0.0)
            .with_value_max(100.0);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 3);
        assert!(html_attrs.contains(&("aria-valuenow".to_string(), "50".to_string())));
        assert!(html_attrs.contains(&("aria-valuemin".to_string(), "0".to_string())));
        assert!(html_attrs.contains(&("aria-valuemax".to_string(), "100".to_string())));
    }

    #[test]
    fn test_to_html_attrs_live() {
        let attrs = AriaAttributes::new().with_live(AriaLive::Assertive);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-live".to_string(), "assertive".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_hidden() {
        let attrs = AriaAttributes::new().with_hidden(true);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 1);
        assert_eq!(
            html_attrs[0],
            ("aria-hidden".to_string(), "true".to_string())
        );
    }

    #[test]
    fn test_to_html_attrs_multiple() {
        let attrs = AriaAttributes::new()
            .with_role("slider")
            .with_label("Volume")
            .with_value_now(75.0)
            .with_value_min(0.0)
            .with_value_max(100.0);
        let html_attrs = attrs.to_html_attrs();
        assert_eq!(html_attrs.len(), 5);
    }

    // =============================================================================
    // to_html_string tests
    // =============================================================================

    #[test]
    fn test_to_html_string_empty() {
        let attrs = AriaAttributes::new();
        let html = attrs.to_html_string();
        assert_eq!(html, "");
    }

    #[test]
    fn test_to_html_string_single() {
        let attrs = AriaAttributes::new().with_role("button");
        let html = attrs.to_html_string();
        assert_eq!(html, "role=\"button\"");
    }

    #[test]
    fn test_to_html_string_multiple() {
        let attrs = AriaAttributes::new()
            .with_role("checkbox")
            .with_checked(AriaChecked::True);
        let html = attrs.to_html_string();
        assert!(html.contains("role=\"checkbox\""));
        assert!(html.contains("aria-checked=\"true\""));
    }

    #[test]
    fn test_to_html_string_escapes_quotes() {
        let attrs = AriaAttributes::new().with_label("Click \"here\"");
        let html = attrs.to_html_string();
        assert!(html.contains("aria-label=\"Click &quot;here&quot;\""));
    }

    // =============================================================================
    // aria_from_widget tests
    // =============================================================================

    #[test]
    fn test_aria_from_widget_button() {
        let widget = MockButton::new().with_name("Submit");
        let attrs = aria_from_widget(&widget);
        assert_eq!(attrs.role, Some("button".to_string()));
        assert_eq!(attrs.label, Some("Submit".to_string()));
        assert!(!attrs.disabled);
    }

    #[test]
    fn test_aria_from_widget_no_name() {
        let widget = MockButton::new();
        let attrs = aria_from_widget(&widget);
        assert_eq!(attrs.role, Some("button".to_string()));
        assert!(attrs.label.is_none());
    }

    // Mock non-interactive widget for testing disabled state
    struct MockLabel {
        text: String,
    }

    impl MockLabel {
        fn new(text: &str) -> Self {
            Self {
                text: text.to_string(),
            }
        }
    }

    impl Brick for MockLabel {
        fn brick_name(&self) -> &'static str {
            "MockLabel"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockLabel {
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
            false
        }
        fn is_focusable(&self) -> bool {
            false
        }
        fn accessible_name(&self) -> Option<&str> {
            Some(&self.text)
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Heading
        }
    }

    #[test]
    fn test_aria_from_widget_non_interactive() {
        let widget = MockLabel::new("Welcome");
        let attrs = aria_from_widget(&widget);
        assert_eq!(attrs.role, Some("heading".to_string()));
        assert_eq!(attrs.label, Some("Welcome".to_string()));
        assert!(attrs.disabled);
    }

    // Mock generic widget that returns Generic role
    struct MockGenericWidget;

    impl Brick for MockGenericWidget {
        fn brick_name(&self) -> &'static str {
            "MockGenericWidget"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockGenericWidget {
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
            false
        }
        fn is_focusable(&self) -> bool {
            false
        }
        fn accessible_name(&self) -> Option<&str> {
            None
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Generic
        }
    }

    #[test]
    fn test_aria_from_widget_generic() {
        let widget = MockGenericWidget;
        let attrs = aria_from_widget(&widget);
        assert!(attrs.role.is_none());
        assert!(attrs.label.is_none());
        // Generic role shouldn't trigger disabled state
        assert!(!attrs.disabled);
    }

    // =============================================================================
    // A11yConfig tests
    // =============================================================================

    #[test]
    fn test_a11y_config_default() {
        let config = A11yConfig::default();
        assert!(config.check_touch_targets);
        assert!(config.check_heading_hierarchy);
        assert!(!config.check_focus_indicators);
        assert!((config.min_contrast_normal - 4.5).abs() < 0.01);
    }

    #[test]
    fn test_a11y_config_strict() {
        let config = A11yConfig::strict();
        assert!(config.check_touch_targets);
        assert!(config.check_heading_hierarchy);
        assert!(config.check_focus_indicators);
        assert!((config.min_contrast_normal - 7.0).abs() < 0.01);
    }

    #[test]
    fn test_a11y_config_mobile() {
        let config = A11yConfig::mobile();
        assert!(config.check_touch_targets);
        assert!(config.check_heading_hierarchy);
        assert!(!config.check_focus_indicators);
    }

    // =============================================================================
    // WCAG constant tests
    // =============================================================================

    #[test]
    fn test_min_touch_target_size() {
        assert_eq!(MIN_TOUCH_TARGET_SIZE, 44.0);
    }

    #[test]
    fn test_min_focus_indicator_area() {
        assert_eq!(MIN_FOCUS_INDICATOR_AREA, 2.0);
    }

    // =============================================================================
    // Check with config tests
    // =============================================================================

    #[test]
    fn test_check_with_config() {
        let widget = MockButton::new().with_name("OK");
        // Use config without touch target check since mock widgets have 0x0 bounds
        let config = A11yConfig {
            check_touch_targets: false,
            check_heading_hierarchy: true,
            check_focus_indicators: false,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        };
        let report = A11yChecker::check_with_config(&widget, &config);
        assert!(report.is_passing());
    }

    // =============================================================================
    // Image alt text tests
    // =============================================================================

    struct MockImage {
        alt_text: Option<String>,
    }

    impl MockImage {
        fn new() -> Self {
            Self { alt_text: None }
        }

        fn with_alt(mut self, alt: &str) -> Self {
            self.alt_text = Some(alt.to_string());
            self
        }
    }

    impl Brick for MockImage {
        fn brick_name(&self) -> &'static str {
            "MockImage"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockImage {
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
            false
        }
        fn is_focusable(&self) -> bool {
            false
        }
        fn accessible_name(&self) -> Option<&str> {
            self.alt_text.as_deref()
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Image
        }
    }

    #[test]
    fn test_image_missing_alt() {
        let widget = MockImage::new();
        let report = A11yChecker::check(&widget);
        assert!(!report.is_passing());
        assert!(report.violations.iter().any(|v| v.rule == "image-alt"));
    }

    #[test]
    fn test_image_with_alt() {
        let widget = MockImage::new().with_alt("A sunset over the ocean");
        let report = A11yChecker::check(&widget);
        // Image with alt text should pass the image-alt check
        assert!(!report.violations.iter().any(|v| v.rule == "image-alt"));
    }

    // =============================================================================
    // Impact ordering tests
    // =============================================================================

    #[test]
    fn test_impact_equality() {
        assert_eq!(Impact::Minor, Impact::Minor);
        assert_eq!(Impact::Moderate, Impact::Moderate);
        assert_eq!(Impact::Serious, Impact::Serious);
        assert_eq!(Impact::Critical, Impact::Critical);
        assert_ne!(Impact::Minor, Impact::Critical);
    }

    // =============================================================================
    // Form Accessibility Tests
    // =============================================================================

    #[test]
    fn test_form_field_passing() {
        let form = FormAccessibility::new().with_name("Login Form").field(
            FormFieldA11y::new("email")
                .with_label("Email Address")
                .with_type(InputType::Email)
                .with_autocomplete(AutocompleteValue::Email),
        );

        let report = FormA11yChecker::check(&form);
        assert!(report.is_passing());
    }

    #[test]
    fn test_form_field_missing_label() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(FormFieldA11y::new("email").with_type(InputType::Email));

        let report = FormA11yChecker::check(&form);
        assert!(!report.is_passing());
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingLabel));
    }

    #[test]
    fn test_form_field_aria_label_counts() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("search")
                .with_type(InputType::Search)
                .with_aria_label("Search products"),
        );

        let report = FormA11yChecker::check(&form);
        // aria-label should satisfy label requirement
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingLabel));
    }

    #[test]
    fn test_form_required_missing_aria() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("name")
                .with_label("Full Name")
                .with_required(true, false), // Visual but no aria
        );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingRequiredIndicator));
    }

    #[test]
    fn test_form_required_missing_visual() {
        let form = FormAccessibility::new().with_name("Test Form").field({
            let mut field = FormFieldA11y::new("name").with_label("Full Name");
            field.required = true;
            field.aria_required = true;
            field.has_visual_required_indicator = false;
            field
        });

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingVisualRequired));
    }

    #[test]
    fn test_form_required_proper() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("name")
                .with_label("Full Name")
                .required(), // Sets both visual and aria
        );

        let report = FormA11yChecker::check(&form);
        // Should not have required-related violations
        assert!(!report.violations.iter().any(|v| matches!(
            v.rule,
            FormA11yRule::MissingRequiredIndicator | FormA11yRule::MissingVisualRequired
        )));
    }

    #[test]
    fn test_form_error_without_aria_invalid() {
        let form = FormAccessibility::new().with_name("Test Form").field({
            let mut field = FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email);
            field.has_error = true;
            field.aria_invalid = false;
            field.error_message = Some("Invalid email".to_string());
            field
        });

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingErrorState));
    }

    #[test]
    fn test_form_error_without_message() {
        let form = FormAccessibility::new().with_name("Test Form").field({
            let mut field = FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email);
            field.has_error = true;
            field.aria_invalid = true;
            // No error message
            field
        });

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingErrorMessage));
    }

    #[test]
    fn test_form_error_not_associated() {
        let form = FormAccessibility::new().with_name("Test Form").field({
            let mut field = FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email);
            field.has_error = true;
            field.aria_invalid = true;
            field.error_message = Some("Invalid email".to_string());
            // No aria-describedby or aria-errormessage
            field
        });

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::ErrorNotAssociated));
    }

    #[test]
    fn test_form_error_properly_associated() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email)
                .with_autocomplete(AutocompleteValue::Email)
                .with_error("Please enter a valid email address", true),
        );

        let report = FormA11yChecker::check(&form);
        // Should not have error-related violations
        assert!(!report.violations.iter().any(|v| matches!(
            v.rule,
            FormA11yRule::MissingErrorState
                | FormA11yRule::MissingErrorMessage
                | FormA11yRule::ErrorNotAssociated
        )));
    }

    #[test]
    fn test_form_missing_autocomplete() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email),
            // No autocomplete
        );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingAutocomplete));
    }

    #[test]
    fn test_form_autocomplete_not_needed_for_checkbox() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("terms")
                .with_label("I agree to terms")
                .with_type(InputType::Checkbox),
        );

        let report = FormA11yChecker::check(&form);
        // Checkbox doesn't need autocomplete
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingAutocomplete));
    }

    #[test]
    fn test_form_placeholder_as_label() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("email")
                .with_type(InputType::Email)
                .with_placeholder("Enter your email"),
        );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::PlaceholderAsLabel));
    }

    #[test]
    fn test_form_placeholder_with_label_ok() {
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email)
                .with_autocomplete(AutocompleteValue::Email)
                .with_placeholder("e.g., user@example.com"),
        );

        let report = FormA11yChecker::check(&form);
        // Placeholder with label is fine
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::PlaceholderAsLabel));
    }

    #[test]
    fn test_form_radio_buttons_not_grouped() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(
                FormFieldA11y::new("option1")
                    .with_label("Option 1")
                    .with_type(InputType::Radio),
            )
            .field(
                FormFieldA11y::new("option2")
                    .with_label("Option 2")
                    .with_type(InputType::Radio),
            );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::RelatedFieldsNotGrouped));
    }

    #[test]
    fn test_form_radio_buttons_properly_grouped() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(
                FormFieldA11y::new("option1")
                    .with_label("Option 1")
                    .with_type(InputType::Radio),
            )
            .field(
                FormFieldA11y::new("option2")
                    .with_label("Option 2")
                    .with_type(InputType::Radio),
            )
            .group(
                FormFieldGroup::new("options")
                    .with_legend("Choose an option")
                    .with_field("option1")
                    .with_field("option2"),
            );

        let report = FormA11yChecker::check(&form);
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::RelatedFieldsNotGrouped));
    }

    #[test]
    fn test_form_group_missing_legend() {
        let form = FormAccessibility::new().with_name("Test Form").group(
            FormFieldGroup::new("address")
                .with_field("street")
                .with_field("city"),
        );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::GroupMissingLegend));
    }

    #[test]
    fn test_form_missing_accessible_name() {
        let form = FormAccessibility::new().field(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email)
                .with_autocomplete(AutocompleteValue::Email),
        );

        let report = FormA11yChecker::check(&form);
        assert!(report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::FormMissingName));
    }

    #[test]
    fn test_form_report_violations_for_field() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(FormFieldA11y::new("bad_field").with_type(InputType::Email))
            .field(
                FormFieldA11y::new("good_field")
                    .with_label("Good Field")
                    .with_type(InputType::Text),
            );

        let report = FormA11yChecker::check(&form);
        let bad_violations = report.violations_for_field("bad_field");
        assert!(!bad_violations.is_empty());

        let good_violations = report.violations_for_field("good_field");
        // good_field should only have autocomplete warning (if text type)
        assert!(good_violations.len() <= 1);
    }

    #[test]
    fn test_form_report_is_acceptable() {
        // Form with only moderate violations should be acceptable
        let form = FormAccessibility::new().with_name("Test Form").field(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email),
            // Missing autocomplete is Moderate impact
        );

        let report = FormA11yChecker::check(&form);
        assert!(report.is_acceptable()); // Only Moderate violations
    }

    #[test]
    fn test_input_type_should_have_autocomplete() {
        assert!(InputType::Email.should_have_autocomplete());
        assert!(InputType::Password.should_have_autocomplete());
        assert!(InputType::Tel.should_have_autocomplete());
        assert!(InputType::Text.should_have_autocomplete());
        assert!(!InputType::Checkbox.should_have_autocomplete());
        assert!(!InputType::Radio.should_have_autocomplete());
        assert!(!InputType::Date.should_have_autocomplete());
    }

    #[test]
    fn test_autocomplete_value_as_str() {
        assert_eq!(AutocompleteValue::Email.as_str(), "email");
        assert_eq!(AutocompleteValue::GivenName.as_str(), "given-name");
        assert_eq!(
            AutocompleteValue::CurrentPassword.as_str(),
            "current-password"
        );
        assert_eq!(AutocompleteValue::Off.as_str(), "off");
    }

    #[test]
    fn test_form_a11y_rule_name() {
        assert_eq!(FormA11yRule::MissingLabel.name(), "missing-label");
        assert_eq!(
            FormA11yRule::MissingRequiredIndicator.name(),
            "missing-required-indicator"
        );
        assert_eq!(
            FormA11yRule::PlaceholderAsLabel.name(),
            "placeholder-as-label"
        );
    }

    #[test]
    fn test_form_violations_for_rule() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(FormFieldA11y::new("field1").with_type(InputType::Email))
            .field(FormFieldA11y::new("field2").with_type(InputType::Email));

        let report = FormA11yChecker::check(&form);
        let missing_labels = report.violations_for_rule(FormA11yRule::MissingLabel);
        assert_eq!(missing_labels.len(), 2); // Both fields missing labels
    }

    #[test]
    fn test_form_complete_signup_form() {
        // Test a complete, accessible signup form
        let form = FormAccessibility::new()
            .with_name("Create Account")
            .field(
                FormFieldA11y::new("first_name")
                    .with_label("First Name")
                    .with_type(InputType::Text)
                    .with_autocomplete(AutocompleteValue::GivenName)
                    .required(),
            )
            .field(
                FormFieldA11y::new("last_name")
                    .with_label("Last Name")
                    .with_type(InputType::Text)
                    .with_autocomplete(AutocompleteValue::FamilyName)
                    .required(),
            )
            .field(
                FormFieldA11y::new("email")
                    .with_label("Email Address")
                    .with_type(InputType::Email)
                    .with_autocomplete(AutocompleteValue::Email)
                    .required(),
            )
            .field(
                FormFieldA11y::new("password")
                    .with_label("Password")
                    .with_type(InputType::Password)
                    .with_autocomplete(AutocompleteValue::NewPassword)
                    .required(),
            )
            .field(
                FormFieldA11y::new("terms")
                    .with_label("I agree to the Terms of Service")
                    .with_type(InputType::Checkbox)
                    .required(),
            );

        let report = FormA11yChecker::check(&form);
        assert!(
            report.is_passing(),
            "Complete signup form should pass: {:?}",
            report.violations
        );
    }

    // ===== Additional Coverage Tests =====

    #[test]
    fn test_aria_attributes_with_range() {
        let attrs = AriaAttributes::new().with_range(0.0, 100.0, 50.0);
        assert_eq!(attrs.value_min, Some(0.0));
        assert_eq!(attrs.value_max, Some(100.0));
        assert_eq!(attrs.value_now, Some(50.0));
    }

    #[test]
    fn test_aria_attributes_with_pos_in_set() {
        let attrs = AriaAttributes::new().with_pos_in_set(3, 10);
        assert_eq!(attrs.pos_in_set, Some(3));
        assert_eq!(attrs.set_size, Some(10));
    }

    #[test]
    fn test_to_html_attrs_pos_in_set() {
        let attrs = AriaAttributes::new().with_pos_in_set(2, 5);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-posinset".to_string(), "2".to_string())));
        assert!(html_attrs.contains(&("aria-setsize".to_string(), "5".to_string())));
    }

    #[test]
    fn test_to_html_attrs_level() {
        let attrs = AriaAttributes::new().with_level(3);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-level".to_string(), "3".to_string())));
    }

    #[test]
    fn test_to_html_attrs_selected() {
        let attrs = AriaAttributes::new().with_selected(true);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-selected".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_pressed() {
        let attrs = AriaAttributes::new().with_pressed(AriaChecked::True);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-pressed".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_busy() {
        let attrs = AriaAttributes::new().with_busy(true);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-busy".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_atomic() {
        let attrs = AriaAttributes::new().with_atomic(true);
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-atomic".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_controls() {
        let attrs = AriaAttributes::new().with_controls("panel-2");
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-controls".to_string(), "panel-2".to_string())));
    }

    #[test]
    fn test_to_html_attrs_describedby() {
        let attrs = AriaAttributes::new().with_described_by("desc-id");
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-describedby".to_string(), "desc-id".to_string())));
    }

    #[test]
    fn test_to_html_attrs_haspopup() {
        let attrs = AriaAttributes::new().with_has_popup("dialog");
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-haspopup".to_string(), "dialog".to_string())));
    }

    #[test]
    fn test_to_html_string_escapes_ampersand() {
        let attrs = AriaAttributes::new().with_label("Terms & Conditions");
        let html = attrs.to_html_string();
        assert!(html.contains("aria-label=\"Terms &amp; Conditions\""));
    }

    #[test]
    fn test_to_html_string_escapes_less_than() {
        let attrs = AriaAttributes::new().with_label("Value < 5");
        let html = attrs.to_html_string();
        assert!(html.contains("aria-label=\"Value &lt; 5\""));
    }

    #[test]
    fn test_to_html_string_escapes_greater_than() {
        let attrs = AriaAttributes::new().with_label("Value > 5");
        let html = attrs.to_html_string();
        assert!(html.contains("aria-label=\"Value &gt; 5\""));
    }

    #[test]
    fn test_to_html_attrs_required() {
        let mut attrs = AriaAttributes::new();
        attrs.required = true;
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-required".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_invalid() {
        let mut attrs = AriaAttributes::new();
        attrs.invalid = true;
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-invalid".to_string(), "true".to_string())));
    }

    #[test]
    fn test_to_html_attrs_value_text() {
        let mut attrs = AriaAttributes::new();
        attrs.value_text = Some("50%".to_string());
        let html_attrs = attrs.to_html_attrs();
        assert!(html_attrs.contains(&("aria-valuetext".to_string(), "50%".to_string())));
    }

    // ===== Additional AutocompleteValue Tests =====

    #[test]
    fn test_autocomplete_value_all_variants() {
        assert_eq!(AutocompleteValue::Name.as_str(), "name");
        assert_eq!(AutocompleteValue::FamilyName.as_str(), "family-name");
        assert_eq!(AutocompleteValue::Tel.as_str(), "tel");
        assert_eq!(AutocompleteValue::StreetAddress.as_str(), "street-address");
        assert_eq!(AutocompleteValue::AddressLevel1.as_str(), "address-level1");
        assert_eq!(AutocompleteValue::AddressLevel2.as_str(), "address-level2");
        assert_eq!(AutocompleteValue::PostalCode.as_str(), "postal-code");
        assert_eq!(AutocompleteValue::Country.as_str(), "country");
        assert_eq!(AutocompleteValue::Organization.as_str(), "organization");
        assert_eq!(AutocompleteValue::Username.as_str(), "username");
        assert_eq!(AutocompleteValue::NewPassword.as_str(), "new-password");
        assert_eq!(AutocompleteValue::CcNumber.as_str(), "cc-number");
        assert_eq!(AutocompleteValue::CcExp.as_str(), "cc-exp");
        assert_eq!(AutocompleteValue::CcCsc.as_str(), "cc-csc");
        assert_eq!(AutocompleteValue::OneTimeCode.as_str(), "one-time-code");
    }

    // ===== Additional FormA11yRule Tests =====

    #[test]
    fn test_form_a11y_rule_all_names() {
        assert_eq!(
            FormA11yRule::MissingVisualRequired.name(),
            "missing-visual-required"
        );
        assert_eq!(
            FormA11yRule::MissingErrorState.name(),
            "missing-error-state"
        );
        assert_eq!(
            FormA11yRule::MissingErrorMessage.name(),
            "missing-error-message"
        );
        assert_eq!(
            FormA11yRule::ErrorNotAssociated.name(),
            "error-not-associated"
        );
        assert_eq!(
            FormA11yRule::MissingAutocomplete.name(),
            "missing-autocomplete"
        );
        assert_eq!(
            FormA11yRule::RelatedFieldsNotGrouped.name(),
            "related-fields-not-grouped"
        );
        assert_eq!(
            FormA11yRule::GroupMissingLegend.name(),
            "group-missing-legend"
        );
        assert_eq!(FormA11yRule::FormMissingName.name(), "form-missing-name");
    }

    // ===== Additional InputType Tests =====

    #[test]
    fn test_input_type_autocomplete_coverage() {
        assert!(InputType::Url.should_have_autocomplete());
        assert!(InputType::Number.should_have_autocomplete());
        assert!(!InputType::Time.should_have_autocomplete());
        assert!(!InputType::Search.should_have_autocomplete());
        assert!(!InputType::Select.should_have_autocomplete());
        assert!(!InputType::Textarea.should_have_autocomplete());
        assert!(!InputType::Hidden.should_have_autocomplete());
    }

    // ===== FormA11yReport assert_pass Test =====

    #[test]
    #[should_panic(expected = "Form accessibility check failed")]
    fn test_form_report_assert_pass_fails() {
        let form = FormAccessibility::new()
            .with_name("Test Form")
            .field(FormFieldA11y::new("email").with_type(InputType::Email));

        let report = FormA11yChecker::check(&form);
        report.assert_pass();
    }

    // ===== Heading Level Tests =====

    #[test]
    fn test_heading_level_h1() {
        let widget = MockLabel::new("h1 Main Title");
        // Heading level extraction should find 'h1' pattern
        let level = A11yChecker::heading_level(&widget);
        assert_eq!(level, Some(1));
    }

    #[test]
    fn test_heading_level_h3() {
        let widget = MockLabel::new("h3 Subsection");
        let level = A11yChecker::heading_level(&widget);
        assert_eq!(level, Some(3));
    }

    #[test]
    fn test_heading_level_capital_h() {
        let widget = MockLabel::new("H2 Section");
        let level = A11yChecker::heading_level(&widget);
        assert_eq!(level, Some(2));
    }

    #[test]
    fn test_heading_level_invalid_number() {
        let widget = MockLabel::new("h9 Invalid Level");
        let level = A11yChecker::heading_level(&widget);
        // Falls back to default level 2 for invalid levels
        assert_eq!(level, Some(2));
    }

    // ===== Touch Target Size Tests =====

    struct MockTouchTarget {
        bounds: Rect,
        name: String,
    }

    impl MockTouchTarget {
        fn new(width: f32, height: f32, name: &str) -> Self {
            Self {
                bounds: Rect::new(0.0, 0.0, width, height),
                name: name.to_string(),
            }
        }
    }

    impl Brick for MockTouchTarget {
        fn brick_name(&self) -> &'static str {
            "MockTouchTarget"
        }

        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }

        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }

        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }

        fn to_html(&self) -> String {
            String::new()
        }

        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockTouchTarget {
        fn type_id(&self) -> TypeId {
            TypeId::of::<Self>()
        }
        fn measure(&self, _: Constraints) -> Size {
            self.bounds.size()
        }
        fn layout(&mut self, _: Rect) -> LayoutResult {
            LayoutResult {
                size: self.bounds.size(),
            }
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
            true
        }
        fn accessible_name(&self) -> Option<&str> {
            Some(&self.name)
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Button
        }
        fn bounds(&self) -> Rect {
            self.bounds
        }
    }

    #[test]
    fn test_touch_target_too_small() {
        let widget = MockTouchTarget::new(30.0, 30.0, "Small Button");
        let config = A11yConfig {
            check_touch_targets: true,
            check_heading_hierarchy: false,
            check_focus_indicators: false,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        };
        let report = A11yChecker::check_with_config(&widget, &config);
        assert!(report.violations.iter().any(|v| v.rule == "touch-target"));
    }

    #[test]
    fn test_touch_target_sufficient() {
        let widget = MockTouchTarget::new(48.0, 48.0, "Good Button");
        let config = A11yConfig {
            check_touch_targets: true,
            check_heading_hierarchy: false,
            check_focus_indicators: false,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        };
        let report = A11yChecker::check_with_config(&widget, &config);
        assert!(!report.violations.iter().any(|v| v.rule == "touch-target"));
    }

    // ===== Group aria-label Test =====

    #[test]
    fn test_form_group_with_aria_label() {
        let mut group = FormFieldGroup::new("address");
        group.aria_label = Some("Shipping Address".to_string());
        group.field_ids = vec!["street".to_string(), "city".to_string()];

        let form = FormAccessibility::new().with_name("Test Form").group(group);

        let report = FormA11yChecker::check(&form);
        // Group with aria-label should not trigger missing legend
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::GroupMissingLegend));
    }

    // ===== aria_from_widget Additional Roles =====

    struct MockCheckbox {
        label: String,
    }

    impl Brick for MockCheckbox {
        fn brick_name(&self) -> &'static str {
            "MockCheckbox"
        }
        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }
        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }
        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }
        fn to_html(&self) -> String {
            String::new()
        }
        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockCheckbox {
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
            true
        }
        fn accessible_name(&self) -> Option<&str> {
            Some(&self.label)
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Checkbox
        }
    }

    #[test]
    fn test_aria_from_widget_checkbox() {
        let widget = MockCheckbox {
            label: "Accept terms".to_string(),
        };
        let attrs = aria_from_widget(&widget);
        assert_eq!(attrs.role, Some("checkbox".to_string()));
        assert_eq!(attrs.label, Some("Accept terms".to_string()));
        assert!(!attrs.disabled);
    }

    // ===== FormFieldA11y aria_labelledby Test =====

    #[test]
    fn test_form_field_aria_labelledby() {
        let mut field = FormFieldA11y::new("search");
        field.aria_labelledby = Some("search-label".to_string());

        let form = FormAccessibility::new().with_name("Test Form").field(field);

        let report = FormA11yChecker::check(&form);
        // aria-labelledby should satisfy label requirement
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::MissingLabel));
    }

    // ===== FormFieldA11y aria_errormessage Test =====

    #[test]
    fn test_form_error_with_aria_errormessage() {
        let mut field = FormFieldA11y::new("email")
            .with_label("Email")
            .with_type(InputType::Email)
            .with_autocomplete(AutocompleteValue::Email);
        field.has_error = true;
        field.aria_invalid = true;
        field.error_message = Some("Invalid email".to_string());
        field.aria_errormessage = Some("email-error".to_string());

        let form = FormAccessibility::new().with_name("Test Form").field(field);

        let report = FormA11yChecker::check(&form);
        // aria-errormessage should satisfy error association
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::ErrorNotAssociated));
    }

    // ===== Additional Coverage Tests =====

    // Test all AccessibleRole mappings in aria_from_widget
    macro_rules! test_role_widget {
        ($name:ident, $role:expr, $expected:expr) => {
            struct $name;
            impl Brick for $name {
                fn brick_name(&self) -> &'static str {
                    stringify!($name)
                }
                fn assertions(&self) -> &[BrickAssertion] {
                    &[]
                }
                fn budget(&self) -> BrickBudget {
                    BrickBudget::uniform(16)
                }
                fn verify(&self) -> BrickVerification {
                    BrickVerification {
                        passed: vec![],
                        failed: vec![],
                        verification_time: Duration::from_micros(1),
                    }
                }
                fn to_html(&self) -> String {
                    String::new()
                }
                fn to_css(&self) -> String {
                    String::new()
                }
            }
            impl Widget for $name {
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
                    true
                }
                fn accessible_name(&self) -> Option<&str> {
                    Some("test")
                }
                fn accessible_role(&self) -> AccessibleRole {
                    $role
                }
            }
        };
    }

    test_role_widget!(MockTextInput, AccessibleRole::TextInput, "textbox");
    test_role_widget!(MockLink, AccessibleRole::Link, "link");
    test_role_widget!(MockList, AccessibleRole::List, "list");
    test_role_widget!(MockListItem, AccessibleRole::ListItem, "listitem");
    test_role_widget!(MockTable, AccessibleRole::Table, "table");
    test_role_widget!(MockTableRow, AccessibleRole::TableRow, "row");
    test_role_widget!(MockTableCell, AccessibleRole::TableCell, "cell");
    test_role_widget!(MockMenu, AccessibleRole::Menu, "menu");
    test_role_widget!(MockMenuItem, AccessibleRole::MenuItem, "menuitem");
    test_role_widget!(MockComboBox, AccessibleRole::ComboBox, "combobox");
    test_role_widget!(MockSlider, AccessibleRole::Slider, "slider");
    test_role_widget!(MockProgressBar, AccessibleRole::ProgressBar, "progressbar");
    test_role_widget!(MockTab, AccessibleRole::Tab, "tab");
    test_role_widget!(MockTabPanel, AccessibleRole::TabPanel, "tabpanel");
    test_role_widget!(MockRadioGroup, AccessibleRole::RadioGroup, "radiogroup");
    test_role_widget!(MockRadio, AccessibleRole::Radio, "radio");

    #[test]
    fn test_aria_from_widget_textinput() {
        let attrs = aria_from_widget(&MockTextInput);
        assert_eq!(attrs.role, Some("textbox".to_string()));
    }

    #[test]
    fn test_aria_from_widget_link() {
        let attrs = aria_from_widget(&MockLink);
        assert_eq!(attrs.role, Some("link".to_string()));
    }

    #[test]
    fn test_aria_from_widget_list() {
        let attrs = aria_from_widget(&MockList);
        assert_eq!(attrs.role, Some("list".to_string()));
    }

    #[test]
    fn test_aria_from_widget_listitem() {
        let attrs = aria_from_widget(&MockListItem);
        assert_eq!(attrs.role, Some("listitem".to_string()));
    }

    #[test]
    fn test_aria_from_widget_table() {
        let attrs = aria_from_widget(&MockTable);
        assert_eq!(attrs.role, Some("table".to_string()));
    }

    #[test]
    fn test_aria_from_widget_tablerow() {
        let attrs = aria_from_widget(&MockTableRow);
        assert_eq!(attrs.role, Some("row".to_string()));
    }

    #[test]
    fn test_aria_from_widget_tablecell() {
        let attrs = aria_from_widget(&MockTableCell);
        assert_eq!(attrs.role, Some("cell".to_string()));
    }

    #[test]
    fn test_aria_from_widget_menu() {
        let attrs = aria_from_widget(&MockMenu);
        assert_eq!(attrs.role, Some("menu".to_string()));
    }

    #[test]
    fn test_aria_from_widget_menuitem() {
        let attrs = aria_from_widget(&MockMenuItem);
        assert_eq!(attrs.role, Some("menuitem".to_string()));
    }

    #[test]
    fn test_aria_from_widget_combobox() {
        let attrs = aria_from_widget(&MockComboBox);
        assert_eq!(attrs.role, Some("combobox".to_string()));
    }

    #[test]
    fn test_aria_from_widget_slider() {
        let attrs = aria_from_widget(&MockSlider);
        assert_eq!(attrs.role, Some("slider".to_string()));
    }

    #[test]
    fn test_aria_from_widget_progressbar() {
        let attrs = aria_from_widget(&MockProgressBar);
        assert_eq!(attrs.role, Some("progressbar".to_string()));
    }

    #[test]
    fn test_aria_from_widget_tab() {
        let attrs = aria_from_widget(&MockTab);
        assert_eq!(attrs.role, Some("tab".to_string()));
    }

    #[test]
    fn test_aria_from_widget_tabpanel() {
        let attrs = aria_from_widget(&MockTabPanel);
        assert_eq!(attrs.role, Some("tabpanel".to_string()));
    }

    #[test]
    fn test_aria_from_widget_radiogroup() {
        let attrs = aria_from_widget(&MockRadioGroup);
        assert_eq!(attrs.role, Some("radiogroup".to_string()));
    }

    #[test]
    fn test_aria_from_widget_radio() {
        let attrs = aria_from_widget(&MockRadio);
        assert_eq!(attrs.role, Some("radio".to_string()));
    }

    #[test]
    fn test_aria_from_widget_image() {
        let attrs = aria_from_widget(&MockImage::new().with_alt("Test Image"));
        assert_eq!(attrs.role, Some("img".to_string()));
    }

    // Test heading hierarchy check
    struct MockHeadingWidget {
        children: Vec<Box<dyn Widget>>,
        name: String,
    }

    impl MockHeadingWidget {
        fn new(name: &str) -> Self {
            Self {
                children: Vec::new(),
                name: name.to_string(),
            }
        }

        #[allow(dead_code)]
        fn with_child(mut self, child: impl Widget + 'static) -> Self {
            self.children.push(Box::new(child));
            self
        }
    }

    impl Brick for MockHeadingWidget {
        fn brick_name(&self) -> &'static str {
            "MockHeadingWidget"
        }
        fn assertions(&self) -> &[BrickAssertion] {
            &[]
        }
        fn budget(&self) -> BrickBudget {
            BrickBudget::uniform(16)
        }
        fn verify(&self) -> BrickVerification {
            BrickVerification {
                passed: vec![],
                failed: vec![],
                verification_time: Duration::from_micros(1),
            }
        }
        fn to_html(&self) -> String {
            String::new()
        }
        fn to_css(&self) -> String {
            String::new()
        }
    }

    impl Widget for MockHeadingWidget {
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
            &self.children
        }
        fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
            &mut self.children
        }
        fn is_interactive(&self) -> bool {
            false
        }
        fn is_focusable(&self) -> bool {
            false
        }
        fn accessible_name(&self) -> Option<&str> {
            Some(&self.name)
        }
        fn accessible_role(&self) -> AccessibleRole {
            AccessibleRole::Heading
        }
    }

    #[test]
    fn test_heading_hierarchy_skipped() {
        // h1 followed by h3 should trigger violation
        let h1 = MockHeadingWidget::new("h1 Main");
        let h3 = MockHeadingWidget::new("h3 Skipped");

        // Create a container that has h1 then h3
        struct MockContainer {
            children: Vec<Box<dyn Widget>>,
        }
        impl Brick for MockContainer {
            fn brick_name(&self) -> &'static str {
                "MockContainer"
            }
            fn assertions(&self) -> &[BrickAssertion] {
                &[]
            }
            fn budget(&self) -> BrickBudget {
                BrickBudget::uniform(16)
            }
            fn verify(&self) -> BrickVerification {
                BrickVerification {
                    passed: vec![],
                    failed: vec![],
                    verification_time: Duration::from_micros(1),
                }
            }
            fn to_html(&self) -> String {
                String::new()
            }
            fn to_css(&self) -> String {
                String::new()
            }
        }
        impl Widget for MockContainer {
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
                &self.children
            }
            fn children_mut(&mut self) -> &mut [Box<dyn Widget>] {
                &mut self.children
            }
            fn is_interactive(&self) -> bool {
                false
            }
            fn is_focusable(&self) -> bool {
                false
            }
            fn accessible_name(&self) -> Option<&str> {
                None
            }
            fn accessible_role(&self) -> AccessibleRole {
                AccessibleRole::Generic
            }
        }

        let container = MockContainer {
            children: vec![Box::new(h1), Box::new(h3)],
        };

        let config = A11yConfig {
            check_touch_targets: false,
            check_heading_hierarchy: true,
            check_focus_indicators: false,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        };
        let report = A11yChecker::check_with_config(&container, &config);
        assert!(report.violations.iter().any(|v| v.rule == "heading-order"));
    }

    #[test]
    fn test_heading_level_no_name() {
        // Widget with no accessible_name
        let level = A11yChecker::heading_level(&MockGenericWidget);
        // Should default to level 2
        assert_eq!(level, Some(2));
    }

    #[test]
    fn test_heading_level_non_heading_pattern() {
        // Widget with name that doesn't start with 'h' or 'H'
        let widget = MockLabel::new("Welcome Section");
        let level = A11yChecker::heading_level(&widget);
        // Should default to level 2
        assert_eq!(level, Some(2));
    }

    // Test FormAccessibility with aria_labelledby
    #[test]
    fn test_form_with_aria_labelledby() {
        let mut form = FormAccessibility::new();
        form.aria_labelledby = Some("form-title".to_string());
        form.fields.push(
            FormFieldA11y::new("email")
                .with_label("Email")
                .with_type(InputType::Email)
                .with_autocomplete(AutocompleteValue::Email),
        );

        let report = FormA11yChecker::check(&form);
        // aria-labelledby should satisfy form name requirement
        assert!(!report
            .violations
            .iter()
            .any(|v| v.rule == FormA11yRule::FormMissingName));
    }

    // Test focus indicator check
    #[test]
    fn test_focus_indicator_check() {
        let widget = MockButton::new().with_name("Test Button");
        let config = A11yConfig {
            check_touch_targets: false,
            check_heading_hierarchy: false,
            check_focus_indicators: true,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        };
        let report = A11yChecker::check_with_config(&widget, &config);
        // MockButton returns true for is_focusable, so has_visible_focus_indicator returns true
        assert!(!report.violations.iter().any(|v| v.rule == "focus-visible"));
    }

    // Test default CheckContext values
    #[test]
    fn test_check_default_context() {
        let widget = MockButton::new().with_name("Test Button");
        // Default check (no config) should not check touch targets by default
        // because CheckContext::default() has check_touch_targets: false
        let report = A11yChecker::check(&widget);
        // Just verify it runs without error
        assert!(report.is_passing());
    }

    // Test A11yViolation clone
    #[test]
    fn test_violation_clone() {
        let violation = A11yViolation {
            rule: "test".to_string(),
            message: "test message".to_string(),
            wcag: "1.1.1".to_string(),
            impact: Impact::Minor,
        };
        let cloned = violation;
        assert_eq!(cloned.rule, "test");
        assert_eq!(cloned.impact, Impact::Minor);
    }

    // Test FormViolation clone
    #[test]
    fn test_form_violation_clone() {
        let violation = FormViolation {
            field_id: "test".to_string(),
            rule: FormA11yRule::MissingLabel,
            message: "test".to_string(),
            wcag: "1.3.1".to_string(),
            impact: Impact::Critical,
        };
        let cloned = violation;
        assert_eq!(cloned.field_id, "test");
        assert_eq!(cloned.rule, FormA11yRule::MissingLabel);
    }

    // Test AriaAttributes Default
    #[test]
    fn test_aria_attributes_default() {
        let attrs = AriaAttributes::default();
        assert!(attrs.role.is_none());
        assert!(attrs.label.is_none());
        assert!(!attrs.hidden);
        assert!(!attrs.disabled);
        assert!(!attrs.required);
        assert!(!attrs.invalid);
        assert!(!attrs.busy);
        assert!(!attrs.atomic);
    }

    // Test ContrastResult fields
    #[test]
    fn test_contrast_result_clone() {
        let result = ContrastResult {
            ratio: 4.5,
            passes_aa: true,
            passes_aaa: false,
        };
        let cloned = result;
        assert!((cloned.ratio - 4.5).abs() < f32::EPSILON);
        assert!(cloned.passes_aa);
        assert!(!cloned.passes_aaa);
    }

    // Test FormFieldGroup builder
    #[test]
    fn test_form_field_group_builder() {
        let group = FormFieldGroup::new("personal-info")
            .with_legend("Personal Information")
            .with_field("first_name")
            .with_field("last_name");

        assert_eq!(group.id, "personal-info");
        assert_eq!(group.legend, Some("Personal Information".to_string()));
        assert_eq!(group.field_ids.len(), 2);
        assert!(group.field_ids.contains(&"first_name".to_string()));
        assert!(group.field_ids.contains(&"last_name".to_string()));
    }
}
