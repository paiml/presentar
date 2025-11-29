//! Error types for YAML parsing.

use std::fmt;

/// Error type for manifest parsing.
#[derive(Debug)]
pub enum ParseError {
    /// YAML parsing error
    Yaml(serde_yaml::Error),
    /// Expression parsing error
    Expression(crate::expression::ExpressionError),
    /// Validation error
    Validation(String),
    /// Missing required field
    MissingField(String),
    /// Invalid value
    InvalidValue {
        /// Field name
        field: String,
        /// Error message
        message: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(e) => write!(f, "YAML error: {e}"),
            Self::Expression(e) => write!(f, "Expression error: {e}"),
            Self::Validation(msg) => write!(f, "Validation error: {msg}"),
            Self::MissingField(field) => write!(f, "Missing required field: {field}"),
            Self::InvalidValue { field, message } => {
                write!(f, "Invalid value for '{field}': {message}")
            }
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Yaml(e) => Some(e),
            Self::Expression(e) => Some(e),
            _ => None,
        }
    }
}

impl From<serde_yaml::Error> for ParseError {
    fn from(e: serde_yaml::Error) -> Self {
        Self::Yaml(e)
    }
}

impl From<crate::expression::ExpressionError> for ParseError {
    fn from(e: crate::expression::ExpressionError) -> Self {
        Self::Expression(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::MissingField("name".to_string());
        assert_eq!(err.to_string(), "Missing required field: name");

        let err = ParseError::InvalidValue {
            field: "columns".to_string(),
            message: "must be positive".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Invalid value for 'columns': must be positive"
        );

        let err = ParseError::Validation("layout is required".to_string());
        assert_eq!(err.to_string(), "Validation error: layout is required");
    }
}
