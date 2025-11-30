//! Declarative validation system for forms and inputs.
//!
//! This module provides a flexible validation framework with:
//! - Built-in validators (required, min/max, pattern, email, etc.)
//! - Custom validator support
//! - Field-level and form-level validation
//! - Validation state management

use std::collections::HashMap;
use std::fmt;

/// Validation result for a single field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationResult {
    /// Validation passed.
    Valid,
    /// Validation failed with an error message.
    Invalid(String),
    /// Validation is pending (e.g., async validation).
    Pending,
}

impl ValidationResult {
    /// Check if validation passed.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Check if validation failed.
    pub fn is_invalid(&self) -> bool {
        matches!(self, Self::Invalid(_))
    }

    /// Check if validation is pending.
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Get the error message if invalid.
    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Invalid(msg) => Some(msg),
            _ => None,
        }
    }
}

/// A validator that can validate a string value.
pub trait Validator: Send + Sync {
    /// Validate the given value.
    fn validate(&self, value: &str) -> ValidationResult;

    /// Get the name of this validator.
    fn name(&self) -> &str;
}

/// Required field validator.
#[derive(Debug, Clone)]
pub struct Required {
    message: String,
}

impl Required {
    /// Create a required validator with default message.
    pub fn new() -> Self {
        Self {
            message: "This field is required".to_string(),
        }
    }

    /// Create with custom message.
    pub fn with_message(message: &str) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl Default for Required {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator for Required {
    fn validate(&self, value: &str) -> ValidationResult {
        if value.trim().is_empty() {
            ValidationResult::Invalid(self.message.clone())
        } else {
            ValidationResult::Valid
        }
    }

    fn name(&self) -> &'static str {
        "required"
    }
}

/// Minimum length validator.
#[derive(Debug, Clone)]
pub struct MinLength {
    min: usize,
    message: String,
}

impl MinLength {
    /// Create a min length validator.
    pub fn new(min: usize) -> Self {
        Self {
            min,
            message: format!("Must be at least {min} characters"),
        }
    }

    /// Create with custom message.
    pub fn with_message(min: usize, message: &str) -> Self {
        Self {
            min,
            message: message.to_string(),
        }
    }
}

impl Validator for MinLength {
    fn validate(&self, value: &str) -> ValidationResult {
        if value.chars().count() < self.min {
            ValidationResult::Invalid(self.message.clone())
        } else {
            ValidationResult::Valid
        }
    }

    fn name(&self) -> &'static str {
        "minLength"
    }
}

/// Maximum length validator.
#[derive(Debug, Clone)]
pub struct MaxLength {
    max: usize,
    message: String,
}

impl MaxLength {
    /// Create a max length validator.
    pub fn new(max: usize) -> Self {
        Self {
            max,
            message: format!("Must be at most {max} characters"),
        }
    }

    /// Create with custom message.
    pub fn with_message(max: usize, message: &str) -> Self {
        Self {
            max,
            message: message.to_string(),
        }
    }
}

impl Validator for MaxLength {
    fn validate(&self, value: &str) -> ValidationResult {
        if value.chars().count() > self.max {
            ValidationResult::Invalid(self.message.clone())
        } else {
            ValidationResult::Valid
        }
    }

    fn name(&self) -> &'static str {
        "maxLength"
    }
}

/// Range validator for numeric values.
#[derive(Debug, Clone)]
pub struct Range {
    min: f64,
    max: f64,
    message: String,
}

impl Range {
    /// Create a range validator.
    pub fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            message: format!("Must be between {min} and {max}"),
        }
    }

    /// Create with custom message.
    pub fn with_message(min: f64, max: f64, message: &str) -> Self {
        Self {
            min,
            max,
            message: message.to_string(),
        }
    }
}

impl Validator for Range {
    fn validate(&self, value: &str) -> ValidationResult {
        match value.parse::<f64>() {
            Ok(num) if num >= self.min && num <= self.max => ValidationResult::Valid,
            Ok(_) => ValidationResult::Invalid(self.message.clone()),
            Err(_) => ValidationResult::Invalid("Must be a valid number".to_string()),
        }
    }

    fn name(&self) -> &'static str {
        "range"
    }
}

/// Pattern validator using regex-like patterns.
/// Note: Uses simple pattern matching, not full regex.
#[derive(Debug, Clone)]
pub struct Pattern {
    pattern: PatternType,
    message: String,
}

/// Type of pattern to match.
#[derive(Debug, Clone)]
pub enum PatternType {
    /// Email address pattern.
    Email,
    /// URL pattern.
    Url,
    /// Phone number (digits, spaces, dashes, parens).
    Phone,
    /// Alphanumeric only.
    Alphanumeric,
    /// Digits only.
    Digits,
    /// Custom pattern (simple glob-style).
    Custom(String),
}

impl Pattern {
    /// Create an email validator.
    pub fn email() -> Self {
        Self {
            pattern: PatternType::Email,
            message: "Must be a valid email address".to_string(),
        }
    }

    /// Create a URL validator.
    pub fn url() -> Self {
        Self {
            pattern: PatternType::Url,
            message: "Must be a valid URL".to_string(),
        }
    }

    /// Create a phone validator.
    pub fn phone() -> Self {
        Self {
            pattern: PatternType::Phone,
            message: "Must be a valid phone number".to_string(),
        }
    }

    /// Create an alphanumeric validator.
    pub fn alphanumeric() -> Self {
        Self {
            pattern: PatternType::Alphanumeric,
            message: "Must contain only letters and numbers".to_string(),
        }
    }

    /// Create a digits-only validator.
    pub fn digits() -> Self {
        Self {
            pattern: PatternType::Digits,
            message: "Must contain only digits".to_string(),
        }
    }

    /// Create with custom message.
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }

    fn matches(&self, value: &str) -> bool {
        match &self.pattern {
            PatternType::Email => {
                // Simple email validation: has @ and . after @
                let parts: Vec<&str> = value.split('@').collect();
                parts.len() == 2
                    && !parts[0].is_empty()
                    && parts[1].contains('.')
                    && !parts[1].starts_with('.')
                    && !parts[1].ends_with('.')
            }
            PatternType::Url => {
                value.starts_with("http://")
                    || value.starts_with("https://")
                    || value.starts_with("ftp://")
            }
            PatternType::Phone => {
                value
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == ' ' || c == '-' || c == '(' || c == ')' || c == '+')
                    && value.chars().filter(char::is_ascii_digit).count() >= 7
            }
            PatternType::Alphanumeric => value.chars().all(char::is_alphanumeric),
            PatternType::Digits => value.chars().all(|c| c.is_ascii_digit()),
            PatternType::Custom(pattern) => {
                // Simple glob matching (* = any chars)
                if pattern.is_empty() {
                    return true;
                }
                let parts: Vec<&str> = pattern.split('*').collect();
                if parts.len() == 1 {
                    return value == pattern;
                }
                let mut remaining = value;
                for (i, part) in parts.iter().enumerate() {
                    if part.is_empty() {
                        continue;
                    }
                    if i == 0 {
                        if !remaining.starts_with(part) {
                            return false;
                        }
                        remaining = &remaining[part.len()..];
                    } else if i == parts.len() - 1 {
                        if !remaining.ends_with(part) {
                            return false;
                        }
                    } else if let Some(pos) = remaining.find(part) {
                        remaining = &remaining[pos + part.len()..];
                    } else {
                        return false;
                    }
                }
                true
            }
        }
    }
}

impl Validator for Pattern {
    fn validate(&self, value: &str) -> ValidationResult {
        if value.is_empty() {
            // Empty values are handled by Required validator
            return ValidationResult::Valid;
        }

        if self.matches(value) {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(self.message.clone())
        }
    }

    fn name(&self) -> &'static str {
        "pattern"
    }
}

/// Custom function validator.
pub struct Custom<F>
where
    F: Fn(&str) -> ValidationResult + Send + Sync,
{
    validator: F,
    name: String,
}

impl<F> Custom<F>
where
    F: Fn(&str) -> ValidationResult + Send + Sync,
{
    /// Create a custom validator.
    pub fn new(name: &str, validator: F) -> Self {
        Self {
            validator,
            name: name.to_string(),
        }
    }
}

impl<F> Validator for Custom<F>
where
    F: Fn(&str) -> ValidationResult + Send + Sync,
{
    fn validate(&self, value: &str) -> ValidationResult {
        (self.validator)(value)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

impl<F> fmt::Debug for Custom<F>
where
    F: Fn(&str) -> ValidationResult + Send + Sync,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Custom").field("name", &self.name).finish()
    }
}

/// Field validation state.
#[derive(Debug, Clone, Default)]
pub struct FieldState {
    /// Current value.
    pub value: String,
    /// Validation result.
    pub result: Option<ValidationResult>,
    /// Whether the field has been touched (focused then blurred).
    pub touched: bool,
    /// Whether the field has been modified.
    pub dirty: bool,
    /// Field-specific errors (from validators).
    pub errors: Vec<String>,
}

impl FieldState {
    /// Create a new field state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with initial value.
    pub fn with_value(value: &str) -> Self {
        Self {
            value: value.to_string(),
            ..Default::default()
        }
    }

    /// Check if field is valid.
    pub fn is_valid(&self) -> bool {
        self.result.as_ref().map_or(true, ValidationResult::is_valid)
    }

    /// Check if field has errors.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get first error message.
    pub fn first_error(&self) -> Option<&str> {
        self.errors.first().map(std::string::String::as_str)
    }

    /// Mark as touched.
    pub fn touch(&mut self) {
        self.touched = true;
    }

    /// Update value.
    pub fn set_value(&mut self, value: &str) {
        if self.value != value {
            self.value = value.to_string();
            self.dirty = true;
        }
    }
}

/// Configuration for a validated field.
#[derive(Default)]
pub struct FieldConfig {
    /// Validators for this field.
    validators: Vec<Box<dyn Validator>>,
    /// When to validate.
    validate_on: ValidateOn,
}

impl std::fmt::Debug for FieldConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FieldConfig")
            .field("validator_count", &self.validators.len())
            .field("validate_on", &self.validate_on)
            .finish()
    }
}

/// When to run validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValidateOn {
    /// Validate on value change.
    #[default]
    Change,
    /// Validate on blur.
    Blur,
    /// Validate only on submit.
    Submit,
}

impl FieldConfig {
    /// Create a new field config.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a validator.
    pub fn add_validator<V: Validator + 'static>(mut self, validator: V) -> Self {
        self.validators.push(Box::new(validator));
        self
    }

    /// Add required validator.
    pub fn required(self) -> Self {
        self.add_validator(Required::new())
    }

    /// Add min length validator.
    pub fn min_length(self, min: usize) -> Self {
        self.add_validator(MinLength::new(min))
    }

    /// Add max length validator.
    pub fn max_length(self, max: usize) -> Self {
        self.add_validator(MaxLength::new(max))
    }

    /// Add range validator.
    pub fn range(self, min: f64, max: f64) -> Self {
        self.add_validator(Range::new(min, max))
    }

    /// Add email pattern validator.
    pub fn email(self) -> Self {
        self.add_validator(Pattern::email())
    }

    /// Set validation trigger.
    pub fn validate_on(mut self, trigger: ValidateOn) -> Self {
        self.validate_on = trigger;
        self
    }

    /// Run all validators on a value.
    pub fn validate(&self, value: &str) -> Vec<String> {
        let mut errors = Vec::new();
        for validator in &self.validators {
            if let ValidationResult::Invalid(msg) = validator.validate(value) {
                errors.push(msg);
            }
        }
        errors
    }
}

/// Form validation state manager.
#[derive(Debug, Default)]
pub struct FormValidator {
    /// Field configurations.
    configs: HashMap<String, FieldConfig>,
    /// Field states.
    states: HashMap<String, FieldState>,
    /// Whether the form has been submitted.
    submitted: bool,
}

impl FormValidator {
    /// Create a new form validator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a field with configuration.
    pub fn register(&mut self, name: &str, config: FieldConfig) {
        self.configs.insert(name.to_string(), config);
        self.states.insert(name.to_string(), FieldState::new());
    }

    /// Register a field with default config.
    pub fn register_field(&mut self, name: &str) {
        self.register(name, FieldConfig::new());
    }

    /// Set field value.
    pub fn set_value(&mut self, name: &str, value: &str) {
        if let Some(state) = self.states.get_mut(name) {
            state.set_value(value);

            // Check if we should validate on change
            if let Some(config) = self.configs.get(name) {
                if config.validate_on == ValidateOn::Change {
                    state.errors = config.validate(value);
                    state.result = if state.errors.is_empty() {
                        Some(ValidationResult::Valid)
                    } else {
                        Some(ValidationResult::Invalid(state.errors[0].clone()))
                    };
                }
            }
        }
    }

    /// Mark field as touched (for blur validation).
    pub fn touch(&mut self, name: &str) {
        if let Some(state) = self.states.get_mut(name) {
            state.touch();

            // Check if we should validate on blur
            if let Some(config) = self.configs.get(name) {
                if config.validate_on == ValidateOn::Blur {
                    state.errors = config.validate(&state.value);
                    state.result = if state.errors.is_empty() {
                        Some(ValidationResult::Valid)
                    } else {
                        Some(ValidationResult::Invalid(state.errors[0].clone()))
                    };
                }
            }
        }
    }

    /// Get field state.
    pub fn field(&self, name: &str) -> Option<&FieldState> {
        self.states.get(name)
    }

    /// Get field value.
    pub fn value(&self, name: &str) -> Option<&str> {
        self.states.get(name).map(|s| s.value.as_str())
    }

    /// Get field errors.
    pub fn errors(&self, name: &str) -> &[String] {
        self.states
            .get(name)
            .map_or(&[], |s| s.errors.as_slice())
    }

    /// Check if a specific field is valid.
    pub fn field_is_valid(&self, name: &str) -> bool {
        self.states.get(name).is_some_and(FieldState::is_valid)
    }

    /// Validate all fields and return overall validity.
    pub fn validate(&mut self) -> bool {
        let mut all_valid = true;

        for (name, config) in &self.configs {
            if let Some(state) = self.states.get_mut(name) {
                state.errors = config.validate(&state.value);
                state.result = if state.errors.is_empty() {
                    Some(ValidationResult::Valid)
                } else {
                    Some(ValidationResult::Invalid(state.errors[0].clone()))
                };

                if !state.errors.is_empty() {
                    all_valid = false;
                }
            }
        }

        self.submitted = true;
        all_valid
    }

    /// Check if form is valid (all fields pass validation).
    pub fn is_valid(&self) -> bool {
        self.states.values().all(FieldState::is_valid)
    }

    /// Check if form has been submitted.
    pub fn is_submitted(&self) -> bool {
        self.submitted
    }

    /// Check if any field is dirty.
    pub fn is_dirty(&self) -> bool {
        self.states.values().any(|s| s.dirty)
    }

    /// Get all validation errors as a map.
    pub fn all_errors(&self) -> HashMap<&str, &[String]> {
        self.states
            .iter()
            .filter(|(_, s)| !s.errors.is_empty())
            .map(|(k, s)| (k.as_str(), s.errors.as_slice()))
            .collect()
    }

    /// Reset all field states.
    pub fn reset(&mut self) {
        for state in self.states.values_mut() {
            state.value.clear();
            state.result = None;
            state.touched = false;
            state.dirty = false;
            state.errors.clear();
        }
        self.submitted = false;
    }

    /// Get field count.
    pub fn field_count(&self) -> usize {
        self.configs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ValidationResult tests
    #[test]
    fn test_validation_result_valid() {
        let result = ValidationResult::Valid;
        assert!(result.is_valid());
        assert!(!result.is_invalid());
        assert!(!result.is_pending());
        assert!(result.error().is_none());
    }

    #[test]
    fn test_validation_result_invalid() {
        let result = ValidationResult::Invalid("Error message".to_string());
        assert!(!result.is_valid());
        assert!(result.is_invalid());
        assert!(!result.is_pending());
        assert_eq!(result.error(), Some("Error message"));
    }

    #[test]
    fn test_validation_result_pending() {
        let result = ValidationResult::Pending;
        assert!(!result.is_valid());
        assert!(!result.is_invalid());
        assert!(result.is_pending());
    }

    // Required validator tests
    #[test]
    fn test_required_validator() {
        let validator = Required::new();
        assert_eq!(validator.name(), "required");

        assert!(validator.validate("hello").is_valid());
        assert!(validator.validate("  content  ").is_valid());
        assert!(validator.validate("").is_invalid());
        assert!(validator.validate("   ").is_invalid());
    }

    #[test]
    fn test_required_custom_message() {
        let validator = Required::with_message("Custom error");
        let result = validator.validate("");
        assert_eq!(result.error(), Some("Custom error"));
    }

    // MinLength validator tests
    #[test]
    fn test_min_length_validator() {
        let validator = MinLength::new(3);
        assert_eq!(validator.name(), "minLength");

        assert!(validator.validate("abc").is_valid());
        assert!(validator.validate("abcd").is_valid());
        assert!(validator.validate("ab").is_invalid());
        assert!(validator.validate("").is_invalid());
    }

    #[test]
    fn test_min_length_unicode() {
        let validator = MinLength::new(3);
        assert!(validator.validate("日本語").is_valid()); // 3 characters
        assert!(validator.validate("日本").is_invalid()); // 2 characters
    }

    // MaxLength validator tests
    #[test]
    fn test_max_length_validator() {
        let validator = MaxLength::new(5);
        assert_eq!(validator.name(), "maxLength");

        assert!(validator.validate("").is_valid());
        assert!(validator.validate("abc").is_valid());
        assert!(validator.validate("abcde").is_valid());
        assert!(validator.validate("abcdef").is_invalid());
    }

    // Range validator tests
    #[test]
    fn test_range_validator() {
        let validator = Range::new(0.0, 100.0);
        assert_eq!(validator.name(), "range");

        assert!(validator.validate("0").is_valid());
        assert!(validator.validate("50").is_valid());
        assert!(validator.validate("100").is_valid());
        assert!(validator.validate("-1").is_invalid());
        assert!(validator.validate("101").is_invalid());
    }

    #[test]
    fn test_range_with_decimals() {
        let validator = Range::new(0.0, 1.0);

        assert!(validator.validate("0.5").is_valid());
        assert!(validator.validate("0.99").is_valid());
        assert!(validator.validate("1.01").is_invalid());
    }

    #[test]
    fn test_range_invalid_number() {
        let validator = Range::new(0.0, 100.0);
        let result = validator.validate("not a number");
        assert!(result.is_invalid());
        assert_eq!(result.error(), Some("Must be a valid number"));
    }

    // Pattern validator tests
    #[test]
    fn test_pattern_email() {
        let validator = Pattern::email();

        assert!(validator.validate("test@example.com").is_valid());
        assert!(validator.validate("user.name@domain.co.uk").is_valid());
        assert!(validator.validate("").is_valid()); // Empty handled by Required

        assert!(validator.validate("invalid").is_invalid());
        assert!(validator.validate("@missing.com").is_invalid());
        assert!(validator.validate("missing@").is_invalid());
        assert!(validator.validate("missing@.com").is_invalid());
    }

    #[test]
    fn test_pattern_url() {
        let validator = Pattern::url();

        assert!(validator.validate("http://example.com").is_valid());
        assert!(validator.validate("https://example.com").is_valid());
        assert!(validator.validate("ftp://files.example.com").is_valid());

        assert!(validator.validate("example.com").is_invalid());
        assert!(validator.validate("www.example.com").is_invalid());
    }

    #[test]
    fn test_pattern_phone() {
        let validator = Pattern::phone();

        assert!(validator.validate("1234567").is_valid());
        assert!(validator.validate("123-456-7890").is_valid());
        assert!(validator.validate("(123) 456-7890").is_valid());
        assert!(validator.validate("+1 234 567 8900").is_valid());

        assert!(validator.validate("123").is_invalid()); // Too short
        assert!(validator.validate("abc-def-ghij").is_invalid());
    }

    #[test]
    fn test_pattern_alphanumeric() {
        let validator = Pattern::alphanumeric();

        assert!(validator.validate("abc123").is_valid());
        assert!(validator.validate("ABC").is_valid());
        assert!(validator.validate("123").is_valid());

        assert!(validator.validate("abc-123").is_invalid());
        assert!(validator.validate("hello world").is_invalid());
    }

    #[test]
    fn test_pattern_digits() {
        let validator = Pattern::digits();

        assert!(validator.validate("123456").is_valid());
        assert!(validator.validate("0").is_valid());

        assert!(validator.validate("123a").is_invalid());
        assert!(validator.validate("12.34").is_invalid());
    }

    // Custom validator tests
    #[test]
    fn test_custom_validator() {
        let validator = Custom::new("even_length", |value| {
            if value.len() % 2 == 0 {
                ValidationResult::Valid
            } else {
                ValidationResult::Invalid("Length must be even".to_string())
            }
        });

        assert_eq!(validator.name(), "even_length");
        assert!(validator.validate("ab").is_valid());
        assert!(validator.validate("abcd").is_valid());
        assert!(validator.validate("abc").is_invalid());
    }

    // FieldState tests
    #[test]
    fn test_field_state_new() {
        let state = FieldState::new();
        assert!(state.value.is_empty());
        assert!(!state.touched);
        assert!(!state.dirty);
        assert!(state.errors.is_empty());
    }

    #[test]
    fn test_field_state_with_value() {
        let state = FieldState::with_value("initial");
        assert_eq!(state.value, "initial");
    }

    #[test]
    fn test_field_state_touch() {
        let mut state = FieldState::new();
        assert!(!state.touched);
        state.touch();
        assert!(state.touched);
    }

    #[test]
    fn test_field_state_set_value() {
        let mut state = FieldState::new();
        assert!(!state.dirty);
        state.set_value("new value");
        assert!(state.dirty);
        assert_eq!(state.value, "new value");
    }

    #[test]
    fn test_field_state_first_error() {
        let mut state = FieldState::new();
        assert!(state.first_error().is_none());

        state.errors.push("First error".to_string());
        state.errors.push("Second error".to_string());
        assert_eq!(state.first_error(), Some("First error"));
    }

    // FieldConfig tests
    #[test]
    fn test_field_config_builder() {
        let config = FieldConfig::new()
            .required()
            .min_length(3)
            .max_length(10)
            .email();

        assert_eq!(config.validators.len(), 4);
    }

    #[test]
    fn test_field_config_validate() {
        let config = FieldConfig::new()
            .required()
            .min_length(3);

        let errors = config.validate("");
        assert_eq!(errors.len(), 2); // Required and MinLength

        let errors = config.validate("ab");
        assert_eq!(errors.len(), 1); // Only MinLength

        let errors = config.validate("abc");
        assert!(errors.is_empty());
    }

    // FormValidator tests
    #[test]
    fn test_form_validator_new() {
        let form = FormValidator::new();
        assert_eq!(form.field_count(), 0);
        assert!(!form.is_submitted());
    }

    #[test]
    fn test_form_validator_register() {
        let mut form = FormValidator::new();
        form.register("email", FieldConfig::new().required().email());
        form.register("name", FieldConfig::new().required());

        assert_eq!(form.field_count(), 2);
    }

    #[test]
    fn test_form_validator_set_value() {
        let mut form = FormValidator::new();
        form.register("name", FieldConfig::new());

        form.set_value("name", "John");
        assert_eq!(form.value("name"), Some("John"));
        assert!(form.is_dirty());
    }

    #[test]
    fn test_form_validator_validate_on_change() {
        let mut form = FormValidator::new();
        form.register(
            "email",
            FieldConfig::new()
                .required()
                .email()
                .validate_on(ValidateOn::Change),
        );

        form.set_value("email", "invalid");
        assert!(!form.field_is_valid("email"));

        form.set_value("email", "valid@example.com");
        assert!(form.field_is_valid("email"));
    }

    #[test]
    fn test_form_validator_validate_on_blur() {
        let mut form = FormValidator::new();
        form.register(
            "email",
            FieldConfig::new()
                .required()
                .validate_on(ValidateOn::Blur),
        );

        form.set_value("email", "");
        // Should not validate yet
        assert!(form.errors("email").is_empty());

        form.touch("email");
        // Now should validate
        assert!(!form.errors("email").is_empty());
    }

    #[test]
    fn test_form_validator_validate_all() {
        let mut form = FormValidator::new();
        form.register("name", FieldConfig::new().required());
        form.register("email", FieldConfig::new().required().email());

        form.set_value("name", "John");
        form.set_value("email", "invalid");

        let valid = form.validate();
        assert!(!valid);
        assert!(form.is_submitted());

        let errors = form.all_errors();
        assert_eq!(errors.len(), 1); // Only email has errors
    }

    #[test]
    fn test_form_validator_reset() {
        let mut form = FormValidator::new();
        form.register("name", FieldConfig::new().required());

        form.set_value("name", "John");
        form.touch("name");
        form.validate();

        form.reset();

        assert!(!form.is_submitted());
        assert!(!form.is_dirty());
        assert_eq!(form.value("name"), Some(""));
    }

    #[test]
    fn test_form_validator_is_valid() {
        let mut form = FormValidator::new();
        form.register("name", FieldConfig::new().required());

        form.set_value("name", "");
        form.validate();
        assert!(!form.is_valid());

        form.set_value("name", "John");
        form.validate();
        assert!(form.is_valid());
    }
}
