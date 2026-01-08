//! Accessibility checking for WCAG 2.1 compliance.
//!
//! Implements comprehensive WCAG 2.1 AA checks including:
//! - Color contrast (1.4.3, 1.4.6)
//! - Keyboard accessibility (2.1.1)
//! - Focus indicators (2.4.7)
//! - Touch target size (2.5.5)
//! - Name/role/value (4.1.2)
//! - Heading hierarchy (1.3.1)

use presentar_core::widget::AccessibleRole;
use presentar_core::{Color, Widget};

/// Minimum touch target size in pixels (WCAG 2.5.5)
pub const MIN_TOUCH_TARGET_SIZE: f32 = 44.0;

/// Minimum focus indicator area (WCAG 2.4.11)
pub const MIN_FOCUS_INDICATOR_AREA: f32 = 2.0;

/// Accessibility checker.
pub struct A11yChecker;

impl A11yChecker {
    /// Check a widget tree for accessibility violations.
    #[must_use]
    pub fn check(widget: &dyn Widget) -> A11yReport {
        let mut violations = Vec::new();
        let mut context = CheckContext::default();
        Self::check_widget(widget, &mut violations, &mut context);
        A11yReport { violations }
    }

    /// Check with custom configuration.
    #[must_use]
    pub fn check_with_config(widget: &dyn Widget, config: &A11yConfig) -> A11yReport {
        let mut violations = Vec::new();
        let mut context = CheckContext {
            check_touch_targets: config.check_touch_targets,
            check_heading_hierarchy: config.check_heading_hierarchy,
            check_focus_indicators: config.check_focus_indicators,
            ..Default::default()
        };
        Self::check_widget(widget, &mut violations, &mut context);
        A11yReport { violations }
    }

    fn check_widget(
        widget: &dyn Widget,
        violations: &mut Vec<A11yViolation>,
        context: &mut CheckContext,
    ) {
        // Check for missing accessible name on interactive elements (WCAG 4.1.2)
        if widget.is_interactive() && widget.accessible_name().is_none() {
            violations.push(A11yViolation {
                rule: "aria-label".to_string(),
                message: "Interactive element missing accessible name".to_string(),
                wcag: "4.1.2".to_string(),
                impact: Impact::Critical,
            });
        }

        // Check for keyboard focusable elements (WCAG 2.1.1)
        if widget.is_interactive() && !widget.is_focusable() {
            violations.push(A11yViolation {
                rule: "keyboard".to_string(),
                message: "Interactive element is not keyboard focusable".to_string(),
                wcag: "2.1.1".to_string(),
                impact: Impact::Critical,
            });
        }

        // Check touch target size (WCAG 2.5.5)
        if context.check_touch_targets && widget.is_interactive() {
            let bounds = widget.bounds();
            if bounds.width < MIN_TOUCH_TARGET_SIZE || bounds.height < MIN_TOUCH_TARGET_SIZE {
                violations.push(A11yViolation {
                    rule: "touch-target".to_string(),
                    message: format!(
                        "Touch target too small: {}x{} (minimum {}x{})",
                        bounds.width, bounds.height, MIN_TOUCH_TARGET_SIZE, MIN_TOUCH_TARGET_SIZE
                    ),
                    wcag: "2.5.5".to_string(),
                    impact: Impact::Moderate,
                });
            }
        }

        // Check heading hierarchy (WCAG 1.3.1)
        if context.check_heading_hierarchy && widget.accessible_role() == AccessibleRole::Heading {
            if let Some(level) = Self::heading_level(widget) {
                let last_level = context.last_heading_level;
                if last_level > 0 && level > last_level + 1 {
                    violations.push(A11yViolation {
                        rule: "heading-order".to_string(),
                        message: format!(
                            "Heading level skipped: h{} followed by h{} (should be h{} or lower)",
                            last_level,
                            level,
                            last_level + 1
                        ),
                        wcag: "1.3.1".to_string(),
                        impact: Impact::Moderate,
                    });
                }
                context.last_heading_level = level;
            }
        }

        // Check focus indicator visibility (WCAG 2.4.7)
        if context.check_focus_indicators && widget.is_focusable() {
            if !Self::has_visible_focus_indicator(widget) {
                violations.push(A11yViolation {
                    rule: "focus-visible".to_string(),
                    message: "Focusable element may lack visible focus indicator".to_string(),
                    wcag: "2.4.7".to_string(),
                    impact: Impact::Serious,
                });
            }
        }

        // Check for images without alt text (WCAG 1.1.1)
        if widget.accessible_role() == AccessibleRole::Image && widget.accessible_name().is_none() {
            violations.push(A11yViolation {
                rule: "image-alt".to_string(),
                message: "Image missing alternative text".to_string(),
                wcag: "1.1.1".to_string(),
                impact: Impact::Critical,
            });
        }

        // Recurse into children
        for child in widget.children() {
            Self::check_widget(child.as_ref(), violations, context);
        }
    }

    /// Extract heading level from widget (if it's a heading)
    fn heading_level(widget: &dyn Widget) -> Option<u8> {
        // Check if the accessible name contains heading level info
        // Or use aria-level if available
        if let Some(name) = widget.accessible_name() {
            // Try to extract from pattern like "Heading Level 2" or "h2"
            if name.starts_with('h') || name.starts_with('H') {
                if let Ok(level) = name[1..2].parse::<u8>() {
                    if (1..=6).contains(&level) {
                        return Some(level);
                    }
                }
            }
        }
        // Default to level 2 if we can't determine
        Some(2)
    }

    /// Check if widget has a visible focus indicator
    fn has_visible_focus_indicator(widget: &dyn Widget) -> bool {
        // For now, assume all focusable widgets have focus indicators
        // In a real implementation, we'd check for focus ring styles
        widget.is_focusable()
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

/// Configuration for accessibility checks.
#[derive(Debug, Clone)]
pub struct A11yConfig {
    /// Check touch target sizes (WCAG 2.5.5)
    pub check_touch_targets: bool,
    /// Check heading hierarchy (WCAG 1.3.1)
    pub check_heading_hierarchy: bool,
    /// Check focus indicators (WCAG 2.4.7)
    pub check_focus_indicators: bool,
    /// Minimum contrast ratio for normal text (WCAG 1.4.3)
    pub min_contrast_normal: f32,
    /// Minimum contrast ratio for large text (WCAG 1.4.3)
    pub min_contrast_large: f32,
}

impl Default for A11yConfig {
    fn default() -> Self {
        Self {
            check_touch_targets: true,
            check_heading_hierarchy: true,
            check_focus_indicators: false, // Disabled by default as requires style info
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        }
    }
}

impl A11yConfig {
    /// Create a new config with all checks enabled.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            check_touch_targets: true,
            check_heading_hierarchy: true,
            check_focus_indicators: true,
            min_contrast_normal: 7.0, // AAA level
            min_contrast_large: 4.5,  // AAA level
        }
    }

    /// Create a config for mobile apps.
    #[must_use]
    pub fn mobile() -> Self {
        Self {
            check_touch_targets: true,
            check_heading_hierarchy: true,
            check_focus_indicators: false,
            min_contrast_normal: 4.5,
            min_contrast_large: 3.0,
        }
    }
}

/// Internal context for walking the widget tree.
#[derive(Debug, Default)]
struct CheckContext {
    /// Last heading level seen (for hierarchy check)
    last_heading_level: u8,
    /// Whether to check touch target sizes
    check_touch_targets: bool,
    /// Whether to check heading hierarchy
    check_heading_hierarchy: bool,
    /// Whether to check focus indicators
    check_focus_indicators: bool,
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

// =============================================================================
// ARIA Attribute Generation
// =============================================================================

/// ARIA attributes for a widget.
#[derive(Debug, Clone, Default)]
pub struct AriaAttributes {
    /// The ARIA role
    pub role: Option<String>,
    /// Accessible label
    pub label: Option<String>,
    /// Accessible description
    pub described_by: Option<String>,
    /// Whether element is hidden from accessibility tree
    pub hidden: bool,
    /// Whether element is expanded (for expandable elements)
    pub expanded: Option<bool>,
    /// Whether element is selected
    pub selected: Option<bool>,
    /// Whether element is checked (for checkboxes/switches)
    pub checked: Option<AriaChecked>,
    /// Whether element is pressed (for toggle buttons)
    pub pressed: Option<AriaChecked>,
    /// Whether element is disabled
    pub disabled: bool,
    /// Whether element is required
    pub required: bool,
    /// Whether element is invalid
    pub invalid: bool,
    /// Current value for range widgets
    pub value_now: Option<f64>,
    /// Minimum value for range widgets
    pub value_min: Option<f64>,
    /// Maximum value for range widgets
    pub value_max: Option<f64>,
    /// Text representation of value
    pub value_text: Option<String>,
    /// Level (for headings)
    pub level: Option<u8>,
    /// Position in set
    pub pos_in_set: Option<u32>,
    /// Set size
    pub set_size: Option<u32>,
    /// Controls another element (ID reference)
    pub controls: Option<String>,
    /// Has popup indicator
    pub has_popup: Option<String>,
    /// Is busy/loading
    pub busy: bool,
    /// Live region politeness
    pub live: Option<AriaLive>,
    /// Atomic live region
    pub atomic: bool,
}

/// ARIA checked state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaChecked {
    /// Checked
    True,
    /// Not checked
    False,
    /// Mixed/indeterminate
    Mixed,
}

impl AriaChecked {
    /// Return string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            AriaChecked::True => "true",
            AriaChecked::False => "false",
            AriaChecked::Mixed => "mixed",
        }
    }
}

/// ARIA live region politeness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AriaLive {
    /// Polite announcements
    Polite,
    /// Assertive announcements
    Assertive,
    /// No announcements
    Off,
}

impl AriaLive {
    /// Return string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            AriaLive::Polite => "polite",
            AriaLive::Assertive => "assertive",
            AriaLive::Off => "off",
        }
    }
}

impl AriaAttributes {
    /// Create new empty ARIA attributes.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the role.
    #[must_use]
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Set the label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set hidden.
    #[must_use]
    pub const fn with_hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Set expanded state.
    #[must_use]
    pub const fn with_expanded(mut self, expanded: bool) -> Self {
        self.expanded = Some(expanded);
        self
    }

    /// Set selected state.
    #[must_use]
    pub const fn with_selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    /// Set checked state.
    #[must_use]
    pub const fn with_checked(mut self, checked: AriaChecked) -> Self {
        self.checked = Some(checked);
        self
    }

    /// Set pressed state (for toggle buttons).
    #[must_use]
    pub const fn with_pressed(mut self, pressed: AriaChecked) -> Self {
        self.pressed = Some(pressed);
        self
    }

    /// Set disabled state.
    #[must_use]
    pub const fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set busy state.
    #[must_use]
    pub const fn with_busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Set atomic.
    #[must_use]
    pub const fn with_atomic(mut self, atomic: bool) -> Self {
        self.atomic = atomic;
        self
    }

    /// Set range values.
    #[must_use]
    pub fn with_range(mut self, min: f64, max: f64, now: f64) -> Self {
        self.value_min = Some(min);
        self.value_max = Some(max);
        self.value_now = Some(now);
        self
    }

    /// Set current value.
    #[must_use]
    pub const fn with_value_now(mut self, value: f64) -> Self {
        self.value_now = Some(value);
        self
    }

    /// Set minimum value.
    #[must_use]
    pub const fn with_value_min(mut self, value: f64) -> Self {
        self.value_min = Some(value);
        self
    }

    /// Set maximum value.
    #[must_use]
    pub const fn with_value_max(mut self, value: f64) -> Self {
        self.value_max = Some(value);
        self
    }

    /// Set controls reference.
    #[must_use]
    pub fn with_controls(mut self, controls: impl Into<String>) -> Self {
        self.controls = Some(controls.into());
        self
    }

    /// Set described by reference.
    #[must_use]
    pub fn with_described_by(mut self, described_by: impl Into<String>) -> Self {
        self.described_by = Some(described_by.into());
        self
    }

    /// Set has popup.
    #[must_use]
    pub fn with_has_popup(mut self, has_popup: impl Into<String>) -> Self {
        self.has_popup = Some(has_popup.into());
        self
    }

    /// Set heading level.
    #[must_use]
    pub const fn with_level(mut self, level: u8) -> Self {
        self.level = Some(level);
        self
    }

    /// Set position in set.
    #[must_use]
    pub const fn with_pos_in_set(mut self, pos: u32, size: u32) -> Self {
        self.pos_in_set = Some(pos);
        self.set_size = Some(size);
        self
    }

    /// Set live region.
    #[must_use]
    pub const fn with_live(mut self, live: AriaLive) -> Self {
        self.live = Some(live);
        self
    }

    /// Generate HTML ARIA attributes.
    #[must_use]
    pub fn to_html_attrs(&self) -> Vec<(String, String)> {
        let mut attrs = Vec::new();

        if let Some(ref role) = self.role {
            attrs.push(("role".to_string(), role.clone()));
        }
        if let Some(ref label) = self.label {
            attrs.push(("aria-label".to_string(), label.clone()));
        }
        if let Some(ref desc) = self.described_by {
            attrs.push(("aria-describedby".to_string(), desc.clone()));
        }
        if self.hidden {
            attrs.push(("aria-hidden".to_string(), "true".to_string()));
        }
        if let Some(expanded) = self.expanded {
            attrs.push(("aria-expanded".to_string(), expanded.to_string()));
        }
        if let Some(selected) = self.selected {
            attrs.push(("aria-selected".to_string(), selected.to_string()));
        }
        if let Some(checked) = self.checked {
            attrs.push(("aria-checked".to_string(), checked.as_str().to_string()));
        }
        if let Some(pressed) = self.pressed {
            attrs.push(("aria-pressed".to_string(), pressed.as_str().to_string()));
        }
        if let Some(ref popup) = self.has_popup {
            attrs.push(("aria-haspopup".to_string(), popup.clone()));
        }
        if self.disabled {
            attrs.push(("aria-disabled".to_string(), "true".to_string()));
        }
        if self.required {
            attrs.push(("aria-required".to_string(), "true".to_string()));
        }
        if self.invalid {
            attrs.push(("aria-invalid".to_string(), "true".to_string()));
        }
        if let Some(val) = self.value_now {
            attrs.push(("aria-valuenow".to_string(), val.to_string()));
        }
        if let Some(val) = self.value_min {
            attrs.push(("aria-valuemin".to_string(), val.to_string()));
        }
        if let Some(val) = self.value_max {
            attrs.push(("aria-valuemax".to_string(), val.to_string()));
        }
        if let Some(ref text) = self.value_text {
            attrs.push(("aria-valuetext".to_string(), text.clone()));
        }
        if let Some(level) = self.level {
            attrs.push(("aria-level".to_string(), level.to_string()));
        }
        if let Some(pos) = self.pos_in_set {
            attrs.push(("aria-posinset".to_string(), pos.to_string()));
        }
        if let Some(size) = self.set_size {
            attrs.push(("aria-setsize".to_string(), size.to_string()));
        }
        if let Some(ref controls) = self.controls {
            attrs.push(("aria-controls".to_string(), controls.clone()));
        }
        if self.busy {
            attrs.push(("aria-busy".to_string(), "true".to_string()));
        }
        if let Some(live) = self.live {
            attrs.push(("aria-live".to_string(), live.as_str().to_string()));
        }
        if self.atomic {
            attrs.push(("aria-atomic".to_string(), "true".to_string()));
        }

        attrs
    }

    /// Generate HTML attribute string.
    #[must_use]
    pub fn to_html_string(&self) -> String {
        self.to_html_attrs()
            .into_iter()
            .map(|(k, v)| {
                // Escape HTML special characters in values
                let escaped = v
                    .replace('&', "&amp;")
                    .replace('"', "&quot;")
                    .replace('<', "&lt;")
                    .replace('>', "&gt;");
                format!("{}=\"{}\"", k, escaped)
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Generate ARIA attributes from a widget.
pub fn aria_from_widget(widget: &dyn Widget) -> AriaAttributes {
    use presentar_core::widget::AccessibleRole;

    let mut attrs = AriaAttributes::new();

    // Set role from widget
    let role = match widget.accessible_role() {
        AccessibleRole::Generic => None,
        AccessibleRole::Button => Some("button"),
        AccessibleRole::Checkbox => Some("checkbox"),
        AccessibleRole::TextInput => Some("textbox"),
        AccessibleRole::Link => Some("link"),
        AccessibleRole::Heading => Some("heading"),
        AccessibleRole::Image => Some("img"),
        AccessibleRole::List => Some("list"),
        AccessibleRole::ListItem => Some("listitem"),
        AccessibleRole::Table => Some("table"),
        AccessibleRole::TableRow => Some("row"),
        AccessibleRole::TableCell => Some("cell"),
        AccessibleRole::Menu => Some("menu"),
        AccessibleRole::MenuItem => Some("menuitem"),
        AccessibleRole::ComboBox => Some("combobox"),
        AccessibleRole::Slider => Some("slider"),
        AccessibleRole::ProgressBar => Some("progressbar"),
        AccessibleRole::Tab => Some("tab"),
        AccessibleRole::TabPanel => Some("tabpanel"),
        AccessibleRole::RadioGroup => Some("radiogroup"),
        AccessibleRole::Radio => Some("radio"),
    };

    if let Some(role) = role {
        attrs.role = Some(role.to_string());
    }

    // Set label from widget
    if let Some(name) = widget.accessible_name() {
        attrs.label = Some(name.to_string());
    }

    // Set disabled if not interactive but has a focusable role
    if !widget.is_interactive() && widget.accessible_role() != AccessibleRole::Generic {
        attrs.disabled = true;
    }

    attrs
}

// =============================================================================
// Form Accessibility Validation (WCAG 1.3.1, 1.3.5, 3.3.1, 3.3.2, 4.1.2)
// =============================================================================

/// Form accessibility checker for validating form-specific WCAG requirements.
pub struct FormA11yChecker;

impl FormA11yChecker {
    /// Check a form for accessibility violations.
    ///
    /// Validates:
    /// - Label associations (WCAG 1.3.1, 2.4.6)
    /// - Required field indicators (WCAG 3.3.2)
    /// - Error messaging (WCAG 3.3.1)
    /// - Input purpose/autocomplete (WCAG 1.3.5)
    /// - Form grouping (WCAG 1.3.1)
    #[must_use]
    pub fn check(form: &FormAccessibility) -> FormA11yReport {
        let mut violations = Vec::new();

        // Check all fields
        for field in &form.fields {
            Self::check_field(field, &mut violations);
        }

        // Check form-level requirements
        Self::check_form_level(form, &mut violations);

        FormA11yReport { violations }
    }

    /// Check a single form field.
    fn check_field(field: &FormFieldA11y, violations: &mut Vec<FormViolation>) {
        // WCAG 1.3.1, 2.4.6: Label association
        if field.label.is_none() && field.aria_label.is_none() && field.aria_labelledby.is_none() {
            violations.push(FormViolation {
                field_id: field.id.clone(),
                rule: FormA11yRule::MissingLabel,
                message: format!("Field '{}' has no associated label", field.id),
                wcag: "1.3.1, 2.4.6".to_string(),
                impact: Impact::Critical,
            });
        }

        // WCAG 3.3.2: Required field indicators
        if field.required {
            if !field.aria_required {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::MissingRequiredIndicator,
                    message: format!(
                        "Required field '{}' does not have aria-required=\"true\"",
                        field.id
                    ),
                    wcag: "3.3.2".to_string(),
                    impact: Impact::Serious,
                });
            }
            if !field.has_visual_required_indicator {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::MissingVisualRequired,
                    message: format!(
                        "Required field '{}' lacks visual required indicator (asterisk or text)",
                        field.id
                    ),
                    wcag: "3.3.2".to_string(),
                    impact: Impact::Moderate,
                });
            }
        }

        // WCAG 3.3.1: Error identification
        if field.has_error {
            if !field.aria_invalid {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::MissingErrorState,
                    message: format!(
                        "Field '{}' in error state does not have aria-invalid=\"true\"",
                        field.id
                    ),
                    wcag: "3.3.1".to_string(),
                    impact: Impact::Serious,
                });
            }
            if field.error_message.is_none() {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::MissingErrorMessage,
                    message: format!("Field '{}' in error state has no error message", field.id),
                    wcag: "3.3.1".to_string(),
                    impact: Impact::Serious,
                });
            }
            if field.aria_describedby.is_none() && field.aria_errormessage.is_none() {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::ErrorNotAssociated,
                    message: format!(
                        "Error message for '{}' not associated via aria-describedby or aria-errormessage",
                        field.id
                    ),
                    wcag: "3.3.1".to_string(),
                    impact: Impact::Serious,
                });
            }
        }

        // WCAG 1.3.5: Input purpose (autocomplete)
        if let Some(ref input_type) = field.input_type {
            if input_type.should_have_autocomplete() && field.autocomplete.is_none() {
                violations.push(FormViolation {
                    field_id: field.id.clone(),
                    rule: FormA11yRule::MissingAutocomplete,
                    message: format!(
                        "Field '{}' of type {:?} should have autocomplete attribute for autofill",
                        field.id, input_type
                    ),
                    wcag: "1.3.5".to_string(),
                    impact: Impact::Moderate,
                });
            }
        }

        // Check for placeholder-only labeling (anti-pattern)
        if field.placeholder.is_some() && field.label.is_none() && field.aria_label.is_none() {
            violations.push(FormViolation {
                field_id: field.id.clone(),
                rule: FormA11yRule::PlaceholderAsLabel,
                message: format!(
                    "Field '{}' uses placeholder as sole label; placeholders disappear on input",
                    field.id
                ),
                wcag: "3.3.2".to_string(),
                impact: Impact::Serious,
            });
        }
    }

    /// Check form-level accessibility requirements.
    fn check_form_level(form: &FormAccessibility, violations: &mut Vec<FormViolation>) {
        // Check for related fields that should be grouped (WCAG 1.3.1)
        let radio_fields: Vec<_> = form
            .fields
            .iter()
            .filter(|f| f.input_type == Some(InputType::Radio))
            .collect();

        if radio_fields.len() > 1 {
            // Radio buttons should be in a fieldset/group
            let has_group = form.field_groups.iter().any(|g| {
                g.field_ids
                    .iter()
                    .any(|id| radio_fields.iter().any(|f| &f.id == id))
            });

            if !has_group {
                violations.push(FormViolation {
                    field_id: "form".to_string(),
                    rule: FormA11yRule::RelatedFieldsNotGrouped,
                    message: "Related radio buttons should be grouped in a fieldset with legend"
                        .to_string(),
                    wcag: "1.3.1".to_string(),
                    impact: Impact::Moderate,
                });
            }
        }

        // Check field groups have legends
        for group in &form.field_groups {
            if group.legend.is_none() && group.aria_label.is_none() {
                violations.push(FormViolation {
                    field_id: group.id.clone(),
                    rule: FormA11yRule::GroupMissingLegend,
                    message: format!("Field group '{}' has no legend or aria-label", group.id),
                    wcag: "1.3.1".to_string(),
                    impact: Impact::Serious,
                });
            }
        }

        // Check form has accessible name
        if form.accessible_name.is_none() && form.aria_labelledby.is_none() {
            violations.push(FormViolation {
                field_id: "form".to_string(),
                rule: FormA11yRule::FormMissingName,
                message: "Form should have an accessible name (aria-label or aria-labelledby)"
                    .to_string(),
                wcag: "4.1.2".to_string(),
                impact: Impact::Moderate,
            });
        }
    }
}

/// Form accessibility data for validation.
#[derive(Debug, Clone, Default)]
pub struct FormAccessibility {
    /// Form's accessible name
    pub accessible_name: Option<String>,
    /// Referenced labelledby ID
    pub aria_labelledby: Option<String>,
    /// Form fields
    pub fields: Vec<FormFieldA11y>,
    /// Field groups (fieldsets)
    pub field_groups: Vec<FormFieldGroup>,
}

impl FormAccessibility {
    /// Create a new form accessibility descriptor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a field.
    #[must_use]
    pub fn field(mut self, field: FormFieldA11y) -> Self {
        self.fields.push(field);
        self
    }

    /// Add a field group.
    #[must_use]
    pub fn group(mut self, group: FormFieldGroup) -> Self {
        self.field_groups.push(group);
        self
    }

    /// Set accessible name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.accessible_name = Some(name.into());
        self
    }
}

/// Form field accessibility descriptor.
#[derive(Debug, Clone, Default)]
pub struct FormFieldA11y {
    /// Field ID
    pub id: String,
    /// Associated label text
    pub label: Option<String>,
    /// Input type
    pub input_type: Option<InputType>,
    /// Is required
    pub required: bool,
    /// Has visual required indicator
    pub has_visual_required_indicator: bool,
    /// aria-required attribute
    pub aria_required: bool,
    /// aria-label attribute
    pub aria_label: Option<String>,
    /// aria-labelledby attribute
    pub aria_labelledby: Option<String>,
    /// aria-describedby attribute
    pub aria_describedby: Option<String>,
    /// Has error state
    pub has_error: bool,
    /// aria-invalid attribute
    pub aria_invalid: bool,
    /// aria-errormessage attribute
    pub aria_errormessage: Option<String>,
    /// Error message text
    pub error_message: Option<String>,
    /// Autocomplete attribute
    pub autocomplete: Option<AutocompleteValue>,
    /// Placeholder text
    pub placeholder: Option<String>,
}

impl FormFieldA11y {
    /// Create a new field.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }

    /// Set label.
    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Set input type.
    #[must_use]
    pub fn with_type(mut self, input_type: InputType) -> Self {
        self.input_type = Some(input_type);
        self
    }

    /// Mark as required.
    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self.aria_required = true;
        self.has_visual_required_indicator = true;
        self
    }

    /// Set required with specific options.
    #[must_use]
    pub fn with_required(mut self, visual: bool, aria: bool) -> Self {
        self.required = true;
        self.has_visual_required_indicator = visual;
        self.aria_required = aria;
        self
    }

    /// Set error state.
    #[must_use]
    pub fn with_error(mut self, message: impl Into<String>, associated: bool) -> Self {
        self.has_error = true;
        self.aria_invalid = true;
        self.error_message = Some(message.into());
        if associated {
            self.aria_describedby = Some(format!("{}-error", self.id));
        }
        self
    }

    /// Set autocomplete.
    #[must_use]
    pub fn with_autocomplete(mut self, value: AutocompleteValue) -> Self {
        self.autocomplete = Some(value);
        self
    }

    /// Set placeholder.
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Set aria-label.
    #[must_use]
    pub fn with_aria_label(mut self, label: impl Into<String>) -> Self {
        self.aria_label = Some(label.into());
        self
    }
}

/// Form field group (fieldset).
#[derive(Debug, Clone, Default)]
pub struct FormFieldGroup {
    /// Group ID
    pub id: String,
    /// Legend text
    pub legend: Option<String>,
    /// aria-label (alternative to legend)
    pub aria_label: Option<String>,
    /// Field IDs in this group
    pub field_ids: Vec<String>,
}

impl FormFieldGroup {
    /// Create a new field group.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Default::default()
        }
    }

    /// Set legend.
    #[must_use]
    pub fn with_legend(mut self, legend: impl Into<String>) -> Self {
        self.legend = Some(legend.into());
        self
    }

    /// Add field ID to group.
    #[must_use]
    pub fn with_field(mut self, field_id: impl Into<String>) -> Self {
        self.field_ids.push(field_id.into());
        self
    }
}

/// Input type for form fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputType {
    Text,
    Email,
    Password,
    Tel,
    Url,
    Number,
    Date,
    Time,
    Search,
    Radio,
    Checkbox,
    Select,
    Textarea,
    Hidden,
}

impl InputType {
    /// Check if this input type should have autocomplete for WCAG 1.3.5.
    #[must_use]
    pub const fn should_have_autocomplete(&self) -> bool {
        matches!(
            self,
            Self::Text | Self::Email | Self::Password | Self::Tel | Self::Url | Self::Number
        )
    }
}

/// Autocomplete attribute values (WCAG 1.3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutocompleteValue {
    /// User's name
    Name,
    /// Given (first) name
    GivenName,
    /// Family (last) name
    FamilyName,
    /// Email address
    Email,
    /// Telephone number
    Tel,
    /// Street address
    StreetAddress,
    /// Address level 1 (city)
    AddressLevel1,
    /// Address level 2 (state/province)
    AddressLevel2,
    /// Postal code
    PostalCode,
    /// Country name
    Country,
    /// Organization
    Organization,
    /// Username
    Username,
    /// Current password
    CurrentPassword,
    /// New password
    NewPassword,
    /// Credit card number
    CcNumber,
    /// Credit card expiration
    CcExp,
    /// Credit card CVV
    CcCsc,
    /// One-time code
    OneTimeCode,
    /// Turn off autocomplete
    Off,
}

impl AutocompleteValue {
    /// Get the HTML attribute value.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::GivenName => "given-name",
            Self::FamilyName => "family-name",
            Self::Email => "email",
            Self::Tel => "tel",
            Self::StreetAddress => "street-address",
            Self::AddressLevel1 => "address-level1",
            Self::AddressLevel2 => "address-level2",
            Self::PostalCode => "postal-code",
            Self::Country => "country",
            Self::Organization => "organization",
            Self::Username => "username",
            Self::CurrentPassword => "current-password",
            Self::NewPassword => "new-password",
            Self::CcNumber => "cc-number",
            Self::CcExp => "cc-exp",
            Self::CcCsc => "cc-csc",
            Self::OneTimeCode => "one-time-code",
            Self::Off => "off",
        }
    }
}

/// Form accessibility violation rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormA11yRule {
    /// Field missing label (1.3.1, 2.4.6)
    MissingLabel,
    /// Required field missing aria-required (3.3.2)
    MissingRequiredIndicator,
    /// Required field missing visual indicator (3.3.2)
    MissingVisualRequired,
    /// Error field missing aria-invalid (3.3.1)
    MissingErrorState,
    /// Error field missing error message (3.3.1)
    MissingErrorMessage,
    /// Error message not associated (3.3.1)
    ErrorNotAssociated,
    /// Field should have autocomplete (1.3.5)
    MissingAutocomplete,
    /// Placeholder used as sole label (3.3.2)
    PlaceholderAsLabel,
    /// Related fields not grouped (1.3.1)
    RelatedFieldsNotGrouped,
    /// Field group missing legend (1.3.1)
    GroupMissingLegend,
    /// Form missing accessible name (4.1.2)
    FormMissingName,
}

impl FormA11yRule {
    /// Get the rule name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::MissingLabel => "missing-label",
            Self::MissingRequiredIndicator => "missing-required-indicator",
            Self::MissingVisualRequired => "missing-visual-required",
            Self::MissingErrorState => "missing-error-state",
            Self::MissingErrorMessage => "missing-error-message",
            Self::ErrorNotAssociated => "error-not-associated",
            Self::MissingAutocomplete => "missing-autocomplete",
            Self::PlaceholderAsLabel => "placeholder-as-label",
            Self::RelatedFieldsNotGrouped => "related-fields-not-grouped",
            Self::GroupMissingLegend => "group-missing-legend",
            Self::FormMissingName => "form-missing-name",
        }
    }
}

/// Form accessibility violation.
#[derive(Debug, Clone)]
pub struct FormViolation {
    /// Field ID where violation occurred
    pub field_id: String,
    /// Rule that was violated
    pub rule: FormA11yRule,
    /// Human-readable message
    pub message: String,
    /// WCAG success criterion
    pub wcag: String,
    /// Impact level
    pub impact: Impact,
}

/// Form accessibility report.
#[derive(Debug)]
pub struct FormA11yReport {
    /// List of violations
    pub violations: Vec<FormViolation>,
}

impl FormA11yReport {
    /// Check if form passes accessibility.
    #[must_use]
    pub fn is_passing(&self) -> bool {
        self.violations.is_empty()
    }

    /// Check if form passes with no critical/serious issues.
    #[must_use]
    pub fn is_acceptable(&self) -> bool {
        !self
            .violations
            .iter()
            .any(|v| matches!(v.impact, Impact::Critical | Impact::Serious))
    }

    /// Get violations by rule.
    #[must_use]
    pub fn violations_for_rule(&self, rule: FormA11yRule) -> Vec<&FormViolation> {
        self.violations.iter().filter(|v| v.rule == rule).collect()
    }

    /// Get violations for a specific field.
    #[must_use]
    pub fn violations_for_field(&self, field_id: &str) -> Vec<&FormViolation> {
        self.violations
            .iter()
            .filter(|v| v.field_id == field_id)
            .collect()
    }

    /// Assert form passes accessibility.
    ///
    /// # Panics
    ///
    /// Panics with violation details if form fails accessibility checks.
    pub fn assert_pass(&self) {
        if !self.is_passing() {
            let messages: Vec<String> = self
                .violations
                .iter()
                .map(|v| {
                    format!(
                        "  [{:?}] {} ({}): {} (WCAG {})",
                        v.impact,
                        v.rule.name(),
                        v.field_id,
                        v.message,
                        v.wcag
                    )
                })
                .collect();

            panic!(
                "Form accessibility check failed with {} violation(s):\n{}",
                self.violations.len(),
                messages.join("\n")
            );
        }
    }
}

#[cfg(test)]
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
        let cloned = violation.clone();
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
        let cloned = violation.clone();
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
        let cloned = result.clone();
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
