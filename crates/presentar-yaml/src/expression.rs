//! Expression language for data binding.
//!
//! Syntax: `{{ source | transform | transform }}`

use serde::{Deserialize, Serialize};

/// Parsed expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Expression {
    /// Source identifier (e.g., "data.transactions")
    pub source: String,
    /// Chain of transforms
    pub transforms: Vec<Transform>,
}

/// A transform operation in the expression pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Transform {
    /// Filter rows: `filter(field=value)`
    Filter {
        /// Field to filter on
        field: String,
        /// Value to match
        value: String,
    },
    /// Select columns: `select(field1, field2)`
    Select {
        /// Fields to select
        fields: Vec<String>,
    },
    /// Sort rows: `sort(field, desc=true)`
    Sort {
        /// Field to sort by
        field: String,
        /// Sort descending
        desc: bool,
    },
    /// Limit rows: `limit(n)`
    Limit {
        /// Maximum number of rows
        n: usize,
    },
    /// Count rows
    Count,
    /// Sum a field: `sum(field)`
    Sum {
        /// Field to sum
        field: String,
    },
    /// Average a field: `mean(field)`
    Mean {
        /// Field to average
        field: String,
    },
    /// Rate over window: `rate(window)`
    Rate {
        /// Time window (e.g., "1m", "5m")
        window: String,
    },
    /// Convert to percentage
    Percentage,
    /// Join with another dataset: `join(other, on=field)`
    Join {
        /// Other dataset
        other: String,
        /// Join field
        on: String,
    },
    /// Sample rows: `sample(n)`
    Sample {
        /// Number of rows to sample
        n: usize,
    },
}

/// Expression parser.
#[derive(Debug, Default)]
pub struct ExpressionParser;

impl ExpressionParser {
    /// Create a new parser.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Parse an expression string.
    ///
    /// # Errors
    ///
    /// Returns an error if the expression is invalid.
    pub fn parse(&self, input: &str) -> Result<Expression, ExpressionError> {
        let input = input.trim();

        // Check for {{ }} wrapper
        let inner = if input.starts_with("{{") && input.ends_with("}}") {
            input[2..input.len() - 2].trim()
        } else {
            input
        };

        // Split by pipe
        let parts: Vec<&str> = inner.split('|').map(str::trim).collect();

        if parts.is_empty() {
            return Err(ExpressionError::EmptyExpression);
        }

        let source = parts[0].to_string();
        let mut transforms = Vec::new();

        for part in &parts[1..] {
            let transform = self.parse_transform(part)?;
            transforms.push(transform);
        }

        Ok(Expression { source, transforms })
    }

    fn parse_transform(&self, input: &str) -> Result<Transform, ExpressionError> {
        let input = input.trim();

        // Check for function call: name(args)
        if let Some(paren_pos) = input.find('(') {
            let name = &input[..paren_pos];
            let args_str = input[paren_pos + 1..].trim_end_matches(')').trim();

            match name {
                "filter" => {
                    let (field, value) = self.parse_key_value(args_str)?;
                    Ok(Transform::Filter { field, value })
                }
                "select" => {
                    let fields: Vec<String> = args_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    Ok(Transform::Select { fields })
                }
                "sort" => {
                    let parts: Vec<&str> = args_str.split(',').map(str::trim).collect();
                    let field = (*parts.first().unwrap_or(&"")).to_string();
                    let desc = parts.get(1).is_some_and(|s| s.contains("desc=true"));
                    Ok(Transform::Sort { field, desc })
                }
                "limit" => {
                    let n = args_str
                        .parse()
                        .map_err(|_| ExpressionError::InvalidArgument("limit".to_string()))?;
                    Ok(Transform::Limit { n })
                }
                "sum" => Ok(Transform::Sum {
                    field: args_str.to_string(),
                }),
                "mean" => Ok(Transform::Mean {
                    field: args_str.to_string(),
                }),
                "rate" => Ok(Transform::Rate {
                    window: args_str.to_string(),
                }),
                "join" => {
                    let parts: Vec<&str> = args_str.split(',').map(str::trim).collect();
                    let other = (*parts.first().unwrap_or(&"")).to_string();
                    let on = parts
                        .get(1)
                        .and_then(|s| s.strip_prefix("on="))
                        .unwrap_or("")
                        .to_string();
                    Ok(Transform::Join { other, on })
                }
                "sample" => {
                    let n = args_str
                        .parse()
                        .map_err(|_| ExpressionError::InvalidArgument("sample".to_string()))?;
                    Ok(Transform::Sample { n })
                }
                _ => Err(ExpressionError::UnknownTransform(name.to_string())),
            }
        } else {
            // Simple transform without args
            match input {
                "count" => Ok(Transform::Count),
                "percentage" => Ok(Transform::Percentage),
                _ => Err(ExpressionError::UnknownTransform(input.to_string())),
            }
        }
    }

    fn parse_key_value(&self, input: &str) -> Result<(String, String), ExpressionError> {
        let parts: Vec<&str> = input.splitn(2, '=').collect();
        if parts.len() != 2 {
            return Err(ExpressionError::InvalidArgument(input.to_string()));
        }
        Ok((parts[0].trim().to_string(), parts[1].trim().to_string()))
    }
}

/// Expression parsing error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionError {
    /// Empty expression
    EmptyExpression,
    /// Unknown transform function
    UnknownTransform(String),
    /// Invalid argument
    InvalidArgument(String),
}

impl std::fmt::Display for ExpressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "empty expression"),
            Self::UnknownTransform(name) => write!(f, "unknown transform: {name}"),
            Self::InvalidArgument(arg) => write!(f, "invalid argument: {arg}"),
        }
    }
}

impl std::error::Error for ExpressionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_source() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("data.transactions").unwrap();
        assert_eq!(expr.source, "data.transactions");
        assert!(expr.transforms.is_empty());
    }

    #[test]
    fn test_parse_with_braces() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data.transactions }}").unwrap();
        assert_eq!(expr.source, "data.transactions");
    }

    #[test]
    fn test_parse_count() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data.items | count }}").unwrap();
        assert_eq!(expr.transforms, vec![Transform::Count]);
    }

    #[test]
    fn test_parse_filter() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data | filter(status=active) }}").unwrap();
        assert_eq!(
            expr.transforms,
            vec![Transform::Filter {
                field: "status".to_string(),
                value: "active".to_string(),
            }]
        );
    }

    #[test]
    fn test_parse_select() {
        let parser = ExpressionParser::new();
        let expr = parser
            .parse("{{ data | select(id, name, email) }}")
            .unwrap();
        assert_eq!(
            expr.transforms,
            vec![Transform::Select {
                fields: vec!["id".to_string(), "name".to_string(), "email".to_string()],
            }]
        );
    }

    #[test]
    fn test_parse_sort() {
        let parser = ExpressionParser::new();
        let expr = parser
            .parse("{{ data | sort(created_at, desc=true) }}")
            .unwrap();
        assert_eq!(
            expr.transforms,
            vec![Transform::Sort {
                field: "created_at".to_string(),
                desc: true,
            }]
        );
    }

    #[test]
    fn test_parse_limit() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data | limit(10) }}").unwrap();
        assert_eq!(expr.transforms, vec![Transform::Limit { n: 10 }]);
    }

    #[test]
    fn test_parse_chain() {
        let parser = ExpressionParser::new();
        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) | count }}")
            .unwrap();

        assert_eq!(expr.source, "data.transactions");
        assert_eq!(expr.transforms.len(), 2);
        assert_eq!(
            expr.transforms[0],
            Transform::Filter {
                field: "status".to_string(),
                value: "completed".to_string(),
            }
        );
        assert_eq!(expr.transforms[1], Transform::Count);
    }

    #[test]
    fn test_parse_join() {
        let parser = ExpressionParser::new();
        let expr = parser
            .parse("{{ data.orders | join(data.customers, on=customer_id) }}")
            .unwrap();

        assert_eq!(
            expr.transforms,
            vec![Transform::Join {
                other: "data.customers".to_string(),
                on: "customer_id".to_string(),
            }]
        );
    }

    #[test]
    fn test_parse_sample() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data | sample(100) }}").unwrap();
        assert_eq!(expr.transforms, vec![Transform::Sample { n: 100 }]);
    }

    #[test]
    fn test_parse_sum() {
        let parser = ExpressionParser::new();
        let expr = parser.parse("{{ data | sum(amount) }}").unwrap();
        assert_eq!(
            expr.transforms,
            vec![Transform::Sum {
                field: "amount".to_string(),
            }]
        );
    }

    #[test]
    fn test_parse_error_unknown_transform() {
        let parser = ExpressionParser::new();
        let result = parser.parse("{{ data | unknown() }}");
        assert!(matches!(result, Err(ExpressionError::UnknownTransform(_))));
    }

    #[test]
    fn test_expression_error_display() {
        assert_eq!(
            ExpressionError::EmptyExpression.to_string(),
            "empty expression"
        );
        assert_eq!(
            ExpressionError::UnknownTransform("foo".to_string()).to_string(),
            "unknown transform: foo"
        );
    }
}
