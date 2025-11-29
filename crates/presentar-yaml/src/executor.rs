//! Expression executor for data transformations.

use crate::expression::{Expression, Transform};
use std::collections::HashMap;

/// A generic value that can hold any data type.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Value {
    /// Null value
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// Numeric value
    Number(f64),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<Value>),
    /// Object (key-value map)
    Object(HashMap<String, Value>),
}

impl Value {
    /// Create a new null value.
    #[must_use]
    pub fn null() -> Self {
        Self::Null
    }

    /// Create a new boolean value.
    #[must_use]
    pub fn bool(v: bool) -> Self {
        Self::Bool(v)
    }

    /// Create a new number value.
    #[must_use]
    pub fn number(v: f64) -> Self {
        Self::Number(v)
    }

    /// Create a new string value.
    #[must_use]
    pub fn string(v: impl Into<String>) -> Self {
        Self::String(v.into())
    }

    /// Create a new array value.
    #[must_use]
    pub fn array(v: Vec<Value>) -> Self {
        Self::Array(v)
    }

    /// Create a new object value.
    #[must_use]
    pub fn object(v: HashMap<String, Value>) -> Self {
        Self::Object(v)
    }

    /// Check if value is null.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Check if value is a boolean.
    #[must_use]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Check if value is a number.
    #[must_use]
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }

    /// Check if value is a string.
    #[must_use]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Check if value is an array.
    #[must_use]
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// Check if value is an object.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// Get as boolean.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as number.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }

    /// Get as string.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Get as array.
    #[must_use]
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Get as mutable array.
    #[must_use]
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Get as object.
    #[must_use]
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Object(v) => Some(v),
            _ => None,
        }
    }

    /// Get field from object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(map) => map.get(key),
            _ => None,
        }
    }

    /// Get array length or object key count.
    #[must_use]
    pub fn len(&self) -> usize {
        match self {
            Self::Array(arr) => arr.len(),
            Self::Object(obj) => obj.len(),
            Self::String(s) => s.len(),
            _ => 0,
        }
    }

    /// Check if array or object is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Number(f64::from(v))
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Number(v as f64)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

/// Execution error.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// Source not found in data context
    SourceNotFound(String),
    /// Expected an array for this transform
    ExpectedArray,
    /// Expected an object
    ExpectedObject,
    /// Field not found
    FieldNotFound(String),
    /// Type mismatch
    TypeMismatch(String),
    /// Invalid transform
    InvalidTransform(String),
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceNotFound(name) => write!(f, "source not found: {name}"),
            Self::ExpectedArray => write!(f, "expected an array"),
            Self::ExpectedObject => write!(f, "expected an object"),
            Self::FieldNotFound(name) => write!(f, "field not found: {name}"),
            Self::TypeMismatch(msg) => write!(f, "type mismatch: {msg}"),
            Self::InvalidTransform(msg) => write!(f, "invalid transform: {msg}"),
        }
    }
}

impl std::error::Error for ExecutionError {}

/// Data context for expression execution.
#[derive(Debug, Clone, Default)]
pub struct DataContext {
    /// Named data sources
    sources: HashMap<String, Value>,
}

impl DataContext {
    /// Create a new empty context.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a data source.
    pub fn insert(&mut self, name: impl Into<String>, value: Value) {
        self.sources.insert(name.into(), value);
    }

    /// Get a data source.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Value> {
        // Support dotted paths like "data.transactions"
        let parts: Vec<&str> = name.split('.').collect();
        let mut current = self.sources.get(parts[0])?;

        for part in &parts[1..] {
            current = current.get(part)?;
        }

        Some(current)
    }

    /// Check if context has a source.
    #[must_use]
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// Expression executor.
#[derive(Debug, Default)]
pub struct ExpressionExecutor;

impl ExpressionExecutor {
    /// Create a new executor.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Execute an expression against a data context.
    ///
    /// # Errors
    ///
    /// Returns an error if execution fails.
    pub fn execute(&self, expr: &Expression, ctx: &DataContext) -> Result<Value, ExecutionError> {
        // Resolve source
        let mut value = ctx
            .get(&expr.source)
            .cloned()
            .ok_or_else(|| ExecutionError::SourceNotFound(expr.source.clone()))?;

        // Apply transforms
        for transform in &expr.transforms {
            value = self.apply_transform(&value, transform)?;
        }

        Ok(value)
    }

    fn apply_transform(
        &self,
        value: &Value,
        transform: &Transform,
    ) -> Result<Value, ExecutionError> {
        match transform {
            Transform::Filter {
                field,
                value: match_value,
            } => self.apply_filter(value, field, match_value),
            Transform::Select { fields } => self.apply_select(value, fields),
            Transform::Sort { field, desc } => self.apply_sort(value, field, *desc),
            Transform::Limit { n } => self.apply_limit(value, *n),
            Transform::Count => Ok(self.apply_count(value)),
            Transform::Sum { field } => self.apply_sum(value, field),
            Transform::Mean { field } => self.apply_mean(value, field),
            Transform::Sample { n } => self.apply_sample(value, *n),
            Transform::Percentage => self.apply_percentage(value),
            Transform::Rate { .. } => {
                // Rate requires time-series data, return value as-is for now
                Ok(value.clone())
            }
            Transform::Join { .. } => {
                // Join requires access to other datasets, return value as-is for now
                Ok(value.clone())
            }
        }
    }

    fn apply_filter(
        &self,
        value: &Value,
        field: &str,
        match_value: &str,
    ) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| {
                if let Some(obj) = item.as_object() {
                    if let Some(val) = obj.get(field) {
                        return self.value_matches(val, match_value);
                    }
                }
                false
            })
            .cloned()
            .collect();

        Ok(Value::Array(filtered))
    }

    fn value_matches(&self, value: &Value, target: &str) -> bool {
        match value {
            Value::String(s) => s == target,
            Value::Number(n) => {
                if let Ok(t) = target.parse::<f64>() {
                    (*n - t).abs() < f64::EPSILON
                } else {
                    false
                }
            }
            Value::Bool(b) => {
                matches!((b, target), (true, "true") | (false, "false"))
            }
            _ => false,
        }
    }

    fn apply_select(&self, value: &Value, fields: &[String]) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let selected: Vec<Value> = arr
            .iter()
            .map(|item| {
                if let Some(obj) = item.as_object() {
                    let mut new_obj = HashMap::new();
                    for field in fields {
                        if let Some(val) = obj.get(field) {
                            new_obj.insert(field.clone(), val.clone());
                        }
                    }
                    Value::Object(new_obj)
                } else {
                    item.clone()
                }
            })
            .collect();

        Ok(Value::Array(selected))
    }

    fn apply_sort(&self, value: &Value, field: &str, desc: bool) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        let mut sorted = arr.clone();

        sorted.sort_by(|a, b| {
            let a_val = a.get(field);
            let b_val = b.get(field);

            let cmp = match (a_val, b_val) {
                (Some(Value::Number(a)), Some(Value::Number(b))) => {
                    a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal)
                }
                (Some(Value::String(a)), Some(Value::String(b))) => a.cmp(b),
                _ => std::cmp::Ordering::Equal,
            };

            if desc {
                cmp.reverse()
            } else {
                cmp
            }
        });

        Ok(Value::Array(sorted))
    }

    fn apply_limit(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;
        Ok(Value::Array(arr.iter().take(n).cloned().collect()))
    }

    fn apply_count(&self, value: &Value) -> Value {
        match value {
            Value::Array(arr) => Value::Number(arr.len() as f64),
            Value::Object(obj) => Value::Number(obj.len() as f64),
            Value::String(s) => Value::Number(s.len() as f64),
            _ => Value::Number(0.0),
        }
    }

    fn apply_sum(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let sum: f64 = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .sum();

        Ok(Value::Number(sum))
    }

    fn apply_mean(&self, value: &Value, field: &str) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        let values: Vec<f64> = arr
            .iter()
            .filter_map(|item| item.get(field)?.as_number())
            .collect();

        if values.is_empty() {
            return Ok(Value::Number(0.0));
        }

        let sum: f64 = values.iter().sum();
        let mean = sum / values.len() as f64;

        Ok(Value::Number(mean))
    }

    fn apply_sample(&self, value: &Value, n: usize) -> Result<Value, ExecutionError> {
        let arr = value.as_array().ok_or(ExecutionError::ExpectedArray)?;

        // Simple deterministic "sampling" - just take first n elements
        // Real implementation would use random sampling
        Ok(Value::Array(arr.iter().take(n).cloned().collect()))
    }

    fn apply_percentage(&self, value: &Value) -> Result<Value, ExecutionError> {
        match value {
            Value::Number(n) => Ok(Value::Number(n * 100.0)),
            _ => Err(ExecutionError::TypeMismatch(
                "percentage requires a number".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::ExpressionParser;

    // ===== Value Tests =====

    #[test]
    fn test_value_null() {
        let v = Value::null();
        assert!(v.is_null());
        assert!(!v.is_bool());
    }

    #[test]
    fn test_value_bool() {
        let v = Value::bool(true);
        assert!(v.is_bool());
        assert_eq!(v.as_bool(), Some(true));
    }

    #[test]
    fn test_value_number() {
        let v = Value::number(42.5);
        assert!(v.is_number());
        assert_eq!(v.as_number(), Some(42.5));
    }

    #[test]
    fn test_value_string() {
        let v = Value::string("hello");
        assert!(v.is_string());
        assert_eq!(v.as_str(), Some("hello"));
    }

    #[test]
    fn test_value_array() {
        let v = Value::array(vec![Value::number(1.0), Value::number(2.0)]);
        assert!(v.is_array());
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn test_value_object() {
        let mut map = HashMap::new();
        map.insert("name".to_string(), Value::string("test"));
        let v = Value::object(map);
        assert!(v.is_object());
        assert_eq!(v.get("name").unwrap().as_str(), Some("test"));
    }

    #[test]
    fn test_value_from_bool() {
        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn test_value_from_number() {
        let v: Value = 42.0f64.into();
        assert_eq!(v, Value::Number(42.0));
    }

    #[test]
    fn test_value_from_i32() {
        let v: Value = 42i32.into();
        assert_eq!(v, Value::Number(42.0));
    }

    #[test]
    fn test_value_from_str() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::String("hello".to_string()));
    }

    #[test]
    fn test_value_default() {
        assert_eq!(Value::default(), Value::Null);
    }

    #[test]
    fn test_value_is_empty() {
        assert!(Value::array(vec![]).is_empty());
        assert!(!Value::array(vec![Value::Null]).is_empty());
    }

    // ===== DataContext Tests =====

    #[test]
    fn test_context_new() {
        let ctx = DataContext::new();
        assert!(!ctx.contains("foo"));
    }

    #[test]
    fn test_context_insert_get() {
        let mut ctx = DataContext::new();
        ctx.insert("users", Value::array(vec![]));
        assert!(ctx.contains("users"));
    }

    #[test]
    fn test_context_dotted_path() {
        let mut ctx = DataContext::new();
        let mut data = HashMap::new();
        data.insert("transactions".to_string(), Value::array(vec![]));
        ctx.insert("data", Value::object(data));

        assert!(ctx.contains("data.transactions"));
    }

    // ===== ExecutionError Tests =====

    #[test]
    fn test_error_display() {
        assert_eq!(
            ExecutionError::SourceNotFound("foo".to_string()).to_string(),
            "source not found: foo"
        );
        assert_eq!(
            ExecutionError::ExpectedArray.to_string(),
            "expected an array"
        );
    }

    // ===== Executor Tests =====

    fn make_test_data() -> DataContext {
        let mut ctx = DataContext::new();

        // Create test transactions
        let transactions: Vec<Value> = vec![
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(1.0));
                t.insert("status".to_string(), Value::string("completed"));
                t.insert("amount".to_string(), Value::number(100.0));
                Value::Object(t)
            },
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(2.0));
                t.insert("status".to_string(), Value::string("pending"));
                t.insert("amount".to_string(), Value::number(50.0));
                Value::Object(t)
            },
            {
                let mut t = HashMap::new();
                t.insert("id".to_string(), Value::number(3.0));
                t.insert("status".to_string(), Value::string("completed"));
                t.insert("amount".to_string(), Value::number(75.0));
                Value::Object(t)
            },
        ];

        let mut data = HashMap::new();
        data.insert("transactions".to_string(), Value::Array(transactions));
        ctx.insert("data", Value::object(data));

        ctx
    }

    #[test]
    fn test_execute_simple_source() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("data.transactions").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_array());
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_execute_source_not_found() {
        let ctx = DataContext::new();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("nonexistent").unwrap();
        let result = executor.execute(&expr, &ctx);

        assert!(matches!(result, Err(ExecutionError::SourceNotFound(_))));
    }

    #[test]
    fn test_execute_filter() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert!(result.is_array());
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_count() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | count }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(3.0));
    }

    #[test]
    fn test_execute_filter_then_count() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) | count }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(2.0));
    }

    #[test]
    fn test_execute_select() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | select(id, status) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);

        // First item should only have id and status
        let first = arr[0].as_object().unwrap();
        assert!(first.contains_key("id"));
        assert!(first.contains_key("status"));
        assert!(!first.contains_key("amount"));
    }

    #[test]
    fn test_execute_sort_asc() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sort(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        let amounts: Vec<f64> = arr
            .iter()
            .filter_map(|v| v.get("amount")?.as_number())
            .collect();
        assert_eq!(amounts, vec![50.0, 75.0, 100.0]);
    }

    #[test]
    fn test_execute_sort_desc() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sort(amount, desc=true) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        let amounts: Vec<f64> = arr
            .iter()
            .filter_map(|v| v.get("amount")?.as_number())
            .collect();
        assert_eq!(amounts, vec![100.0, 75.0, 50.0]);
    }

    #[test]
    fn test_execute_limit() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | limit(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_sum() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | sum(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(225.0)); // 100 + 50 + 75
    }

    #[test]
    fn test_execute_mean() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | mean(amount) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(75.0)); // 225 / 3
    }

    #[test]
    fn test_execute_sample() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ data.transactions | sample(2) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_execute_percentage() {
        let mut ctx = DataContext::new();
        ctx.insert("ratio", Value::number(0.75));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ ratio | percentage }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(75.0));
    }

    #[test]
    fn test_execute_chain() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(status=completed) | sort(amount, desc=true) | limit(1) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("amount").unwrap().as_number(), Some(100.0));
    }

    #[test]
    fn test_filter_numeric_match() {
        let ctx = make_test_data();
        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser
            .parse("{{ data.transactions | filter(amount=100) }}")
            .unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_execute_on_empty_array() {
        let mut ctx = DataContext::new();
        ctx.insert("items", Value::array(vec![]));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | count }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(0.0));
    }

    #[test]
    fn test_execute_mean_empty_array() {
        let mut ctx = DataContext::new();
        ctx.insert("items", Value::array(vec![]));

        let parser = ExpressionParser::new();
        let executor = ExpressionExecutor::new();

        let expr = parser.parse("{{ items | mean(value) }}").unwrap();
        let result = executor.execute(&expr, &ctx).unwrap();

        assert_eq!(result.as_number(), Some(0.0));
    }
}
